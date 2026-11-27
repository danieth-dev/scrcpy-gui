pub mod adb;
pub mod commands;
pub mod decoder;
pub mod health;
pub mod profile;
pub mod scrcpy;
pub mod tools;
pub mod vcam;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

use crate::commands::*;
use crate::scrcpy::ScrcpyManager;
use crate::vcam::VcamManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let scrcpy_manager = ScrcpyManager::new();
    let vcam_manager = VcamManager::new();

    tauri::Builder::default()
        .manage(scrcpy_manager)
        .manage(vcam_manager)
        .manage(crate::health::HealthMonitor::new())

        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Configuración del System Tray (Bandeja del sistema)
            let show_i = MenuItem::with_id(app, "show", "Mostrar scrcpy GUI", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Salir completamente", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let mut tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("scrcpy GUI — Control de Android");

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let _tray = tray_builder
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if let Ok(visible) = window.is_visible() {
                                if visible {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    }
                })
                .build(app);

            if let Some(window) = app.get_webview_window("main") {
                if let Some(icon) = app.default_window_icon() {
                    let _ = window.set_icon(icon.clone());
                }
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let scrcpy_manager = app.state::<ScrcpyManager>();
                if scrcpy_manager.has_active_sessions() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            win_minimize,
            win_maximize,
            win_close,
            get_tools_status,
            get_app_info,
            check_updates,
            open_external,
            adb_get_devices,
            adb_pair,
            adb_connect,
            adb_disconnect,
            adb_enable_tcpip,
            scrcpy_launch,
            scrcpy_stop,
            scrcpy_get_sessions,
            vcam_get_status,
            vcam_start,
            vcam_stop,
            vcam_install_driver,
            device_profile,
            apply_settings
        ])

        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
