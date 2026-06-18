//! `file history` operation — binds `lore::file::history`.
//!
//! Retrieves the revision history for a specific file. Emits one
//! `LoreEvent::FileHistory` per revision in which the file was modified.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileHistoryArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`history`].
///
/// Mirrors `LoreFileHistoryArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistoryArgs {
    /// Repository-relative path to the file.
    pub path: String,
    /// Optional revision specifier to start from.
    #[serde(default)]
    pub revision: String,
    /// Restrict history to revisions on this branch.
    #[serde(default)]
    pub branch: String,
    /// Number of revisions to list (0 = default 100).
    #[serde(default)]
    pub length: u32,
    /// Number of revisions to search initially (0 = default 10).
    #[serde(default)]
    pub depth: u32,
}

impl FileHistoryArgs {
    fn into_lore(self) -> LoreFileHistoryArgs {
        LoreFileHistoryArgs {
            path: LoreString::from_str(&self.path),
            revision: LoreString::from_str(&self.revision),
            branch: LoreString::from_str(&self.branch),
            length: self.length,
            depth: self.depth,
        }
    }
}

/// The action applied to a file at a given revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileHistoryAction {
    Keep,
    Add,
    Delete,
    Move,
    Copy,
}

/// A single entry in the file's revision history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistoryEntry {
    /// Path of the file at this revision.
    pub path: String,
    /// Repository identifier.
    pub repository: String,
    /// Revision hash.
    pub revision: String,
    /// Sequential revision number.
    pub revision_number: u64,
    /// Parent revision hashes (up to 2).
    pub parents: Vec<String>,
    /// Content address at this revision.
    pub address: String,
    /// File size in bytes at this revision.
    pub size: u64,
    /// Action applied to the file at this revision.
    pub action: FileHistoryAction,
}

/// Result returned on successful file history query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistoryResult {
    /// History entries, one per revision that modified the file.
    pub entries: Vec<FileHistoryEntry>,
}

/// Retrieve the revision history for a specific file.
///
/// Calls the upstream `lore::file::history` in-process and collects
/// `FileHistory` events into a typed result.
pub async fn history(api: &LoreApi, args: FileHistoryArgs) -> Result<FileHistoryResult> {
    let (callback, rx) = collect_events();

    let status = lore::file::history(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file history failed with status {status}"),
        )));
    }

    let entries: Vec<FileHistoryEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::FileHistory(data) = event {
                let action = match data.action {
                    lore::interface::LoreFileAction::Keep => FileHistoryAction::Keep,
                    lore::interface::LoreFileAction::Add => FileHistoryAction::Add,
                    lore::interface::LoreFileAction::Delete => FileHistoryAction::Delete,
                    lore::interface::LoreFileAction::Move => FileHistoryAction::Move,
                    lore::interface::LoreFileAction::Copy => FileHistoryAction::Copy,
                };
                let parents: Vec<String> = data
                    .parent
                    .iter()
                    .filter(|h| !h.is_zero())
                    .map(|h| format!("{h}"))
                    .collect();
                Some(FileHistoryEntry {
                    path: data.path.as_str().to_string(),
                    repository: format!("{}", data.repository),
                    revision: format!("{}", data.revision),
                    revision_number: data.revision_number,
                    parents,
                    address: format!("{}", data.address),
                    size: data.size,
                    action,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(FileHistoryResult { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_args_serializes() {
        let args = FileHistoryArgs {
            path: "src/main.rs".into(),
            revision: "abc123".into(),
            branch: String::new(),
            length: 50,
            depth: 0,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("abc123"));
        assert!(json.contains("50"));
    }

    #[test]
    fn history_args_deserializes_with_defaults() {
        let json = r#"{"path": "README.md"}"#;
        let args: FileHistoryArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.path, "README.md");
        assert_eq!(args.revision, "");
        assert_eq!(args.branch, "");
        assert_eq!(args.length, 0);
        assert_eq!(args.depth, 0);
    }

    #[test]
    fn history_args_into_lore_conversion() {
        let args = FileHistoryArgs {
            path: "assets/texture.png".into(),
            revision: "rev1".into(),
            branch: "main".into(),
            length: 25,
            depth: 5,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.path.as_str(), "assets/texture.png");
        assert_eq!(lore_args.revision.as_str(), "rev1");
        assert_eq!(lore_args.branch.as_str(), "main");
        assert_eq!(lore_args.length, 25);
        assert_eq!(lore_args.depth, 5);
    }

    #[test]
    fn history_action_serializes() {
        assert_eq!(
            serde_json::to_string(&FileHistoryAction::Keep).unwrap(),
            r#""keep""#
        );
        assert_eq!(
            serde_json::to_string(&FileHistoryAction::Add).unwrap(),
            r#""add""#
        );
        assert_eq!(
            serde_json::to_string(&FileHistoryAction::Delete).unwrap(),
            r#""delete""#
        );
        assert_eq!(
            serde_json::to_string(&FileHistoryAction::Move).unwrap(),
            r#""move""#
        );
        assert_eq!(
            serde_json::to_string(&FileHistoryAction::Copy).unwrap(),
            r#""copy""#
        );
    }

    #[test]
    fn history_entry_serializes() {
        let entry = FileHistoryEntry {
            path: "src/lib.rs".into(),
            repository: "repo-abc".into(),
            revision: "rev-def".into(),
            revision_number: 42,
            parents: vec!["parent1".into()],
            address: "addr-ghi".into(),
            size: 1024,
            action: FileHistoryAction::Add,
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains("rev-def"));
        assert!(json.contains("42"));
        assert!(json.contains("1024"));
        assert!(json.contains(r#""add""#));
    }

    #[test]
    fn history_result_serializes() {
        let result = FileHistoryResult {
            entries: vec![
                FileHistoryEntry {
                    path: "a.txt".into(),
                    repository: "r".into(),
                    revision: "r1".into(),
                    revision_number: 1,
                    parents: vec![],
                    address: "a1".into(),
                    size: 100,
                    action: FileHistoryAction::Add,
                },
                FileHistoryEntry {
                    path: "a.txt".into(),
                    repository: "r".into(),
                    revision: "r2".into(),
                    revision_number: 2,
                    parents: vec!["r1".into()],
                    address: "a2".into(),
                    size: 200,
                    action: FileHistoryAction::Keep,
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("r1"));
        assert!(json.contains("r2"));
    }

    #[test]
    fn history_result_empty() {
        let result = FileHistoryResult { entries: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }
}
