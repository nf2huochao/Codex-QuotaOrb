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
use std::{net::SocketAddr, sync::Arc};

pub const LAN_PORT: u16 = 18765;

#[derive(Clone)]
pub struct LanState {
    pub store: SnapshotStore,
    pub token: Arc<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PairingInfo {
    pub address: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
struct PairQuery {
    pair: Option<String>,
}

pub fn create_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
pub fn pairing_info(token: &str) -> PairingInfo {
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
        address: format!("http://{host}:{LAN_PORT}/?pair={token}"),
        token: token.to_owned(),
    }
}

pub fn router(store: SnapshotStore, token: Arc<String>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/manifest.webmanifest", get(manifest))
        .route("/api/snapshot", get(snapshot))
        .route("/api/tasks/{id}/acknowledge", post(acknowledge))
        .route("/ws", get(websocket))
        .route("/health", get(health))
        .with_state(LanState { store, token })
}

pub fn spawn(store: SnapshotStore, token: Arc<String>) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let app = router(store, token);
        let address = SocketAddr::from(([0, 0, 0, 0], LAN_PORT));
        let listener = match tokio::net::TcpListener::bind(address).await {
            Ok(listener) => listener,
            Err(_) => return,
        };
        let _ = axum::serve(listener, app).await;
    })
}

fn authorized(headers: &HeaderMap, query: &PairQuery, expected: &str) -> bool {
    if query.pair.as_deref() == Some(expected) {
        return true;
    }
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|value| value == expected)
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
    if !authorized(&headers, &query, &state.token) {
        return (StatusCode::UNAUTHORIZED, "需要局域网配对码").into_response();
    }
    Json(state.store.current()).into_response()
}

async fn acknowledge(
    Path(id): Path<String>,
    State(state): State<LanState>,
    headers: HeaderMap,
    Query(query): Query<PairQuery>,
) -> Response {
    if !authorized(&headers, &query, &state.token) {
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
    if !authorized(&headers, &query, &state.token) {
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
        let token = Arc::new("secret".to_owned());
        let response = router(store(), token)
            .oneshot(Request::get("/api/snapshot").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    #[tokio::test]
    async fn paired_snapshot_is_read_only() {
        let token = Arc::new("secret".to_owned());
        let response = router(store(), token)
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
        let token = Arc::new("secret".to_owned());
        let response = router(store.clone(), token)
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
    #[test]
    fn pairing_info_contains_token() {
        let info = pairing_info("abc");
        assert!(info.address.contains("pair=abc"));
        assert_eq!(info.token, "abc");
    }
}
