mod protocol;
mod server;
mod session_mirror;
mod static_client;

use agent_settings::{AgentSettings, MobileCompanionSettings};
use anyhow::{Context as _, Result};
use futures::StreamExt as _;
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global, Subscription, Task};
use gpui_tokio::Tokio;
use paths::data_dir;
use settings::{Settings as _, SettingsStore};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub use protocol::{
    CompanionAccessMode, CompanionAttachment, CompanionCommand, CompanionCommandKind,
    CompanionConnectionMetadata, CompanionEvent, CompanionMessage, CompanionMessageRole,
    CompanionMessageStatus, CompanionPermissionChoice, CompanionPermissionOption,
    CompanionPermissionRequest, CompanionRunStatus, CompanionSessionSummary, CompanionSnapshot,
    CompanionTimelineEntry, CompanionTimelinePage, CompanionTimelineToolCall,
    CompanionTimelineToolCallDetail, CompanionToolCall, CompanionToolCallStatus, CompanionUpload,
};
pub use server::{CompanionServerHandle, CompanionServerStart, CompanionServerState};
pub use session_mirror::{CompanionSessionMirror, CompanionSessionSource};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompanionManagerStatus {
    pub state: CompanionServiceState,
    pub selection_mode: CompanionSessionMode,
    pub shared_session: Option<CompanionSessionSummary>,
    pub access_info: Option<CompanionAccessInfo>,
    pub persistent_access_info: Option<CompanionAccessInfo>,
    pub access_mode: CompanionAccessMode,
    pub settings: MobileCompanionSettings,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum CompanionServiceState {
    Starting,
    Running,
    Stopping,
    Failed {
        message: String,
    },
    #[default]
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompanionAccessInfo {
    pub url: String,
    pub token_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompanionAsset {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub disposition: CompanionAssetDisposition,
    pub source: CompanionAssetSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompanionAssetDisposition {
    Inline,
    Attachment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompanionAssetSource {
    Bytes(Vec<u8>),
    File(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompanionSessionTarget {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompanionSessionMode {
    #[default]
    FollowActive,
    Pinned,
}

#[derive(Clone, Debug)]
pub enum CompanionManagerEvent {
    StatusChanged,
}

struct GlobalCompanionManager(Entity<CompanionManager>);

impl Global for GlobalCompanionManager {}

pub struct CompanionManager {
    mirror: Entity<CompanionSessionMirror>,
    status: CompanionManagerStatus,
    persistent_state: PersistentCompanionState,
    server: Option<CompanionServerHandle>,
    follow_source: Option<CompanionSessionSource>,
    pinned_source: Option<CompanionSessionSource>,
    _mirror_subscription: Subscription,
    _command_task: Option<Task<Result<()>>>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct PersistentCompanionState {
    token_id: String,
    #[serde(default)]
    last_host: Option<String>,
}

impl EventEmitter<CompanionManagerEvent> for CompanionManager {}

impl CompanionManager {
    pub fn init(cx: &mut App) {
        if Self::try_global(cx).is_some() {
            return;
        }

        let manager = cx.new(|cx| Self::new(cx));
        cx.set_global(GlobalCompanionManager(manager));
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalCompanionManager>().0.clone()
    }

    pub fn try_global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<GlobalCompanionManager>()
            .map(|global| global.0.clone())
    }

    pub fn bootstrap(cx: &mut App) {
        let manager = Self::global(cx);
        manager.update(cx, |manager, cx| {
            manager.apply_settings(AgentSettings::get_global(cx).mobile_companion.clone(), cx);
        });

        cx.observe_global::<SettingsStore>({
            let manager = manager.clone();
            move |cx| {
                manager.update(cx, |manager, cx| {
                    manager
                        .apply_settings(AgentSettings::get_global(cx).mobile_companion.clone(), cx);
                });
            }
        })
        .detach();

        if manager.read(cx).status().settings.auto_start {
            manager
                .update(cx, |manager, cx| manager.start(cx))
                .detach_and_log_err(cx);
        }
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        let persistent_state = load_or_create_persistent_state();
        let mirror = cx.new(|_| {
            CompanionSessionMirror::new(CompanionConnectionMetadata {
                token_id: String::new(),
                access_mode: CompanionAccessMode::Control,
                issued_at_unix_ms: None,
                expires_at_unix_ms: None,
            })
        });
        let mirror_subscription = cx.subscribe(&mirror, Self::handle_mirror_event);

        let mut manager = Self {
            mirror,
            status: CompanionManagerStatus::default(),
            persistent_state,
            server: None,
            follow_source: None,
            pinned_source: None,
            _mirror_subscription: mirror_subscription,
            _command_task: None,
        };
        manager.refresh_persistent_access_info();
        manager
    }

    pub fn mirror(&self) -> Entity<CompanionSessionMirror> {
        self.mirror.clone()
    }

    pub fn status(&self) -> &CompanionManagerStatus {
        &self.status
    }

    pub fn set_access_mode(&mut self, access_mode: CompanionAccessMode, cx: &mut Context<Self>) {
        if self.status.access_mode == access_mode {
            return;
        }

        self.status.access_mode = access_mode;

        if let Some(server) = &self.server {
            server.set_access_mode(access_mode);
            let snapshot = self.mirror.read(cx).snapshot().clone();
            server.publish_snapshot(snapshot.clone());
            server.publish_event(CompanionEvent::SnapshotReplaced { snapshot });
        }

        self.emit_status(cx);
    }

    pub fn set_follow_source(
        &mut self,
        source: Option<CompanionSessionSource>,
        cx: &mut Context<Self>,
    ) {
        self.follow_source = source;
        if self.status.selection_mode == CompanionSessionMode::FollowActive {
            self.sync_effective_source(cx);
            self.emit_status(cx);
        }
    }

    pub fn pin_source(&mut self, source: CompanionSessionSource, cx: &mut Context<Self>) {
        self.pinned_source = Some(source);
        self.status.selection_mode = CompanionSessionMode::Pinned;
        self.sync_effective_source(cx);
        self.emit_status(cx);
    }

    pub fn follow_active_session(&mut self, cx: &mut Context<Self>) {
        self.status.selection_mode = CompanionSessionMode::FollowActive;
        self.sync_effective_source(cx);
        self.emit_status(cx);
    }

    pub fn start(&mut self, cx: &mut Context<Self>) -> Task<Result<CompanionAccessInfo>> {
        self.sync_effective_source(cx);
        self.refresh_persistent_access_info();

        if let Some(access_info) = self.status.access_info.clone() {
            self.status.state = CompanionServiceState::Running;
            self.emit_status(cx);
            return Task::ready(Ok(access_info));
        }

        let token_id = Uuid::new_v4().to_string();
        let connection = CompanionConnectionMetadata {
            token_id: token_id.clone(),
            access_mode: self.status.access_mode,
            issued_at_unix_ms: unix_time_ms(),
            expires_at_unix_ms: None,
        };
        self.mirror
            .update(cx, |mirror, cx| mirror.set_connection(connection, cx));

        let initial_snapshot = self.mirror.read(cx).snapshot().clone();
        let initial_assets = self.mirror.read(cx).assets().to_vec();
        self.status.state = CompanionServiceState::Starting;
        self.status.shared_session = initial_snapshot.session.clone();
        self.emit_status(cx);

        let startup = Tokio::spawn_result(
            cx,
            server::start_server(
                initial_snapshot,
                initial_assets,
                token_id,
                self.persistent_state.token_id.clone(),
                self.status.settings.port,
                self.status.access_mode,
                server::default_client_html().to_string(),
            ),
        );

        cx.spawn(async move |this, cx| match startup.await {
            Ok(CompanionServerStart {
                handle,
                command_rx,
                connect_host,
            }) => {
                let access_info = handle.access_info().clone();

                this.update(cx, |this, cx| {
                    let command_task = cx.spawn(async move |this, cx| {
                        let mut command_rx = command_rx;
                        while let Some(request) = command_rx.next().await {
                            let server::CompanionCommandRequest {
                                command,
                                response_tx,
                            } = request;
                            let result = match this
                                .update(cx, |this, cx| this.handle_command(command, cx))
                            {
                                Ok(task) => task.await,
                                Err(error) => Err(error),
                            };
                            if response_tx
                                .send(result.map_err(|error| error.to_string()))
                                .is_err()
                            {
                                log::debug!(
                                    "mobile companion command response receiver dropped before completion"
                                );
                            }
                        }
                        Ok(())
                    });

                    this.update_persistent_host(connect_host);
                    this._command_task = Some(command_task);
                    this.server = Some(handle);
                    this.status.state = CompanionServiceState::Running;
                    this.status.access_info = Some(access_info.clone());
                    this.refresh_persistent_access_info();
                    this.emit_status(cx);
                    Ok(access_info)
                })?
            }
            Err(error) => {
                let message = error.to_string();
                this.update(cx, |this, cx| {
                    this.status.state = CompanionServiceState::Failed { message };
                    this.status.access_info = None;
                    this.refresh_persistent_access_info();
                    this.mirror.update(cx, |mirror, cx| {
                        mirror.set_connection(
                            CompanionConnectionMetadata {
                                token_id: String::new(),
                                access_mode: this.status.access_mode,
                                issued_at_unix_ms: None,
                                expires_at_unix_ms: None,
                            },
                            cx,
                        );
                    });
                    this.emit_status(cx);
                })?;
                Err(error)
            }
        })
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) -> Task<Result<()>> {
        self._command_task = None;

        let Some(server) = self.server.take() else {
            self.status.state = CompanionServiceState::Stopped;
            self.status.access_info = None;
            self.refresh_persistent_access_info();
            self.emit_status(cx);
            return Task::ready(Ok(()));
        };

        self.status.state = CompanionServiceState::Stopping;
        self.status.access_info = None;
        self.refresh_persistent_access_info();
        self.mirror.update(cx, |mirror, cx| {
            mirror.set_connection(
                CompanionConnectionMetadata {
                    token_id: String::new(),
                    access_mode: self.status.access_mode,
                    issued_at_unix_ms: None,
                    expires_at_unix_ms: None,
                },
                cx,
            );
        });
        self.emit_status(cx);

        cx.spawn(async move |this, cx| {
            server.stop().await?;
            this.update(cx, |this, cx| {
                this.status.state = CompanionServiceState::Stopped;
                this.status.access_info = None;
                this.refresh_persistent_access_info();
                this.emit_status(cx);
                Ok(())
            })?
        })
    }

    fn apply_settings(&mut self, settings: MobileCompanionSettings, cx: &mut Context<Self>) {
        let previous_settings = self.status.settings.clone();
        let previous_mode = self.status.selection_mode;
        self.status.settings = settings.clone();
        self.refresh_persistent_access_info();

        if self.pinned_source.is_none() {
            self.status.selection_mode = if settings.follow_active_session {
                CompanionSessionMode::FollowActive
            } else {
                CompanionSessionMode::Pinned
            };
        }

        if previous_mode != self.status.selection_mode {
            self.sync_effective_source(cx);
        }

        if previous_settings.port != self.status.settings.port && self.server.is_some() {
            let stop_task = self.stop(cx);
            cx.spawn(async move |this, cx| {
                stop_task.await?;
                let start_task = this.update(cx, |this, cx| this.start(cx))?;
                start_task.await?;
                Ok::<(), anyhow::Error>(())
            })
            .detach_and_log_err(cx);
            return;
        }

        if previous_settings != self.status.settings || previous_mode != self.status.selection_mode
        {
            self.emit_status(cx);
        }
    }

    fn effective_source(&self) -> Option<CompanionSessionSource> {
        match self.status.selection_mode {
            CompanionSessionMode::FollowActive => self.follow_source.clone(),
            CompanionSessionMode::Pinned => self.pinned_source.clone(),
        }
    }

    fn sync_effective_source(&mut self, cx: &mut Context<Self>) {
        let source = self.effective_source();
        self.mirror
            .update(cx, |mirror, cx| mirror.set_source(source, cx));
        self.status.shared_session = self.mirror.read(cx).snapshot().session.clone();
    }

    fn refresh_persistent_access_info(&mut self) {
        self.status.persistent_access_info = Some(self.persistent_access_info());
    }

    fn persistent_access_info(&self) -> CompanionAccessInfo {
        let connect_host = self
            .persistent_state
            .last_host
            .clone()
            .unwrap_or_else(server::discover_lan_host);
        server::build_access_info(
            &connect_host,
            self.status.settings.port,
            &self.persistent_state.token_id,
        )
    }

    fn update_persistent_host(&mut self, connect_host: String) {
        if self.persistent_state.last_host.as_deref() == Some(connect_host.as_str()) {
            return;
        }

        self.persistent_state.last_host = Some(connect_host);
        if let Err(error) = save_persistent_state(&self.persistent_state) {
            log::warn!("failed to save persistent mobile companion state: {error}");
        }
    }

    fn handle_command(
        &mut self,
        command: CompanionCommand,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        match command {
            CompanionCommand::SendMessage { text, attachments } => self
                .mirror
                .update(cx, |mirror, cx| mirror.send_message(text, attachments, cx)),
            CompanionCommand::AuthorizeToolCall {
                tool_call_id,
                option_id,
                option_kind,
            } => self.mirror.update(cx, |mirror, cx| {
                mirror.authorize_tool_call(tool_call_id, option_id, option_kind, cx)
            }),
            CompanionCommand::StopRun => self.mirror.update(cx, |mirror, cx| mirror.stop_run(cx)),
        }
    }

    fn handle_mirror_event(
        &mut self,
        mirror: Entity<CompanionSessionMirror>,
        event: &CompanionEvent,
        cx: &mut Context<Self>,
    ) {
        self.status.shared_session = mirror.read(cx).snapshot().session.clone();

        if let Some(server) = &self.server {
            server.publish_snapshot(mirror.read(cx).snapshot().clone());
            server.replace_assets(mirror.read(cx).assets().to_vec());
            server.publish_event(event.clone());
        }

        self.emit_status(cx);
    }

    fn emit_status(&mut self, cx: &mut Context<Self>) {
        cx.emit(CompanionManagerEvent::StatusChanged);
        cx.notify();
    }
}

fn unix_time_ms() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| duration.as_millis().try_into().ok())
}

fn load_or_create_persistent_state() -> PersistentCompanionState {
    match load_or_create_persistent_state_result() {
        Ok(state) => state,
        Err(error) => {
            log::warn!("failed to load persistent mobile companion state: {error}");
            let state = PersistentCompanionState {
                token_id: Uuid::new_v4().to_string(),
                last_host: None,
            };
            if let Err(save_error) = save_persistent_state(&state) {
                log::warn!("failed to save replacement mobile companion state: {save_error}");
            }
            state
        }
    }
}

fn load_or_create_persistent_state_result() -> Result<PersistentCompanionState> {
    let path = persistent_state_path();

    if path.exists() {
        let contents = fs::read_to_string(&path)?;
        let state = serde_json::from_str::<PersistentCompanionState>(&contents)?;
        if !state.token_id.trim().is_empty() {
            return Ok(state);
        }
    }

    let state = PersistentCompanionState {
        token_id: Uuid::new_v4().to_string(),
        last_host: None,
    };
    save_persistent_state(&state)?;
    Ok(state)
}

fn save_persistent_state(state: &PersistentCompanionState) -> Result<()> {
    let path = persistent_state_path();
    let parent = path
        .parent()
        .context("persistent mobile companion state path must have a parent")?;
    fs::create_dir_all(parent)?;
    fs::write(&path, serde_json::to_vec_pretty(state)?)?;
    Ok(())
}

fn persistent_state_path() -> PathBuf {
    data_dir()
        .join("mobile_companion")
        .join("persistent_token.json")
}
