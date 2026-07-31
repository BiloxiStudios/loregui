//! `lock file_query` operation — binds `lore::lock::file_query`.
//!
//! Queries file locks on a branch, optionally filtered by owner and path.
//! Emits `LockFileQueryBegin` followed by `LockFileQuery` events for each
//! lock matching the query.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::lock::LoreLockFileQueryArgs;
use serde::{Deserialize, Serialize};

/// Convert a Unix-millisecond timestamp (as emitted by the lore server) to
/// Unix seconds so downstream consumers (e.g. `new Date(secs * 1000)` in the
/// UI) render the correct date without double-scaling.
fn ms_to_seconds(ms: u64) -> u64 {
    ms / 1000
}

/// Arguments for [`file_query`].
///
/// Mirrors `LoreLockFileQueryArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileQueryArgs {
    /// Branch to query locks on.
    pub branch: String,
    /// Owner filter; empty matches any owner.
    pub owner: String,
    /// Path filter; empty matches any path.
    pub path: String,
}

impl FileQueryArgs {
    fn into_lore(self) -> LoreLockFileQueryArgs {
        LoreLockFileQueryArgs {
            branch: LoreString::from_str(&self.branch),
            owner: LoreString::from_str(&self.owner),
            path: LoreString::from_str(&self.path),
        }
    }
}

/// A single lock entry returned by the query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    /// Branch identifier the lock belongs to.
    pub branch: String,
    /// Path the lock applies to.
    pub path: String,
    /// Owner of the lock (user ID).
    pub owner: String,
    /// Timestamp when the lock was acquired (Unix seconds; converted from
    /// server-emitted milliseconds to avoid double-scaling in date formatting).
    pub locked_at: u64,
}

/// Result returned on successful file lock query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileQueryResult {
    /// Total number of matching locks reported by the server.
    pub count: u64,
    /// Individual lock entries.
    pub locks: Vec<LockEntry>,
}

/// Queries file locks on a branch, optionally filtered by owner and path.
///
/// Calls the upstream `lore::lock::file_query` in-process and collects
/// the `LockFileQuery` events to return a typed result.
pub async fn file_query(api: &LoreApi, args: FileQueryArgs) -> Result<FileQueryResult> {
    let (callback, rx) = collect_events();

    let status = lore::lock::file_query(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file_query failed with status {status}"),
        )));
    }

    let mut count = 0u64;
    let mut locks = Vec::new();

    for event in &stream.events {
        match event {
            LoreEvent::LockFileQueryBegin(data) => {
                count = data.count;
            }
            LoreEvent::LockFileQuery(data) => {
                locks.push(LockEntry {
                    branch: data.branch.to_string(),
                    path: data.path.as_str().to_string(),
                    owner: data.owner.as_str().to_string(),
                    locked_at: ms_to_seconds(data.locked_at),
                });
            }
            _ => {}
        }
    }

    Ok(FileQueryResult { count, locks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_deserialise_minimal() {
        let args: FileQueryArgs =
            serde_json::from_str(
                r#"{"branch":"main","owner":"","path":""}"#,
            )
            .expect("deserialise");
        assert_eq!(args.branch, "main");
        assert_eq!(args.owner, "");
        assert_eq!(args.path, "");
    }

    #[test]
    fn args_deserialise_with_filters() {
        let args: FileQueryArgs = serde_json::from_str(
            r#"{"branch":"dev","owner":"alice","path":"Content/"}"#,
        )
        .expect("deserialise");
        assert_eq!(args.branch, "dev");
        assert_eq!(args.owner, "alice");
        assert_eq!(args.path, "Content/");
    }

    #[test]
    fn result_serialises() {
        // 1718000000000 ms → 1718000000 s (2024-06-10)
        let result = FileQueryResult {
            count: 1,
            locks: vec![LockEntry {
                branch: "main".into(),
                path: "Content/Maps/main.umap".into(),
                owner: "user-42".into(),
                locked_at: 1718000000,
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("1718000000"));
        assert!(!json.contains("1718000000000"));
    }

    #[test]
    fn ms_to_seconds_converts_canonical_values() {
        // Canonical millisecond timestamp from 2024-06-10
        assert_eq!(ms_to_seconds(1718000000000), 1718000000);
        // Zero stays zero
        assert_eq!(ms_to_seconds(0), 0);
        // Truncation toward zero (not rounding)
        assert_eq!(ms_to_seconds(1718000000999), 1718000000);
    }
}
