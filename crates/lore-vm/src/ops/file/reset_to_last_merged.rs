//! `file reset_to_last_merged` operation — binds `lore::file::reset_to_last_merged`.
//!
//! Resets one or more files to the state they were in at the last merged
//! revision on a given branch, optionally purging untracked files.
//! Emits `FileResetFile` per file, with summary counts in `FileResetEnd`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileResetToLastMergedArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`reset_to_last_merged`].
///
/// Mirrors `LoreFileResetToLastMergedArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResetToLastMergedArgs {
    /// Repository-relative paths to reset.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Branch whose last merged revision to reset to.
    pub branch: String,
    /// Whether to purge untracked files.
    #[serde(default)]
    pub purge: bool,
}

impl FileResetToLastMergedArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileResetToLastMergedArgs {
        LoreFileResetToLastMergedArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
            branch: LoreString::from_str(&self.branch),
            purge: u8::from(self.purge),
        }
    }
}

/// Entry describing one file affected by the reset operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResetToLastMergedEntry {
    /// Repository-relative path.
    pub path: String,
    /// Action applied: "keep", "add", "delete", "move", or "copy".
    pub action: String,
    /// Previous path when the file was moved; empty otherwise.
    #[serde(default)]
    pub from_path: String,
}

/// Summary counts from the reset operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileResetToLastMergedCounts {
    pub directory_reset_count: u64,
    pub directory_delete_count: u64,
    pub file_reset_count: u64,
    pub file_delete_count: u64,
}

/// Result returned on successful file reset to last merged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResetToLastMergedResult {
    /// One entry per file affected.
    pub files: Vec<FileResetToLastMergedEntry>,
    /// Summary counts.
    pub counts: FileResetToLastMergedCounts,
}

fn action_to_string(action: &lore::interface::LoreFileAction) -> String {
    match action {
        lore::interface::LoreFileAction::Keep => "keep".into(),
        lore::interface::LoreFileAction::Add => "add".into(),
        lore::interface::LoreFileAction::Delete => "delete".into(),
        lore::interface::LoreFileAction::Move => "move".into(),
        lore::interface::LoreFileAction::Copy => "copy".into(),
    }
}

/// Reset one or more files to the last merged revision on a branch.
///
/// Calls the upstream `lore::file::reset_to_last_merged` in-process and collects
/// `FileResetFile` / `FileResetEnd` events into typed results.
pub async fn reset_to_last_merged(
    api: &LoreApi,
    args: FileResetToLastMergedArgs,
) -> Result<FileResetToLastMergedResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::file::reset_to_last_merged(globals.build(), args.into_lore(&repo_root), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file reset_to_last_merged failed with status {status}"),
        )));
    }

    let mut files = Vec::new();
    let mut counts = FileResetToLastMergedCounts::default();

    for event in &stream.events {
        match event {
            LoreEvent::FileResetFile(data) => {
                files.push(FileResetToLastMergedEntry {
                    path: data.path.as_str().to_string(),
                    action: action_to_string(&data.action),
                    from_path: data.from_path.as_str().to_string(),
                });
            }
            LoreEvent::FileResetEnd(data) => {
                counts = FileResetToLastMergedCounts {
                    directory_reset_count: data.count.directory_reset_count,
                    directory_delete_count: data.count.directory_delete_count,
                    file_reset_count: data.count.file_reset_count,
                    file_delete_count: data.count.file_delete_count,
                };
            }
            _ => {}
        }
    }

    Ok(FileResetToLastMergedResult { files, counts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = FileResetToLastMergedArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
            branch: "main".into(),
            purge: true,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
        assert!(json.contains(r#""branch":"main""#));
        assert!(json.contains(r#""purge":true"#));
    }

    #[test]
    fn args_deserializes_with_defaults() {
        let json = r#"{"branch":"main"}"#;
        let args: FileResetToLastMergedArgs =
            serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
        assert_eq!(args.branch, "main");
        assert!(!args.purge);
    }

    #[test]
    fn args_deserializes_full() {
        let json = r#"{"paths":["a.txt","b/c.rs"],"branch":"develop","purge":true}"#;
        let args: FileResetToLastMergedArgs =
            serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.paths, vec!["a.txt", "b/c.rs"]);
        assert_eq!(args.branch, "develop");
        assert!(args.purge);
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = FileResetToLastMergedArgs {
            paths: vec!["hello.md".into(), "world.txt".into()],
            branch: "feature-branch".into(),
            purge: false,
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.paths.len(), 2);
        assert_eq!(lore_args.branch.as_str(), "feature-branch");
        assert_eq!(lore_args.purge, 0);
    }

    #[test]
    fn args_into_lore_purge_flag() {
        let args = FileResetToLastMergedArgs {
            paths: vec![],
            branch: String::new(),
            purge: true,
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.purge, 1);
    }

    #[test]
    fn entry_serializes() {
        let entry = FileResetToLastMergedEntry {
            path: "src/lib.rs".into(),
            action: "keep".into(),
            from_path: String::new(),
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains("keep"));
    }

    #[test]
    fn entry_with_move_serializes() {
        let entry = FileResetToLastMergedEntry {
            path: "new/path.rs".into(),
            action: "move".into(),
            from_path: "old/path.rs".into(),
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("new/path.rs"));
        assert!(json.contains("old/path.rs"));
        assert!(json.contains("move"));
    }

    #[test]
    fn counts_default() {
        let counts = FileResetToLastMergedCounts::default();
        assert_eq!(counts.directory_reset_count, 0);
        assert_eq!(counts.directory_delete_count, 0);
        assert_eq!(counts.file_reset_count, 0);
        assert_eq!(counts.file_delete_count, 0);
    }

    #[test]
    fn counts_serializes() {
        let counts = FileResetToLastMergedCounts {
            directory_reset_count: 2,
            directory_delete_count: 1,
            file_reset_count: 5,
            file_delete_count: 3,
        };
        let json = serde_json::to_string(&counts).expect("should serialize");
        assert!(json.contains(r#""file_reset_count":5"#));
        assert!(json.contains(r#""file_delete_count":3"#));
        assert!(json.contains(r#""directory_reset_count":2"#));
        assert!(json.contains(r#""directory_delete_count":1"#));
    }

    #[test]
    fn result_serializes() {
        let result = FileResetToLastMergedResult {
            files: vec![
                FileResetToLastMergedEntry {
                    path: "a.txt".into(),
                    action: "keep".into(),
                    from_path: String::new(),
                },
                FileResetToLastMergedEntry {
                    path: "b.txt".into(),
                    action: "delete".into(),
                    from_path: String::new(),
                },
            ],
            counts: FileResetToLastMergedCounts {
                directory_reset_count: 0,
                directory_delete_count: 0,
                file_reset_count: 1,
                file_delete_count: 1,
            },
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("a.txt"));
        assert!(json.contains("b.txt"));
        assert!(json.contains("keep"));
        assert!(json.contains("delete"));
    }

    #[test]
    fn result_empty() {
        let result = FileResetToLastMergedResult {
            files: vec![],
            counts: FileResetToLastMergedCounts::default(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""files":[]"#));
    }

    #[test]
    fn result_round_trip() {
        let result = FileResetToLastMergedResult {
            files: vec![FileResetToLastMergedEntry {
                path: "test.rs".into(),
                action: "keep".into(),
                from_path: String::new(),
            }],
            counts: FileResetToLastMergedCounts {
                directory_reset_count: 0,
                directory_delete_count: 0,
                file_reset_count: 1,
                file_delete_count: 0,
            },
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: FileResetToLastMergedResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.files.len(), 1);
        assert_eq!(deserialized.files[0].path, "test.rs");
        assert_eq!(deserialized.files[0].action, "keep");
        assert_eq!(deserialized.counts.file_reset_count, 1);
    }
}
