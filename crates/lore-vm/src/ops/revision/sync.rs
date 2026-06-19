//! `revision sync` operation — binds `lore::revision::sync`.
//!
//! Synchronises the working directory to a target revision, optionally
//! merging divergent branches. Emits per-file change events, progress
//! counters, and the resulting revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreEvent, LoreString};
use lore::revision::LoreRevisionSyncArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`sync`].
///
/// Mirrors `LoreRevisionSyncArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionSyncArgs {
    /// Revision to synchronise to; empty for branch tip.
    #[serde(default)]
    pub revision: String,
    /// Fast-forward and keep local changes when syncing to a local revision.
    #[serde(default)]
    pub forward_changes: bool,
    /// Reset local modified files to match the incoming revision.
    #[serde(default)]
    pub reset: bool,
    /// Root files for dependency-based selective sync.
    #[serde(default)]
    pub root_files: Vec<String>,
    /// Tags to filter dependencies by during resolution.
    #[serde(default)]
    pub dependency_tags: Vec<String>,
    /// Follow transitive dependencies recursively.
    #[serde(default)]
    pub dependency_recursive: bool,
    /// Maximum dependency traversal depth; 0 means unlimited.
    #[serde(default)]
    pub dependency_depth_limit: u32,
}

impl RevisionSyncArgs {
    fn into_lore(self) -> LoreRevisionSyncArgs {
        LoreRevisionSyncArgs {
            revision: LoreString::from_str(&self.revision),
            forward_changes: u8::from(self.forward_changes),
            reset: u8::from(self.reset),
            root_files: LoreArray::from_vec(
                self.root_files
                    .iter()
                    .map(|s| LoreString::from_str(s))
                    .collect(),
            ),
            dependency_tags: LoreArray::from_vec(
                self.dependency_tags
                    .iter()
                    .map(|s| LoreString::from_str(s))
                    .collect(),
            ),
            dependency_recursive: u8::from(self.dependency_recursive),
            dependency_depth_limit: self.dependency_depth_limit,
        }
    }
}

/// A single file changed during a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFileEntry {
    /// Path relative to the repository root.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// Action applied to the file (add, delete, modify, …).
    pub action: String,
    /// Whether this entry is a file (not a directory).
    pub is_file: bool,
}

/// The resulting revision after a sync completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRevisionInfo {
    /// Branch identifier.
    pub branch: String,
    /// Resulting revision hash signature.
    pub revision: String,
    /// Resulting revision number, or 0 if sync produced a merge.
    pub revision_number: u64,
    /// Whether sync resulted in a staged merge revision.
    pub is_merge: bool,
    /// Whether the merge has conflicts.
    pub has_conflicts: bool,
}

/// Result returned on a successful sync.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionSyncResult {
    /// Files changed during sync.
    pub files: Vec<SyncFileEntry>,
    /// The resulting revision(s); typically one, but a merge may produce more.
    pub revisions: Vec<SyncRevisionInfo>,
    /// Total files updated.
    pub files_updated: usize,
    /// Total files deleted.
    pub files_deleted: usize,
}

/// Synchronise the working directory to a target revision.
///
/// Calls the upstream `lore::revision::sync` in-process and collects
/// sync events into a typed result.
pub async fn sync(api: &LoreApi, args: RevisionSyncArgs) -> Result<RevisionSyncResult> {
    let (callback, rx) = collect_events();

    let status = lore::revision::sync(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision sync failed with status {status}"),
        )));
    }

    let mut result = RevisionSyncResult::default();

    for event in &stream.events {
        match event {
            LoreEvent::RevisionSyncFile(data) => {
                result.files.push(SyncFileEntry {
                    path: data.path.as_str().to_string(),
                    size: data.size,
                    action: format!("{:?}", data.action),
                    is_file: data.flag_file != 0,
                });
            }
            LoreEvent::RevisionSyncProgress(data) => {
                result.files_updated = data.file_update;
                result.files_deleted = data.file_delete;
            }
            LoreEvent::RevisionSyncRevision(data) => {
                result.revisions.push(SyncRevisionInfo {
                    branch: format!("{}", data.branch),
                    revision: format!("{}", data.revision),
                    revision_number: data.revision_number,
                    is_merge: data.flag_merge != 0,
                    has_conflicts: data.flag_conflict != 0,
                });
            }
            _ => {}
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_args_defaults() {
        let json = r#"{}"#;
        let args: RevisionSyncArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.revision, "");
        assert!(!args.forward_changes);
        assert!(!args.reset);
        assert!(args.root_files.is_empty());
        assert!(args.dependency_tags.is_empty());
        assert!(!args.dependency_recursive);
        assert_eq!(args.dependency_depth_limit, 0);
    }

    #[test]
    fn sync_args_into_lore_conversion() {
        let args = RevisionSyncArgs {
            revision: "abc123".into(),
            forward_changes: true,
            reset: false,
            root_files: vec!["a.txt".into(), "b.txt".into()],
            dependency_tags: vec!["tag1".into()],
            dependency_recursive: true,
            dependency_depth_limit: 5,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision.as_str(), "abc123");
        assert_eq!(lore_args.forward_changes, 1);
        assert_eq!(lore_args.reset, 0);
        assert_eq!(lore_args.root_files.as_slice().len(), 2);
        assert_eq!(lore_args.dependency_tags.as_slice().len(), 1);
        assert_eq!(lore_args.dependency_recursive, 1);
        assert_eq!(lore_args.dependency_depth_limit, 5);
    }

    #[test]
    fn sync_result_serializes() {
        let result = RevisionSyncResult {
            files: vec![SyncFileEntry {
                path: "src/main.rs".into(),
                size: 1024,
                action: "Add".into(),
                is_file: true,
            }],
            revisions: vec![SyncRevisionInfo {
                branch: "main".into(),
                revision: "abc123".into(),
                revision_number: 42,
                is_merge: false,
                has_conflicts: false,
            }],
            files_updated: 1,
            files_deleted: 0,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("abc123"));
        assert!(json.contains("main"));
    }

    #[test]
    fn sync_args_round_trips_through_json() {
        let args = RevisionSyncArgs {
            revision: "rev1".into(),
            forward_changes: true,
            reset: true,
            root_files: vec!["file1.txt".into()],
            dependency_tags: vec!["tag1".into(), "tag2".into()],
            dependency_recursive: false,
            dependency_depth_limit: 10,
        };
        let json = serde_json::to_string(&args).expect("serialise");
        let back: RevisionSyncArgs = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.revision, args.revision);
        assert_eq!(back.forward_changes, args.forward_changes);
        assert_eq!(back.reset, args.reset);
        assert_eq!(back.root_files, args.root_files);
        assert_eq!(back.dependency_tags, args.dependency_tags);
    }

    #[test]
    fn sync_result_defaults() {
        let result = RevisionSyncResult::default();
        assert!(result.files.is_empty());
        assert!(result.revisions.is_empty());
        assert_eq!(result.files_updated, 0);
        assert_eq!(result.files_deleted, 0);
    }
}
