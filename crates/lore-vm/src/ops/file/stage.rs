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
    /// Convert to the upstream lore args, resolving every incoming path against
    /// `repo_root`.
    ///
    /// The upstream `lore::file::stage` only stages files when handed an
    /// **absolute** path; a repository-relative path (e.g. `"src/main.rs"`) is
    /// silently dropped and stages nothing. Every external driver — the VS Code
    /// extension (`path.relative(repoRoot, uri)`), the MCP server, and CLI
    /// callers — sends repo-relative paths today, so we join each relative path
    /// onto the repo root here. Paths that are already absolute are passed
    /// through unchanged.
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileStageArgs {
        let lore_paths: Vec<LoreString> = self
            .paths
            .iter()
            .map(|p| {
                let path = std::path::Path::new(p);
                if path.is_absolute() {
                    LoreString::from_str(p)
                } else {
                    LoreString::from_path(repo_root.join(path))
                }
            })
            .collect();
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

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status = lore::file::stage(globals.build(), args.into_lore(&repo_root), callback).await;

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
        let repo_root = std::path::Path::new("/repo");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(lore_args.paths.len(), 2);
        assert_eq!(lore_args.case_change, 2);
        assert_eq!(lore_args.scan, 1);
    }

    /// Regression for BUG #1 (SBAI-4080): repository-relative paths — what every
    /// external driver (VS Code, MCP, CLI) sends — must be resolved against the
    /// repo root so the upstream engine actually stages them. Previously they
    /// were passed through verbatim and the engine silently staged nothing.
    #[test]
    fn stage_args_resolves_relative_paths_against_repo_root() {
        let args = FileStageArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
            case_change: CaseChange::Error,
            scan: false,
        };
        let repo_root = std::path::Path::new("/work/myrepo");
        let lore_args = args.into_lore(repo_root);

        let resolved: Vec<String> = lore_args
            .paths
            .as_slice()
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
        assert_eq!(resolved.len(), 2);
        // Both relative paths are now joined onto the repo root.
        assert!(
            resolved.contains(&"/work/myrepo/src/main.rs".to_string()),
            "relative path should be resolved against repo root, got {resolved:?}"
        );
        assert!(
            resolved.contains(&"/work/myrepo/README.md".to_string()),
            "relative path should be resolved against repo root, got {resolved:?}"
        );
    }

    /// Already-absolute paths must pass through unchanged (the engine accepted
    /// these all along; the fix must not double-prefix them).
    #[test]
    fn stage_args_passes_absolute_paths_through() {
        let abs = if cfg!(windows) {
            r"C:\work\myrepo\rel.txt"
        } else {
            "/work/myrepo/rel.txt"
        };
        let args = FileStageArgs {
            paths: vec![abs.into()],
            case_change: CaseChange::Error,
            scan: false,
        };
        let repo_root = std::path::Path::new("/some/other/root");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(lore_args.paths.len(), 1);
        assert_eq!(lore_args.paths.as_slice()[0].as_str(), abs);
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
