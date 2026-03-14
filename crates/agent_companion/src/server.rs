use anyhow::{Context as _, Result};
use axum::{
    Json, Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::Arc,
};
use tokio::{
    net::TcpListener,
    sync::{broadcast, watch},
    task::JoinHandle,
};

use crate::static_client;
use crate::{CompanionCommand, CompanionEvent, CompanionSnapshot};

#[derive(Clone)]
pub struct CompanionServerState {
    token_id: Arc<str>,
    snapshot_rx: watch::Receiver<CompanionSnapshot>,
    event_tx: broadcast::Sender<CompanionEvent>,
    command_tx: UnboundedSender<CompanionCommand>,
    client_html: Arc<str>,
}

pub struct CompanionServerStart {
    pub handle: CompanionServerHandle,
    pub command_rx: UnboundedReceiver<CompanionCommand>,
}

pub struct CompanionServerHandle {
    snapshot_tx: watch::Sender<CompanionSnapshot>,
    event_tx: broadcast::Sender<CompanionEvent>,
    shutdown_tx: watch::Sender<bool>,
    join_handle: JoinHandle<Result<()>>,
    access_info: crate::CompanionAccessInfo,
}

impl CompanionServerHandle {
    pub fn access_info(&self) -> &crate::CompanionAccessInfo {
        &self.access_info
    }

    pub fn publish_snapshot(&self, snapshot: CompanionSnapshot) {
        let _ = self.snapshot_tx.send(snapshot);
    }

    pub fn publish_event(&self, event: CompanionEvent) {
        let _ = self.event_tx.send(event);
    }

    pub async fn stop(self) -> Result<()> {
        let _ = self.shutdown_tx.send(true);
        self.join_handle
            .await
            .context("companion server join error")?
    }
}

#[derive(serde::Deserialize)]
struct CompanionAuthQuery {
    token: String,
}

pub async fn start_server(
    initial_snapshot: CompanionSnapshot,
    token_id: String,
    client_html: String,
) -> Result<CompanionServerStart> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))
        .await
        .context("failed to bind companion server listener")?;
    let local_addr = listener
        .local_addr()
        .context("failed to read companion server address")?;

    let (snapshot_tx, snapshot_rx) = watch::channel(initial_snapshot);
    let (event_tx, _) = broadcast::channel(64);
    let (command_tx, command_rx) = mpsc::unbounded();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let app_state = CompanionServerState {
        token_id: Arc::from(token_id.clone()),
        snapshot_rx,
        event_tx: event_tx.clone(),
        command_tx,
        client_html: Arc::from(client_html),
    };

    let std_listener = listener
        .into_std()
        .context("failed to convert companion listener")?;
    std_listener
        .set_nonblocking(true)
        .context("failed to set companion listener nonblocking")?;

    let join_handle = tokio::spawn(async move {
        let mut shutdown_rx = shutdown_rx.clone();
        axum::Server::from_tcp(std_listener)
            .context("failed to create companion server")?
            .serve(router(app_state).into_make_service())
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.changed().await;
            })
            .await
            .context("companion server failed")
    });

    let connect_host = discover_lan_host();
    let access_info = crate::CompanionAccessInfo {
        url: format!(
            "http://{connect_host}:{}/companion?token={token_id}",
            local_addr.port()
        ),
        token_id,
    };

    Ok(CompanionServerStart {
        handle: CompanionServerHandle {
            snapshot_tx,
            event_tx,
            shutdown_tx,
            join_handle,
            access_info,
        },
        command_rx,
    })
}

pub fn default_client_html() -> &'static str {
    static_client::default_client_html()
}

fn router(state: CompanionServerState) -> Router {
    Router::new()
        .route("/companion", get(companion_client))
        .route("/companion/snapshot", get(companion_snapshot))
        .route("/companion/events", get(companion_events))
        .route("/companion/command", post(companion_command))
        .with_state(state)
}

async fn companion_client(
    State(state): State<CompanionServerState>,
    Query(query): Query<CompanionAuthQuery>,
) -> Result<Html<String>, StatusCode> {
    authorize(&state, &query)?;
    Ok(Html(state.client_html.to_string()))
}

async fn companion_snapshot(
    State(state): State<CompanionServerState>,
    Query(query): Query<CompanionAuthQuery>,
) -> Result<Json<CompanionSnapshot>, StatusCode> {
    authorize(&state, &query)?;
    Ok(Json(state.snapshot_rx.borrow().clone()))
}

async fn companion_command(
    State(state): State<CompanionServerState>,
    Query(query): Query<CompanionAuthQuery>,
    Json(command): Json<CompanionCommand>,
) -> Result<StatusCode, StatusCode> {
    authorize(&state, &query)?;
    state
        .command_tx
        .unbounded_send(command)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(StatusCode::ACCEPTED)
}

async fn companion_events(
    ws: WebSocketUpgrade,
    State(state): State<CompanionServerState>,
    Query(query): Query<CompanionAuthQuery>,
) -> Result<Response, StatusCode> {
    authorize(&state, &query)?;
    Ok(ws
        .on_upgrade(move |socket| stream_events(socket, state))
        .into_response())
}

async fn stream_events(mut socket: WebSocket, state: CompanionServerState) {
    let mut receiver = state.event_tx.subscribe();

    loop {
        match receiver.recv().await {
            Ok(event) => {
                let Ok(payload) = serde_json::to_string(&event) else {
                    break;
                };
                if socket.send(Message::Text(payload)).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

fn authorize(state: &CompanionServerState, query: &CompanionAuthQuery) -> Result<(), StatusCode> {
    if query.token == state.token_id.as_ref() {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn discover_lan_host() -> String {
    let Ok(socket) = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))) else {
        return Ipv4Addr::LOCALHOST.to_string();
    };
    if socket
        .connect(SocketAddr::from(([8, 8, 8, 8], 80)))
        .is_err()
    {
        return Ipv4Addr::LOCALHOST.to_string();
    }

    match socket.local_addr().map(|addr| addr.ip()) {
        Ok(IpAddr::V4(ip)) if !ip.is_loopback() => ip.to_string(),
        Ok(IpAddr::V6(ip)) if !ip.is_loopback() => ip.to_string(),
        Ok(_) | Err(_) => Ipv4Addr::LOCALHOST.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::http::{Method, Request};
    use axum::{body::Body, http::StatusCode};
    use futures::StreamExt as _;
    use futures::channel::mpsc;
    use hyper::body::to_bytes;
    use tokio::sync::{broadcast, watch};
    use tower::ServiceExt as _;

    use super::{CompanionServerState, default_client_html, router};
    use crate::{
        CompanionCommand, CompanionConnectionMetadata, CompanionRunStatus, CompanionSnapshot,
    };

    fn test_state() -> (
        CompanionServerState,
        futures::channel::mpsc::UnboundedReceiver<CompanionCommand>,
    ) {
        let (snapshot_tx, snapshot_rx) = watch::channel(CompanionSnapshot {
            protocol_version: 1,
            session: None,
            connection: CompanionConnectionMetadata {
                token_id: "test-token".into(),
                issued_at_unix_ms: None,
                expires_at_unix_ms: None,
            },
            messages: Vec::new(),
            streaming_text: None,
            tool_calls: Vec::new(),
            run_status: CompanionRunStatus::Idle,
            available_commands: Vec::new(),
        });
        let _snapshot_tx = snapshot_tx;
        let (event_tx, _) = broadcast::channel(16);
        let (command_tx, command_rx) = mpsc::unbounded();

        (
            CompanionServerState {
                token_id: Arc::from("test-token"),
                snapshot_rx,
                event_tx,
                command_tx,
                client_html: Arc::from(default_client_html().to_string()),
            },
            command_rx,
        )
    }

    #[tokio::test]
    async fn snapshot_route_requires_token() {
        let (state, _) = test_state();
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/companion/snapshot?token=wrong")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn client_route_returns_embedded_ui() {
        let (state, _) = test_state();
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/companion?token=test-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body()).await.expect("body");
        let html = String::from_utf8(body.to_vec()).expect("utf8");
        assert!(html.contains("Agent Companion"));
        assert!(html.contains("Tool Timeline"));
    }

    #[tokio::test]
    async fn snapshot_route_returns_current_snapshot() {
        let (state, _) = test_state();
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/companion/snapshot?token=test-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body()).await.expect("body");
        let snapshot: CompanionSnapshot = serde_json::from_slice(&body).expect("snapshot");
        assert_eq!(snapshot.connection.token_id, "test-token");
        assert_eq!(snapshot.run_status, CompanionRunStatus::Idle);
    }

    #[tokio::test]
    async fn command_route_forwards_payload() {
        let (state, mut command_rx) = test_state();
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/companion/command?token=test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&CompanionCommand::SendMessage {
                            text: "hello from mobile".into(),
                        })
                        .expect("json"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let command = command_rx.next().await.expect("command");
        assert_eq!(
            command,
            CompanionCommand::SendMessage {
                text: "hello from mobile".into(),
            }
        );
    }
}
