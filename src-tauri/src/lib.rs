pub(crate) mod domain;
pub(crate) mod codex_protocol;
pub(crate) mod codex_client;
pub(crate) mod snapshot_store;
pub(crate) mod poller;
pub(crate) mod tray;
pub(crate) mod lan_server;

use snapshot_store::{empty_snapshot, SnapshotStore};
use std::sync::Arc;
use std::path::Path;
use tokio::sync::Mutex as AsyncMutex;
use tauri::Manager;

pub struct AppState { pub store: SnapshotStore, pub client: Arc<AsyncMutex<Option<Arc<codex_client::CodexClient>>>> }

#[tauri::command]
fn get_snapshot(state: tauri::State<'_, AppState>) -> domain::Snapshot { state.store.current() }

#[tauri::command]
async fn refresh_now(state: tauri::State<'_, AppState>) -> Result<domain::Snapshot, String> {
  let Some(client) = state.client.lock().await.clone() else { return Err("Codex app-server 尚未连接".into()); };
  let previous = state.store.current();
  Ok(poller::poll_once(&state.store, &client, &previous, now()).await)
}

#[tauri::command]
fn acknowledge_task(task_id: String, state: tauri::State<'_, AppState>) -> bool { state.store.acknowledge(&task_id) }
#[cfg(test)]
mod codex_client_tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let (store, _) = SnapshotStore::new(empty_snapshot());
  let client = Arc::new(AsyncMutex::new(None));
  tauri::Builder::default()
    .plugin(tauri_plugin_autostart::Builder::new().build())
    .manage(AppState { store, client })
    .invoke_handler(tauri::generate_handler![get_snapshot, refresh_now, acknowledge_task])
    .setup(|app| {
      tray::setup_tray(app.handle())?;
      let state = app.state::<AppState>();
      let store = state.store.clone();
      let event_store = state.store.clone();
      let _ = lan_server::spawn(store.clone());
      let client_slot = Arc::clone(&state.client);
      tauri::async_runtime::spawn(async move {
        match codex_client::CodexClient::spawn(Path::new("codex.exe")).await {
          Ok(client) => {
            let client = Arc::new(client);
            *client_slot.lock().await = Some(Arc::clone(&client));
            let _ = poller::spawn_poll_loop(store, client);
            if let Ok(event_client) = codex_client::CodexClient::spawn(Path::new("codex.exe")).await {
              let _ = poller::spawn_event_loop(event_store, Arc::new(event_client));
            }
          }
          Err(error) => {
            let mut snapshot = store.current();
            snapshot.status = domain::DataStatus::Error;
            snapshot.error = Some(format!("无法连接 Codex app-server: {error}"));
            store.publish(snapshot);
          }
        }
      });
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|duration| duration.as_secs() as i64).unwrap_or(0) }
