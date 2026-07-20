//! `lock file_acquire_as_owner` operation — binds `lore::lock::file_acquire_as_owner`.
//!
//! Acquires exclusive locks on one or more files on behalf of a specified owner.
//! Same as `file_acquire` but allows specifying who the lock is being acquired for.
//! Emits a `LockFileAcquireBegin` event with `count` and `ignored` flag,
//! then `count` `LockFileAcquire` events for the affected paths.
//! If `ignored != 0`, those paths were already owned (SBAI-5434: API change).

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreEvent, LoreString};
use lore::lock::LoreLockFileAcquireArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`file_acquire_as_owner`].
///
/// Mirrors `LoreLockFileAcquireArgs` from the upstream `lore` crate
/// plus an `owner` field identifying who the lock is acquired for.
/// Uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAcquireAsOwnerArgs {
    /// Paths to acquire locks on.
    pub paths: Vec<String>,
    /// Branch the locks are acquired on.
    pub branch: String,
    /// Owner on whose behalf the locks are being acquired.
    pub owner: String,
}

impl FileAcquireAsOwnerArgs {
    fn into_lore(self) -> (LoreLockFileAcquireArgs, LoreString) {
        let lore_paths: Vec<LoreString> = self
            .paths
            .into_iter()
            .map(|p| LoreString::from_str(&p))
            .collect();
        let args = LoreLockFileAcquireArgs {
            paths: LoreArray::from_vec(lore_paths),
            branch: LoreString::from_str(&self.branch),
        };
        let owner = LoreString::from_str(&self.owner);
        (args, owner)
    }
}

/// Result returned on successful file lock acquisition as owner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAcquireAsOwnerResult {
    /// Paths for which locks were successfully acquired.
    pub acquired: Vec<String>,
    /// Paths that were skipped because locks were already owned.
    pub ignored: Vec<String>,
}

/// Acquires file locks on the specified paths for a given branch on behalf of an owner.
///
/// Calls the upstream `lore::lock::file_acquire_as_owner` in-process and collects
/// the `LockFileAcquire` and `LockFileAcquireBegin` events to return
/// a typed result.
pub async fn file_acquire_as_owner(
    api: &LoreApi,
    args: FileAcquireAsOwnerArgs,
) -> Result<FileAcquireAsOwnerResult> {
    let (callback, rx) = collect_events();

    let (lore_args, owner) = args.into_lore();
    let status =
        lore::lock::file_acquire_as_owner(api.globals().build(), lore_args, callback, owner).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file_acquire_as_owner failed with status {status}"),
        )));
    }

    let mut acquired = Vec::new();
    let mut ignored = Vec::new();

    // v0.8.5: LockFileAcquireBegin fires with `count` and `ignored` flag,
    // then `count` LockFileAcquire events follow. If ignored != 0, those
    // paths were already owned by the specified owner.
    let mut pending_ignored = false;

    for event in &stream.events {
        match event {
            LoreEvent::LockFileAcquireBegin(data) => {
                pending_ignored = data.ignored != 0;
            }
            LoreEvent::LockFileAcquire(data) => {
                let path = data.path.as_str().to_string();
                if pending_ignored {
                    ignored.push(path);
                } else {
                    acquired.push(path);
                }
            }
            _ => {}
        }
    }

    Ok(FileAcquireAsOwnerResult { acquired, ignored })
}
