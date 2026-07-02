//! `file info` operation — binds `lore::file::info`.
//!
//! Retrieves metadata for one or more files: size, hash, staged status,
//! modification flags, and optionally local/filtered sizes.
//! Emits one `LoreEvent::FileInfo` per file on success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileInfoArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`info`].
///
/// Mirrors `LoreFileInfoArgs` from the upstream `lore` crate
/// but uses plain `String`/`Vec` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfoArgs {
    /// Repository-relative paths to query.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Optional revision specifier (empty = working copy).
    #[serde(default)]
    pub revision: String,
    /// Calculate the filtered local filesystem hash and size.
    #[serde(default)]
    pub local: bool,
    /// Calculate the filtered repository size.
    #[serde(default)]
    pub filtered: bool,
}

impl FileInfoArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileInfoArgs {
        LoreFileInfoArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
            revision: LoreString::from_str(&self.revision),
            local: u8::from(self.local),
            filtered: u8::from(self.filtered),
        }
    }
}

/// Metadata for a single file or directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfoEntry {
    /// Repository-relative path.
    pub path: String,
    /// Context identifier for this file.
    pub context: String,
    /// Content hash.
    pub hash: String,
    /// Whether this entry is a file.
    pub is_file: bool,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Whether the entry has been modified.
    pub flag_modified: bool,
    /// Whether the entry has been deleted.
    pub flag_deleted: bool,
    /// Whether the entry has been added.
    pub flag_added: bool,
    /// Whether the entry is in conflict.
    pub flag_conflict: bool,
    /// File mode bits.
    pub mode: u16,
    /// Size in the repository (bytes).
    pub size: u64,
    /// Size on the local filesystem (bytes).
    pub local_size: u64,
    /// Content hash on the local filesystem.
    pub local_hash: String,
    /// Size after filters are applied (bytes).
    pub filter_size: u64,
}

/// Result returned on successful file info query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfoResult {
    /// One entry per queried file/directory.
    pub entries: Vec<FileInfoEntry>,
}

/// Retrieve metadata for one or more files.
///
/// Calls the upstream `lore::file::info` in-process and collects
/// `FileInfo` events into a typed result.
pub async fn info(api: &LoreApi, args: FileInfoArgs) -> Result<FileInfoResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status = lore::file::info(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file info failed with status {status}"),
        )));
    }

    let entries: Vec<FileInfoEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::FileInfo(data) = event {
                Some(FileInfoEntry {
                    path: data.path.as_str().to_string(),
                    context: format!("{}", data.context),
                    hash: format!("{}", data.hash),
                    is_file: data.is_file != 0,
                    is_dir: data.is_dir != 0,
                    flag_modified: data.flag_modified != 0,
                    flag_deleted: data.flag_deleted != 0,
                    flag_added: data.flag_added != 0,
                    flag_conflict: data.flag_conflict != 0,
                    mode: data.mode,
                    size: data.size,
                    local_size: data.local_size,
                    local_hash: format!("{}", data.local_hash),
                    filter_size: data.filter_size,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(FileInfoResult { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_info_args_serializes() {
        let args = FileInfoArgs {
            paths: vec!["src/main.rs".into()],
            revision: String::new(),
            local: true,
            filtered: false,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains(r#""local":true"#));
    }

    #[test]
    fn file_info_args_deserializes_with_defaults() {
        let json = r#"{"paths": ["README.md"]}"#;
        let args: FileInfoArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.paths, vec!["README.md"]);
        assert_eq!(args.revision, "");
        assert!(!args.local);
        assert!(!args.filtered);
    }

    #[test]
    fn file_info_args_deserializes_empty() {
        let json = r#"{}"#;
        let args: FileInfoArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
    }

    #[test]
    fn file_info_args_into_lore_conversion() {
        let args = FileInfoArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
            revision: "rev1".into(),
            local: true,
            filtered: true,
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.revision.as_str(), "rev1");
        assert_eq!(lore_args.local, 1);
        assert_eq!(lore_args.filtered, 1);
    }

    #[test]
    fn file_info_entry_serializes() {
        let entry = FileInfoEntry {
            path: "src/lib.rs".into(),
            context: "ctx-abc".into(),
            hash: "hash-def".into(),
            is_file: true,
            is_dir: false,
            flag_modified: true,
            flag_deleted: false,
            flag_added: false,
            flag_conflict: false,
            mode: 0o100644,
            size: 2048,
            local_size: 2100,
            local_hash: "hash-local".into(),
            filter_size: 1900,
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("src/lib.rs"));
        assert!(json.contains("hash-def"));
        assert!(json.contains("2048"));
        assert!(json.contains(r#""is_file":true"#));
        assert!(json.contains(r#""flag_modified":true"#));
    }

    #[test]
    fn file_info_result_serializes() {
        let result = FileInfoResult {
            entries: vec![FileInfoEntry {
                path: "test.txt".into(),
                context: "c".into(),
                hash: "h".into(),
                is_file: true,
                is_dir: false,
                flag_modified: false,
                flag_deleted: false,
                flag_added: true,
                flag_conflict: false,
                mode: 0,
                size: 100,
                local_size: 0,
                local_hash: String::new(),
                filter_size: 0,
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("test.txt"));
    }

    #[test]
    fn file_info_result_empty() {
        let result = FileInfoResult { entries: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }
}
