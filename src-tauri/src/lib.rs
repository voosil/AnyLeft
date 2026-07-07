//! AnyLeft 剩了么 — native bridge entry point.
//!
//! Wires up plugins, shared state, the menu-bar tray, the global shortcut, and
//! the command handlers, then runs the Tauri event loop.

mod catalog;
mod commands;
mod error;
mod models;
mod providers;
mod secrets;
mod settings;
mod state;
mod tray;
mod windows;

use std::path::PathBuf;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::state::AppState;
use crate::windows::{PANEL_LABEL, SETTINGS_LABEL};

/// Resolve the path to the persisted settings file inside the app config dir.
fn settings_path(app: &tauri::App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = app.path().app_config_dir()?;
    Ok(dir.join("settings.json"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        tray::toggle_panel(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            // Shared state (loads persisted settings from disk).
            let config_path = settings_path(app)?;
            app.manage(AppState::new(config_path));

            // A menu-bar app: no Dock icon, no app-switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Tray icon + live percentage.
            tray::create(app.handle())?;
            tray::refresh_tray(app.handle());

            // Summon-panel global shortcut (⌘⇧U). Non-fatal if it can't bind.
            let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyU);
            if let Err(err) = app.global_shortcut().register(shortcut) {
                eprintln!("[anyleft] could not register panel shortcut: {err}");
            }

            // Promote the panel to a non-activating NSPanel so it can float over
            // another app's full-screen Space; this also wires click-outside-to-close.
            if let Some(panel) = app.get_webview_window(PANEL_LABEL) {
                windows::configure_overlay_panel(app.handle(), &panel)?;
            }

            // Closing the settings window hides it instead of tearing it down.
            if let Some(settings) = app.get_webview_window(SETTINGS_LABEL) {
                let handle = settings.clone();
                settings.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = handle.hide();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_catalog,
            commands::get_settings,
            commands::get_dashboard,
            commands::refresh,
            commands::connect_account,
            commands::disconnect_account,
            commands::set_account_enabled,
            commands::set_preferences,
            commands::open_settings,
            commands::close_settings,
            commands::hide_panel,
            commands::quit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AnyLeft");
}
