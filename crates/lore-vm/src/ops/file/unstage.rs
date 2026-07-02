//! `file unstage` operation — binds `lore::file::unstage`.
//!
//! Removes one or more files from the staged changeset.
//! Emits `FileUnstageFile` per file, with summary counts in `FileUnstageEnd`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileUnstageArgs;
use lore::interface::LoreEvent;
use serde::{Deserialize, Serialize};

/// Arguments for [`unstage`].
///
/// Mirrors `LoreFileUnstageArgs` from the upstream `lore` crate
/// but uses plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUnstageArgs {
    /// Repository-relative paths to unstage.
    #[serde(default)]
    pub paths: Vec<String>,
}

impl FileUnstageArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileUnstageArgs {
        LoreFileUnstageArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
        }
    }
}

/// Entry describing one file affected by the unstage operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUnstageEntry {
    /// Repository-relative path.
    pub path: String,
    /// Action applied: "keep" (unstaged in place) or "delete" (discarded).
    pub action: String,
}

/// Summary counts from the unstage operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileUnstageCounts {
    pub directory_unstaged_count: u64,
    pub directory_discarded_count: u64,
    pub file_unstaged_count: u64,
    pub file_discarded_count: u64,
    pub total_count: u64,
}

/// Result returned on successful file unstage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUnstageResult {
    /// One entry per file affected.
    pub files: Vec<FileUnstageEntry>,
    /// Summary counts.
    pub counts: FileUnstageCounts,
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

/// Remove one or more files from the staged changeset.
///
/// Calls the upstream `lore::file::unstage` in-process and collects
/// `FileUnstageFile` / `FileUnstageEnd` events into typed results.
pub async fn unstage(api: &LoreApi, args: FileUnstageArgs) -> Result<FileUnstageResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status = lore::file::unstage(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file unstage failed with status {status}"),
        )));
    }

    let mut files = Vec::new();
    let mut counts = FileUnstageCounts::default();

    for event in &stream.events {
        match event {
            LoreEvent::FileUnstageFile(data) => {
                files.push(FileUnstageEntry {
                    path: data.path.as_str().to_string(),
                    action: action_to_string(&data.action),
                });
            }
            LoreEvent::FileUnstageEnd(data) => {
                counts = FileUnstageCounts {
                    directory_unstaged_count: data.count.directory_unstaged_count,
                    directory_discarded_count: data.count.directory_discarded_count,
                    file_unstaged_count: data.count.file_unstaged_count,
                    file_discarded_count: data.count.file_discarded_count,
                    total_count: data.count.total_count,
                };
            }
            _ => {}
        }
    }

    Ok(FileUnstageResult { files, counts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_unstage_args_serializes() {
        let args = FileUnstageArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn file_unstage_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: FileUnstageArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
    }

    #[test]
    fn file_unstage_args_deserializes_with_paths() {
        let json = r#"{"paths":["a.txt","b/c.rs"]}"#;
        let args: FileUnstageArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.paths, vec!["a.txt", "b/c.rs"]);
    }

    #[test]
    fn file_unstage_args_into_lore_conversion() {
        let args = FileUnstageArgs {
            paths: vec!["hello.md".into(), "world.txt".into()],
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.paths.len(), 2);
    }

    #[test]
    fn file_unstage_entry_serializes() {
        let entry = FileUnstageEntry {
            path: "src/lib.rs".into(),
            action: "keep".into(),
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains("keep"));
    }

    #[test]
    fn file_unstage_counts_default() {
        let counts = FileUnstageCounts::default();
        assert_eq!(counts.total_count, 0);
        assert_eq!(counts.file_unstaged_count, 0);
        assert_eq!(counts.file_discarded_count, 0);
        assert_eq!(counts.directory_unstaged_count, 0);
        assert_eq!(counts.directory_discarded_count, 0);
    }

    #[test]
    fn file_unstage_counts_serializes() {
        let counts = FileUnstageCounts {
            directory_unstaged_count: 1,
            directory_discarded_count: 0,
            file_unstaged_count: 3,
            file_discarded_count: 1,
            total_count: 5,
        };
        let json = serde_json::to_string(&counts).expect("should serialize");
        assert!(json.contains(r#""total_count":5"#));
        assert!(json.contains(r#""file_unstaged_count":3"#));
    }

    #[test]
    fn file_unstage_result_serializes() {
        let result = FileUnstageResult {
            files: vec![
                FileUnstageEntry {
                    path: "a.txt".into(),
                    action: "keep".into(),
                },
                FileUnstageEntry {
                    path: "b.txt".into(),
                    action: "delete".into(),
                },
            ],
            counts: FileUnstageCounts {
                directory_unstaged_count: 0,
                directory_discarded_count: 0,
                file_unstaged_count: 1,
                file_discarded_count: 1,
                total_count: 2,
            },
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("a.txt"));
        assert!(json.contains("b.txt"));
        assert!(json.contains("keep"));
        assert!(json.contains("delete"));
    }

    #[test]
    fn file_unstage_result_empty() {
        let result = FileUnstageResult {
            files: vec![],
            counts: FileUnstageCounts::default(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""files":[]"#));
    }

    #[test]
    fn file_unstage_result_round_trip() {
        let result = FileUnstageResult {
            files: vec![FileUnstageEntry {
                path: "test.rs".into(),
                action: "keep".into(),
            }],
            counts: FileUnstageCounts {
                directory_unstaged_count: 0,
                directory_discarded_count: 0,
                file_unstaged_count: 1,
                file_discarded_count: 0,
                total_count: 1,
            },
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: FileUnstageResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.files.len(), 1);
        assert_eq!(deserialized.files[0].path, "test.rs");
        assert_eq!(deserialized.files[0].action, "keep");
        assert_eq!(deserialized.counts.total_count, 1);
    }
}
