//! Window show/hide helpers shared by the tray handlers and the commands.
//!
//! The panel and settings windows are created up-front (see `tauri.conf.json`)
//! and toggled visible on demand — closing never destroys them.

use tauri::{AppHandle, Manager};

pub const PANEL_LABEL: &str = "panel";
pub const SETTINGS_LABEL: &str = "settings";

/// Reveal and focus the settings window; hide the panel behind it.
pub fn show_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        window.show()?;
        let _ = window.unminimize();
        window.set_focus()?;
    }
    hide_panel(app);
    Ok(())
}

/// Hide (not destroy) the settings window.
pub fn hide_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        window.hide()?;
    }
    Ok(())
}

/// Hide the menu-bar panel.
pub fn hide_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PANEL_LABEL) {
        let _ = window.hide();
    }
}
