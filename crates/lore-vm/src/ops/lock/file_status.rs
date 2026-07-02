//! `lock file_status` operation — binds `lore::lock::file_status`.
//!
//! Returns the lock status of the specified files on a given branch.
//! Emits `LockFileStatusBegin` event followed by `LockFileStatus` events
//! for each locked file with owner and timestamp details.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::lock::LoreLockFileStatusArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`file_status`].
///
/// Mirrors `LoreLockFileStatusArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatusArgs {
    /// Paths to get the lock status of.
    pub paths: Vec<String>,
    /// Branch the locks were acquired on.
    pub branch: String,
}

impl FileStatusArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreLockFileStatusArgs {
        LoreLockFileStatusArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
            branch: LoreString::from_str(&self.branch),
        }
    }
}

/// Lock status information for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockStatus {
    /// Path of the locked file.
    pub path: String,
    /// Owner of the lock (user ID).
    pub owner: String,
    /// Timestamp when the lock was acquired (Unix timestamp in milliseconds).
    pub locked_at: u64,
}

/// Result returned on successful file status query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatusResult {
    /// List of locked files with their status information.
    pub locks: Vec<LockStatus>,
}

/// Returns the lock status of the specified files on a given branch.
///
/// Calls the upstream `lore::lock::file_status` in-process and collects
/// the `LockFileStatus` events to return a typed result.
pub async fn file_status(api: &LoreApi, args: FileStatusArgs) -> Result<FileStatusResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::lock::file_status(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file_status failed with status {status}"),
        )));
    }

    let mut locks = Vec::new();

    for event in &stream.events {
        if let LoreEvent::LockFileStatus(data) = event {
            locks.push(LockStatus {
                path: data.path.as_str().to_string(),
                owner: data.owner.as_str().to_string(),
                locked_at: data.locked_at,
            });
        }
    }

    Ok(FileStatusResult { locks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_deserialise_minimal() {
        let args: FileStatusArgs =
            serde_json::from_str(r#"{"paths":["a.txt"],"branch":"main"}"#).expect("deserialise");
        assert_eq!(args.paths, vec!["a.txt"]);
        assert_eq!(args.branch, "main");
    }

    #[test]
    fn args_deserialise_multiple_paths() {
        let args: FileStatusArgs =
            serde_json::from_str(r#"{"paths":["a.txt","b/c.png","d.uasset"],"branch":"dev"}"#)
                .expect("deserialise");
        assert_eq!(args.paths.len(), 3);
        assert_eq!(args.branch, "dev");
    }

    #[test]
    fn args_into_lore_maps_fields() {
        let args = FileStatusArgs {
            paths: vec!["foo.txt".into(), "bar.png".into()],
            branch: "release".into(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.branch.as_str(), "release");
        assert_eq!(lore_args.paths.len(), 2);
    }

    #[test]
    fn result_serialises() {
        let result = FileStatusResult {
            locks: vec![LockStatus {
                path: "models/hero.fbx".into(),
                owner: "user-42".into(),
                locked_at: 1700000000000,
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("models/hero.fbx"));
        assert!(json.contains("user-42"));
        assert!(json.contains("1700000000000"));
    }

    #[test]
    fn result_empty_locks_serialises() {
        let result = FileStatusResult { locks: vec![] };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("[]"));
    }

    #[test]
    fn result_round_trips_through_json() {
        let result = FileStatusResult {
            locks: vec![
                LockStatus {
                    path: "a.txt".into(),
                    owner: "alice".into(),
                    locked_at: 100,
                },
                LockStatus {
                    path: "b.png".into(),
                    owner: "bob".into(),
                    locked_at: 200,
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        let back: FileStatusResult = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.locks.len(), 2);
        assert_eq!(back.locks[0].path, "a.txt");
        assert_eq!(back.locks[1].owner, "bob");
    }
}
