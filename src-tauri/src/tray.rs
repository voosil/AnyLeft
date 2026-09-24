//! The menu-bar tray icon, its context menu, and the show/hide logic for the
//! panel dropdown.

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Rect, WebviewWindow,
};

use crate::state::AppState;
use crate::windows::{self, PANEL_LABEL};

/// Menu-bar template icon, embedded at compile time. Source:
/// `src-tauri/icons/tray-icon.svg` (the "1a Half gauge · 半月表" direction
/// from `AnyLeft Icons.dc.html`, stripped to a monochrome ◑ glyph so the
/// system can recolour it for light/dark menu bars via `icon_as_template`).
#[cfg(target_os = "macos")]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/trayTemplate@2x.png");

// Windows notification icons are not template images, so use the full-colour
// application artwork instead of the monochrome macOS menu-bar glyph.
#[cfg(not(target_os = "macos"))]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/32x32.png");

fn tray_icon() -> tauri::Result<Image<'static>> {
    Image::from_bytes(TRAY_ICON_BYTES).map_err(Into::into)
}

pub const TRAY_ID: &str = "main";

/// Fallbacks used only when the OS can't report the real panel/monitor size.
const FALLBACK_PANEL: PhysicalSize<u32> = PhysicalSize::new(360, 468);
const EDGE_MARGIN: f64 = 8.0;

#[cfg(target_os = "macos")]
const SETTINGS_ACCELERATOR: &str = "Cmd+,";
#[cfg(not(target_os = "macos"))]
const SETTINGS_ACCELERATOR: &str = "Ctrl+,";

#[cfg(target_os = "macos")]
const QUIT_ACCELERATOR: &str = "Cmd+Q";
#[cfg(not(target_os = "macos"))]
const QUIT_ACCELERATOR: &str = "Ctrl+Q";

/// Build the tray icon with its context menu and click handler.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let settings_item =
        MenuItem::with_id(app, "settings", "设置…", true, Some(SETTINGS_ACCELERATOR))?;
    let refresh_item = MenuItem::with_id(app, "refresh", "刷新用量", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 AnyLeft", true, Some(QUIT_ACCELERATOR))?;
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
        .icon_as_template(cfg!(target_os = "macos"))
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
                rect,
                ..
            } = event
            {
                toggle_panel_at(tray.app_handle(), Some(rect));
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
    toggle_panel_at(app, None);
}

fn toggle_panel_at(app: &AppHandle, event_rect: Option<Rect>) {
    if windows::panel_is_visible(app) {
        windows::hide_panel(app);
    } else {
        if let Some(window) = app.get_webview_window(PANEL_LABEL) {
            position_panel(app, &window, event_rect);
        }
        windows::show_panel(app);
    }
}

/// Position the panel beside the tray icon (or top-right as a fallback),
/// clamped to the monitor work area.
fn position_panel(app: &AppHandle, window: &WebviewWindow, event_rect: Option<Rect>) {
    let size = window.outer_size().unwrap_or(FALLBACK_PANEL);

    let monitors = window.available_monitors().unwrap_or_default();

    // The click event has the freshest icon bounds on Windows. The tray query
    // keeps global-shortcut positioning working when there is no click event.
    let anchor = event_rect
        .or_else(|| {
            app.tray_by_id(TRAY_ID)
                .and_then(|tray| tray.rect().ok().flatten())
        })
        .and_then(|rect| tray_anchor(rect, &monitors));

    let monitor = anchor
        .as_ref()
        .map(|(_, monitor)| monitor.clone())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());


    let (work_area, scale) = match monitor.as_ref() {
        Some(m) => {
            let area = m.work_area();
            (
                Bounds {
                    x: area.position.x as f64,
                    y: area.position.y as f64,
                    width: area.size.width as f64,
                    height: area.size.height as f64,
                },
                m.scale_factor(),
            )
        }
        None => (Bounds::new(0.0, 0.0, 1440.0, 900.0), 1.0),
    };

    let margin = EDGE_MARGIN * scale;

    let tray_bounds = anchor.map(|(rect, _)| {
        let pos = rect.position.to_physical::<f64>(scale);
        let size = rect.size.to_physical::<f64>(scale);
        Bounds::new(pos.x, pos.y, size.width, size.height)
    });

    let (x, y) = panel_position(size, tray_bounds, work_area, margin);

    let _ = window.set_position(PhysicalPosition::new(x.round(), y.round()));
}

/// The tray icon's reported rect, paired with the monitor it sits on — `None`
/// when that rect cannot anchor the panel.
///
/// Until the OS lays a freshly created status item out, it reports a zero-sized
/// rect in a screen corner (0,2160 with size 68×0 physical, measured on a 4K
/// display). Read as icon bounds that anchors the panel to the far corner instead
/// of the menu bar, so such a rect counts as "no icon bounds known" and the panel
/// falls back to the work area's top-right corner.
fn tray_anchor(rect: Rect, monitors: &[tauri::Monitor]) -> Option<(Rect, tauri::Monitor)> {
    if !rect_has_area(&rect) {
        return None;
    }

    monitor_containing(&rect, monitors).map(|monitor| (rect, monitor))
}

/// Whether a reported tray rect is bigger than nothing. The size is only ever
/// compared against zero, so the unit it is expressed in does not matter.
fn rect_has_area(rect: &Rect) -> bool {
    let size = rect.size.to_physical::<f64>(1.0);
    size.width > 0.0 && size.height > 0.0
}

/// The monitor whose bounds contain the tray icon's position, if any. `None`
/// means the rect is unusable — a status item the OS has not placed yet — rather
/// than that the icon sits on no screen.
fn monitor_containing(rect: &Rect, monitors: &[tauri::Monitor]) -> Option<tauri::Monitor> {
    monitors.iter().find_map(|m| {
        let p = rect.position.to_physical::<f64>(m.scale_factor());
        let pos = m.position();
        let size = m.size();
        let inside = p.x >= pos.x as f64
            && p.x <= pos.x as f64 + size.width as f64
            && p.y >= pos.y as f64
            && p.y <= pos.y as f64 + size.height as f64;
        inside.then(|| m.clone())
    })
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Bounds {
    const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn right(self) -> f64 {
        self.x + self.width
    }

    fn bottom(self) -> f64 {
        self.y + self.height
    }
}

/// Place the panel on the work-area side of the tray icon. This handles the
/// macOS top menu bar plus Windows taskbars docked to any screen edge.
fn panel_position(
    panel: PhysicalSize<u32>,
    tray: Option<Bounds>,
    work: Bounds,
    margin: f64,
) -> (f64, f64) {
    let panel_width = panel.width as f64;
    let panel_height = panel.height as f64;
    let min_x = work.x + margin;
    let max_x = (work.right() - panel_width - margin).max(min_x);
    let min_y = work.y + margin;
    let max_y = (work.bottom() - panel_height - margin).max(min_y);

    let Some(tray) = tray else {
        return (max_x, min_y);
    };

    if tray.right() <= work.x {
        // Taskbar on the left edge.
        let x = (tray.right() + margin).clamp(min_x, max_x);
        let y = (tray.y + tray.height / 2.0 - panel_height / 2.0).clamp(min_y, max_y);
        return (x, y);
    }

    if tray.x >= work.right() {
        // Taskbar on the right edge.
        let x = (tray.x - panel_width - margin).clamp(min_x, max_x);
        let y = (tray.y + tray.height / 2.0 - panel_height / 2.0).clamp(min_y, max_y);
        return (x, y);
    }

    let x = (tray.x + tray.width / 2.0 - panel_width / 2.0).clamp(min_x, max_x);
    let below = tray.bottom() + margin;
    let above = tray.y - panel_height - margin;
    let y = if below + panel_height <= work.bottom() {
        below
    } else if above >= work.y {
        above
    } else {
        below.clamp(min_y, max_y)
    };
    (x, y.clamp(min_y, max_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PANEL: PhysicalSize<u32> = PhysicalSize::new(340, 360);

    #[test]
    fn rejects_the_rect_of_a_status_item_the_os_has_not_laid_out() {
        // Measured on a 4K macOS display: just after the icon is created, the
        // rect comes back as 68×0 sitting at the screen's bottom-left. Treated as
        // icon bounds it would place the panel in the corner opposite the menu
        // bar, so it has to read as "no icon bounds known".
        let not_laid_out = Rect {
            position: PhysicalPosition::new(0, 2160).into(),
            size: PhysicalSize::new(68, 0).into(),
        };
        assert!(!rect_has_area(&not_laid_out));

        let laid_out = Rect {
            position: PhysicalPosition::new(3400, 0).into(),
            size: PhysicalSize::new(48, 48).into(),
        };
        assert!(rect_has_area(&laid_out));
    }

    #[test]
    fn opens_below_a_top_menu_bar() {
        let work = Bounds::new(0.0, 30.0, 1440.0, 870.0);
        let tray = Bounds::new(1200.0, 0.0, 24.0, 24.0);
        assert_eq!(panel_position(PANEL, Some(tray), work, 8.0), (1042.0, 38.0));
    }

    #[test]
    fn opens_above_a_bottom_windows_taskbar() {
        let work = Bounds::new(0.0, 0.0, 1920.0, 1040.0);
        let tray = Bounds::new(1800.0, 1040.0, 32.0, 40.0);
        assert_eq!(
            panel_position(PANEL, Some(tray), work, 8.0),
            (1572.0, 672.0)
        );
    }

    #[test]
    fn opens_inside_a_right_docked_taskbar() {
        let work = Bounds::new(0.0, 0.0, 1872.0, 1080.0);
        let tray = Bounds::new(1872.0, 800.0, 48.0, 32.0);
        assert_eq!(
            panel_position(PANEL, Some(tray), work, 8.0),
            (1524.0, 636.0)
        );
    }
}
