use crate::snapshot_store::SnapshotStore;
use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::{fs, net::SocketAddr, path::Path as FsPath, sync::{Arc, RwLock}};

pub const LAN_PORT: u16 = 18765;

#[derive(Clone)]
pub struct LanState {
    pub store: SnapshotStore,
    pub pairing: Arc<RwLock<PairingState>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PairingInfo {
    pub address: String,
    pub code: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PairingState {
    pub code: String,
    pub session_token: String,
}

#[derive(Debug, Deserialize)]
struct PairRequest {
    code: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PairResponse {
    token: String,
}

#[derive(Debug, Deserialize)]
struct PairQuery {
    pair: Option<String>,
}

pub fn create_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
pub fn create_code() -> String {
    format!("{:04}", uuid::Uuid::new_v4().as_u128() % 10_000)
}

pub fn load_or_create(path: &FsPath) -> Result<PairingState, String> {
    if let Ok(bytes) = fs::read(path) {
        if let Ok(state) = serde_json::from_slice::<PairingState>(&bytes) {
            if state.code.len() == 4
                && state.code.chars().all(|character| character.is_ascii_digit())
                && !state.session_token.is_empty()
            {
                return Ok(state);
            }
        }
    }
    let state = PairingState { code: create_code(), session_token: create_token() };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, serde_json::to_vec_pretty(&state).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    Ok(state)
}

pub fn save(state: &PairingState, path: &FsPath) -> Result<(), String> {
    fs::write(path, serde_json::to_vec_pretty(state).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

pub fn pairing_info(state: &PairingState) -> PairingInfo {
    let host = std::net::UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|socket| {
            let _ = socket.connect("1.1.1.1:80");
            socket
                .local_addr()
                .ok()
                .map(|address| address.ip().to_string())
        })
        .unwrap_or_else(|| "本机局域网IP".into());
    PairingInfo {
        address: format!("http://{host}:{LAN_PORT}/"),
        code: state.code.clone(),
    }
}

pub fn router(store: SnapshotStore, pairing: Arc<RwLock<PairingState>>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/manifest.webmanifest", get(manifest))
        .route("/api/pair", post(pair))
        .route("/api/snapshot", get(snapshot))
        .route("/api/tasks/{id}/acknowledge", post(acknowledge))
        .route("/ws", get(websocket))
        .route("/health", get(health))
        .with_state(LanState { store, pairing })
}

pub fn spawn(store: SnapshotStore, pairing: Arc<RwLock<PairingState>>) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let app = router(store, pairing);
        let address = SocketAddr::from(([0, 0, 0, 0], LAN_PORT));
        let listener = match tokio::net::TcpListener::bind(address).await {
            Ok(listener) => listener,
            Err(_) => return,
        };
        let _ = axum::serve(listener, app).await;
    })
}

fn authorized(headers: &HeaderMap, query: &PairQuery, pairing: &PairingState) -> bool {
    if query.pair.as_deref() == Some(pairing.session_token.as_str()) {
        return true;
    }
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|value| value == pairing.session_token)
}

async fn pair(State(state): State<LanState>, Json(request): Json<PairRequest>) -> Response {
    let pairing = state.pairing.read().expect("pairing lock").clone();
    if request.code.trim() != pairing.code {
        return (StatusCode::UNAUTHORIZED, "配对码不正确").into_response();
    }
    (
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(PairResponse { token: pairing.session_token }),
    )
        .into_response()
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../web/index.html"))
}
async fn manifest() -> ([(axum::http::HeaderName, HeaderValue); 1], &'static str) {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/manifest+json"),
        )],
        include_str!("../../web/manifest.webmanifest"),
    )
}
async fn health() -> &'static str {
    "ok"
}

async fn snapshot(
    State(state): State<LanState>,
    headers: HeaderMap,
    Query(query): Query<PairQuery>,
) -> Response {
    let pairing = state.pairing.read().expect("pairing lock").clone();
    if !authorized(&headers, &query, &pairing) {
        return (StatusCode::UNAUTHORIZED, "需要局域网配对码").into_response();
    }
    (
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(state.store.current()),
    )
        .into_response()
}

async fn acknowledge(
    Path(id): Path<String>,
    State(state): State<LanState>,
    headers: HeaderMap,
    Query(query): Query<PairQuery>,
) -> Response {
    let pairing = state.pairing.read().expect("pairing lock").clone();
    if !authorized(&headers, &query, &pairing) {
        return (StatusCode::UNAUTHORIZED, "需要局域网配对码").into_response();
    }
    Json(serde_json::json!({ "acknowledged": state.store.acknowledge(&id) })).into_response()
}

async fn websocket(
    ws: WebSocketUpgrade,
    State(state): State<LanState>,
    headers: HeaderMap,
    Query(query): Query<PairQuery>,
) -> Response {
    let pairing = state.pairing.read().expect("pairing lock").clone();
    if !authorized(&headers, &query, &pairing) {
        return (StatusCode::UNAUTHORIZED, "需要局域网配对码").into_response();
    }
    ws.on_upgrade(move |socket| send_snapshots(socket, state.store))
        .into_response()
}

async fn send_snapshots(mut socket: WebSocket, store: SnapshotStore) {
    let mut receiver = store.subscribe();
    let _ = futures_util::SinkExt::send(
        &mut socket,
        Message::Text(
            serde_json::to_string(&store.current())
                .unwrap_or_default()
                .into(),
        ),
    )
    .await;
    while receiver.changed().await.is_ok() {
        let payload = match serde_json::to_string(&receiver.borrow().clone()) {
            Ok(payload) => payload,
            Err(_) => continue,
        };
        if futures_util::SinkExt::send(&mut socket, Message::Text(payload.into()))
            .await
            .is_err()
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{DataStatus, Snapshot},
        snapshot_store::SnapshotStore,
    };
    use axum::body::Body;
    use axum::http::Request;
    use tower::util::ServiceExt;
    fn store() -> SnapshotStore {
        SnapshotStore::new(Snapshot {
            status: DataStatus::Stale,
            changed_at: None,
            source: None,
            fetched_at: None,
            quota_remaining_percent: None,
            quota_resets_at: None,
            plan: None,
            reset_credits: None,
            today_tokens: None,
            usage_date: None,
            active_task_count: 0,
            tasks: vec![crate::domain::TaskSummary {
                id: "done".into(),
                title: "完成".into(),
                status: crate::domain::TaskStatus::Completed,
                token_count: None,
                updated_at: 0,
                acknowledged: false,
            }],
            error: Some("等待".into()),
            schema_version: "1.0".into(),
        })
        .0
    }
    #[tokio::test]
    async fn rejects_unpaired_snapshot() {
        let pairing = Arc::new(RwLock::new(PairingState { code: "1234".into(), session_token: "secret".into() }));
        let response = router(store(), pairing)
            .oneshot(Request::get("/api/snapshot").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    #[tokio::test]
    async fn paired_snapshot_is_read_only() {
        let pairing = Arc::new(RwLock::new(PairingState { code: "1234".into(), session_token: "secret".into() }));
        let response = router(store(), pairing)
            .oneshot(
                Request::get("/api/snapshot?pair=secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let snapshot: Snapshot = serde_json::from_slice(&body).unwrap();
        assert_eq!(snapshot.status, DataStatus::Stale);
    }
    #[tokio::test]
    async fn paired_ack_changes_only_local_state() {
        let store = store();
        let pairing = Arc::new(RwLock::new(PairingState { code: "1234".into(), session_token: "secret".into() }));
        let response = router(store.clone(), pairing)
            .oneshot(
                Request::post("/api/tasks/done/acknowledge?pair=secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(store.current().tasks[0].acknowledged);
    }
    #[tokio::test]
    async fn four_digit_pairing_returns_session_token() {
        let pairing = Arc::new(RwLock::new(PairingState { code: "1234".into(), session_token: "secret".into() }));
        let response = router(store(), pairing)
            .oneshot(Request::post("/api/pair").header("content-type", "application/json").body(Body::from(r#"{"code":"1234"}"#)).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        assert_eq!(serde_json::from_slice::<PairResponse>(&body).unwrap().token, "secret");
    }

    #[test]
    fn pairing_info_contains_code_without_long_token() {
        let info = pairing_info(&PairingState { code: "1234".into(), session_token: "secret".into() });
        assert!(!info.address.contains("pair="));
        assert_eq!(info.code, "1234");
    }
}
