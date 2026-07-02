//! `file stage_move` operation — binds `lore::file::stage_move`.
//!
//! Stages a file move operation in the working directory. This moves a file
//! from one path to another in the staging area, flagging it as a move for
//! the next commit.
//!
//! Emits `FileStageFile` with the move action and `FileStageRevision` with
//! the resulting staged-revision identifier — the same event types as
//! `file stage` and `file stage_merge`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileStageMoveArgs;
use lore::interface::LoreEvent;
use serde::{Deserialize, Serialize};

/// Arguments for [`stage_move`].
///
/// Mirrors `LoreFileStageMoveArgs` from the upstream `lore` crate but uses
/// plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageMoveArgs {
    /// Original path of the file to move.
    pub from_path: String,
    /// New destination path.
    pub to_path: String,
}

impl FileStageMoveArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileStageMoveArgs {
        LoreFileStageMoveArgs {
            from_path: crate::ops::paths::lore_path_arg(repo_root, &self.from_path),
            to_path: crate::ops::paths::lore_path_arg(repo_root, &self.to_path),
        }
    }
}

/// The action applied to a file when it was staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStageMoveAction {
    Keep,
    Add,
    Delete,
    Move,
    Copy,
}

fn map_action(action: &lore::interface::LoreFileAction) -> FileStageMoveAction {
    match action {
        lore::interface::LoreFileAction::Keep => FileStageMoveAction::Keep,
        lore::interface::LoreFileAction::Add => FileStageMoveAction::Add,
        lore::interface::LoreFileAction::Delete => FileStageMoveAction::Delete,
        lore::interface::LoreFileAction::Move => FileStageMoveAction::Move,
        lore::interface::LoreFileAction::Copy => FileStageMoveAction::Copy,
    }
}

/// One file affected by the stage-move operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageMoveEntry {
    /// Repository-relative path that was staged.
    pub path: String,
    /// Previous path when the file was moved; empty otherwise.
    pub from_path: String,
    /// Action applied to the file.
    pub action: FileStageMoveAction,
}

/// Result returned on a successful stage-move.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageMoveResult {
    /// One entry per file affected.
    pub files: Vec<FileStageMoveEntry>,
    /// Resulting staged-revision identifier (empty when none was reported).
    pub revision: String,
}

/// Stage a file move operation in the working directory.
///
/// Calls the upstream `lore::file::stage_move` in-process and collects
/// `FileStageFile` / `FileStageRevision` events into a typed result.
pub async fn stage_move(api: &LoreApi, args: FileStageMoveArgs) -> Result<FileStageMoveResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::file::stage_move(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file stage_move failed with status {status}"),
        )));
    }

    let mut files = Vec::new();
    let mut revision = String::new();

    for event in &stream.events {
        match event {
            LoreEvent::FileStageFile(data) => {
                files.push(FileStageMoveEntry {
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

    Ok(FileStageMoveResult { files, revision })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = FileStageMoveArgs {
            from_path: "src/main.rs".into(),
            to_path: "src/app.rs".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("src/app.rs"));
    }

    #[test]
    fn args_deserializes() {
        let json = r#"{"from_path":"a.txt","to_path":"b.txt"}"#;
        let args: FileStageMoveArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.from_path, "a.txt");
        assert_eq!(args.to_path, "b.txt");
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = FileStageMoveArgs {
            from_path: "hello.md".into(),
            to_path: "world.md".into(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.from_path.as_str(), "/repo/hello.md");
        assert_eq!(lore_args.to_path.as_str(), "/repo/world.md");
    }

    #[test]
    fn action_serde() {
        assert_eq!(
            serde_json::to_string(&FileStageMoveAction::Keep).unwrap(),
            r#""keep""#
        );
        assert_eq!(
            serde_json::to_string(&FileStageMoveAction::Add).unwrap(),
            r#""add""#
        );
        assert_eq!(
            serde_json::to_string(&FileStageMoveAction::Delete).unwrap(),
            r#""delete""#
        );
        assert_eq!(
            serde_json::to_string(&FileStageMoveAction::Move).unwrap(),
            r#""move""#
        );
        assert_eq!(
            serde_json::to_string(&FileStageMoveAction::Copy).unwrap(),
            r#""copy""#
        );
    }

    #[test]
    fn entry_serializes() {
        let entry = FileStageMoveEntry {
            path: "src/lib.rs".into(),
            from_path: String::new(),
            action: FileStageMoveAction::Add,
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains(r#""add""#));
    }

    #[test]
    fn entry_with_move_serializes() {
        let entry = FileStageMoveEntry {
            path: "new/path.rs".into(),
            from_path: "old/path.rs".into(),
            action: FileStageMoveAction::Move,
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("new/path.rs"));
        assert!(json.contains("old/path.rs"));
        assert!(json.contains(r#""move""#));
    }

    #[test]
    fn result_serializes() {
        let result = FileStageMoveResult {
            files: vec![FileStageMoveEntry {
                path: "a.txt".into(),
                from_path: String::new(),
                action: FileStageMoveAction::Add,
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
        let result = FileStageMoveResult {
            files: vec![],
            revision: String::new(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""files":[]"#));
    }

    #[test]
    fn result_round_trip() {
        let result = FileStageMoveResult {
            files: vec![FileStageMoveEntry {
                path: "test.rs".into(),
                from_path: "old.rs".into(),
                action: FileStageMoveAction::Move,
            }],
            revision: "rev42".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: FileStageMoveResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.files.len(), 1);
        assert_eq!(deserialized.files[0].path, "test.rs");
        assert_eq!(deserialized.files[0].from_path, "old.rs");
        assert_eq!(deserialized.files[0].action, FileStageMoveAction::Move);
        assert_eq!(deserialized.revision, "rev42");
    }
}
