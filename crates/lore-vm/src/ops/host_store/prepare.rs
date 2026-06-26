//! `host_store::prepare` — create a local host-store directory (idempotent).
//!
//! This is a filesystem-native op (no lore crate binding) used by the
//! "Host a server" wizard to prepare a plain directory for the local-FS
//! backend path. It creates the store directory and optionally a separate
//! mutable-store directory, both via `create_dir_all` (idempotent).

use serde::{Deserialize, Serialize};

use crate::error::{LoreError, Result};

/// Arguments for [`prepare`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrepareArgs {
    /// Directory that will back the immutable + mutable stores.
    /// The user-selected path from the wizard's "Choose Storage Backend" step.
    pub store_dir: String,
    /// Optional separate mutable-store directory. When `None` or empty, only
    /// the main `store_dir` is created (the default local-FS host topology
    /// keeps both stores under the same root).
    #[serde(default)]
    pub mutable_store: Option<String>,
}

impl PrepareArgs {
    fn store_dir_trimmed(&self) -> &str {
        self.store_dir.trim()
    }

    fn mutable_store_trimmed(&self) -> Option<&str> {
        self.mutable_store
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

/// Result returned on successful prepare.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareResult {
    /// Absolute (or user-supplied) path of the prepared store directory.
    pub store_dir: String,
    /// Whether a separate mutable-store directory was also created.
    pub mutable_store_created: bool,
}

/// Create the local host-store directory (idempotent).
///
/// Calls `std::fs::create_dir_all` on `store_dir` and, if provided,
/// `mutable_store`. Returns the resolved store path.
///
/// This is the **local-FS** path: unlike `storage::open` (which requires an
/// existing `.lore` repository) or `shared_store::create` (which requires a
/// remote URL), this operates on a plain directory that the standalone
/// `loreserver` will later populate with its `immutable/` + `mutable/` layout.
pub fn prepare(args: PrepareArgs) -> Result<PrepareResult> {
    let store_dir = args.store_dir_trimmed();
    if store_dir.is_empty() {
        return Err(LoreError::CommandFailed(
            "a local storage path is required".into(),
        ));
    }

    let store_path = std::path::PathBuf::from(store_dir);
    std::fs::create_dir_all(&store_path).map_err(|e| {
        LoreError::CommandFailed(format!(
            "could not create local store directory {}: {e}",
            store_path.display()
        ))
    })?;

    let mutable_created = if let Some(mut_dir) = args.mutable_store_trimmed() {
        let mut_path = std::path::PathBuf::from(mut_dir);
        std::fs::create_dir_all(&mut_path).map_err(|e| {
            LoreError::CommandFailed(format!(
                "could not create mutable store directory {}: {e}",
                mut_path.display()
            ))
        })?;
        true
    } else {
        false
    };

    Ok(PrepareResult {
        store_dir: store_path.to_string_lossy().into_owned(),
        mutable_store_created: mutable_created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_deserialise_defaults() {
        let args: PrepareArgs =
            serde_json::from_str(r#"{"store_dir":"/tmp/test-store"}"#).expect("deserialise");
        assert_eq!(args.store_dir, "/tmp/test-store");
        assert!(args.mutable_store.is_none());
    }

    #[test]
    fn args_with_mutable_store() {
        let args: PrepareArgs = serde_json::from_str(
            r#"{"store_dir":"/tmp/store","mutable_store":"/tmp/mutable"}"#,
        )
        .expect("deserialise");
        assert_eq!(args.store_dir, "/tmp/store");
        assert_eq!(args.mutable_store, Some("/tmp/mutable".into()));
    }

    #[test]
    fn args_trim_empty_strings() {
        let args = PrepareArgs {
            store_dir: "  /tmp/store  ".into(),
            mutable_store: Some("   ".into()),
        };
        assert_eq!(args.store_dir_trimmed(), "/tmp/store");
        assert!(args.mutable_store_trimmed().is_none());
    }

    #[test]
    fn prepare_rejects_empty_path() {
        let err = prepare(PrepareArgs {
            store_dir: String::new(),
            mutable_store: None,
        })
        .unwrap_err();
        match err {
            LoreError::CommandFailed(msg) => {
                assert!(msg.contains("required"), "got: {msg}");
            }
            other => panic!("expected CommandFailed, got: {other:?}"),
        }
    }

    #[test]
    fn prepare_creates_directory() {
        let temp = std::env::temp_dir();
        let store = temp.join(format!("lore-vm-prepare-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&store); // clean from prior runs

        let result = prepare(PrepareArgs {
            store_dir: store.to_string_lossy().into_owned(),
            mutable_store: None,
        })
        .expect("prepare should succeed");

        assert!(store.is_dir(), "store directory should exist");
        assert!(!result.mutable_store_created);

        let _ = std::fs::remove_dir_all(&store);
    }

    #[test]
    fn prepare_idempotent() {
        let temp = std::env::temp_dir();
        let store = temp.join(format!("lore-vm-prepare-idem-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&store);

        // First call creates it.
        prepare(PrepareArgs {
            store_dir: store.to_string_lossy().into_owned(),
            mutable_store: None,
        })
        .expect("first prepare should succeed");

        // Second call is idempotent — should NOT error.
        prepare(PrepareArgs {
            store_dir: store.to_string_lossy().into_owned(),
            mutable_store: None,
        })
        .expect("second prepare should succeed (idempotent)");

        let _ = std::fs::remove_dir_all(&store);
    }

    #[test]
    fn prepare_creates_both_directories() {
        let temp = std::env::temp_dir();
        let store = temp.join(format!("lore-vm-prepare-both-s-{}", std::process::id()));
        let mutable = temp.join(format!("lore-vm-prepare-both-m-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&store);
        let _ = std::fs::remove_dir_all(&mutable);

        let result = prepare(PrepareArgs {
            store_dir: store.to_string_lossy().into_owned(),
            mutable_store: Some(mutable.to_string_lossy().into_owned()),
        })
        .expect("prepare should succeed");

        assert!(store.is_dir());
        assert!(mutable.is_dir());
        assert!(result.mutable_store_created);

        let _ = std::fs::remove_dir_all(&store);
        let _ = std::fs::remove_dir_all(&mutable);
    }

    #[test]
    fn result_serialises() {
        let result = PrepareResult {
            store_dir: "/tmp/store".into(),
            mutable_store_created: true,
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("/tmp/store"));
        assert!(json.contains("true"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"store_dir":"/tmp/x","mutable_store_created":false}"#;
        let result: PrepareResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.store_dir, "/tmp/x");
        assert!(!result.mutable_store_created);
    }
}
