//! `lock file_acquire` operation — binds `lore::lock::file_acquire`.
//!
//! Acquires exclusive locks on one or more files in the repository.
//! Emits `LockFileAcquire` events for each successfully acquired lock,
//! and `LockFileAcquireIgnore` events for each file already owned by the user.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreEvent, LoreString};
use lore::lock::LoreLockFileAcquireArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`file_acquire`].
///
/// Mirrors `LoreLockFileAcquireArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAcquireArgs {
    /// Paths to acquire locks on.
    pub paths: Vec<String>,
    /// Branch the locks are acquired on.
    pub branch: String,
}

impl FileAcquireArgs {
    fn into_lore(self) -> LoreLockFileAcquireArgs {
        let lore_paths: Vec<LoreString> =
            self.paths.into_iter().map(|p| LoreString::from_str(&p)).collect();
        LoreLockFileAcquireArgs {
            paths: LoreArray::from_vec(lore_paths),
            branch: LoreString::from_str(&self.branch),
        }
    }
}

/// Result returned on successful file lock acquisition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAcquireResult {
    /// Paths for which locks were successfully acquired.
    pub acquired: Vec<String>,
    /// Paths that were skipped because locks were already owned.
    pub ignored: Vec<String>,
}

/// Acquires file locks on the specified paths for a given branch.
///
/// Calls the upstream `lore::lock::file_acquire` in-process and collects
/// the `LockFileAcquire` and `LockFileAcquireIgnore` events to return
/// a typed result.
pub async fn file_acquire(
    api: &LoreApi,
    args: FileAcquireArgs,
) -> Result<FileAcquireResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::lock::file_acquire(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file_acquire failed with status {status}"),
        )));
    }

    let mut acquired = Vec::new();
    let mut ignored = Vec::new();

    for event in &stream.events {
        match event {
            LoreEvent::LockFileAcquire(data) => {
                acquired.push(data.path.as_str().to_string());
            }
            LoreEvent::LockFileAcquireIgnore(data) => {
                ignored.push(data.path.as_str().to_string());
            }
            _ => {}
        }
    }

    Ok(FileAcquireResult { acquired, ignored })
}
