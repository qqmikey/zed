use anyhow::{Context as _, Result};
use axum::{
    Json, Router,
    body::Body,
    extract::{
        DefaultBodyLimit, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{
        StatusCode,
        header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE},
    },
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use paths::data_dir;
use std::{
    collections::{HashMap, HashSet},
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    path::{Path as StdPath, PathBuf},
    sync::{Arc, RwLock},
};
use tokio::{
    fs,
    net::TcpListener,
    sync::{broadcast, watch},
    task::JoinHandle,
};

use crate::static_client;
use crate::{
    CompanionAccessMode, CompanionAsset, CompanionAssetDisposition, CompanionAssetSource,
    CompanionCommand, CompanionCommandKind, CompanionEvent, CompanionSnapshot,
    CompanionTimelineEntry, CompanionTimelinePage,
};

const COMMAND_BODY_LIMIT_BYTES: usize = 32 * 1024 * 1024;
const TIMELINE_PAGE_SIZE: usize = 40;
const MAX_TIMELINE_PAGE_SIZE: usize = 100;

#[derive(Clone)]
pub struct CompanionServerState {
    authorized_tokens: Arc<HashSet<String>>,
    snapshot_rx: watch::Receiver<CompanionSnapshot>,
    event_tx: broadcast::Sender<CompanionEvent>,
    command_tx: UnboundedSender<CompanionCommand>,
    access_mode: Arc<RwLock<CompanionAccessMode>>,
    assets: Arc<RwLock<HashMap<String, CompanionAsset>>>,
    client_html: Arc<str>,
}

pub struct CompanionServerStart {
    pub handle: CompanionServerHandle,
    pub command_rx: UnboundedReceiver<CompanionCommand>,
}

pub struct CompanionServerHandle {
    snapshot_tx: watch::Sender<CompanionSnapshot>,
    event_tx: broadcast::Sender<CompanionEvent>,
    access_mode: Arc<RwLock<CompanionAccessMode>>,
    assets: Arc<RwLock<HashMap<String, CompanionAsset>>>,
    asset_storage_dir: PathBuf,
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

    pub fn set_access_mode(&self, access_mode: CompanionAccessMode) {
        if let Ok(mut current_access_mode) = self.access_mode.write() {
            *current_access_mode = access_mode;
        }
    }

    pub fn replace_assets(&self, assets: Vec<CompanionAsset>) {
        if let Ok(mut registered_assets) = self.assets.write() {
            let previous_assets = registered_assets.clone();
            *registered_assets = assets
                .into_iter()
                .map(|asset| materialize_asset(asset, &previous_assets, &self.asset_storage_dir))
                .map(|asset| (asset.id.clone(), asset))
                .collect();
        }
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

#[derive(serde::Deserialize)]
struct CompanionTimelineQuery {
    token: String,
    #[serde(default)]
    before_entry_id: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

pub async fn start_server(
    initial_snapshot: CompanionSnapshot,
    initial_assets: Vec<CompanionAsset>,
    token_id: String,
    persistent_token_id: String,
    access_mode: CompanionAccessMode,
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
    let access_mode = Arc::new(RwLock::new(access_mode));
    let asset_storage_dir = companion_asset_storage_dir(&token_id);
    let assets = Arc::new(RwLock::new(
        initial_assets
            .into_iter()
            .map(|asset| {
                let existing_assets = HashMap::new();
                materialize_asset(asset, &existing_assets, &asset_storage_dir)
            })
            .map(|asset| (asset.id.clone(), asset))
            .collect(),
    ));

    let app_state = CompanionServerState {
        authorized_tokens: Arc::new(
            [token_id.clone(), persistent_token_id]
                .into_iter()
                .collect(),
        ),
        snapshot_rx,
        event_tx: event_tx.clone(),
        command_tx,
        access_mode: access_mode.clone(),
        assets: assets.clone(),
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
            access_mode,
            assets,
            asset_storage_dir,
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
        .route("/companion/timeline", get(companion_timeline))
        .route("/companion/events", get(companion_events))
        .route("/companion/assets/:asset_id", get(companion_asset))
        .route(
            "/companion/command",
            post(companion_command).layer(DefaultBodyLimit::max(COMMAND_BODY_LIMIT_BYTES)),
        )
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
    Ok(Json(snapshot_for_access_mode(
        &state.snapshot_rx.borrow(),
        current_access_mode(&state),
    )))
}

async fn companion_timeline(
    State(state): State<CompanionServerState>,
    Query(query): Query<CompanionTimelineQuery>,
) -> Result<Json<CompanionTimelinePage>, StatusCode> {
    authorize(
        &state,
        &CompanionAuthQuery {
            token: query.token.clone(),
        },
    )?;
    let limit = query
        .limit
        .unwrap_or(TIMELINE_PAGE_SIZE)
        .min(MAX_TIMELINE_PAGE_SIZE)
        .max(1);
    let page = timeline_page(
        &state.snapshot_rx.borrow().timeline,
        query.before_entry_id.as_deref(),
        limit,
    );
    Ok(Json(page))
}

async fn companion_command(
    State(state): State<CompanionServerState>,
    Query(query): Query<CompanionAuthQuery>,
    Json(command): Json<CompanionCommand>,
) -> Result<StatusCode, StatusCode> {
    authorize(&state, &query)?;
    if current_access_mode(&state) == CompanionAccessMode::ReadOnly {
        return Err(StatusCode::FORBIDDEN);
    }
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

async fn companion_asset(
    State(state): State<CompanionServerState>,
    Path(asset_id): Path<String>,
    Query(query): Query<CompanionAuthQuery>,
) -> Result<Response, StatusCode> {
    authorize(&state, &query)?;

    let asset = state
        .assets
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .get(&asset_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;

    let body = match &asset.source {
        CompanionAssetSource::Bytes(bytes) => bytes.clone(),
        CompanionAssetSource::File(path) => fs::read(path).await.map_err(asset_read_status_code)?,
    };

    let content_disposition = match asset.disposition {
        CompanionAssetDisposition::Inline => "inline",
        CompanionAssetDisposition::Attachment => "attachment",
    };
    let filename = sanitize_filename(&asset.name);
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, asset.mime_type)
        .header(CACHE_CONTROL, "no-store")
        .header(
            CONTENT_DISPOSITION,
            format!("{content_disposition}; filename=\"{filename}\""),
        )
        .body(Body::from(body))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response.into_response())
}

fn asset_read_status_code(error: io::Error) -> StatusCode {
    match error.kind() {
        io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
        _ => StatusCode::FORBIDDEN,
    }
}

async fn stream_events(mut socket: WebSocket, state: CompanionServerState) {
    let mut receiver = state.event_tx.subscribe();

    loop {
        match receiver.recv().await {
            Ok(event) => {
                let event = windowed_event(
                    &event,
                    &state.snapshot_rx.borrow(),
                    current_access_mode(&state),
                );
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
    if state.authorized_tokens.contains(&query.token) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn current_access_mode(state: &CompanionServerState) -> CompanionAccessMode {
    state
        .access_mode
        .read()
        .map(|access_mode| *access_mode)
        .unwrap_or(CompanionAccessMode::Control)
}

fn snapshot_for_access_mode(
    snapshot: &CompanionSnapshot,
    access_mode: CompanionAccessMode,
) -> CompanionSnapshot {
    let mut snapshot = snapshot.clone();
    let page = timeline_page(&snapshot.timeline, None, TIMELINE_PAGE_SIZE);
    snapshot.connection.access_mode = access_mode;
    snapshot.timeline = page.timeline;
    snapshot.has_more_timeline_before = page.has_more_before;
    snapshot.available_commands =
        filter_available_commands(&snapshot.available_commands, access_mode);
    snapshot
}

fn filter_available_commands(
    available_commands: &[CompanionCommandKind],
    access_mode: CompanionAccessMode,
) -> Vec<CompanionCommandKind> {
    if access_mode == CompanionAccessMode::Control {
        return available_commands.to_vec();
    }

    available_commands
        .iter()
        .filter(|command| {
            !matches!(
                command,
                CompanionCommandKind::SendMessage
                    | CompanionCommandKind::SendAttachments
                    | CompanionCommandKind::AuthorizeToolCall
                    | CompanionCommandKind::StopRun
            )
        })
        .cloned()
        .collect()
}

fn windowed_event(
    event: &CompanionEvent,
    snapshot: &CompanionSnapshot,
    access_mode: CompanionAccessMode,
) -> CompanionEvent {
    match event {
        CompanionEvent::SnapshotReplaced { snapshot } => CompanionEvent::SnapshotReplaced {
            snapshot: snapshot_for_access_mode(snapshot, access_mode),
        },
        CompanionEvent::TimelineChanged { .. } => {
            let page = timeline_page(&snapshot.timeline, None, TIMELINE_PAGE_SIZE);
            CompanionEvent::TimelineChanged {
                timeline: page.timeline,
                has_more_before: page.has_more_before,
            }
        }
        _ => event.clone(),
    }
}

fn timeline_page(
    timeline: &[CompanionTimelineEntry],
    before_entry_id: Option<&str>,
    limit: usize,
) -> CompanionTimelinePage {
    let end = before_entry_id
        .and_then(|before_entry_id| {
            timeline
                .iter()
                .position(|entry| timeline_entry_id(entry) == before_entry_id)
        })
        .unwrap_or(timeline.len());
    let start = end.saturating_sub(limit);

    CompanionTimelinePage {
        timeline: timeline[start..end].to_vec(),
        has_more_before: start > 0,
    }
}

fn timeline_entry_id(entry: &CompanionTimelineEntry) -> &str {
    match entry {
        CompanionTimelineEntry::Message { message } => &message.id,
        CompanionTimelineEntry::ToolCall { tool_call } => &tool_call.id,
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

fn companion_asset_storage_dir(token_id: &str) -> PathBuf {
    data_dir().join("mobile_companion_assets").join(token_id)
}

fn materialize_asset(
    asset: CompanionAsset,
    previous_assets: &HashMap<String, CompanionAsset>,
    asset_storage_dir: &StdPath,
) -> CompanionAsset {
    let CompanionAssetSource::File(source_path) = &asset.source else {
        return asset;
    };

    if source_path.starts_with(asset_storage_dir) {
        return asset;
    }

    if let Some(previous_asset) = previous_assets.get(&asset.id)
        && previous_asset.name == asset.name
        && previous_asset.mime_type == asset.mime_type
        && previous_asset.disposition == asset.disposition
        && let CompanionAssetSource::File(previous_path) = &previous_asset.source
        && previous_path.starts_with(asset_storage_dir)
        && previous_path.exists()
    {
        return previous_asset.clone();
    }

    let Ok(materialized_path) =
        copy_asset_to_storage(source_path, asset_storage_dir, &asset.id, &asset.name)
    else {
        return asset;
    };

    CompanionAsset {
        source: CompanionAssetSource::File(materialized_path),
        ..asset
    }
}

fn copy_asset_to_storage(
    source_path: &StdPath,
    asset_storage_dir: &StdPath,
    asset_id: &str,
    asset_name: &str,
) -> Result<PathBuf> {
    std::fs::create_dir_all(asset_storage_dir)
        .context("failed to create mobile companion asset storage directory")?;

    let file_name = format!("{asset_id}-{}", sanitize_filename(asset_name));
    let destination_path = asset_storage_dir.join(file_name);

    if destination_path.exists() {
        return Ok(destination_path);
    }

    std::fs::copy(source_path, &destination_path).with_context(|| {
        format!(
            "failed to copy companion asset from {}",
            source_path.display()
        )
    })?;

    Ok(destination_path)
}

fn sanitize_filename(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "attachment".into();
    }

    trimmed
        .chars()
        .map(|character| match character {
            '"' | '\\' | '/' | '\n' | '\r' | '\t' => '_',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use axum::http::{
        Method, Request,
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    };
    use axum::{body::Body, http::StatusCode};
    use futures::channel::mpsc;
    use futures::{FutureExt as _, StreamExt as _};
    use hyper::body::to_bytes;
    use tokio::sync::{broadcast, watch};
    use tower::ServiceExt as _;

    use super::{CompanionServerState, default_client_html, router};
    use crate::{
        CompanionAccessMode, CompanionAsset, CompanionAssetDisposition, CompanionAssetSource,
        CompanionCommand, CompanionCommandKind, CompanionConnectionMetadata, CompanionMessage,
        CompanionMessageRole, CompanionMessageStatus, CompanionRunStatus, CompanionSnapshot,
        CompanionTimelineEntry, CompanionTimelinePage, CompanionUpload,
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
                access_mode: CompanionAccessMode::Control,
                issued_at_unix_ms: None,
                expires_at_unix_ms: None,
            },
            timeline: Vec::new(),
            has_more_timeline_before: false,
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
                authorized_tokens: Arc::new(
                    ["test-token".to_string(), "stable-token".to_string()]
                        .into_iter()
                        .collect(),
                ),
                snapshot_rx,
                event_tx,
                command_tx,
                access_mode: Arc::new(RwLock::new(CompanionAccessMode::Control)),
                assets: Arc::default(),
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
    async fn snapshot_route_accepts_stable_token() {
        let (state, _) = test_state();
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/companion/snapshot?token=stable-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
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
        assert!(html.contains("id=\"action-button\""));
        assert!(html.contains("message-attachments"));
        assert!(html.contains("id=\"permission-card\""));
        assert!(html.contains("id=\"allow-button\""));
        assert!(html.contains("id=\"reject-button\""));
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
        assert_eq!(
            snapshot.connection.access_mode,
            CompanionAccessMode::Control
        );
        assert_eq!(snapshot.run_status, CompanionRunStatus::Idle);
        assert!(!snapshot.has_more_timeline_before);
    }

    #[tokio::test]
    async fn snapshot_route_filters_mutating_commands_in_read_only_mode() {
        let (state, _) = test_state();
        if let Ok(mut access_mode) = state.access_mode.write() {
            *access_mode = CompanionAccessMode::ReadOnly;
        }

        let snapshot = CompanionSnapshot {
            protocol_version: 1,
            session: None,
            connection: CompanionConnectionMetadata {
                token_id: "test-token".into(),
                access_mode: CompanionAccessMode::Control,
                issued_at_unix_ms: None,
                expires_at_unix_ms: None,
            },
            timeline: Vec::new(),
            has_more_timeline_before: false,
            streaming_text: None,
            tool_calls: Vec::new(),
            run_status: CompanionRunStatus::Idle,
            available_commands: vec![
                CompanionCommandKind::SendMessage,
                CompanionCommandKind::SendAttachments,
                CompanionCommandKind::AuthorizeToolCall,
                CompanionCommandKind::StopRun,
            ],
        };
        let (snapshot_tx, snapshot_rx) = watch::channel(snapshot);
        let _snapshot_tx = snapshot_tx;
        let state = CompanionServerState {
            snapshot_rx,
            ..state
        };
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
        assert_eq!(
            snapshot.connection.access_mode,
            CompanionAccessMode::ReadOnly
        );
        assert!(snapshot.available_commands.is_empty());
    }

    #[tokio::test]
    async fn timeline_route_returns_older_page() {
        let (state, _) = test_state();
        let timeline = (0..45)
            .map(|index| CompanionTimelineEntry::Message {
                message: CompanionMessage {
                    id: format!("message-{index}"),
                    role: CompanionMessageRole::Assistant,
                    status: CompanionMessageStatus::Done,
                    text: format!("message {index}"),
                    rendered_html: None,
                    attachments: Vec::new(),
                },
            })
            .collect::<Vec<_>>();
        let snapshot = CompanionSnapshot {
            protocol_version: 1,
            session: None,
            connection: CompanionConnectionMetadata {
                token_id: "test-token".into(),
                access_mode: CompanionAccessMode::Control,
                issued_at_unix_ms: None,
                expires_at_unix_ms: None,
            },
            timeline,
            has_more_timeline_before: false,
            streaming_text: None,
            tool_calls: Vec::new(),
            run_status: CompanionRunStatus::Completed,
            available_commands: Vec::new(),
        };
        let (snapshot_tx, snapshot_rx) = watch::channel(snapshot);
        let _snapshot_tx = snapshot_tx;
        let state = CompanionServerState {
            snapshot_rx,
            ..state
        };
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/companion/timeline?token=test-token&before_entry_id=message-40")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body()).await.expect("body");
        let page: CompanionTimelinePage = serde_json::from_slice(&body).expect("timeline page");
        assert_eq!(page.timeline.len(), 40);
        assert!(matches!(
            &page.timeline[0],
            CompanionTimelineEntry::Message { message } if message.id == "message-0"
        ));
        assert!(matches!(
            &page.timeline[39],
            CompanionTimelineEntry::Message { message } if message.id == "message-39"
        ));
        assert!(!page.has_more_before);
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
                            attachments: Vec::new(),
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
                attachments: Vec::new(),
            }
        );
    }

    #[tokio::test]
    async fn command_route_is_forbidden_in_read_only_mode() {
        let (state, mut command_rx) = test_state();
        if let Ok(mut access_mode) = state.access_mode.write() {
            *access_mode = CompanionAccessMode::ReadOnly;
        }

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
                            attachments: Vec::new(),
                        })
                        .expect("json"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(!matches!(
            command_rx.next().now_or_never(),
            Some(Some(CompanionCommand::SendMessage { .. }))
                | Some(Some(CompanionCommand::AuthorizeToolCall { .. }))
                | Some(Some(CompanionCommand::StopRun))
        ));
    }

    #[tokio::test]
    async fn command_route_forwards_authorize_payload() {
        let (state, mut command_rx) = test_state();
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/companion/command?token=test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&CompanionCommand::AuthorizeToolCall {
                            tool_call_id: "tool-1".into(),
                            option_id: "allow".into(),
                            option_kind: "AllowOnce".into(),
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
            CompanionCommand::AuthorizeToolCall {
                tool_call_id: "tool-1".into(),
                option_id: "allow".into(),
                option_kind: "AllowOnce".into(),
            }
        );
    }

    #[tokio::test]
    async fn command_route_accepts_large_attachment_payload() {
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
                            text: String::new(),
                            attachments: vec![CompanionUpload {
                                name: "large.png".into(),
                                mime_type: "image/png".into(),
                                data_base64: "A".repeat(3 * 1024 * 1024),
                            }],
                        })
                        .expect("json"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let command = command_rx.next().await.expect("command");
        match command {
            CompanionCommand::SendMessage { attachments, .. } => {
                assert_eq!(attachments.len(), 1);
                assert_eq!(attachments[0].name, "large.png");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[tokio::test]
    async fn asset_route_requires_token() {
        let (state, _) = test_state();
        if let Ok(mut assets) = state.assets.write() {
            assets.insert(
                "asset-1".into(),
                CompanionAsset {
                    id: "asset-1".into(),
                    name: "preview.png".into(),
                    mime_type: "image/png".into(),
                    disposition: CompanionAssetDisposition::Inline,
                    source: CompanionAssetSource::Bytes(vec![1, 2, 3]),
                },
            );
        }
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/companion/assets/asset-1?token=wrong")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn asset_route_serves_inline_image_bytes() {
        let (state, _) = test_state();
        if let Ok(mut assets) = state.assets.write() {
            assets.insert(
                "asset-1".into(),
                CompanionAsset {
                    id: "asset-1".into(),
                    name: "preview.png".into(),
                    mime_type: "image/png".into(),
                    disposition: CompanionAssetDisposition::Inline,
                    source: CompanionAssetSource::Bytes(vec![1, 2, 3]),
                },
            );
        }
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/companion/assets/asset-1?token=test-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "image/png");
        assert_eq!(
            response.headers()[CONTENT_DISPOSITION],
            "inline; filename=\"preview.png\""
        );
        let body = to_bytes(response.into_body()).await.expect("body");
        assert_eq!(body.as_ref(), &[1, 2, 3]);
    }

    #[tokio::test]
    async fn asset_route_serves_local_file_download() {
        let (state, _) = test_state();
        let path =
            std::env::temp_dir().join(format!("agent-companion-asset-{}.txt", std::process::id()));
        std::fs::write(&path, b"hello from file").expect("write temp file");

        if let Ok(mut assets) = state.assets.write() {
            assets.insert(
                "asset-2".into(),
                CompanionAsset {
                    id: "asset-2".into(),
                    name: "notes.txt".into(),
                    mime_type: "text/plain".into(),
                    disposition: CompanionAssetDisposition::Attachment,
                    source: CompanionAssetSource::File(path.clone()),
                },
            );
        }
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/companion/assets/asset-2?token=test-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let _ = std::fs::remove_file(&path);

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "text/plain");
        assert_eq!(
            response.headers()[CONTENT_DISPOSITION],
            "attachment; filename=\"notes.txt\""
        );
        let body = to_bytes(response.into_body()).await.expect("body");
        assert_eq!(body.as_ref(), b"hello from file");
    }

    #[tokio::test]
    async fn asset_route_returns_not_found_for_missing_local_file() {
        let (state, _) = test_state();
        let path = std::env::temp_dir().join(format!(
            "agent-companion-missing-{}.txt",
            std::process::id()
        ));

        if let Ok(mut assets) = state.assets.write() {
            assets.insert(
                "asset-missing".into(),
                CompanionAsset {
                    id: "asset-missing".into(),
                    name: "missing.txt".into(),
                    mime_type: "text/plain".into(),
                    disposition: CompanionAssetDisposition::Attachment,
                    source: CompanionAssetSource::File(path),
                },
            );
        }
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/companion/assets/asset-missing?token=test-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn asset_route_returns_forbidden_for_unreadable_local_file() {
        let (state, _) = test_state();
        let path =
            std::env::temp_dir().join(format!("agent-companion-directory-{}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp directory");

        if let Ok(mut assets) = state.assets.write() {
            assets.insert(
                "asset-forbidden".into(),
                CompanionAsset {
                    id: "asset-forbidden".into(),
                    name: "directory".into(),
                    mime_type: "application/octet-stream".into(),
                    disposition: CompanionAssetDisposition::Attachment,
                    source: CompanionAssetSource::File(path.clone()),
                },
            );
        }
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/companion/assets/asset-forbidden?token=test-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let _ = std::fs::remove_dir_all(&path);

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
