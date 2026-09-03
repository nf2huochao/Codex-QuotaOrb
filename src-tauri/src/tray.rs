use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};
#[cfg(not(windows))]
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;

#[cfg(windows)]
const RUN_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
#[cfg(windows)]
const STARTUP_APPROVED_KEY: &str =
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";

#[cfg(windows)]
pub(crate) fn autostart_is_enabled(app: &AppHandle) -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let name = app.package_info().name.as_str();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu.open_subkey(RUN_KEY);
    let has_command = run
        .as_ref()
        .ok()
        .and_then(|key| key.get_value::<String, _>(name).ok())
        .is_some();
    if !has_command {
        return false;
    }

    let approved = hkcu.open_subkey(STARTUP_APPROVED_KEY);
    approved
        .ok()
        .and_then(|key| key.get_raw_value(name).ok())
        .map(|value| {
            value.bytes.len() >= 8 && value.bytes.iter().rev().take(8).all(|byte| *byte == 0)
        })
        .unwrap_or(true)
}

#[cfg(not(windows))]
pub(crate) fn autostart_is_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[cfg(windows)]
pub(crate) fn set_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    use std::env::current_exe;
    use winreg::enums::RegType::REG_BINARY;
    use winreg::{
        enums::{HKEY_CURRENT_USER, KEY_SET_VALUE},
        RegKey, RegValue,
    };

    let name = app.package_info().name.as_str();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
        .map_err(|error| format!("打开 Windows 启动项失败：{error}"))?;

    if enabled {
        let exe = current_exe().map_err(|error| format!("获取程序路径失败：{error}"))?;
        let exe = exe.to_string_lossy().replace('"', "");
        let command = format!("\"{exe}\" --autostart");
        run.set_value(name, &command)
            .map_err(|error| format!("写入 Windows 启动项失败：{error}"))?;

        if let Ok(approved) = hkcu.open_subkey_with_flags(STARTUP_APPROVED_KEY, KEY_SET_VALUE) {
            approved
                .set_raw_value(
                    name,
                    &RegValue {
                        vtype: REG_BINARY,
                        bytes: vec![2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    },
                )
                .map_err(|error| format!("启用 Windows 启动项失败：{error}"))?;
        }
    } else {
        let _ = run.delete_value(name);
        if let Ok(approved) = hkcu.open_subkey_with_flags(STARTUP_APPROVED_KEY, KEY_SET_VALUE) {
            let _ = approved.delete_value(name);
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn set_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let result = if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    };
    result.map_err(|error| error.to_string())
}

pub const MENU_ITEMS: [&str; 7] = [
    "显示悬浮球",
    "隐藏悬浮球",
    "更新数据",
    "开机自启",
    "检查更新",
    "关于",
    "退出",
];

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id("show", MENU_ITEMS[0]).build(app)?;
    let hide = MenuItemBuilder::with_id("hide", MENU_ITEMS[1]).build(app)?;
    let refresh = MenuItemBuilder::with_id("refresh", MENU_ITEMS[2]).build(app)?;
    let autostart_enabled = autostart_is_enabled(app);
    let autostart = CheckMenuItemBuilder::with_id("autostart", MENU_ITEMS[3])
        .checked(autostart_enabled)
        .build(app)?;
    let update = MenuItemBuilder::with_id("update", MENU_ITEMS[4]).build(app)?;
    let about = MenuItemBuilder::with_id("about", MENU_ITEMS[5]).build(app)?;
    let quit = MenuItemBuilder::with_id("quit", MENU_ITEMS[6]).build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&show, &hide, &refresh, &autostart, &update, &about, &quit])
        .build()?;
    let autostart_item = autostart.clone();
    TrayIconBuilder::with_id("codex-quota-tray")
        .icon(app.default_window_icon().expect("application icon").clone())
        .menu(&menu)
        .tooltip("Codex 额度悬浮窗")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }
            "hide" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            "refresh" => {
                let _ = app.emit("refresh-requested", ());
            }
            "autostart" => {
                let enabled = autostart_is_enabled(app);
                if let Err(error) = set_autostart(app, !enabled) {
                    log::error!("Failed to configure autostart: {error}");
                    let _ = app.emit("autostart-error", error);
                } else {
                    let _ = autostart_item.set_checked(autostart_is_enabled(app));
                }
            }
            "update" => {
                let _ = app.emit("update-check-requested", ());
            }
            "about" => {
                let _ = app.emit("info-requested", event.id().as_ref());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::MENU_ITEMS;
    #[test]
    fn menu_contains_core_controls() {
        assert!(MENU_ITEMS.contains(&"显示悬浮球"));
        assert!(MENU_ITEMS.contains(&"更新数据"));
        assert!(MENU_ITEMS.contains(&"开机自启"));
        assert!(MENU_ITEMS.contains(&"退出"));
    }
}
