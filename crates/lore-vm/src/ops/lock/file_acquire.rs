//! `lock file_acquire` operation — binds `lore::lock::file_acquire`.
//!
//! Acquires exclusive locks on one or more files in the repository.
//! Emits a `LockFileAcquireBegin` event with `count` and `ignored` flag,
//! then `count` `LockFileAcquire` events for the affected paths.
//! If `ignored != 0`, those paths were already owned (SBAI-5434: API change).

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
        let lore_paths: Vec<LoreString> = self
            .paths
            .into_iter()
            .map(|p| LoreString::from_str(&p))
            .collect();
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
/// the `LockFileAcquire` and `LockFileAcquireBegin` events to return
/// a typed result.
pub async fn file_acquire(api: &LoreApi, args: FileAcquireArgs) -> Result<FileAcquireResult> {
    let (callback, rx) = collect_events();

    let status = lore::lock::file_acquire(api.globals().build(), args.into_lore(), callback).await;

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

    // v0.8.5: LockFileAcquireBegin fires with `count` and `ignored` flag,
    // then `count` LockFileAcquire events follow. If ignored != 0, those
    // paths were already owned by the user.
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

    Ok(FileAcquireResult { acquired, ignored })
}
