//! Application settings persistence.
//!
//! Stores user preferences (autostart, close-to-tray) in a JSON file
//! in the app's config directory.

use crate::context::{validate_no_raw_secrets, ContextSettings};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;

/// User-configurable application settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
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
    /// Versioned server/repository/project context. Credentials are represented
    /// only by opaque OS-store references inside this model.
    #[serde(default)]
    pub context: ContextSettings,
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

        match Self::parse_settings(&content) {
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

    fn parse_settings(content: &str) -> Result<AppSettings, String> {
        let raw: Value = serde_json::from_str(content)
            .map_err(|error| format!("settings JSON is malformed: {error}"))?;
        validate_no_raw_secrets(&raw)?;
        let settings: AppSettings = serde_json::from_value(raw)
            .map_err(|error| format!("settings schema is invalid: {error}"))?;
        settings.context.validate_for_persistence()?;
        Ok(settings)
    }

    /// Get the current settings.
    pub fn get(&self) -> AppSettings {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Validate and persist a candidate update before publishing it in memory.
    /// Failed validation or disk writes leave both cache and disk unchanged.
    pub fn update(&self, f: impl FnOnce(&mut AppSettings)) -> Result<(), String> {
        let mut settings = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let mut candidate = settings.clone();
        f(&mut candidate);
        self.persist(&candidate)?;
        *settings = candidate;
        Ok(())
    }

    /// Validate a context update before publishing it to disk and memory.
    /// Invalid or unpersistable updates leave both the file and cache unchanged.
    pub fn update_context(&self, context: ContextSettings) -> Result<(), String> {
        context.validate_for_persistence()?;
        self.update(move |settings| settings.context = context)
    }

    pub fn update_context_selection(
        &self,
        context: ContextSettings,
        active_repository: Option<PathBuf>,
    ) -> Result<(), String> {
        context.validate_for_persistence()?;
        self.update(move |settings| {
            settings.context = context;
            settings.active_repository = active_repository;
        })
    }

    fn persist(&self, settings: &AppSettings) -> Result<(), String> {
        let raw = serde_json::to_value(settings)
            .map_err(|error| format!("could not serialize settings: {error}"))?;
        validate_no_raw_secrets(&raw)?;
        settings.context.validate()?;
        let json = serde_json::to_string_pretty(settings)
            .map_err(|error| format!("could not serialize settings: {error}"))?;
        if let Some(parent) = self.settings_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "could not create settings directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        std::fs::write(&self.settings_path, json).map_err(|error| {
            format!(
                "could not persist settings to {}: {error}",
                self.settings_path.display()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsManager;
    use crate::context::{AuthMode, ContextSettings, ServerProfile, ServerSource};

    fn complete_context() -> ContextSettings {
        let mut context = ContextSettings::default();
        context.servers.push(ServerProfile {
            id: "server-1".into(),
            alias: "Local Lore".into(),
            url: "lore://127.0.0.1:41337/repository-1".into(),
            source: ServerSource::Manual,
            favorite: false,
            auth_mode: AuthMode::NotRequired,
            credential_ref: None,
            last_seen_at: None,
        });
        context
    }

    #[test]
    fn context_selection_persists_context_and_repository_as_one_candidate() {
        let tmp = tempfile::tempdir().expect("temp settings directory");
        let settings = SettingsManager::new(tmp.path().to_path_buf());
        let context = complete_context();
        let path = tmp.path().join("project-a");

        settings
            .update_context_selection(context.clone(), Some(path.clone()))
            .expect("atomic context selection");

        let reloaded = SettingsManager::new(tmp.path().to_path_buf()).get();
        assert_eq!(reloaded.context, context);
        assert_eq!(reloaded.active_repository, Some(path));
    }

    #[test]
    fn failed_context_selection_retains_cache_and_disk() {
        let tmp = tempfile::tempdir().expect("temp root");
        let blocked = tmp.path().join("blocked-config");
        std::fs::write(&blocked, "not-a-directory").expect("blocking file");
        let settings = SettingsManager::new(blocked.clone());
        let before = settings.get();

        assert!(settings
            .update_context_selection(complete_context(), Some(tmp.path().join("candidate")))
            .is_err());
        let after = settings.get();
        assert_eq!(after.context, before.context);
        assert_eq!(after.active_repository, before.active_repository);
        assert_eq!(after.autostart_enabled, before.autostart_enabled);
        assert_eq!(after.close_to_tray, before.close_to_tray);
        assert_eq!(std::fs::read_to_string(blocked).unwrap(), "not-a-directory");
    }

    #[test]
    fn active_repository_round_trips_as_the_only_repository_context() {
        let tmp = tempfile::tempdir().expect("temp settings directory");
        let expected = tmp.path().join("client-working-tree");

        let settings = SettingsManager::new(tmp.path().to_path_buf());
        settings
            .update(|value| value.active_repository = Some(expected.clone()))
            .expect("persist active repository");

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
            vec![
                "active_repository",
                "autostart_enabled",
                "close_to_tray",
                "context"
            ]
        );
        assert_eq!(
            object.get("active_repository"),
            Some(&serde_json::json!(expected))
        );
    }

    #[test]
    fn empty_legacy_settings_migrate_to_schema_v1_context() {
        let tmp = tempfile::tempdir().expect("temp settings directory");
        std::fs::write(tmp.path().join("settings.json"), "{}").expect("legacy settings");

        let settings = SettingsManager::new(tmp.path().to_path_buf()).get();
        assert_eq!(settings.context, ContextSettings::default());
    }

    #[test]
    fn malformed_or_unknown_settings_fail_closed_with_recoverable_backup() {
        for contents in [
            r#"{"context":{"schema_version":99}}"#,
            r#"{"unknown_setting":true}"#,
            r#"{"context":{"token":"raw-token"}}"#,
            "{not-json",
        ] {
            let tmp = tempfile::tempdir().expect("temp settings directory");
            let path = tmp.path().join("settings.json");
            std::fs::write(&path, contents).expect("invalid settings");

            let settings = SettingsManager::new(tmp.path().to_path_buf()).get();
            assert_eq!(settings.context, ContextSettings::default());
            assert!(!path.exists(), "invalid primary file should be moved");
            assert!(tmp.path().join("settings.json.bak").exists());
        }
    }

    #[test]
    fn url_userinfo_context_update_is_rejected_without_persistence() {
        let tmp = tempfile::tempdir().expect("temp settings directory");
        let settings = SettingsManager::new(tmp.path().to_path_buf());
        settings
            .update(|value| value.close_to_tray = true)
            .expect("persist baseline settings");
        let original = std::fs::read_to_string(tmp.path().join("settings.json"))
            .expect("baseline persisted settings");

        let mut context = ContextSettings::default();
        context.servers.push(ServerProfile {
            id: "server-1".into(),
            alias: "EROS".into(),
            url: "https://user:raw-password@example.test".into(),
            source: ServerSource::Manual,
            favorite: false,
            auth_mode: AuthMode::Unknown,
            credential_ref: None,
            last_seen_at: None,
        });

        assert!(settings.update_context(context).is_err());
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("settings.json"))
                .expect("settings remain persisted"),
            original
        );
        assert!(settings.get().context.servers.is_empty());
    }

    #[test]
    fn failed_generic_update_leaves_cache_and_disk_unchanged() {
        let tmp = tempfile::tempdir().expect("temp settings directory");
        let settings = SettingsManager::new(tmp.path().to_path_buf());
        settings
            .update(|value| value.close_to_tray = true)
            .expect("persist baseline settings");
        let original_cache = settings.get();
        let original_disk = std::fs::read_to_string(tmp.path().join("settings.json"))
            .expect("baseline persisted settings");

        let result = settings.update(|value| {
            value.close_to_tray = false;
            value.context.servers.push(ServerProfile {
                id: "server-1".into(),
                alias: "unsafe".into(),
                url: "https://example.test".into(),
                source: ServerSource::Manual,
                favorite: false,
                auth_mode: AuthMode::Required,
                credential_ref: Some("raw-password-value".into()),
                last_seen_at: None,
            });
        });
        assert!(result.is_err());

        let current = settings.get();
        assert_eq!(current.close_to_tray, original_cache.close_to_tray);
        assert_eq!(current.context, original_cache.context);
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("settings.json"))
                .expect("settings remain persisted"),
            original_disk
        );
    }

    #[test]
    fn generic_update_write_failure_leaves_cache_and_disk_unchanged() {
        let tmp = tempfile::tempdir().expect("temp settings directory");
        let blocked_config_dir = tmp.path().join("not-a-directory");
        std::fs::write(&blocked_config_dir, "unchanged").expect("blocking file");
        let settings = SettingsManager::new(blocked_config_dir.clone());
        let original_cache = settings.get();

        let result = settings.update(|value| value.close_to_tray = true);

        assert!(result.is_err());
        assert_eq!(settings.get().close_to_tray, original_cache.close_to_tray);
        assert_eq!(
            std::fs::read_to_string(blocked_config_dir).expect("blocking file remains"),
            "unchanged"
        );
    }
}
