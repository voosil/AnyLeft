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

/// Open the app's main window — the settings screen.
///
/// This answers "the user launched AnyLeft", whether that is the first launch or
/// a repeat one that the single-instance plugin (or, on macOS, a reopen Apple
/// Event) routed to the instance already running.
///
/// It opens the window rather than the status-icon panel on purpose: the panel is
/// a popover, and one that no click summoned cannot hold macOS's focus — the app
/// is an accessory, so activation is handed straight back to the app the user is
/// working in — which would dismiss it within a fraction of a second and read as
/// a flicker.
///
/// The work is marshalled onto the main thread because it reaches AppKit: opening
/// the window also hides the panel, and the panel is an `NSPanel` (see
/// `windows::configure_overlay_panel`). The single-instance callback arrives on a
/// runtime worker thread, where AppKit traps instead of drawing.
fn open_main_window(app: &tauri::AppHandle) {
    let handle = app.clone();
    let queued = app.run_on_main_thread(move || {
        if let Err(err) = windows::show_settings(&handle) {
            eprintln!("[anyleft] could not open the main window: {err}");
        }
    });
    if let Err(err) = queued {
        eprintln!("[anyleft] could not open the main window: {err}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Registered first, and that ordering is the point: a second launch is
    // detected while plugins initialize — before the event loop creates any
    // window — and exits right there. So opening the app twice never yields two
    // processes, two taskbar entries, two status icons, or two usage fetches.
    // Instead the window of the instance already running comes back to the front,
    // which is what asking for the app again means.
    let builder = tauri::Builder::default().plugin(tauri_plugin_single_instance::init(
        |app, _argv, _cwd| open_main_window(app),
    ));

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
            let state = AppState::new(config_path);
            // A fresh install has never shown its interface (see
            // `AppSettings::first_launch_done`); every launch after the first —
            // including the launch-at-login one — stays quietly in the menu bar.
            let first_launch = !state.settings_snapshot().first_launch_done;
            app.manage(state);

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

            // First launch opens the main window. Without this the app starts
            // silent — no Dock icon, no window, just a status icon the user has
            // never seen before — so a fresh install looks like nothing happened.
            if first_launch {
                let state = app.state::<AppState>();
                // Recorded before opening the window: should showing it fail, the
                // next launch still behaves as a normal one.
                if let Err(err) = state
                    .commit_settings(state.settings_snapshot().with_first_launch_done())
                {
                    eprintln!("[anyleft] could not record the first launch: {err}");
                }
                open_main_window(app.handle());
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
        .build(tauri::generate_context!())
        .expect("error while building AnyLeft")
        .run(|app, event| {
            // On macOS "opening the app again" while it runs (a second
            // double-click in Finder, `open -a AnyLeft`) is an Apple Event
            // handled by the running process, not a second process: the
            // single-instance plugin never sees it. Answer it the same way.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = event
            {
                open_main_window(app);
            }

            // Windows has no reopen event; its single-instance callback covers
            // the same case.
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
