//! `lock file_release` operation — binds `lore::lock::file_release`.
//!
//! Releases exclusive locks on one or more files in the repository.
//! Upstream emits one `LockFileReleaseBegin { count, not_found }` report
//! header — `not_found = 1` when no matching lock existed — followed by a
//! `LockFileRelease` event per released path.

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
/// the `LockFileRelease` and `LockFileReleaseBegin` events to return
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

    let (released, not_found) = classify_release_events(&stream.events);

    Ok(FileReleaseResult {
        released,
        not_found,
    })
}

/// Fold a `file_release` event stream into (released paths, any-not-found).
///
/// Since lore v0.8.5 the `LockFileReleaseBegin` header is emitted on EVERY
/// release call and carries the outcome in its `not_found` flag: success is
/// `Begin { count > 0, not_found: 0 }` followed by per-path `LockFileRelease`
/// events; a missing lock is `Begin { count: 0, not_found: 1 }`. Reading the
/// mere presence of `Begin` as "not found" (the 0.8.4 `LockFileReleaseNotFound`
/// semantics) reports every successful release as not-found — keep the flag.
fn classify_release_events(events: &[LoreEvent]) -> (Vec<String>, bool) {
    let mut released = Vec::new();
    let mut not_found = false;

    for event in events {
        match event {
            LoreEvent::LockFileRelease(data) => {
                released.push(data.path.as_str().to_string());
            }
            LoreEvent::LockFileReleaseBegin(data) => {
                not_found |= data.not_found != 0;
            }
            _ => {}
        }
    }

    (released, not_found)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a LoreEvent from its wire shape (`{"tagName":…,"data":…}`), the
    /// same serde contract the engine emits — the data structs are not
    /// re-exported through the umbrella `lore` crate, so construction goes
    /// through deserialization.
    fn event(json: serde_json::Value) -> LoreEvent {
        serde_json::from_value(json).expect("test event must deserialize")
    }

    #[test]
    fn successful_release_reports_released_path_and_not_found_false() {
        let events = vec![
            event(serde_json::json!({
                "tagName": "lockFileReleaseBegin",
                "data": { "count": 1, "notFound": 0 }
            })),
            event(serde_json::json!({
                "tagName": "lockFileRelease",
                "data": { "path": "content/hero.fbx" }
            })),
        ];

        let (released, not_found) = classify_release_events(&events);

        assert_eq!(released, vec!["content/hero.fbx".to_string()]);
        assert!(
            !not_found,
            "a successful release must NOT report not_found (0.8.5 regression)"
        );
    }

    #[test]
    fn missing_lock_reports_not_found_true() {
        let events = vec![event(serde_json::json!({
            "tagName": "lockFileReleaseBegin",
            "data": { "count": 0, "notFound": 1 }
        }))];

        let (released, not_found) = classify_release_events(&events);

        assert!(released.is_empty());
        assert!(not_found, "a missing lock must report not_found");
    }

    #[test]
    fn mixed_stream_still_flags_not_found() {
        // One path released, one missing: any not_found=1 header flags the op.
        let events = vec![
            event(serde_json::json!({
                "tagName": "lockFileReleaseBegin",
                "data": { "count": 1, "notFound": 0 }
            })),
            event(serde_json::json!({
                "tagName": "lockFileRelease",
                "data": { "path": "a.txt" }
            })),
            event(serde_json::json!({
                "tagName": "lockFileReleaseBegin",
                "data": { "count": 0, "notFound": 1 }
            })),
        ];

        let (released, not_found) = classify_release_events(&events);

        assert_eq!(released, vec!["a.txt".to_string()]);
        assert!(not_found);
    }
}
