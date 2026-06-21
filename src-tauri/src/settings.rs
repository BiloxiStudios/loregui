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
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => AppSettings::default(),
        }
    }

    /// Get the current settings.
    pub fn get(&self) -> AppSettings {
        self.cache.lock().unwrap().clone()
    }

    /// Save updated settings to disk.
    pub fn update(&self, f: impl FnOnce(&mut AppSettings)) {
        let mut settings = self.cache.lock().unwrap();
        f(&mut settings);
        if let Ok(json) = serde_json::to_string_pretty(&*settings) {
            if let Some(parent) = self.settings_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&self.settings_path, json);
        }
    }
}
