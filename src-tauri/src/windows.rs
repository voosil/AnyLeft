//! Window show/hide helpers shared by the tray handlers and the commands.
//!
//! The panel and settings windows are created up-front (see `tauri.conf.json`)
//! and toggled visible on demand — closing never destroys them. The panel is
//! promoted to a non-activating `NSPanel` so it can float over another app's
//! full-screen space without pulling that app out of full screen or switching
//! the user off to a different Space.

use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_nspanel::{ManagerExt, WebviewWindowExt};

pub const PANEL_LABEL: &str = "panel";
pub const SETTINGS_LABEL: &str = "settings";

/// `NSFloatingWindowLevel` — above ordinary windows, below the menu bar.
#[allow(non_upper_case_globals)]
const NS_FLOATING_WINDOW_LEVEL: i32 = 4;

/// `NSWindowStyleMaskNonactivatingPanel` — lets the panel take key focus (so its
/// inputs work) without activating the app, which is what would otherwise yank
/// the user off a full-screen Space.
#[allow(non_upper_case_globals)]
const NS_NONACTIVATING_PANEL_MASK: i32 = 1 << 7;

/// Promote the menu-bar panel window into a non-activating `NSPanel` that floats
/// over full-screen spaces, and wire "resign key" to hide it (click-outside to
/// close). Call once, at setup.
#[allow(deprecated)]
pub fn configure_overlay_panel(
    app: &AppHandle,
    window: &WebviewWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
    use tauri_nspanel::panel_delegate;

    let panel = window
        .to_panel()
        .map_err(|_| "failed to promote the panel window into an NSPanel")?;

    panel.set_level(NS_FLOATING_WINDOW_LEVEL);
    // Non-activating: showing the panel never steals app activation.
    panel.set_style_mask(NS_NONACTIVATING_PANEL_MASK);
    // Join every Space *and* draw on top of other apps' full-screen windows.
    panel.set_collection_behaviour(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
    );

    // Dismiss on focus loss, the native-panel equivalent of click-outside-to-close.
    let delegate = panel_delegate!(AnyLeftPanelDelegate {
        window_did_resign_key
    });
    let handle = app.clone();
    delegate.set_listener(Box::new(move |event: String| {
        if event == "window_did_resign_key" {
            hide_panel(&handle);
        }
    }));
    panel.set_delegate(delegate);

    Ok(())
}

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

/// Hide the menu-bar panel by ordering it out (keeps the window alive).
pub fn hide_panel(app: &AppHandle) {
    if let Ok(panel) = app.get_webview_panel(PANEL_LABEL) {
        panel.order_out(None);
    }
}
