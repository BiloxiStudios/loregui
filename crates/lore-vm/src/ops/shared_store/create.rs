//! `shared_store create` operation — binds `lore::shared_store::create`.
//!
//! Creates a new shared store at the specified path, optionally setting it as
//! the default. Emits `LoreEvent::SharedStoreCreate` on success containing
//! the filesystem path of the newly created store.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::interface::LoreString;
use lore::shared_store::LoreSharedStoreCreateArgs;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Resolve a filesystem path argument against `repo_root` so the upstream
/// engine receives an absolute path. Already-absolute paths pass through
/// unchanged. (`remote_url` is NOT a filesystem path.)
fn resolve_path(p: &str, repo_root: &Path) -> LoreString {
    let path = std::path::Path::new(p);
    if path.is_absolute() {
        LoreString::from_str(p)
    } else {
        LoreString::from_path(repo_root.join(path))
    }
}

/// Arguments for [`create`].
///
/// Mirrors `LoreSharedStoreCreateArgs` from the upstream `lore` crate
/// but uses idiomatic Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedStoreCreateArgs {
    /// Remote URL backing the store.
    pub remote_url: String,
    /// Path where the store will be created; empty or `None` uses the default location.
    #[serde(default)]
    pub path: Option<String>,
    /// Set this as the default shared store in the global config.
    #[serde(default)]
    pub make_default: bool,
}

impl SharedStoreCreateArgs {
    fn into_lore(self, repo_root: &Path) -> LoreSharedStoreCreateArgs {
        LoreSharedStoreCreateArgs {
            remote_url: LoreString::from_str(&self.remote_url),
            path: LoreString::from_str(
                self.path
                    .as_deref()
                    .map(|p| {
                        let resolved = resolve_path(p, repo_root);
                        resolved.as_str().to_string()
                    })
                    .as_deref()
                    .unwrap_or(""),
            ),
            make_default: if self.make_default { 1 } else { 0 },
        }
    }
}

/// Result returned on successful shared store creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedStoreCreateResult {
    /// Filesystem path of the newly created store.
    pub path: String,
}

/// Create a new shared store at the specified path.
///
/// Calls the upstream `lore::shared_store::create` in-process and collects
/// the `SharedStoreCreate` event to return the path of the newly created store.
pub async fn create(api: &LoreApi, args: SharedStoreCreateArgs) -> Result<SharedStoreCreateResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::shared_store::create(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("shared_store create failed with status {status}"),
        )));
    }

    let data = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::SharedStoreCreate(data) = event {
                Some(data.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::Parse(
                "shared_store create succeeded but no SharedStoreCreate event emitted".into(),
            )
        })?;

    Ok(SharedStoreCreateResult {
        path: data.path.as_str().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes_cleanly() {
        let args = SharedStoreCreateArgs {
            remote_url: "https://example.com/repo".into(),
            path: Some("/tmp/store".into()),
            make_default: true,
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains("https://example.com/repo"));
        assert!(json.contains("/tmp/store"));
        assert!(json.contains("true"));
    }

    #[test]
    fn args_defaults_work() {
        let json = r#"{"remote_url":"https://example.com/repo"}"#;
        let args: SharedStoreCreateArgs = serde_json::from_str(json).unwrap();
        assert_eq!(args.remote_url, "https://example.com/repo");
        assert!(args.path.is_none());
        assert!(!args.make_default);
    }

    #[test]
    fn into_lore_maps_fields_correctly() {
        let args = SharedStoreCreateArgs {
            remote_url: "https://example.com/repo".into(),
            path: Some("/tmp/store".into()),
            make_default: true,
        };
        let repo_root = std::path::Path::new("/work/myrepo");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(lore_args.remote_url.as_str(), "https://example.com/repo");
        assert_eq!(lore_args.path.as_str(), "/tmp/store");
        assert_eq!(lore_args.make_default, 1);
    }

    #[test]
    fn into_lore_relative_path_resolved() {
        let args = SharedStoreCreateArgs {
            remote_url: "https://example.com/repo".into(),
            path: Some(".lore/store".into()),
            make_default: false,
        };
        let repo_root = std::path::Path::new("/work/myrepo");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(lore_args.path.as_str(), "/work/myrepo/.lore/store");
    }

    #[test]
    fn into_lore_none_path_maps_to_empty() {
        let args = SharedStoreCreateArgs {
            remote_url: "https://example.com/repo".into(),
            path: None,
            make_default: false,
        };
        let repo_root = std::path::Path::new("/work/myrepo");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(lore_args.path.as_str(), "");
        assert_eq!(lore_args.make_default, 0);
    }

    #[test]
    fn result_serializes() {
        let result = SharedStoreCreateResult {
            path: "/home/user/.lore/shared-store".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("/home/user/.lore/shared-store"));
    }
}
