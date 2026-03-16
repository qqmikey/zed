mod protocol;
mod server;
mod session_mirror;
mod static_client;

use agent_client_protocol as acp;
use agent_settings::{AgentSettings, MobileCompanionSettings};
use anyhow::{Context as _, Result};
use futures::StreamExt as _;
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global, Subscription, Task};
use gpui_tokio::Tokio;
use paths::data_dir;
use settings::{Settings as _, SettingsStore};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub use protocol::{
    CompanionAccessMode, CompanionAttachment, CompanionCommand, CompanionCommandKind,
    CompanionCommandResponse, CompanionComposerDraft, CompanionConnectionMetadata, CompanionEvent,
    CompanionMessage, CompanionMessageRole, CompanionMessageStatus, CompanionPermissionChoice,
    CompanionPermissionOption, CompanionPermissionRequest, CompanionQueuedMessage,
    CompanionRunStatus, CompanionSessionSummary, CompanionSnapshot, CompanionTimelineEntry,
    CompanionTimelinePage, CompanionTimelineToolCall, CompanionTimelineToolCallDetail,
    CompanionToolCall, CompanionToolCallStatus, CompanionUpload,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompanionQueueState {
    pub queued_messages: Vec<CompanionQueuedMessage>,
    pub assets: Vec<CompanionAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompanionQueueDraft {
    pub composer_draft: CompanionComposerDraft,
    pub assets: Vec<CompanionAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompanionComposerSubmission {
    pub text: String,
    pub attachments: Vec<CompanionUpload>,
    pub draft_id: Option<String>,
    pub retained_attachment_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompanionPreparedContent {
    pub text: String,
    pub rendered_html: Option<String>,
    pub attachments: Vec<CompanionAttachment>,
    pub assets: Vec<CompanionAsset>,
}

pub trait CompanionQueueController {
    fn queue_state(&self, token_id: &str, cx: &App) -> Result<CompanionQueueState>;

    fn send_or_queue_message(
        &self,
        submission: CompanionComposerSubmission,
        cx: &mut App,
    ) -> Task<Result<()>>;

    fn remove_queued_message(&self, queued_message_id: &str, cx: &mut App) -> Result<()>;

    fn send_queued_message_now(&self, queued_message_id: &str, cx: &mut App) -> Task<Result<()>>;

    fn edit_queued_message(
        &self,
        queued_message_id: &str,
        token_id: &str,
        cx: &mut App,
    ) -> Result<CompanionQueueDraft>;

    fn discard_draft(&self, draft_id: &str, cx: &mut App) -> Result<()>;

    fn clear_queued_messages(&self, cx: &mut App) -> Result<()>;
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
    queue_controller: Option<Rc<dyn CompanionQueueController>>,
    queue_state: CompanionQueueState,
    draft_assets_by_id: HashMap<String, Vec<CompanionAsset>>,
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
            queue_controller: None,
            queue_state: CompanionQueueState::default(),
            draft_assets_by_id: HashMap::default(),
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

    pub fn set_queue_controller(
        &mut self,
        queue_controller: Option<Rc<dyn CompanionQueueController>>,
        cx: &mut Context<Self>,
    ) {
        let did_change = match (&self.queue_controller, &queue_controller) {
            (Some(current), Some(next)) => !Rc::ptr_eq(current, next),
            (None, None) => false,
            _ => true,
        };
        if !did_change {
            self.refresh_queue_state(cx);
            return;
        }

        self.queue_controller = queue_controller;
        self.draft_assets_by_id.clear();
        if self.queue_controller.is_none() {
            self.queue_state = CompanionQueueState::default();
        } else {
            self.refresh_queue_state(cx);
        }
        if let Some(server) = &self.server {
            server.replace_assets(self.combined_assets(cx));
            server.publish_snapshot(self.snapshot(cx));
        }
    }

    pub fn refresh_queue_state(&mut self, cx: &mut Context<Self>) {
        let Some(next_state) = self.current_queue_state(cx) else {
            return;
        };

        if self.queue_state == next_state {
            return;
        }

        self.queue_state = next_state;

        if let Some(server) = &self.server {
            let snapshot = self.snapshot(cx);
            server.replace_assets(self.combined_assets(cx));
            server.publish_snapshot(snapshot.clone());
            server.publish_event(CompanionEvent::QueueChanged {
                available_commands: snapshot.available_commands.clone(),
                queued_messages: snapshot.queued_messages,
            });
        }

        self.emit_status(cx);
    }

    fn current_queue_state(&self, cx: &App) -> Option<CompanionQueueState> {
        self.queue_controller
            .as_ref()
            .map(|controller| controller.queue_state(&self.current_token_id(cx), cx))
            .transpose()
            .map_err(|error| {
                log::warn!("failed to read companion queue state: {error:#}");
            })
            .ok()
            .flatten()
    }

    pub fn set_access_mode(&mut self, access_mode: CompanionAccessMode, cx: &mut Context<Self>) {
        if self.status.access_mode == access_mode {
            return;
        }

        self.status.access_mode = access_mode;

        if let Some(server) = &self.server {
            server.set_access_mode(access_mode);
            let snapshot = self.snapshot(cx);
            server.publish_snapshot(snapshot.clone());
            server.replace_assets(self.combined_assets(cx));
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

        self.refresh_queue_state(cx);
        let initial_snapshot = self.snapshot(cx);
        let initial_assets = self.combined_assets(cx);
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

    fn snapshot(&self, cx: &App) -> CompanionSnapshot {
        self.snapshot_with_queue_state(&self.queue_state, cx)
    }

    fn snapshot_with_queue_state(
        &self,
        queue_state: &CompanionQueueState,
        cx: &App,
    ) -> CompanionSnapshot {
        let mut snapshot = self.mirror.read(cx).snapshot().clone();
        snapshot.available_commands =
            self.available_commands(&snapshot.available_commands, queue_state);
        snapshot.queued_messages = queue_state.queued_messages.clone();
        snapshot
    }

    fn available_commands(
        &self,
        mirror_commands: &[CompanionCommandKind],
        queue_state: &CompanionQueueState,
    ) -> Vec<CompanionCommandKind> {
        let mut commands = mirror_commands.to_vec();
        if !queue_state.queued_messages.is_empty() {
            commands.extend([
                CompanionCommandKind::EditQueuedMessage,
                CompanionCommandKind::SendQueuedMessageNow,
                CompanionCommandKind::RemoveQueuedMessage,
                CompanionCommandKind::ClearQueuedMessages,
            ]);
        }
        commands
    }

    fn combined_assets(&self, cx: &App) -> Vec<CompanionAsset> {
        self.combined_assets_with_queue_state(&self.queue_state, cx)
    }

    fn combined_assets_with_queue_state(
        &self,
        queue_state: &CompanionQueueState,
        cx: &App,
    ) -> Vec<CompanionAsset> {
        let mut assets = self.mirror.read(cx).assets().to_vec();
        assets.extend(queue_state.assets.iter().cloned());
        assets.extend(
            self.draft_assets_by_id
                .values()
                .flat_map(|assets| assets.iter().cloned()),
        );
        assets
    }

    fn current_token_id(&self, cx: &App) -> String {
        self.mirror.read(cx).snapshot().connection.token_id.clone()
    }

    fn handle_command(
        &mut self,
        command: CompanionCommand,
        cx: &mut Context<Self>,
    ) -> Task<Result<CompanionCommandResponse>> {
        match command {
            CompanionCommand::SendMessage {
                text,
                attachments,
                draft_id,
                retained_attachment_ids,
            } => {
                let submission = CompanionComposerSubmission {
                    text,
                    attachments,
                    draft_id: draft_id.clone(),
                    retained_attachment_ids,
                };

                if let Some(queue_controller) = self.queue_controller.clone() {
                    let task = queue_controller.send_or_queue_message(submission, cx);
                    return cx.spawn(async move |this, cx| {
                        task.await?;
                        if let Some(draft_id) = draft_id {
                            this.update(cx, |this, cx| {
                                this.draft_assets_by_id.remove(&draft_id);
                                if let Some(server) = &this.server {
                                    server.replace_assets(this.combined_assets(cx));
                                }
                            })?;
                        }
                        Ok(CompanionCommandResponse::default())
                    });
                }

                if draft_id.is_some() {
                    return Task::ready(Err(anyhow::anyhow!(
                        "companion draft editing is unavailable for this session"
                    )));
                }

                self.mirror.update(cx, |mirror, cx| {
                    let task = mirror.send_message(submission.text, submission.attachments, cx);
                    cx.spawn(async move |_this, _cx| {
                        task.await?;
                        Ok(CompanionCommandResponse::default())
                    })
                })
            }
            CompanionCommand::RemoveQueuedMessage { queued_message_id } => {
                let Some(queue_controller) = self.queue_controller.clone() else {
                    return Task::ready(Err(anyhow::anyhow!(
                        "message queue is unavailable for this session"
                    )));
                };
                let result = queue_controller.remove_queued_message(&queued_message_id, cx);
                if result.is_ok() {
                    self.refresh_queue_state(cx);
                }
                Task::ready(result.map(|()| CompanionCommandResponse::default()))
            }
            CompanionCommand::SendQueuedMessageNow { queued_message_id } => {
                let Some(queue_controller) = self.queue_controller.clone() else {
                    return Task::ready(Err(anyhow::anyhow!(
                        "message queue is unavailable for this session"
                    )));
                };
                let task = queue_controller.send_queued_message_now(&queued_message_id, cx);
                cx.spawn(async move |_this, _cx| {
                    task.await?;
                    Ok(CompanionCommandResponse::default())
                })
            }
            CompanionCommand::EditQueuedMessage { queued_message_id } => {
                let Some(queue_controller) = self.queue_controller.clone() else {
                    return Task::ready(Err(anyhow::anyhow!(
                        "message queue is unavailable for this session"
                    )));
                };
                let draft = queue_controller.edit_queued_message(
                    &queued_message_id,
                    &self.current_token_id(cx),
                    cx,
                );
                match draft {
                    Ok(draft) => {
                        self.draft_assets_by_id
                            .insert(draft.composer_draft.id.clone(), draft.assets.clone());
                        self.refresh_queue_state(cx);
                        if let Some(server) = &self.server {
                            server.replace_assets(self.combined_assets(cx));
                        }
                        Task::ready(Ok(CompanionCommandResponse {
                            composer_draft: Some(draft.composer_draft),
                        }))
                    }
                    Err(error) => Task::ready(Err(error)),
                }
            }
            CompanionCommand::DiscardComposerDraft { draft_id } => {
                let Some(queue_controller) = self.queue_controller.clone() else {
                    return Task::ready(Err(anyhow::anyhow!(
                        "message queue is unavailable for this session"
                    )));
                };
                let result = queue_controller.discard_draft(&draft_id, cx);
                if result.is_ok() {
                    self.draft_assets_by_id.remove(&draft_id);
                    if let Some(server) = &self.server {
                        server.replace_assets(self.combined_assets(cx));
                    }
                }
                Task::ready(result.map(|()| CompanionCommandResponse::default()))
            }
            CompanionCommand::ClearQueuedMessages => {
                let Some(queue_controller) = self.queue_controller.clone() else {
                    return Task::ready(Err(anyhow::anyhow!(
                        "message queue is unavailable for this session"
                    )));
                };
                let result = queue_controller.clear_queued_messages(cx);
                if result.is_ok() {
                    self.refresh_queue_state(cx);
                }
                Task::ready(result.map(|()| CompanionCommandResponse::default()))
            }
            CompanionCommand::AuthorizeToolCall {
                tool_call_id,
                option_id,
                option_kind,
            } => self.mirror.update(cx, |mirror, cx| {
                let task = mirror.authorize_tool_call(tool_call_id, option_id, option_kind, cx);
                cx.spawn(async move |_this, _cx| {
                    task.await?;
                    Ok(CompanionCommandResponse::default())
                })
            }),
            CompanionCommand::StopRun => self.mirror.update(cx, |mirror, cx| {
                let task = mirror.stop_run(cx);
                cx.spawn(async move |_this, _cx| {
                    task.await?;
                    Ok(CompanionCommandResponse::default())
                })
            }),
        }
    }

    fn handle_mirror_event(
        &mut self,
        mirror: Entity<CompanionSessionMirror>,
        event: &CompanionEvent,
        cx: &mut Context<Self>,
    ) {
        self.status.shared_session = mirror.read(cx).snapshot().session.clone();
        let queue_state = self
            .current_queue_state(cx)
            .unwrap_or_else(|| self.queue_state.clone());
        let queue_changed = self.queue_state != queue_state;
        self.queue_state = queue_state;

        if let Some(server) = &self.server {
            let snapshot = self.snapshot_with_queue_state(&self.queue_state, cx);
            server.publish_snapshot(snapshot.clone());
            server.replace_assets(self.combined_assets_with_queue_state(&self.queue_state, cx));
            if queue_changed {
                server.publish_event(CompanionEvent::QueueChanged {
                    available_commands: snapshot.available_commands.clone(),
                    queued_messages: snapshot.queued_messages.clone(),
                });
            }
            let outbound_event = match event {
                CompanionEvent::SnapshotReplaced { .. } => {
                    CompanionEvent::SnapshotReplaced { snapshot }
                }
                _ => event.clone(),
            };
            server.publish_event(outbound_event);
        }

        self.emit_status(cx);
    }

    fn emit_status(&mut self, cx: &mut Context<Self>) {
        cx.emit(CompanionManagerEvent::StatusChanged);
        cx.notify();
    }
}

pub fn prepare_companion_content(
    blocks: &[acp::ContentBlock],
    message_id: &str,
    token_id: &str,
) -> CompanionPreparedContent {
    let content = session_mirror::prepared_content_from_acp_blocks(blocks, message_id, token_id);
    CompanionPreparedContent {
        text: content.text,
        rendered_html: content.rendered_html,
        attachments: content.attachments,
        assets: content.assets,
    }
}

pub fn build_companion_upload_blocks(
    uploads: Vec<CompanionUpload>,
) -> Result<Vec<acp::ContentBlock>> {
    session_mirror::companion_upload_blocks(uploads)
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

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use gpui::TestAppContext;

    use super::*;

    enum QueueStateResponse {
        State(CompanionQueueState),
        Unavailable,
    }

    struct FakeQueueController {
        response: QueueStateResponse,
    }

    impl FakeQueueController {
        fn with_state(queue_state: CompanionQueueState) -> Self {
            Self {
                response: QueueStateResponse::State(queue_state),
            }
        }

        fn unavailable() -> Self {
            Self {
                response: QueueStateResponse::Unavailable,
            }
        }
    }

    impl CompanionQueueController for FakeQueueController {
        fn queue_state(&self, _token_id: &str, _cx: &App) -> Result<CompanionQueueState> {
            match &self.response {
                QueueStateResponse::State(queue_state) => Ok(queue_state.clone()),
                QueueStateResponse::Unavailable => Err(anyhow!("queue state unavailable")),
            }
        }

        fn send_or_queue_message(
            &self,
            _submission: CompanionComposerSubmission,
            _cx: &mut App,
        ) -> Task<Result<()>> {
            Task::ready(Ok(()))
        }

        fn remove_queued_message(&self, _queued_message_id: &str, _cx: &mut App) -> Result<()> {
            Ok(())
        }

        fn send_queued_message_now(
            &self,
            _queued_message_id: &str,
            _cx: &mut App,
        ) -> Task<Result<()>> {
            Task::ready(Ok(()))
        }

        fn edit_queued_message(
            &self,
            _queued_message_id: &str,
            _token_id: &str,
            _cx: &mut App,
        ) -> Result<CompanionQueueDraft> {
            Err(anyhow!("unused in test"))
        }

        fn discard_draft(&self, _draft_id: &str, _cx: &mut App) -> Result<()> {
            Ok(())
        }

        fn clear_queued_messages(&self, _cx: &mut App) -> Result<()> {
            Ok(())
        }
    }

    fn queued_state(id: &str, text: &str) -> CompanionQueueState {
        CompanionQueueState {
            queued_messages: vec![CompanionQueuedMessage {
                id: id.to_string(),
                is_next: true,
                text: text.to_string(),
                rendered_html: None,
                attachments: Vec::new(),
            }],
            assets: Vec::new(),
        }
    }

    #[gpui::test]
    async fn set_queue_controller_preserves_existing_queue_on_read_failure(
        cx: &mut TestAppContext,
    ) {
        let manager = cx.update(|cx| cx.new(|cx| CompanionManager::new(cx)));
        let previous_queue_state = queued_state("queued-1", "keep me");

        cx.update(|cx| {
            manager.update(cx, |manager, cx| {
                manager.queue_state = previous_queue_state.clone();
                manager.set_queue_controller(Some(Rc::new(FakeQueueController::unavailable())), cx);
            });
        });

        let actual_queue_state = cx.read(|cx| manager.read(cx).queue_state.clone());
        assert_eq!(actual_queue_state, previous_queue_state);
    }

    #[gpui::test]
    async fn set_queue_controller_clears_queue_when_controller_removed(cx: &mut TestAppContext) {
        let manager = cx.update(|cx| cx.new(|cx| CompanionManager::new(cx)));
        let queue_state = queued_state("queued-1", "clear me");

        cx.update(|cx| {
            manager.update(cx, |manager, cx| {
                manager.set_queue_controller(
                    Some(Rc::new(FakeQueueController::with_state(
                        queue_state.clone(),
                    ))),
                    cx,
                );
                manager.set_queue_controller(None, cx);
            });
        });

        let actual_queue_state = cx.read(|cx| manager.read(cx).queue_state.clone());
        assert_eq!(actual_queue_state, CompanionQueueState::default());
    }

    #[gpui::test]
    async fn set_queue_controller_replaces_queue_when_new_state_is_available(
        cx: &mut TestAppContext,
    ) {
        let manager = cx.update(|cx| cx.new(|cx| CompanionManager::new(cx)));
        let previous_queue_state = queued_state("queued-1", "old");
        let next_queue_state = queued_state("queued-2", "new");

        cx.update(|cx| {
            manager.update(cx, |manager, cx| {
                manager.queue_state = previous_queue_state;
                manager.set_queue_controller(
                    Some(Rc::new(FakeQueueController::with_state(
                        next_queue_state.clone(),
                    ))),
                    cx,
                );
            });
        });

        let actual_queue_state = cx.read(|cx| manager.read(cx).queue_state.clone());
        assert_eq!(actual_queue_state, next_queue_state);
    }
}
