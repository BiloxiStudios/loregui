//! Application settings persistence.
//!
//! Stores user preferences (autostart, close-to-tray) in a JSON file
//! in the app's config directory.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// User-configurable application settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    /// Whether LoreGUI should start automatically at login.
    #[serde(default)]
    pub autostart_enabled: bool,
    /// Whether closing the main window hides to tray instead of quitting.
    #[serde(default)]
    pub close_to_tray: bool,
    /// Last repository path that completed real backend validation. This is a
    /// local, non-secret path only: remote URLs, credentials, and tokens never
    /// belong in desktop settings.
    #[serde(default)]
    pub active_repository: Option<PathBuf>,
}

/// Manages loading and saving app settings to disk.
pub struct SettingsManager {
    settings_path: PathBuf,
    cache: Mutex<AppSettings>,
}

impl SettingsManager {
    /// Create a new settings manager, loading from the config directory.
    pub fn new(config_dir: PathBuf) -> Self {
        let settings_path = config_dir.join("settings.json");
        let cache = Mutex::new(Self::load_from_disk(&settings_path));
        Self {
            settings_path,
            cache,
        }
    }

    fn load_from_disk(path: &PathBuf) -> AppSettings {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                // Absent file is the normal first-run case — default silently.
                // Any other read error (permissions, etc.) is worth a warning,
                // but we still fall back to defaults so startup never breaks.
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "could not read settings file; using defaults"
                    );
                }
                return AppSettings::default();
            }
        };

        match serde_json::from_str(&content) {
            Ok(settings) => settings,
            Err(e) => {
                // The file exists but is corrupt/unparseable. Silently defaulting
                // here (the old behaviour) would overwrite the user's real prefs
                // on the next `update`. Preserve the bad file as a `.bak` and warn
                // so the data is recoverable, then fall back to defaults.
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "settings file is corrupt; backing it up and using defaults"
                );
                let backup = path.with_extension("json.bak");
                if let Err(rename_err) = std::fs::rename(path, &backup) {
                    tracing::warn!(
                        path = %path.display(),
                        backup = %backup.display(),
                        error = %rename_err,
                        "failed to back up corrupt settings file"
                    );
                }
                AppSettings::default()
            }
        }
    }

    /// Get the current settings.
    pub fn get(&self) -> AppSettings {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Save updated settings to disk.
    ///
    /// Write failures are logged (previously they were silently swallowed, so a
    /// preference change could appear to take effect in-memory yet never persist
    /// — and be lost on the next launch). The in-memory cache is still updated
    /// regardless so the running session reflects the change.
    pub fn update(&self, f: impl FnOnce(&mut AppSettings)) {
        let mut settings = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut settings);
        match serde_json::to_string_pretty(&*settings) {
            Ok(json) => {
                if let Some(parent) = self.settings_path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        tracing::warn!(
                            path = %parent.display(),
                            error = %e,
                            "could not create settings directory"
                        );
                    }
                }
                if let Err(e) = std::fs::write(&self.settings_path, json) {
                    tracing::warn!(
                        path = %self.settings_path.display(),
                        error = %e,
                        "could not persist settings to disk"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not serialize settings");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsManager;

    #[test]
    fn active_repository_round_trips_as_the_only_repository_context() {
        let tmp = tempfile::tempdir().expect("temp settings directory");
        let expected = tmp.path().join("client-working-tree");

        let settings = SettingsManager::new(tmp.path().to_path_buf());
        settings.update(|value| value.active_repository = Some(expected.clone()));

        let reloaded = SettingsManager::new(tmp.path().to_path_buf()).get();
        assert_eq!(reloaded.active_repository, Some(expected.clone()));
        let json = std::fs::read_to_string(tmp.path().join("settings.json"))
            .expect("persisted settings json");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid settings json");
        let object = parsed.as_object().expect("settings object");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["active_repository", "autostart_enabled", "close_to_tray"]
        );
        assert_eq!(
            object.get("active_repository"),
            Some(&serde_json::json!(expected))
        );
    }
}
