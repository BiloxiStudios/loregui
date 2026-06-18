//! `file diff` operation — binds `lore::file::diff`.
//!
//! Computes the unified diff of files between two revisions.
//! Emits one `LoreEvent::FileDiff` per changed file containing the
//! path, unified-diff patch text, and the action (add/delete/move/copy/keep).

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileDiffArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`diff`].
///
/// Mirrors `LoreFileDiffArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffArgs {
    /// File paths to diff; empty diffs all changed files.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Source revision (empty = working tree).
    #[serde(default)]
    pub source_revision: String,
    /// Target revision (empty = working tree).
    #[serde(default)]
    pub target_revision: String,
    /// Produce three-way merge output with conflict markers.
    #[serde(default)]
    pub diff3: bool,
    /// Number of unchanged context lines per unified-diff hunk.
    #[serde(default = "default_context_lines")]
    pub context_lines: u32,
    /// Treat lines that differ only in trailing whitespace as equal.
    #[serde(default)]
    pub ignore_whitespace_eol: bool,
    /// Collapse runs of internal whitespace to a single space for comparison.
    #[serde(default)]
    pub ignore_whitespace_inline: bool,
}

fn default_context_lines() -> u32 {
    3
}

impl DiffArgs {
    fn into_lore(self) -> LoreFileDiffArgs {
        LoreFileDiffArgs {
            paths: lore::interface::LoreArray::from_vec(
                self.paths
                    .into_iter()
                    .map(|p| LoreString::from_str(&p))
                    .collect(),
            ),
            source_revision: LoreString::from_str(&self.source_revision),
            target_revision: LoreString::from_str(&self.target_revision),
            diff3: u8::from(self.diff3),
            context_lines: self.context_lines,
            ignore_whitespace_eol: u8::from(self.ignore_whitespace_eol),
            ignore_whitespace_inline: u8::from(self.ignore_whitespace_inline),
        }
    }
}

/// The action applied to a diffed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileAction {
    Keep,
    Add,
    Delete,
    Move,
    Copy,
}

/// One entry in the diff result — a single file's patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiffEntry {
    /// Path of the file.
    pub path: String,
    /// Unified-diff patch text describing the change.
    pub patch: String,
    /// Action applied to the file (add, delete, move, copy, keep).
    pub action: FileAction,
}

/// Compute the diff of files between two revisions.
///
/// Calls the upstream `lore::file::diff` in-process and collects
/// all `FileDiff` events into a typed result vector.
pub async fn diff(api: &LoreApi, args: DiffArgs) -> Result<Vec<FileDiffEntry>> {
    let (callback, rx) = collect_events();

    let status = lore::file::diff(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file diff failed with status {status}"),
        )));
    }

    let entries: Vec<FileDiffEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::FileDiff(data) = event {
                let action = match data.action {
                    lore::interface::LoreFileAction::Keep => FileAction::Keep,
                    lore::interface::LoreFileAction::Add => FileAction::Add,
                    lore::interface::LoreFileAction::Delete => FileAction::Delete,
                    lore::interface::LoreFileAction::Move => FileAction::Move,
                    lore::interface::LoreFileAction::Copy => FileAction::Copy,
                };
                Some(FileDiffEntry {
                    path: data.path.as_str().to_string(),
                    patch: data.patch.as_str().to_string(),
                    action,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_args_serializes() {
        let args = DiffArgs {
            paths: vec!["src/main.rs".into()],
            source_revision: "abc123".into(),
            target_revision: "def456".into(),
            diff3: false,
            context_lines: 3,
            ignore_whitespace_eol: false,
            ignore_whitespace_inline: false,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("abc123"));
        assert!(json.contains("def456"));
    }

    #[test]
    fn diff_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: DiffArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
        assert_eq!(args.source_revision, "");
        assert_eq!(args.target_revision, "");
        assert!(!args.diff3);
        assert_eq!(args.context_lines, 3);
        assert!(!args.ignore_whitespace_eol);
        assert!(!args.ignore_whitespace_inline);
    }

    #[test]
    fn diff_args_into_lore_conversion() {
        let args = DiffArgs {
            paths: vec!["file1.txt".into(), "file2.txt".into()],
            source_revision: "rev1".into(),
            target_revision: "rev2".into(),
            diff3: true,
            context_lines: 5,
            ignore_whitespace_eol: true,
            ignore_whitespace_inline: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.source_revision.as_str(), "rev1");
        assert_eq!(lore_args.target_revision.as_str(), "rev2");
        assert_eq!(lore_args.diff3, 1);
        assert_eq!(lore_args.context_lines, 5);
        assert_eq!(lore_args.ignore_whitespace_eol, 1);
        assert_eq!(lore_args.ignore_whitespace_inline, 1);
    }

    #[test]
    fn diff_args_into_lore_paths() {
        let args = DiffArgs {
            paths: vec!["a.rs".into(), "b.rs".into()],
            source_revision: String::new(),
            target_revision: String::new(),
            diff3: false,
            context_lines: 3,
            ignore_whitespace_eol: false,
            ignore_whitespace_inline: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 2);
    }

    #[test]
    fn file_diff_entry_serializes() {
        let entry = FileDiffEntry {
            path: "src/lib.rs".into(),
            patch: "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,4 @@\n+use foo;\n".into(),
            action: FileAction::Add,
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains(r#""action":"add""#));
    }

    #[test]
    fn file_action_serializes_all_variants() {
        assert_eq!(
            serde_json::to_string(&FileAction::Keep).unwrap(),
            r#""keep""#
        );
        assert_eq!(serde_json::to_string(&FileAction::Add).unwrap(), r#""add""#);
        assert_eq!(
            serde_json::to_string(&FileAction::Delete).unwrap(),
            r#""delete""#
        );
        assert_eq!(
            serde_json::to_string(&FileAction::Move).unwrap(),
            r#""move""#
        );
        assert_eq!(
            serde_json::to_string(&FileAction::Copy).unwrap(),
            r#""copy""#
        );
    }

    #[test]
    fn file_action_roundtrips() {
        for action in [
            FileAction::Keep,
            FileAction::Add,
            FileAction::Delete,
            FileAction::Move,
            FileAction::Copy,
        ] {
            let json = serde_json::to_string(&action).unwrap();
            let parsed: FileAction = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, action);
        }
    }
}
