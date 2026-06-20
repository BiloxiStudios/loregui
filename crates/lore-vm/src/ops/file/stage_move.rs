//! `file stage_move` operation — binds `lore::file::stage_move`.
//!
//! Stages a file move from one path to another in the staging area.
//! Emits `FileStageFile` per affected path and `FileStageRevision`
//! with the resulting staged-revision identifier.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileStageMoveArgs;
use lore::interface::{LoreEvent, LoreString};
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
    fn into_lore(self) -> LoreFileStageMoveArgs {
        LoreFileStageMoveArgs {
            from_path: LoreString::from_str(&self.from_path),
            to_path: LoreString::from_str(&self.to_path),
        }
    }
}

/// The action applied to a file during stage-move.
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

/// One path affected by the stage-move operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageMoveEntry {
    /// Repository-relative path that was staged.
    pub path: String,
    /// Previous path, when the file was moved. Empty otherwise.
    pub from_path: String,
    /// Action applied to the file.
    pub action: FileStageMoveAction,
}

/// Result returned on a successful stage-move.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageMoveResult {
    /// Entries for each path affected (typically two: delete original + add new).
    pub files: Vec<FileStageMoveEntry>,
    /// Resulting staged-revision identifier (empty when none was reported).
    pub revision: String,
}

/// Stage a file move from one path to another.
///
/// Calls the upstream `lore::file::stage_move` in-process and collects
/// `FileStageFile` / `FileStageRevision` events into a typed result.
pub async fn stage_move(api: &LoreApi, args: FileStageMoveArgs) -> Result<FileStageMoveResult> {
    let (callback, rx) = collect_events();

    let status = lore::file::stage_move(api.globals().build(), args.into_lore(), callback).await;

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
    fn stage_move_args_serializes() {
        let args = FileStageMoveArgs {
            from_path: "src/main.rs".into(),
            to_path: "src/app.rs".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("src/app.rs"));
    }

    #[test]
    fn stage_move_args_deserializes() {
        let json = r#"{"from_path":"a.txt","to_path":"b.txt"}"#;
        let args: FileStageMoveArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.from_path, "a.txt");
        assert_eq!(args.to_path, "b.txt");
    }

    #[test]
    fn stage_move_args_into_lore_conversion() {
        let args = FileStageMoveArgs {
            from_path: "hello.md".into(),
            to_path: "world.md".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.from_path.as_str(), "hello.md");
        assert_eq!(lore_args.to_path.as_str(), "world.md");
    }

    #[test]
    fn stage_move_action_serde() {
        assert_eq!(
            serde_json::to_string(&FileStageMoveAction::Keep).unwrap(),
            r#""keep""#
        );
        assert_eq!(
            serde_json::to_string(&FileStageMoveAction::Delete).unwrap(),
            r#""delete""#
        );
        assert_eq!(
            serde_json::to_string(&FileStageMoveAction::Move).unwrap(),
            r#""move""#
        );
    }

    #[test]
    fn stage_move_entry_serializes() {
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
    fn stage_move_result_serializes() {
        let result = FileStageMoveResult {
            files: vec![
                FileStageMoveEntry {
                    path: "old/file.txt".into(),
                    from_path: String::new(),
                    action: FileStageMoveAction::Delete,
                },
                FileStageMoveEntry {
                    path: "new/file.txt".into(),
                    from_path: "old/file.txt".into(),
                    action: FileStageMoveAction::Move,
                },
            ],
            revision: "rev42".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: FileStageMoveResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.files.len(), 2);
        assert_eq!(deserialized.revision, "rev42");
    }
}
