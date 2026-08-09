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
    let builder = tauri::Builder::default();

    // The native NSPanel plugin cannot be linked on Windows. The platform
    // fallback uses the regular Tauri window APIs instead.
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
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

            // A menu-bar app on macOS: no Dock icon or app-switcher entry.
            // Windows keeps the process reachable through its system tray icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Tray icon + live percentage.
            tray::create(app.handle())?;
            tray::refresh_tray(app.handle());

            // Summon-panel global shortcut (⌘⇧U on macOS, Ctrl+Shift+U on
            // Windows). Non-fatal if another app already owns it.
            let command_or_control = if cfg!(target_os = "macos") {
                Modifiers::SUPER
            } else {
                Modifiers::CONTROL
            };
            let shortcut = Shortcut::new(Some(command_or_control | Modifiers::SHIFT), Code::KeyU);
            if let Err(err) = app.global_shortcut().register(shortcut) {
                eprintln!("[anyleft] could not register panel shortcut: {err}");
            }

            // macOS promotes this to an NSPanel. Windows keeps the configured
            // borderless always-on-top window and wires click-outside-to-close.
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
