//! `file stage_merge` operation — binds `lore::file::stage_merge`.
//!
//! Stages one or more files as merge resolutions. This is used during an
//! active merge to mark files as resolved and stage them for the merge commit.
//!
//! Emits `FileStageFile` per file and `FileStageRevision` with the resulting
//! staged-revision identifier — the same event types as `file stage`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileStageMergeArgs;
use lore::interface::{LoreArray, LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`stage_merge`].
///
/// Mirrors `LoreFileStageMergeArgs` from the upstream `lore` crate but uses
/// plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageMergeArgs {
    /// Paths to files to stage as merge resolutions.
    #[serde(default)]
    pub paths: Vec<String>,
}

impl FileStageMergeArgs {
    fn into_lore(self) -> LoreFileStageMergeArgs {
        let lore_paths: Vec<LoreString> =
            self.paths.iter().map(|p| LoreString::from_str(p)).collect();
        LoreFileStageMergeArgs {
            paths: LoreArray::from_vec(lore_paths),
        }
    }
}

/// The action applied to a file when it was staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStageMergeAction {
    Keep,
    Add,
    Delete,
    Move,
    Copy,
}

fn map_action(action: &lore::interface::LoreFileAction) -> FileStageMergeAction {
    match action {
        lore::interface::LoreFileAction::Keep => FileStageMergeAction::Keep,
        lore::interface::LoreFileAction::Add => FileStageMergeAction::Add,
        lore::interface::LoreFileAction::Delete => FileStageMergeAction::Delete,
        lore::interface::LoreFileAction::Move => FileStageMergeAction::Move,
        lore::interface::LoreFileAction::Copy => FileStageMergeAction::Copy,
    }
}

/// One file affected by the stage-merge operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageMergeEntry {
    /// Repository-relative path that was staged.
    pub path: String,
    /// Previous path when the file was moved; empty otherwise.
    pub from_path: String,
    /// Action applied to the file.
    pub action: FileStageMergeAction,
}

/// Result returned on a successful stage-merge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageMergeResult {
    /// One entry per file affected.
    pub files: Vec<FileStageMergeEntry>,
    /// Resulting staged-revision identifier (empty when none was reported).
    pub revision: String,
}

/// Stage one or more files as merge resolutions.
///
/// Calls the upstream `lore::file::stage_merge` in-process and collects
/// `FileStageFile` / `FileStageRevision` events into a typed result.
pub async fn stage_merge(api: &LoreApi, args: FileStageMergeArgs) -> Result<FileStageMergeResult> {
    let (callback, rx) = collect_events();

    let status = lore::file::stage_merge(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file stage_merge failed with status {status}"),
        )));
    }

    let mut files = Vec::new();
    let mut revision = String::new();

    for event in &stream.events {
        match event {
            LoreEvent::FileStageFile(data) => {
                files.push(FileStageMergeEntry {
                    path: data.path.as_str().to_string(),
                    from_path: data.from_path.as_str().to_string(),
                    action: map_action(&data.action),
                });
            }
            LoreEvent::FileStageRevision(data) => {
                revision = format!("{}", data.revision);
            }
            _ => {}
        }
    }

    Ok(FileStageMergeResult { files, revision })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = FileStageMergeArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: FileStageMergeArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
    }

    #[test]
    fn args_deserializes_full() {
        let json = r#"{"paths":["a.txt","b/c.rs"]}"#;
        let args: FileStageMergeArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.paths, vec!["a.txt", "b/c.rs"]);
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = FileStageMergeArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 2);
    }

    #[test]
    fn args_into_lore_empty() {
        let args = FileStageMergeArgs { paths: vec![] };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 0);
    }

    #[test]
    fn action_serde() {
        assert_eq!(
            serde_json::to_string(&FileStageMergeAction::Keep).unwrap(),
            r#""keep""#
        );
        assert_eq!(
            serde_json::to_string(&FileStageMergeAction::Add).unwrap(),
            r#""add""#
        );
        assert_eq!(
            serde_json::to_string(&FileStageMergeAction::Delete).unwrap(),
            r#""delete""#
        );
        assert_eq!(
            serde_json::to_string(&FileStageMergeAction::Move).unwrap(),
            r#""move""#
        );
        assert_eq!(
            serde_json::to_string(&FileStageMergeAction::Copy).unwrap(),
            r#""copy""#
        );
    }

    #[test]
    fn entry_serializes() {
        let entry = FileStageMergeEntry {
            path: "src/lib.rs".into(),
            from_path: String::new(),
            action: FileStageMergeAction::Add,
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains(r#""add""#));
    }

    #[test]
    fn entry_with_move_serializes() {
        let entry = FileStageMergeEntry {
            path: "new/path.rs".into(),
            from_path: "old/path.rs".into(),
            action: FileStageMergeAction::Move,
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("new/path.rs"));
        assert!(json.contains("old/path.rs"));
        assert!(json.contains(r#""move""#));
    }

    #[test]
    fn result_serializes() {
        let result = FileStageMergeResult {
            files: vec![FileStageMergeEntry {
                path: "a.txt".into(),
                from_path: String::new(),
                action: FileStageMergeAction::Add,
            }],
            revision: "abc123".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("a.txt"));
        assert!(json.contains("abc123"));
        assert!(json.contains(r#""add""#));
    }

    #[test]
    fn result_empty() {
        let result = FileStageMergeResult {
            files: vec![],
            revision: String::new(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""files":[]"#));
    }

    #[test]
    fn result_round_trip() {
        let result = FileStageMergeResult {
            files: vec![FileStageMergeEntry {
                path: "test.rs".into(),
                from_path: String::new(),
                action: FileStageMergeAction::Keep,
            }],
            revision: "rev42".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: FileStageMergeResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.files.len(), 1);
        assert_eq!(deserialized.files[0].path, "test.rs");
        assert_eq!(deserialized.files[0].action, FileStageMergeAction::Keep);
        assert_eq!(deserialized.revision, "rev42");
    }
}
