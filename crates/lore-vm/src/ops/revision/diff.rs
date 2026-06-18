//! `revision diff` operation — binds `lore::revision::diff`.
//!
//! Computes file-level differences between two revisions.
//! Emits `LoreEvent::RevisionDiffFile` for each changed file.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionDiffArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`diff`].
///
/// Mirrors `LoreRevisionDiffArgs` from the upstream `lore` crate
/// but uses plain `String` / `Vec<String>` so it serialises cleanly
/// across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionDiffArgs {
    /// Source revision to diff from.
    pub revision_source: String,
    /// Target revision to diff to; empty string means current working state.
    #[serde(default)]
    pub revision_target: String,
    /// Repository-relative paths to restrict the diff to; empty means all files.
    #[serde(default)]
    pub paths: Vec<String>,
}

impl RevisionDiffArgs {
    fn into_lore(self) -> LoreRevisionDiffArgs {
        LoreRevisionDiffArgs {
            revision_source: LoreString::from_str(&self.revision_source),
            revision_target: LoreString::from_str(&self.revision_target),
            paths: lore::interface::LoreArray::from_vec(
                self.paths.iter().map(|p| LoreString::from_str(p)).collect(),
            ),
        }
    }
}

/// The action applied to a file in the diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffFileAction {
    Keep,
    Add,
    Delete,
    Move,
    Copy,
}

/// A single file entry in the revision diff result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionDiffFile {
    /// Path of the file relative to the repository root.
    pub path: String,
    /// The action applied to this file.
    pub action: DiffFileAction,
    /// Short action label (A/D/V/C/M).
    pub action_short: String,
    /// Whether the source side is a file (vs directory).
    pub old_is_file: bool,
    /// Whether the target side is a file (vs directory).
    pub new_is_file: bool,
    /// Content address on the source side.
    pub old_address: String,
    /// Content address on the target side.
    pub new_address: String,
}

/// Result returned on successful revision diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionDiffResult {
    /// All files that differ between the two revisions.
    pub files: Vec<RevisionDiffFile>,
}

/// Compute file-level differences between two revisions.
///
/// Calls the upstream `lore::revision::diff` in-process and collects
/// `RevisionDiffFile` events into a typed result.
pub async fn diff(api: &LoreApi, args: RevisionDiffArgs) -> Result<RevisionDiffResult> {
    let (callback, rx) = collect_events();

    let status = lore::revision::diff(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision diff failed with status {status}"),
        )));
    }

    let files: Vec<RevisionDiffFile> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::RevisionDiffFile(data) = event {
                let action = match data.action {
                    lore::interface::LoreFileAction::Keep => DiffFileAction::Keep,
                    lore::interface::LoreFileAction::Add => DiffFileAction::Add,
                    lore::interface::LoreFileAction::Delete => DiffFileAction::Delete,
                    lore::interface::LoreFileAction::Move => DiffFileAction::Move,
                    lore::interface::LoreFileAction::Copy => DiffFileAction::Copy,
                };
                Some(RevisionDiffFile {
                    path: data.path.as_str().to_string(),
                    action,
                    action_short: data.action_as_string_short().to_string(),
                    old_is_file: data.old_is_file != 0,
                    new_is_file: data.new_is_file != 0,
                    old_address: format!("{}", data.old_address),
                    new_address: format!("{}", data.new_address),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(RevisionDiffResult { files })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_args_serializes() {
        let args = RevisionDiffArgs {
            revision_source: "abc123".into(),
            revision_target: "def456".into(),
            paths: vec!["src/main.rs".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("abc123"));
        assert!(json.contains("def456"));
        assert!(json.contains("src/main.rs"));
    }

    #[test]
    fn diff_args_deserializes_with_defaults() {
        let json = r#"{"revision_source": "abc123"}"#;
        let args: RevisionDiffArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.revision_source, "abc123");
        assert_eq!(args.revision_target, "");
        assert!(args.paths.is_empty());
    }

    #[test]
    fn diff_args_into_lore_conversion() {
        let args = RevisionDiffArgs {
            revision_source: "rev1".into(),
            revision_target: "rev2".into(),
            paths: vec!["a.txt".into(), "b.txt".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision_source.as_str(), "rev1");
        assert_eq!(lore_args.revision_target.as_str(), "rev2");
        assert_eq!(lore_args.paths.len(), 2);
    }

    #[test]
    fn diff_file_action_serializes() {
        let action = DiffFileAction::Add;
        let json = serde_json::to_string(&action).expect("should serialize");
        assert_eq!(json, r#""add""#);
    }

    #[test]
    fn diff_file_action_all_variants() {
        assert_eq!(
            serde_json::to_string(&DiffFileAction::Keep).unwrap(),
            r#""keep""#
        );
        assert_eq!(
            serde_json::to_string(&DiffFileAction::Delete).unwrap(),
            r#""delete""#
        );
        assert_eq!(
            serde_json::to_string(&DiffFileAction::Move).unwrap(),
            r#""move""#
        );
        assert_eq!(
            serde_json::to_string(&DiffFileAction::Copy).unwrap(),
            r#""copy""#
        );
    }

    #[test]
    fn diff_result_serializes() {
        let result = RevisionDiffResult {
            files: vec![
                RevisionDiffFile {
                    path: "src/main.rs".into(),
                    action: DiffFileAction::Add,
                    action_short: "A".into(),
                    old_is_file: false,
                    new_is_file: true,
                    old_address: "0-0".into(),
                    new_address: "abc-def".into(),
                },
                RevisionDiffFile {
                    path: "README.md".into(),
                    action: DiffFileAction::Delete,
                    action_short: "D".into(),
                    old_is_file: true,
                    new_is_file: false,
                    old_address: "ghi-jkl".into(),
                    new_address: "0-0".into(),
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
        assert!(json.contains(r#""add""#));
        assert!(json.contains(r#""delete""#));
    }

    #[test]
    fn diff_result_empty() {
        let result = RevisionDiffResult { files: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }
}
