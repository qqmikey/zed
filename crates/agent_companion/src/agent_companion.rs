mod protocol;
mod server;
mod session_mirror;
mod static_client;

use anyhow::Result;
use futures::StreamExt as _;
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global, Subscription, Task};
use gpui_tokio::Tokio;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub use protocol::{
    CompanionCommand, CompanionCommandKind, CompanionConnectionMetadata, CompanionEvent,
    CompanionMessage, CompanionMessageRole, CompanionMessageStatus, CompanionRunStatus,
    CompanionSessionSummary, CompanionSnapshot, CompanionToolCall, CompanionToolCallStatus,
};
pub use server::{CompanionServerHandle, CompanionServerStart, CompanionServerState};
pub use session_mirror::{CompanionSessionMirror, CompanionSessionSource};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompanionManagerStatus {
    pub state: CompanionServiceState,
    pub active_session: Option<CompanionSessionSummary>,
    pub access_info: Option<CompanionAccessInfo>,
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
pub struct CompanionSessionTarget {
    pub id: String,
    pub title: String,
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
    server: Option<CompanionServerHandle>,
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

    pub fn new(cx: &mut Context<Self>) -> Self {
        let mirror = cx.new(|_| {
            CompanionSessionMirror::new(CompanionConnectionMetadata {
                token_id: String::new(),
                issued_at_unix_ms: None,
                expires_at_unix_ms: None,
            })
        });
        let mirror_subscription = cx.subscribe(&mirror, Self::handle_mirror_event);

        Self {
            mirror,
            status: CompanionManagerStatus::default(),
            server: None,
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

    pub fn set_source(&mut self, source: Option<CompanionSessionSource>, cx: &mut Context<Self>) {
        self.mirror
            .update(cx, |mirror, cx| mirror.set_source(source, cx));
        self.status.active_session = self.mirror.read(cx).snapshot().session.clone();
        self.emit_status(cx);
    }

    pub fn start(
        &mut self,
        source: CompanionSessionSource,
        cx: &mut Context<Self>,
    ) -> Task<Result<CompanionAccessInfo>> {
        self.set_source(Some(source), cx);

        if let Some(access_info) = self.status.access_info.clone() {
            self.status.state = CompanionServiceState::Running;
            self.emit_status(cx);
            return Task::ready(Ok(access_info));
        }

        let token_id = Uuid::new_v4().to_string();
        let connection = CompanionConnectionMetadata {
            token_id: token_id.clone(),
            issued_at_unix_ms: unix_time_ms(),
            expires_at_unix_ms: None,
        };
        self.mirror
            .update(cx, |mirror, cx| mirror.set_connection(connection, cx));

        let initial_snapshot = self.mirror.read(cx).snapshot().clone();
        self.status.state = CompanionServiceState::Starting;
        self.status.active_session = initial_snapshot.session.clone();
        self.emit_status(cx);

        let startup = Tokio::spawn_result(
            cx,
            server::start_server(
                initial_snapshot,
                token_id,
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
                    this.emit_status(cx);
                    Ok(access_info)
                })?
            }
            Err(error) => {
                let message = error.to_string();
                this.update(cx, |this, cx| {
                    this.status.state = CompanionServiceState::Failed { message };
                    this.status.access_info = None;
                    this.mirror.update(cx, |mirror, cx| {
                        mirror.set_connection(
                            CompanionConnectionMetadata {
                                token_id: String::new(),
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
            self.emit_status(cx);
            return Task::ready(Ok(()));
        };

        self.status.state = CompanionServiceState::Stopping;
        self.status.access_info = None;
        self.mirror.update(cx, |mirror, cx| {
            mirror.set_connection(
                CompanionConnectionMetadata {
                    token_id: String::new(),
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
                this.emit_status(cx);
                Ok(())
            })?
        })
    }

    fn handle_command(&mut self, command: CompanionCommand, cx: &mut Context<Self>) -> Result<()> {
        match command {
            CompanionCommand::SendMessage { text } => {
                self.mirror
                    .update(cx, |mirror, cx| mirror.send_message(text, cx))
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
        self.status.active_session = mirror.read(cx).snapshot().session.clone();

        if let Some(server) = &self.server {
            server.publish_snapshot(mirror.read(cx).snapshot().clone());
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
