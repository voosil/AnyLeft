//! The Tauri command surface — the "native bridge" the React frontend calls.
//!
//! Every mutation validates its input, builds a *new* settings value, persists
//! it, refreshes the menu-bar title, and returns the fresh settings so the UI
//! can render from a single source of truth.

use tauri::{AppHandle, State};

use crate::catalog;
use crate::error::{AppError, AppResult};
use crate::models::{AuthMethod, CatalogProvider, Dashboard};
use crate::secrets;
use crate::settings::{AppSettings, Preferences};
use crate::state::AppState;
use crate::tray;
use crate::windows;

/// Full provider catalog for the "add account" screen.
#[tauri::command]
pub fn get_catalog() -> Vec<CatalogProvider> {
    catalog::all()
}

/// Current persisted settings.
#[tauri::command]
pub fn get_settings(state: State<AppState>) -> AppSettings {
    state.settings_snapshot()
}

/// The panel payload (enabled providers + live usage, served from cache).
#[tauri::command]
pub async fn get_dashboard(state: State<'_, AppState>) -> AppResult<Dashboard> {
    state.dashboard(false).await
}

/// Force a fresh usage fetch and update the menu-bar number.
#[tauri::command]
pub async fn refresh(app: AppHandle, state: State<'_, AppState>) -> AppResult<Dashboard> {
    let dashboard = state.dashboard(true).await?;
    tray::refresh_tray(&app);
    Ok(dashboard)
}

/// Connect (or re-connect) a provider account. For API-key auth the key is
/// written to the keychain and only a `has_secret` flag is persisted.
#[tauri::command]
pub fn connect_account(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    auth_method: AuthMethod,
    api_key: Option<String>,
) -> AppResult<AppSettings> {
    if !catalog::exists(&id) {
        return Err(AppError::UnknownProvider(id));
    }

    let mut has_secret = false;
    if auth_method == AuthMethod::Key {
        let key = api_key.as_deref().map(str::trim).unwrap_or_default();
        if key.is_empty() {
            return Err(AppError::Invalid("API Key 不能为空".into()));
        }
        secrets::set_key(&id, key)?;
        has_secret = true;
    }

    let next = state
        .settings_snapshot()
        .with_account(&id, auth_method, has_secret);
    let saved = state.commit_settings(next)?;
    tray::refresh_tray(&app);
    Ok(saved)
}

/// Disconnect a provider, deleting any stored key (best-effort).
#[tauri::command]
pub fn disconnect_account(
    app: AppHandle,
    state: State<AppState>,
    id: String,
) -> AppResult<AppSettings> {
    if let Err(err) = secrets::delete_key(&id) {
        eprintln!("[anyleft] failed to delete key for {id}: {err}");
    }
    let next = state.settings_snapshot().without_account(&id);
    let saved = state.commit_settings(next)?;
    tray::refresh_tray(&app);
    Ok(saved)
}

/// Pause or resume tracking for a connected provider.
#[tauri::command]
pub fn set_account_enabled(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    enabled: bool,
) -> AppResult<AppSettings> {
    let current = state.settings_snapshot();
    if current.account(&id).is_none() {
        return Err(AppError::UnknownProvider(id));
    }
    let saved = state.commit_settings(current.with_account_enabled(&id, enabled))?;
    tray::refresh_tray(&app);
    Ok(saved)
}

/// Replace the preferences block.
#[tauri::command]
pub fn set_preferences(
    app: AppHandle,
    state: State<AppState>,
    preferences: Preferences,
) -> AppResult<AppSettings> {
    let preferences = validate_preferences(preferences)?;
    let next = state.settings_snapshot().with_preferences(preferences);
    let saved = state.commit_settings(next)?;
    tray::refresh_tray(&app);
    Ok(saved)
}

/// Open the settings window.
#[tauri::command]
pub fn open_settings(app: AppHandle) -> AppResult<()> {
    windows::show_settings(&app)?;
    Ok(())
}

/// Hide the settings window (the traffic-light close button).
#[tauri::command]
pub fn close_settings(app: AppHandle) -> AppResult<()> {
    windows::hide_settings(&app)?;
    Ok(())
}

/// Hide the menu-bar panel.
#[tauri::command]
pub fn hide_panel(app: AppHandle) {
    windows::hide_panel(&app);
}

/// Quit the whole app.
#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

/// Light validation for the accent color and shortcut string.
fn validate_preferences(preferences: Preferences) -> AppResult<Preferences> {
    let accent = preferences.accent.trim();
    if !is_hex_color(accent) {
        return Err(AppError::Invalid(format!("颜色值无效 / invalid color: {accent}")));
    }
    if preferences.shortcut.trim().is_empty() {
        return Err(AppError::Invalid("快捷键不能为空 / shortcut is empty".into()));
    }
    Ok(preferences)
}

/// Accept `#RGB` or `#RRGGBB`.
fn is_hex_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}
