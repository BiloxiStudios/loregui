//! `file stage` operation — binds `lore::file::stage`.
//!
//! Stages one or more files for inclusion in the next commit. Each path is
//! classified as a file or directory by the upstream engine; directory paths
//! honour the `scan` flag for a recursive filesystem walk.
//!
//! Emits `FileStageFile` per file and `FileStageRevision` with the resulting
//! staged-revision identifier.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileStageArgs;
use lore::interface::{LoreArray, LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Case-change handling for staged paths — mirrors the upstream `case_change`
/// integer with serde-friendly naming for the Tauri boundary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseChange {
    /// Error on a case-only change (0).
    #[default]
    Error,
    /// Update the filesystem to match the repository (1).
    Keep,
    /// Update the repository to match the filesystem (2).
    Rename,
}

impl CaseChange {
    fn as_u32(self) -> u32 {
        match self {
            CaseChange::Error => 0,
            CaseChange::Keep => 1,
            CaseChange::Rename => 2,
        }
    }
}

/// Arguments for [`stage`].
///
/// Mirrors `LoreFileStageArgs` from the upstream `lore` crate but uses plain
/// `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageArgs {
    /// Paths to stage. Individual files are always reconciled against the
    /// filesystem; directory paths honour [`scan`](FileStageArgs::scan).
    #[serde(default)]
    pub paths: Vec<String>,
    /// How to handle case-only path changes.
    #[serde(default)]
    pub case_change: CaseChange,
    /// Force a recursive filesystem scan of directory paths.
    #[serde(default)]
    pub scan: bool,
}

impl FileStageArgs {
    fn into_lore(self) -> LoreFileStageArgs {
        let lore_paths: Vec<LoreString> =
            self.paths.iter().map(|p| LoreString::from_str(p)).collect();
        LoreFileStageArgs {
            paths: LoreArray::from_vec(lore_paths),
            case_change: self.case_change.as_u32(),
            scan: u8::from(self.scan),
        }
    }
}

/// The action applied to a file when it was staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStageAction {
    Keep,
    Add,
    Delete,
    Move,
    Copy,
}

fn map_action(action: &lore::interface::LoreFileAction) -> FileStageAction {
    match action {
        lore::interface::LoreFileAction::Keep => FileStageAction::Keep,
        lore::interface::LoreFileAction::Add => FileStageAction::Add,
        lore::interface::LoreFileAction::Delete => FileStageAction::Delete,
        lore::interface::LoreFileAction::Move => FileStageAction::Move,
        lore::interface::LoreFileAction::Copy => FileStageAction::Copy,
    }
}

/// One file affected by the stage operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageEntry {
    /// Repository-relative path that was staged.
    pub path: String,
    /// Previous path, when the file was moved. Empty otherwise.
    pub from_path: String,
    /// Action applied to the file.
    pub action: FileStageAction,
}

/// Result returned on a successful stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageResult {
    /// One entry per file affected.
    pub files: Vec<FileStageEntry>,
    /// Resulting staged-revision identifier (empty when none was reported).
    pub revision: String,
}

/// Stage one or more files for the next commit.
///
/// Calls the upstream `lore::file::stage` in-process and collects
/// `FileStageFile` / `FileStageRevision` events into a typed result.
pub async fn stage(api: &LoreApi, args: FileStageArgs) -> Result<FileStageResult> {
    let (callback, rx) = collect_events();

    let status = lore::file::stage(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file stage failed with status {status}"),
        )));
    }

    let mut files = Vec::new();
    let mut revision = String::new();

    for event in &stream.events {
        match event {
            LoreEvent::FileStageFile(data) => {
                files.push(FileStageEntry {
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

    Ok(FileStageResult { files, revision })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_args_serializes() {
        let args = FileStageArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
            case_change: CaseChange::Error,
            scan: true,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn stage_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: FileStageArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
        assert_eq!(args.case_change, CaseChange::Error);
        assert!(!args.scan);
    }

    #[test]
    fn stage_args_into_lore_conversion() {
        let args = FileStageArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
            case_change: CaseChange::Rename,
            scan: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 2);
        assert_eq!(lore_args.case_change, 2);
        assert_eq!(lore_args.scan, 1);
    }

    #[test]
    fn case_change_serde() {
        assert_eq!(
            serde_json::to_string(&CaseChange::Error).unwrap(),
            r#""error""#
        );
        assert_eq!(
            serde_json::to_string(&CaseChange::Keep).unwrap(),
            r#""keep""#
        );
        assert_eq!(
            serde_json::to_string(&CaseChange::Rename).unwrap(),
            r#""rename""#
        );
    }

    #[test]
    fn stage_result_serializes() {
        let result = FileStageResult {
            files: vec![FileStageEntry {
                path: "a.txt".into(),
                from_path: String::new(),
                action: FileStageAction::Add,
            }],
            revision: "abc123".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("a.txt"));
        assert!(json.contains("abc123"));
        assert!(json.contains(r#""add""#));
    }
}
