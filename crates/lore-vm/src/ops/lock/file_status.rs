//! `lock file_status` operation — binds `lore::lock::file_status`.
//!
//! Returns the lock status of the specified files on a given branch.
//! Emits `LockFileStatusBegin` event followed by `LockFileStatus` events
//! for each locked file with owner and timestamp details.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreEvent, LoreString};
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
    fn into_lore(self) -> LoreLockFileStatusArgs {
        let lore_paths: Vec<LoreString> = self
            .paths
            .into_iter()
            .map(|p| LoreString::from_str(&p))
            .collect();
        LoreLockFileStatusArgs {
            paths: LoreArray::from_vec(lore_paths),
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

    let status =
        lore::lock::file_status(api.globals().build(), args.into_lore(), callback).await;

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
        match event {
            LoreEvent::LockFileStatus(data) => {
                locks.push(LockStatus {
                    path: data.path.as_str().to_string(),
                    owner: data.owner.as_str().to_string(),
                    locked_at: data.locked_at,
                });
            }
            _ => {}
        }
    }

    Ok(FileStatusResult { locks })
}
