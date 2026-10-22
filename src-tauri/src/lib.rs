mod commands;
mod database;
mod operations;
mod remote;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::{
    Emitter, Manager, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use windows_network::NetworkChangeWatcher;

pub(crate) struct AppPreferences {
    close_to_tray: AtomicBool,
}
impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            close_to_tray: AtomicBool::new(true),
        }
    }
}
impl AppPreferences {
    pub(crate) fn set_close_to_tray(&self, value: bool) {
        self.close_to_tray.store(value, Ordering::Relaxed);
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let monitor = Arc::new(commands::NetworkMonitor::new());
    let watcher_monitor = Arc::clone(&monitor);
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main_window(app)))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .manage(monitor)
        .manage(AppPreferences::default())
        .setup(move |app| {
            let database_path = app.path().app_data_dir()?.join("charroutex.sqlite3");
            app.manage(database::Database::open(&database_path).map_err(std::io::Error::other)?);
            let handle = app.handle().clone();
            match NetworkChangeWatcher::start(move || {
                watcher_monitor.invalidate_health();
                if let Err(error) = handle.emit("network-changed", ()) {
                    tracing::warn!(%error, "failed to emit native network change event");
                }
            }) {
                Ok(watcher) => {
                    app.manage(watcher);
                    tracing::info!("native interface and route change notifications enabled");
                }
                Err(error) => tracing::warn!(%error, "native network change notifications unavailable; reconciliation polling remains active"),
            }
            let open = MenuItem::with_id(app, "open", "Open CharRouteX", true, None::<&str>)?;
            let refresh = MenuItem::with_id(app, "refresh", "Refresh network status", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "Pause monitoring", true, None::<&str>)?;
            let exit = MenuItem::with_id(app, "exit", "Exit CharRouteX", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&open, &refresh, &pause, &separator, &exit])?;
            let pause_for_menu = pause.clone();
            TrayIconBuilder::with_id("charroutex-tray")
                .icon(app.default_window_icon().ok_or("application icon is unavailable")?.clone())
                .tooltip("CharRouteX — Network monitoring active")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "open" => show_main_window(app),
                    "refresh" => {
                        let monitor = app.state::<Arc<commands::NetworkMonitor>>();
                        monitor.invalidate_health();
                        let _ = app.emit("network-changed", ());
                    }
                    "pause" => {
                        let monitor = app.state::<Arc<commands::NetworkMonitor>>();
                        let paused = !monitor.is_paused();
                        monitor.set_paused(paused);
                        let _ = pause_for_menu.set_text(if paused { "Resume monitoring" } else { "Pause monitoring" });
                        let _ = app.emit("monitoring-state-changed", paused);
                    }
                    "exit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(event, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. }) {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let preferences = window.app_handle().state::<AppPreferences>();
                if preferences.close_to_tray.load(Ordering::Relaxed) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_network_adapters,
            commands::validate_ipv4_configuration,
            commands::list_network_profiles,
            commands::create_network_profile,
            commands::delete_network_profile,
            commands::duplicate_network_profile,
            commands::update_network_profile,
            commands::get_profile_apply_plan,
            commands::export_network_profile,
            commands::import_network_profile,
            commands::list_activity_events
            ,commands::apply_address_configuration,
            commands::get_network_operation_status,
            commands::confirm_network_operation,
            commands::restore_network_operation,
            commands::verify_network_operation,
            commands::get_failover_configuration,
            commands::save_failover_configuration,
            commands::get_failover_status,
            commands::activate_failover_candidate,
            commands::get_health_history,
            commands::list_remote_hosts,
            commands::enroll_remote_host,
            commands::revoke_remote_host,
            commands::probe_remote_host,
            commands::set_monitoring_paused,
            commands::get_monitoring_paused,
            commands::set_health_probe_interval,
            commands::set_close_to_tray
        ])
        .run(tauri::generate_context!())
        .expect("failed to run CharRouteX");
}
