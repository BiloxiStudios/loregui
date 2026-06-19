//! `lock file_release` operation — binds `lore::lock::file_release`.
//!
//! Releases exclusive locks on one or more files in the repository.
//! Emits `LockFileRelease` events for each successfully released lock,
//! and `LockFileReleaseNotFound` events for files whose locks were not found.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreEvent, LoreString};
use lore::lock::LoreLockFileReleaseArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`file_release`].
///
/// Mirrors `LoreLockFileReleaseArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReleaseArgs {
    /// Paths to release locks on.
    pub paths: Vec<String>,
    /// Branch the locks were acquired on.
    pub branch: String,
    /// Owner of the lock.
    pub owner: String,
    /// Owner id of the lock.
    pub owner_id: String,
}

impl FileReleaseArgs {
    fn into_lore(self) -> LoreLockFileReleaseArgs {
        let lore_paths: Vec<LoreString> = self
            .paths
            .into_iter()
            .map(|p| LoreString::from_str(&p))
            .collect();
        LoreLockFileReleaseArgs {
            paths: LoreArray::from_vec(lore_paths),
            branch: LoreString::from_str(&self.branch),
            owner: LoreString::from_str(&self.owner),
            owner_id: LoreString::from_str(&self.owner_id),
        }
    }
}

/// Result returned on successful file lock release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReleaseResult {
    /// Paths for which locks were successfully released.
    pub released: Vec<String>,
    /// Whether any requested locks were not found.
    pub not_found: bool,
}

/// Releases file locks on the specified paths for a given branch and owner.
///
/// Calls the upstream `lore::lock::file_release` in-process and collects
/// the `LockFileRelease` and `LockFileReleaseNotFound` events to return
/// a typed result.
pub async fn file_release(api: &LoreApi, args: FileReleaseArgs) -> Result<FileReleaseResult> {
    let (callback, rx) = collect_events();

    let status = lore::lock::file_release(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file_release failed with status {status}"),
        )));
    }

    let mut released = Vec::new();
    let mut not_found = false;

    for event in &stream.events {
        match event {
            LoreEvent::LockFileRelease(data) => {
                released.push(data.path.as_str().to_string());
            }
            LoreEvent::LockFileReleaseNotFound(_) => {
                not_found = true;
            }
            _ => {}
        }
    }

    Ok(FileReleaseResult {
        released,
        not_found,
    })
}
