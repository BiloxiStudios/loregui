//! Desktop integration commands: autostart at login and close-to-tray behavior.
//!
//! Exposes Tauri commands for the frontend to toggle "Start at login" and
//! "Close to tray instead of quitting" preferences.

use crate::settings::SettingsManager;
use serde::Serialize;
use tauri::State;
use tauri_plugin_autostart::ManagerExt;

/// Result returned when querying or changing desktop settings.
#[derive(Debug, Clone, Serialize)]
pub struct SettingsResult {
    pub autostart_enabled: bool,
    pub close_to_tray: bool,
}

/// Get the current desktop integration settings.
#[tauri::command]
pub fn get_desktop_settings(
    settings: State<'_, SettingsManager>,
) -> Result<SettingsResult, String> {
    let s = settings.get();
    Ok(SettingsResult {
        autostart_enabled: s.autostart_enabled,
        close_to_tray: s.close_to_tray,
    })
}

/// Enable or disable autostart at login.
///
/// This delegates to `tauri-plugin-autostart` for the platform-specific
/// registration (launchd plist on macOS, registry on Windows, .desktop on Linux).
#[tauri::command]
pub async fn set_autostart(
    app_handle: tauri::AppHandle,
    settings: State<'_, SettingsManager>,
    enabled: bool,
) -> Result<(), String> {
    // Update the persisted setting.
    settings.update(|s| s.autostart_enabled = enabled)?;

    // Register or unregister with the autostart plugin.
    if enabled {
        app_handle
            .autolaunch()
            .enable()
            .map_err(|e| format!("failed to enable autostart: {e}"))?;
    } else {
        app_handle
            .autolaunch()
            .disable()
            .map_err(|e| format!("failed to disable autostart: {e}"))?;
    }

    Ok(())
}

/// Enable or disable "close to tray" behavior.
#[tauri::command]
pub fn set_close_to_tray(
    settings: State<'_, SettingsManager>,
    enabled: bool,
) -> Result<(), String> {
    settings.update(|s| s.close_to_tray = enabled)?;
    Ok(())
}
