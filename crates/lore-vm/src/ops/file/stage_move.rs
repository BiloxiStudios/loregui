//! `file stage_move` operation — binds `lore::file::stage_move`.
//!
//! Stages a file move from one path to another. The original path is deleted
//! and the new path is staged in a single atomic operation.
//!
//! Emits `FileStageFile` per affected file (deletion of `from_path`, addition
//! of `to_path`) and `FileStageRevision` with the resulting staged-revision
//! identifier.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileStageMoveArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// The action applied to a file during the stage move.
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
    /// Repository-relative path.
    pub path: String,
    /// Previous path (for the moved file). Empty otherwise.
    pub from_path: String,
    /// Action applied to the file.
    pub action: FileStageMoveAction,
}

/// Arguments for [`stage_move`].
///
/// Mirrors `LoreFileStageMoveArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageMoveArgs {
    /// Original path of the file to move.
    pub from_path: String,
    /// New path for the file.
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

/// Result returned on a successful stage move.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageMoveResult {
    /// Files affected by the move (typically two: delete of `from_path`,
    /// add of `to_path`).
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
            from_path: "src/old.rs".into(),
            to_path: "src/new.rs".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/old.rs"));
        assert!(json.contains("src/new.rs"));
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
            from_path: "old/path.txt".into(),
            to_path: "new/path.txt".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.from_path.as_str(), "old/path.txt");
        assert_eq!(lore_args.to_path.as_str(), "new/path.txt");
    }

    #[test]
    fn stage_move_result_serializes() {
        let result = FileStageMoveResult {
            files: vec![
                FileStageMoveEntry {
                    path: "old.rs".into(),
                    from_path: String::new(),
                    action: FileStageMoveAction::Delete,
                },
                FileStageMoveEntry {
                    path: "new.rs".into(),
                    from_path: "old.rs".into(),
                    action: FileStageMoveAction::Add,
                },
            ],
            revision: "rev123".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("old.rs"));
        assert!(json.contains("new.rs"));
        assert!(json.contains("rev123"));
    }
}
