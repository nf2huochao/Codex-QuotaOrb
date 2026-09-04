pub(crate) mod codex_client;
pub(crate) mod codex_protocol;
pub(crate) mod diagnostics;
pub(crate) mod domain;
pub(crate) mod hook_bridge;
pub(crate) mod hook_diagnostics;
pub(crate) mod lan_server;
pub(crate) mod poller;
pub(crate) mod rollout_watcher;
pub(crate) mod snapshot_cache;
pub(crate) mod snapshot_store;
pub(crate) mod task_registry;
pub(crate) mod tray;

use snapshot_store::{empty_snapshot, SnapshotStore};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tauri::{Emitter, Manager};
#[cfg(desktop)]
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::Mutex as AsyncMutex;

pub struct AppState {
    pub store: SnapshotStore,
    pub client: Arc<AsyncMutex<Option<Arc<codex_client::CodexClient>>>>,
    pub pairing: Arc<RwLock<lan_server::PairingState>>,
    pub pairing_path: PathBuf,
    pub hook_bridge: hook_bridge::HookBridge,
}

#[derive(serde::Serialize)]
struct UpdateStatus {
    current_version: String,
    available: bool,
    latest_version: Option<String>,
    message: String,
}

#[tauri::command]
async fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateStatus, String> {
    #[cfg(desktop)]
    {
        let updater = app
            .updater()
            .map_err(|error| format!("更新服务初始化失败：{error}"))?;
        let update = match updater.check().await {
            Ok(update) => update,
            Err(error) => {
                // GitHub Releases may exist before its signed updater artifacts are
                // uploaded. Report that state explicitly instead of showing the
                // misleading generic "error sending request" message.
                let detail = error.to_string();
                return Ok(UpdateStatus {
                    current_version: env!("CARGO_PKG_VERSION").into(),
                    available: false,
                    latest_version: None,
                    message: format!(
                        "GitHub 更新源暂不可用：发行版需要同时提供 latest.json 和签名文件（{detail}）"
                    ),
                });
            }
        };
        if let Some(update) = update {
            let latest_version = update.version.clone();
            update
                .download_and_install(|_, _| {}, || {})
                .await
                .map_err(|error| format!("更新下载或验签失败：{error}"))?;
            return Ok(UpdateStatus {
                current_version: env!("CARGO_PKG_VERSION").into(),
                available: true,
                latest_version: Some(latest_version.clone()),
                message: format!("已安装 {latest_version}，应用即将重启"),
            });
        }
    }
    Ok(UpdateStatus {
        current_version: env!("CARGO_PKG_VERSION").into(),
        available: false,
        latest_version: None,
        message: "当前已是最新版本".into(),
    })
}

#[tauri::command]
fn relaunch_app(app: tauri::AppHandle) {
    app.restart();
}

#[tauri::command]
fn get_pairing_info(state: tauri::State<'_, AppState>) -> lan_server::PairingInfo {
    let pairing = state.pairing.read().expect("pairing lock").clone();
    lan_server::pairing_info(&pairing)
}

#[tauri::command]
fn reset_pairing(state: tauri::State<'_, AppState>) -> Result<lan_server::PairingInfo, String> {
    let next = lan_server::PairingState {
        code: lan_server::create_code(),
        session_token: lan_server::create_token(),
    };
    lan_server::save(&next, &state.pairing_path)?;
    *state.pairing.write().expect("pairing lock") = next.clone();
    Ok(lan_server::pairing_info(&next))
}

#[tauri::command]
fn get_snapshot(state: tauri::State<'_, AppState>) -> domain::Snapshot {
    state.store.current()
}

#[tauri::command]
async fn refresh_now(state: tauri::State<'_, AppState>) -> Result<domain::Snapshot, String> {
    let previous = state.store.current();
    let refreshed_at = now();
    let Some(client) = state.client.lock().await.clone() else {
        return Ok(poller::refresh_local_tasks(
            &state.store,
            &previous,
            refreshed_at,
        ));
    };
    let mut snapshot = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        poller::poll_once(&state.store, &client, &previous, refreshed_at),
    )
    .await
    .map_err(|_| "同步数据超时，请稍后重试".to_owned())?;
    snapshot.changed_at = Some(refreshed_at);
    snapshot.source = Some("manual-refresh".into());
    Ok(snapshot)
}

#[tauri::command]
async fn respond_to_approval(
    task_id: String,
    decision: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if !matches!(decision.as_str(), "accept" | "decline") {
        return Err("批准决定无效".into());
    }
    let request_id = state
        .store
        .current()
        .tasks
        .into_iter()
        .find(|task| task.id == task_id)
        .and_then(|task| task.approval_request_id)
        .ok_or_else(|| "任务当前没有待处理的批准请求".to_owned())?;
    if request_id.starts_with("hook:") {
        if state.hook_bridge.resolve(&request_id, &decision).await {
            state.store.resolve_approval(&task_id, &decision);
            return Ok(());
        }
        return Err("批准请求已失效或已处理".into());
    }
    let client = state
        .client
        .lock()
        .await
        .clone()
        .ok_or_else(|| "Codex app-server 尚未连接".to_owned())?;
    client
        .respond_to_approval(&request_id, &decision)
        .await
        .map_err(|error| error.to_string())?;
    state.store.resolve_approval(&task_id, &decision);
    Ok(())
}

#[tauri::command]
fn acknowledge_task(task_id: String, state: tauri::State<'_, AppState>) -> bool {
    state.store.acknowledge(&task_id)
}

#[tauri::command]
fn set_window_expanded(
    expanded: bool,
    height: Option<f64>,
    width: Option<f64>,
    window: tauri::Window,
) -> Result<(), String> {
    let height = if expanded {
        // Leave enough room for the trend card, task list and refresh action.
        // The task list still scrolls when it exceeds the available screen area.
        height.unwrap_or(440.0).clamp(360.0, 1100.0)
    } else {
        height.unwrap_or(120.0).clamp(96.0, 220.0)
    };
    let width = width.unwrap_or(520.0).clamp(96.0, 900.0);
    window
        .set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> bool {
    tray::autostart_is_enabled(&app)
}

#[tauri::command]
fn set_autostart(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    tray::set_autostart(&app, enabled)
}

#[tauri::command]
fn get_always_on_top(window: tauri::Window) -> Result<bool, String> {
    window.is_always_on_top().map_err(|error| error.to_string())
}

#[tauri::command]
fn set_always_on_top(enabled: bool, window: tauri::Window) -> Result<(), String> {
    window.set_always_on_top(enabled).map_err(|error| error.to_string())
}
#[cfg(test)]
mod codex_client_tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (store, _) = SnapshotStore::new(empty_snapshot());
    let client = Arc::new(AsyncMutex::new(None));
    let hook_bridge = hook_bridge::HookBridge::default();
    let pairing_path = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.codex.quota-floating-window")
        .join("pairing.json");
    let pairing = Arc::new(RwLock::new(
        lan_server::load_or_create(&pairing_path).unwrap_or_else(|_| lan_server::PairingState {
            code: lan_server::create_code(),
            session_token: lan_server::create_token(),
        }),
    ));
    let codex_binary = resolve_codex_binary();
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tauri_plugin_window_state::StateFlags::POSITION)
                .build(),
        )
        .manage(AppState {
            store,
            client,
            pairing: Arc::clone(&pairing),
            pairing_path,
            hook_bridge: hook_bridge.clone(),
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_pairing_info,
            reset_pairing,
            check_for_updates,
            relaunch_app,
            refresh_now,
            acknowledge_task,
            respond_to_approval,
            set_window_expanded,
            get_autostart,
            set_autostart,
            get_always_on_top,
            set_always_on_top
        ])
        .setup(move |app| {
            tray::setup_tray(app.handle())?;
            let state = app.state::<AppState>();
            let store = state.store.clone();
            // Task lifecycle comes from Codex's local session records, not the
            // app-server child process started by this monitor.
            let _ = poller::spawn_local_task_loop(store.clone());
            let cache_path = app
                .path()
                .app_data_dir()?
                .join("last-successful-snapshot.json");
            if let Some(mut cached) = snapshot_cache::load(&cache_path) {
                // A cached snapshot is useful for quota/Token continuity, but
                // it cannot prove that a task is still live after a restart.
                // Clear lifecycle status until a fresh Hook or app-server
                // event arrives instead of showing stale green/yellow/red.
                for task in &mut cached.tasks {
                    task.status = domain::TaskStatus::None;
                    task.waiting_reason = None;
                    task.approval_request_id = None;
                    task.source = None;
                    task.turn_id = None;
                    task.received_at = 0;
                }
                cached.active_task_count = 0;
                cached.task_counts = domain::TaskCounts::from_tasks(&cached.tasks);
                cached.status = domain::DataStatus::Stale;
                cached.error = Some("已恢复历史数据，等待实时状态确认".into());
                cached.source = Some("cache-unconfirmed".into());
                store.publish_if_changed(cached);
            }
            let mut snapshot_events = state.store.subscribe();
            let event_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while snapshot_events.changed().await.is_ok() {
                    let snapshot = snapshot_events.borrow().clone();
                    if let Err(error) = snapshot_cache::save(&cache_path, &snapshot) {
                        log::warn!("Failed to save Codex snapshot cache: {error}");
                    }
                    let _ = event_handle.emit("snapshot-updated", snapshot);
                }
            });
            let _ = lan_server::spawn(
                store.clone(),
                Arc::clone(&pairing),
                Arc::clone(&state.client),
                hook_bridge.clone(),
            );
            let client_slot = Arc::clone(&state.client);
            let codex_binary = codex_binary.clone();
            tauri::async_runtime::spawn(async move {
                let mut retry_delay = std::time::Duration::from_secs(2);
                loop {
                    match codex_client::CodexClient::spawn(&codex_binary).await {
                        Ok(client) => {
                            let client = Arc::new(client);
                            *client_slot.lock().await = Some(Arc::clone(&client));
                            let _ =
                                poller::spawn_poll_loop(store.clone(), Arc::clone(&client)).await;
                            let _ = client.stop().await;
                            *client_slot.lock().await = None;
                            let mut snapshot = store.current();
                            snapshot.status = domain::DataStatus::Stale;
                            snapshot.error = Some("Codex app-server 已断开，正在重连…".into());
                            store.publish_if_changed(snapshot);
                            retry_delay = std::time::Duration::from_secs(2);
                        }
                        Err(error) => {
                            let mut snapshot = store.current();
                            snapshot.status = domain::DataStatus::Error;
                            snapshot.error =
                                Some(format!("无法连接 Codex app-server，正在重试… ({error})"));
                            store.publish_if_changed(snapshot);
                        }
                    }
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(std::time::Duration::from_secs(30));
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

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn resolve_codex_binary() -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(value) = std::env::var("CODEX_BINARY") {
        candidates.push(PathBuf::from(value));
    }
    if let Ok(path) = std::env::var("PATH") {
        for directory in std::env::split_paths(&path) {
            candidates.push(directory.join(if cfg!(windows) { "codex.exe" } else { "codex" }));
        }
    }
    #[cfg(windows)]
    if let Ok(app_data) = std::env::var("APPDATA") {
        candidates.push(PathBuf::from(app_data).join("npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe"));
    }
    #[cfg(target_os = "macos")]
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        for prefix in [
            home.join(".npm/node_modules"),
            home.join(".npm-global/lib/node_modules"),
            home.join(".local/lib/node_modules"),
        ] {
            candidates.push(prefix.join("@openai/codex/node_modules/@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex"));
            candidates.push(prefix.join("@openai/codex/node_modules/@openai/codex-darwin-x64/vendor/x86_64-apple-darwin/bin/codex"));
        }
        candidates.push(home.join(".local/bin/codex"));
        candidates.push(home.join(".volta/bin/codex"));
    }
    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/opt/homebrew/bin/codex"));
        candidates.push(PathBuf::from("/usr/local/bin/codex"));
        candidates.push(PathBuf::from("/usr/bin/codex"));
    }
    #[cfg(windows)]
    candidates.push(PathBuf::from("codex.exe"));
    #[cfg(not(any(windows, target_os = "macos")))]
    candidates.push(PathBuf::from("codex"));
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from("codex.exe"))
}
