use tauri::{AppHandle, Runtime};
use tauri::Manager;
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::menu::{MenuBuilder, MenuItemBuilder};

pub fn create_system_tray<R: Runtime>(app: &AppHandle<R>) -> TrayIcon<R> {
    let dashboard_item = MenuItemBuilder::new("Dashboard")
        .id("dashboard")
        .build(app)
        .unwrap();
    let quit_item = MenuItemBuilder::new("Quit")
        .id("quit")
        .build(app)
        .unwrap();
    let menu = MenuBuilder::new(app)
        .item(&dashboard_item)
        .separator()
        .item(&quit_item)
        .build()
        .unwrap();
    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(|app, event| {
            if event.id.0 == "dashboard" {
                if let Some(window) = app.get_webview_window("dashboard") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    #[cfg(target_os = "macos")]
                    {
                        crate::platform::macos::update_dock_icon_for_app(app);
                    }
                }
            } else if event.id.0 == "quit" {
                std::process::exit(0);
            }
        })
        .build(app)
        .unwrap()
} 