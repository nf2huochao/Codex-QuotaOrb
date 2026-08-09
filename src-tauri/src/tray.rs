use tauri::{menu::{MenuBuilder, MenuItemBuilder}, tray::TrayIconBuilder, AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;

pub const MENU_ITEMS: [&str; 7] = ["显示悬浮球", "隐藏悬浮球", "立即更新", "开机自启", "检查更新", "关于", "退出"];

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id("show", MENU_ITEMS[0]).build(app)?;
    let hide = MenuItemBuilder::with_id("hide", MENU_ITEMS[1]).build(app)?;
    let refresh = MenuItemBuilder::with_id("refresh", MENU_ITEMS[2]).build(app)?;
    let autostart = MenuItemBuilder::with_id("autostart", MENU_ITEMS[3]).build(app)?;
    let update = MenuItemBuilder::with_id("update", MENU_ITEMS[4]).build(app)?;
    let about = MenuItemBuilder::with_id("about", MENU_ITEMS[5]).build(app)?;
    let quit = MenuItemBuilder::with_id("quit", MENU_ITEMS[6]).build(app)?;
    let menu = MenuBuilder::new(app).items(&[&show, &hide, &refresh, &autostart, &update, &about, &quit]).build()?;
    TrayIconBuilder::new().menu(&menu).tooltip("Codex 额度悬浮窗").on_menu_event(|app, event| {
        match event.id().as_ref() {
            "show" => { if let Some(window) = app.get_webview_window("main") { let _ = window.show(); } }
            "hide" => { if let Some(window) = app.get_webview_window("main") { let _ = window.hide(); } }
            "refresh" => { let _ = app.emit("refresh-requested", ()); }
            "autostart" => {
                if let Ok(enabled) = app.autolaunch().is_enabled() {
                    let _ = if enabled { app.autolaunch().disable() } else { app.autolaunch().enable() };
                }
            }
            "update" | "about" => { let _ = app.emit("info-requested", event.id().as_ref()); }
            "quit" => { app.exit(0); }
            _ => {}
        }
    }).build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::MENU_ITEMS;
    #[test]
    fn menu_contains_core_controls() { assert!(MENU_ITEMS.contains(&"显示悬浮球")); assert!(MENU_ITEMS.contains(&"立即更新")); assert!(MENU_ITEMS.contains(&"退出")); }
}
