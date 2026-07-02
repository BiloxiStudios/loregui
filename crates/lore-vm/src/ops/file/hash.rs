//! `file hash` operation — binds `lore::file::hash`.
//!
//! Computes the content hash (BLAKE3) and size of one or more files in the
//! repository. Emits one `LoreEvent::FileHash` per file.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileHashArgs;
use lore::interface::LoreEvent;
use serde::{Deserialize, Serialize};

/// Arguments for [`hash`].
///
/// Mirrors `LoreFileHashArgs` from the upstream `lore` crate
/// but uses plain `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHashArgs {
    /// File paths to hash (relative to the repository root).
    pub paths: Vec<String>,
}

impl FileHashArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileHashArgs {
        LoreFileHashArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
        }
    }
}

/// Hash and size of a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHashEntry {
    /// Path of the file.
    pub path: String,
    /// Size of the file in bytes.
    pub size: u64,
    /// BLAKE3 content hash (hex string).
    pub hash: String,
}

/// Result returned on successful file hash computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHashResult {
    /// One entry per file that was hashed.
    pub files: Vec<FileHashEntry>,
}

/// Compute the content hash and size of one or more files.
///
/// Calls the upstream `lore::file::hash` in-process and collects
/// `FileHash` events to return typed results.
pub async fn hash(api: &LoreApi, args: FileHashArgs) -> Result<FileHashResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status = lore::file::hash(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file hash failed with status {status}"),
        )));
    }

    let files: Vec<FileHashEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::FileHash(data) = event {
                Some(FileHashEntry {
                    path: data.path.as_str().to_string(),
                    size: data.size,
                    hash: format!("{}", data.hash),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(FileHashResult { files })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_hash_args_serializes() {
        let args = FileHashArgs {
            paths: vec!["foo.txt".into(), "bar/baz.rs".into()],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("foo.txt"));
        assert!(json.contains("bar/baz.rs"));
    }

    #[test]
    fn file_hash_args_deserializes() {
        let json = r#"{"paths":["a.txt"]}"#;
        let args: FileHashArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.paths, vec!["a.txt"]);
    }

    #[test]
    fn file_hash_args_into_lore_conversion() {
        let args = FileHashArgs {
            paths: vec!["hello.md".into()],
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        let slice = lore_args.paths.as_slice();
        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].as_str(), "/repo/hello.md");
    }

    #[test]
    fn file_hash_result_serializes() {
        let result = FileHashResult {
            files: vec![FileHashEntry {
                path: "test.txt".into(),
                size: 42,
                hash: "abc123def456".into(),
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("test.txt"));
        assert!(json.contains("42"));
        assert!(json.contains("abc123def456"));
    }

    #[test]
    fn file_hash_result_empty_files() {
        let result = FileHashResult { files: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""files":[]"#));
    }
}
