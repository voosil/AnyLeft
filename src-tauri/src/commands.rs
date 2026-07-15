//! The Tauri command surface — the "native bridge" the React frontend calls.
//!
//! Every mutation validates its input, builds a *new* settings value, persists
//! it, refreshes the menu-bar title, and returns the fresh settings so the UI
//! can render from a single source of truth.

use tauri::{ipc::Channel, AppHandle, State};

use crate::catalog;
use crate::error::{AppError, AppResult};
use crate::models::{AuthMethod, CatalogProvider, Dashboard, DashboardProvider};
use crate::secrets;
use crate::settings::{Account, AppSettings, Preferences};
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
pub async fn get_dashboard(
    channel: Channel<DashboardProvider>,
    state: State<'_, AppState>,
) -> AppResult<Dashboard> {
    state.stream_dashboard(false, channel).await
}

/// Force a fresh usage fetch and update the menu-bar number.
#[tauri::command]
pub async fn refresh(
    app: AppHandle,
    channel: Channel<DashboardProvider>,
    state: State<'_, AppState>,
) -> AppResult<Dashboard> {
    let dashboard = state.stream_dashboard(true, channel).await?;
    tray::refresh_tray(&app);
    Ok(dashboard)
}

/// Connect, re-configure, or rename a provider account. A provider may hold
/// several accounts (e.g. two Kimi logins); single-instance providers (Claude,
/// ChatGPT — see `catalog::is_single_instance`) are pinned to one.
///
/// * `account_id` present → reconfigure that existing account (edit key/label).
/// * absent, single-instance → the account id equals the provider id.
/// * absent, multi-instance → a fresh unique account id is minted.
///
/// For API-key auth the key is written to the keychain under the account id and
/// only a `has_secret` flag is persisted. A blank key while reconfiguring keeps
/// the previously stored one (so the label can be changed without re-pasting).
#[tauri::command]
pub fn connect_account(
    app: AppHandle,
    state: State<AppState>,
    provider_id: String,
    auth_method: AuthMethod,
    api_key: Option<String>,
    label: Option<String>,
    account_id: Option<String>,
) -> AppResult<AppSettings> {
    if !catalog::exists(&provider_id) {
        return Err(AppError::UnknownProvider(provider_id));
    }

    let current = state.settings_snapshot();
    let target_id = resolve_account_id(&provider_id, account_id, &current);
    let existing = current.account(&target_id).cloned();

    let mut has_secret = existing.as_ref().map(|a| a.has_secret).unwrap_or(false);
    if auth_method == AuthMethod::Key {
        let key = api_key.as_deref().map(str::trim).unwrap_or_default();
        if key.is_empty() {
            if !has_secret {
                return Err(AppError::Invalid("API Key 不能为空".into()));
            }
        } else {
            secrets::set_key(&target_id, key)?;
            has_secret = true;
        }
    } else {
        has_secret = false;
    }

    let account = Account {
        account_id: target_id,
        provider_id,
        label: normalize_label(label),
        enabled: existing.as_ref().map(|a| a.enabled).unwrap_or(true),
        auth_method,
        has_secret,
    };
    let saved = state.commit_settings(current.with_account(account))?;
    tray::refresh_tray(&app);
    Ok(saved)
}

/// Disconnect an account by its id, deleting any stored key (best-effort).
#[tauri::command]
pub fn disconnect_account(
    app: AppHandle,
    state: State<AppState>,
    account_id: String,
) -> AppResult<AppSettings> {
    if let Err(err) = secrets::delete_key(&account_id) {
        eprintln!("[anyleft] failed to delete key for {account_id}: {err}");
    }
    let next = state.settings_snapshot().without_account(&account_id);
    let saved = state.commit_settings(next)?;
    tray::refresh_tray(&app);
    Ok(saved)
}

/// Pause or resume tracking for a connected account.
#[tauri::command]
pub fn set_account_enabled(
    app: AppHandle,
    state: State<AppState>,
    account_id: String,
    enabled: bool,
) -> AppResult<AppSettings> {
    let current = state.settings_snapshot();
    if current.account(&account_id).is_none() {
        return Err(AppError::Invalid("账户不存在 / unknown account".into()));
    }
    let saved = state.commit_settings(current.with_account_enabled(&account_id, enabled))?;
    tray::refresh_tray(&app);
    Ok(saved)
}

/// Resolve which account id `connect_account` should write to.
fn resolve_account_id(
    provider_id: &str,
    requested: Option<String>,
    current: &AppSettings,
) -> String {
    if catalog::is_single_instance(provider_id) {
        return provider_id.to_string();
    }
    match requested
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        // Only honor a requested id that already exists (reconfigure); otherwise
        // mint a fresh one so a stale/spoofed id can't collide.
        Some(id) if current.account(id).is_some() => id.to_string(),
        _ => generate_account_id(provider_id),
    }
}

/// A unique account id, e.g. `kimi-1736380800000`. Uniqueness relies on the
/// millisecond clock — a user cannot add two accounts within the same tick.
fn generate_account_id(provider_id: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{provider_id}-{millis}")
}

/// Trim a label to `None` when empty so absent and blank read the same.
fn normalize_label(label: Option<String>) -> Option<String> {
    label.map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
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
