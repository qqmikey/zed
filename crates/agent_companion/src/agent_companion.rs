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
use url::Url;
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
    persistent_token_id: String,
    server: Option<CompanionServerHandle>,
    follow_source: Option<CompanionSessionSource>,
    pinned_source: Option<CompanionSessionSource>,
    _mirror_subscription: Subscription,
    _command_task: Option<Task<Result<()>>>,
}

impl EventEmitter<CompanionManagerEvent> for CompanionManager {}

impl CompanionManager {
    pub fn init(cx: &mut App) {
        let manager = cx.new(|cx| Self::new(cx));
        cx.set_global(GlobalCompanionManager(manager));
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalCompanionManager>().0.clone()
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
        let persistent_token_id = load_or_create_persistent_token();
        let mirror = cx.new(|_| {
            CompanionSessionMirror::new(CompanionConnectionMetadata {
                token_id: String::new(),
                access_mode: CompanionAccessMode::Control,
                issued_at_unix_ms: None,
                expires_at_unix_ms: None,
            })
        });
        let mirror_subscription = cx.subscribe(&mirror, Self::handle_mirror_event);

        Self {
            mirror,
            status: CompanionManagerStatus::default(),
            persistent_token_id,
            server: None,
            follow_source: None,
            pinned_source: None,
            _mirror_subscription: mirror_subscription,
            _command_task: None,
        }
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
                self.persistent_token_id.clone(),
                self.status.access_mode,
                server::default_client_html().to_string(),
            ),
        );

        cx.spawn(async move |this, cx| match startup.await {
            Ok(CompanionServerStart { handle, command_rx }) => {
                let access_info = handle.access_info().clone();

                this.update(cx, |this, cx| {
                    let command_task = cx.spawn(async move |this, cx| {
                        let mut command_rx = command_rx;
                        while let Some(command) = command_rx.next().await {
                            this.update(cx, |this, cx| this.handle_command(command, cx))??;
                        }
                        Ok(())
                    });

                    this._command_task = Some(command_task);
                    this.server = Some(handle);
                    this.status.state = CompanionServiceState::Running;
                    this.status.access_info = Some(access_info.clone());
                    this.status.persistent_access_info = Some(CompanionAccessInfo {
                        url: rewrite_companion_token(&access_info.url, &this.persistent_token_id)?,
                        token_id: this.persistent_token_id.clone(),
                    });
                    this.emit_status(cx);
                    Ok(access_info)
                })?
            }
            Err(error) => {
                let message = error.to_string();
                this.update(cx, |this, cx| {
                    this.status.state = CompanionServiceState::Failed { message };
                    this.status.access_info = None;
                    this.status.persistent_access_info = None;
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
            self.status.persistent_access_info = None;
            self.emit_status(cx);
            return Task::ready(Ok(()));
        };

        self.status.state = CompanionServiceState::Stopping;
        self.status.access_info = None;
        self.status.persistent_access_info = None;
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
                this.status.persistent_access_info = None;
                this.emit_status(cx);
                Ok(())
            })?
        })
    }

    fn apply_settings(&mut self, settings: MobileCompanionSettings, cx: &mut Context<Self>) {
        let previous_settings = self.status.settings.clone();
        let previous_mode = self.status.selection_mode;
        self.status.settings = settings.clone();

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

    fn handle_command(&mut self, command: CompanionCommand, cx: &mut Context<Self>) -> Result<()> {
        match command {
            CompanionCommand::SendMessage { text, attachments } => {
                self.mirror
                    .update(cx, |mirror, cx| mirror.send_message(text, attachments, cx))
                    .detach_and_log_err(cx);
            }
            CompanionCommand::AuthorizeToolCall {
                tool_call_id,
                option_id,
                option_kind,
            } => {
                self.mirror
                    .update(cx, |mirror, cx| {
                        mirror.authorize_tool_call(tool_call_id, option_id, option_kind, cx)
                    })
                    .detach_and_log_err(cx);
            }
            CompanionCommand::StopRun => {
                self.mirror
                    .update(cx, |mirror, cx| mirror.stop_run(cx))
                    .detach_and_log_err(cx);
            }
        }

        Ok(())
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

#[derive(serde::Deserialize, serde::Serialize)]
struct PersistentCompanionToken {
    token_id: String,
}

fn load_or_create_persistent_token() -> String {
    match load_or_create_persistent_token_result() {
        Ok(token_id) => token_id,
        Err(error) => {
            log::warn!("failed to load persistent mobile companion token: {error}");
            Uuid::new_v4().to_string()
        }
    }
}

fn load_or_create_persistent_token_result() -> Result<String> {
    let path = persistent_token_path();

    if path.exists() {
        let contents = fs::read_to_string(&path)?;
        let token = serde_json::from_str::<PersistentCompanionToken>(&contents)?;
        if !token.token_id.trim().is_empty() {
            return Ok(token.token_id);
        }
    }

    let token = PersistentCompanionToken {
        token_id: Uuid::new_v4().to_string(),
    };
    let parent = path
        .parent()
        .context("persistent mobile companion token path must have a parent")?;
    fs::create_dir_all(parent)?;
    fs::write(&path, serde_json::to_vec_pretty(&token)?)?;
    Ok(token.token_id)
}

fn persistent_token_path() -> PathBuf {
    data_dir()
        .join("mobile_companion")
        .join("persistent_token.json")
}

fn rewrite_companion_token(url: &str, token_id: &str) -> Result<String> {
    let mut url = Url::parse(url)?;
    {
        let mut query_pairs = url.query_pairs_mut();
        query_pairs.clear().append_pair("token", token_id);
    }
    Ok(url.to_string())
}
