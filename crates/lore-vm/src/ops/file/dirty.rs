//! `file dirty` operation — binds `lore::file::dirty`.
//!
//! Marks one or more files as dirty in the staged state. The action for each
//! path is inferred from the filesystem + current revision:
//!   - File on disk + in revision → Modify
//!   - File on disk + not in revision → Add
//!   - Not on disk + in revision → Delete
//!   - Not on disk + not in revision + staged add → Revert add
//! Respects ignore and view filters.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileDirtyArgs;
use lore::interface::{LoreArray, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`dirty`].
///
/// Mirrors `LoreFileDirtyArgs` from the upstream `lore` crate but uses
/// `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDirtyArgs {
    /// Paths to mark dirty.
    #[serde(default)]
    pub paths: Vec<String>,
}

impl FileDirtyArgs {
    fn into_lore(self) -> LoreFileDirtyArgs {
        let lore_paths: Vec<LoreString> =
            self.paths.iter().map(|p| LoreString::from_str(p)).collect();
        LoreFileDirtyArgs {
            paths: LoreArray::from_vec(lore_paths),
        }
    }
}

/// Result returned on a successful dirty operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDirtyResult {
    /// The paths that were marked dirty.
    pub paths: Vec<String>,
}

/// Mark one or more files as dirty in the staging area.
///
/// Calls the upstream `lore::file::dirty` in-process. The operation infers
/// the appropriate action (add/modify/delete) per path from the filesystem
/// and current revision state. Emits only standard `Complete`/`Error` events.
pub async fn dirty(api: &LoreApi, args: FileDirtyArgs) -> Result<FileDirtyResult> {
    let paths_echo = args.paths.clone();

    let (callback, rx) = collect_events();

    let status = lore::file::dirty(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file dirty failed with status {status}"),
        )));
    }

    Ok(FileDirtyResult { paths: paths_echo })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_args_serializes() {
        let args = FileDirtyArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn dirty_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: FileDirtyArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
    }

    #[test]
    fn dirty_args_into_lore_conversion() {
        let args = FileDirtyArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 2);
    }

    #[test]
    fn dirty_result_serializes() {
        let result = FileDirtyResult {
            paths: vec!["src/lib.rs".into(), "Cargo.toml".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains("Cargo.toml"));
    }

    #[test]
    fn dirty_result_round_trip() {
        let result = FileDirtyResult {
            paths: vec!["assets/texture.png".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: FileDirtyResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.paths, result.paths);
    }
}
