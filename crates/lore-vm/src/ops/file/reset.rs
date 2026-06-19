//! `file reset` operation — binds `lore::file::reset`.
//!
//! Resets one or more files to a specified revision, optionally purging
//! untracked files.  Emits `FileResetFile` per file, with summary counts
//! in `FileResetEnd`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileResetArgs;
use lore::interface::{LoreArray, LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`reset`].
///
/// Mirrors `LoreFileResetArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResetArgs {
    /// Repository-relative paths to reset.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Revision to reset files to (e.g. a branch name, changelist number, or
    /// special identifier).
    #[serde(default)]
    pub revision: String,
    /// Whether to purge untracked files.
    #[serde(default)]
    pub purge: bool,
}

impl FileResetArgs {
    fn into_lore(self) -> LoreFileResetArgs {
        let lore_paths: Vec<LoreString> =
            self.paths.iter().map(|p| LoreString::from_str(p)).collect();
        LoreFileResetArgs {
            paths: LoreArray::from_vec(lore_paths),
            revision: LoreString::from_str(&self.revision),
            purge: u8::from(self.purge),
        }
    }
}

/// Entry describing one file affected by the reset operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResetEntry {
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
pub struct FileResetCounts {
    pub directory_reset_count: u64,
    pub directory_delete_count: u64,
    pub file_reset_count: u64,
    pub file_delete_count: u64,
}

/// Result returned on successful file reset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResetResult {
    /// One entry per file affected.
    pub files: Vec<FileResetEntry>,
    /// Summary counts.
    pub counts: FileResetCounts,
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

/// Reset one or more files to a specified revision.
///
/// Calls the upstream `lore::file::reset` in-process and collects
/// `FileResetFile` / `FileResetEnd` events into typed results.
pub async fn reset(api: &LoreApi, args: FileResetArgs) -> Result<FileResetResult> {
    let (callback, rx) = collect_events();

    let status = lore::file::reset(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file reset failed with status {status}"),
        )));
    }

    let mut files = Vec::new();
    let mut counts = FileResetCounts::default();

    for event in &stream.events {
        match event {
            LoreEvent::FileResetFile(data) => {
                files.push(FileResetEntry {
                    path: data.path.as_str().to_string(),
                    action: action_to_string(&data.action),
                    from_path: data.from_path.as_str().to_string(),
                });
            }
            LoreEvent::FileResetEnd(data) => {
                counts = FileResetCounts {
                    directory_reset_count: data.count.directory_reset_count,
                    directory_delete_count: data.count.directory_delete_count,
                    file_reset_count: data.count.file_reset_count,
                    file_delete_count: data.count.file_delete_count,
                };
            }
            _ => {}
        }
    }

    Ok(FileResetResult { files, counts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_reset_args_serializes() {
        let args = FileResetArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
            revision: "head".into(),
            purge: true,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
        assert!(json.contains(r#""revision":"head""#));
        assert!(json.contains(r#""purge":true"#));
    }

    #[test]
    fn file_reset_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: FileResetArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
        assert!(args.revision.is_empty());
        assert!(!args.purge);
    }

    #[test]
    fn file_reset_args_deserializes_full() {
        let json = r#"{"paths":["a.txt","b/c.rs"],"revision":"42","purge":true}"#;
        let args: FileResetArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.paths, vec!["a.txt", "b/c.rs"]);
        assert_eq!(args.revision, "42");
        assert!(args.purge);
    }

    #[test]
    fn file_reset_args_into_lore_conversion() {
        let args = FileResetArgs {
            paths: vec!["hello.md".into(), "world.txt".into()],
            revision: "latest".into(),
            purge: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.paths.len(), 2);
        assert_eq!(lore_args.revision.as_str(), "latest");
        assert_eq!(lore_args.purge, 0);
    }

    #[test]
    fn file_reset_args_into_lore_purge_flag() {
        let args = FileResetArgs {
            paths: vec![],
            revision: String::new(),
            purge: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.purge, 1);
    }

    #[test]
    fn file_reset_entry_serializes() {
        let entry = FileResetEntry {
            path: "src/lib.rs".into(),
            action: "keep".into(),
            from_path: String::new(),
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains("keep"));
    }

    #[test]
    fn file_reset_entry_with_move_serializes() {
        let entry = FileResetEntry {
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
    fn file_reset_counts_default() {
        let counts = FileResetCounts::default();
        assert_eq!(counts.directory_reset_count, 0);
        assert_eq!(counts.directory_delete_count, 0);
        assert_eq!(counts.file_reset_count, 0);
        assert_eq!(counts.file_delete_count, 0);
    }

    #[test]
    fn file_reset_counts_serializes() {
        let counts = FileResetCounts {
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
    fn file_reset_result_serializes() {
        let result = FileResetResult {
            files: vec![
                FileResetEntry {
                    path: "a.txt".into(),
                    action: "keep".into(),
                    from_path: String::new(),
                },
                FileResetEntry {
                    path: "b.txt".into(),
                    action: "delete".into(),
                    from_path: String::new(),
                },
            ],
            counts: FileResetCounts {
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
    fn file_reset_result_empty() {
        let result = FileResetResult {
            files: vec![],
            counts: FileResetCounts::default(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""files":[]"#));
    }

    #[test]
    fn file_reset_result_round_trip() {
        let result = FileResetResult {
            files: vec![FileResetEntry {
                path: "test.rs".into(),
                action: "keep".into(),
                from_path: String::new(),
            }],
            counts: FileResetCounts {
                directory_reset_count: 0,
                directory_delete_count: 0,
                file_reset_count: 1,
                file_delete_count: 0,
            },
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: FileResetResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.files.len(), 1);
        assert_eq!(deserialized.files[0].path, "test.rs");
        assert_eq!(deserialized.files[0].action, "keep");
        assert_eq!(deserialized.counts.file_reset_count, 1);
    }
}
