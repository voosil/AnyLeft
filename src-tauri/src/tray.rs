//! The menu-bar tray icon, its context menu, and the show/hide logic for the
//! panel dropdown.

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow,
};
use tauri_nspanel::ManagerExt;

use crate::state::AppState;
use crate::windows::{self, PANEL_LABEL};

/// Menu-bar template icon, embedded at compile time. Source:
/// `src-tauri/icons/tray-icon.svg` (the "1a Half gauge · 半月表" direction
/// from `AnyLeft Icons.dc.html`, stripped to a monochrome ◑ glyph so the
/// system can recolour it for light/dark menu bars via `icon_as_template`).
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/trayTemplate@2x.png");

fn tray_icon() -> tauri::Result<Image<'static>> {
    Image::from_bytes(TRAY_ICON_BYTES).map_err(Into::into)
}

pub const TRAY_ID: &str = "main";

/// Fallbacks used only when the OS can't report the real panel/monitor size.
const FALLBACK_PANEL: PhysicalSize<u32> = PhysicalSize::new(360, 468);
const EDGE_MARGIN: f64 = 8.0;
const MENU_BAR_HEIGHT: f64 = 30.0;

/// Build the tray icon with its context menu and click handler.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let settings_item = MenuItem::with_id(app, "settings", "设置…", true, Some("Cmd+,"))?;
    let refresh_item = MenuItem::with_id(app, "refresh", "刷新用量", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 AnyLeft", true, Some("Cmd+Q"))?;
    let menu = Menu::with_items(
        app,
        &[
            &settings_item,
            &refresh_item,
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon()?)
        .icon_as_template(true)
        .tooltip("AnyLeft 剩了么")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                let _ = windows::show_settings(app);
            }
            "refresh" => refresh_usage(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_panel(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Keep the menu-bar item icon-only. Older builds displayed a live `◐ NN%`
/// title here; clearing it on every refresh path also removes stale titles from
/// already-running dev builds after rebuild/relaunch.
pub fn refresh_tray(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_title(None::<String>);
    }
}

/// Force-refresh usage from the context menu without restoring the removed
/// menu-bar percentage title.
fn refresh_usage(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = {
            let state = app.state::<AppState>();
            state.dashboard(true).await
        };
        if let Err(err) = result {
            eprintln!("[anyleft] manual refresh failed: {err}");
        }
        refresh_tray(&app);
    });
}

/// Show the panel if hidden, hide it if visible. Placement always comes from the
/// tray icon's live screen rect, so a left-click and the keyboard shortcut open
/// the panel in exactly the same spot.
///
/// The panel is an `NSPanel` (see `windows::configure_overlay_panel`): showing
/// it via the panel API — not `WebviewWindow::show` + `set_focus` — is what lets
/// it float over another app's full-screen Space instead of switching away.
pub fn toggle_panel(app: &AppHandle) {
    let Ok(panel) = app.get_webview_panel(PANEL_LABEL) else {
        return;
    };
    if panel.is_visible() {
        panel.order_out(None);
    } else {
        if let Some(window) = app.get_webview_window(PANEL_LABEL) {
            position_panel(app, &window);
        }
        panel.show();
    }
}

/// Position the panel under the tray icon (or top-right as a fallback), clamped
/// to the monitor that hosts the menu bar.
fn position_panel(app: &AppHandle, window: &WebviewWindow) {
    let size = window.outer_size().unwrap_or(FALLBACK_PANEL);
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());

    let (mon_x, mon_y, mon_w, scale) = match monitor.as_ref() {
        Some(m) => (
            m.position().x as f64,
            m.position().y as f64,
            m.size().width as f64,
            m.scale_factor(),
        ),
        None => (0.0, 0.0, 1440.0, 1.0),
    };

    let margin = EDGE_MARGIN * scale;
    let (mut x, y) = match tray_icon_anchor(app, scale) {
        Some(p) => (p.x - size.width as f64 / 2.0, p.y + margin),
        None => (
            mon_x + mon_w - size.width as f64 - margin,
            mon_y + MENU_BAR_HEIGHT * scale,
        ),
    };

    let min_x = mon_x + margin;
    let max_x = mon_x + mon_w - size.width as f64 - margin;
    x = x.clamp(min_x, max_x.max(min_x));

    let _ = window.set_position(PhysicalPosition::new(x.round(), y.round()));
}

/// The point directly under the tray icon: horizontally centred on the icon, at
/// its bottom edge. Read from the icon's live screen rect so every entry point
/// (tray click, keyboard shortcut) anchors to the same place. `None` when the OS
/// can't report the rect, letting the caller fall back to a fixed corner.
fn tray_icon_anchor(app: &AppHandle, scale: f64) -> Option<PhysicalPosition<f64>> {
    let rect = app.tray_by_id(TRAY_ID)?.rect().ok().flatten()?;
    let pos = rect.position.to_physical::<f64>(scale);
    let size = rect.size.to_physical::<f64>(scale);
    Some(PhysicalPosition::new(
        pos.x + size.width / 2.0,
        pos.y + size.height,
    ))
}
