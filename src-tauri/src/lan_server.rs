use crate::snapshot_store::SnapshotStore;
use axum::{extract::State, http::HeaderValue, response::Html, routing::get, Json, Router};
use std::{net::SocketAddr, sync::Arc};

pub const LAN_PORT: u16 = 18765;

pub fn spawn(store: SnapshotStore) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let state = Arc::new(store);
        let app = Router::new().route("/", get(index)).route("/manifest.webmanifest", get(manifest)).route("/api/snapshot", get(snapshot)).route("/health", get(health)).with_state(state);
        let address = SocketAddr::from(([0, 0, 0, 0], LAN_PORT));
        let listener = match tokio::net::TcpListener::bind(address).await { Ok(listener) => listener, Err(_) => return };
        let _ = axum::serve(listener, app).await;
    })
}

async fn index() -> Html<&'static str> { Html(include_str!("../../web/index.html")) }
async fn manifest() -> ([(axum::http::HeaderName, HeaderValue); 1], &'static str) { ([(axum::http::header::CONTENT_TYPE, HeaderValue::from_static("application/manifest+json"))], include_str!("../../web/manifest.webmanifest")) }
async fn snapshot(State(store): State<Arc<SnapshotStore>>) -> Json<crate::domain::Snapshot> { Json(store.current()) }
async fn health() -> &'static str { "ok" }
