//! `file dump` operation — binds `lore::file::dump`.
//!
//! Dumps the binary content of a file by path or address.
//! Emits `LoreEvent::FileDump` on success containing address, flags,
//! payload size, content size, and whether a match was made.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileDumpArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`dump`].
///
/// Mirrors `LoreFileDumpArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDumpArgs {
    /// Address of data to dump; takes precedence over `path` when non-empty.
    #[serde(default)]
    pub address: String,
    /// Repository-relative path to dump; used when `address` is empty.
    #[serde(default)]
    pub path: String,
}

impl FileDumpArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileDumpArgs {
        LoreFileDumpArgs {
            address: LoreString::from_str(&self.address),
            path: crate::ops::paths::lore_path_arg(repo_root, &self.path),
        }
    }
}

/// A single dump entry returned on success.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDumpEntry {
    /// Content address.
    pub address: String,
    /// Flags describing the stored content.
    pub flags: u32,
    /// Size of the stored payload in bytes.
    pub size_payload: u32,
    /// Size of the content in bytes.
    pub size_content: u64,
    /// Whether a matching stored object was found.
    pub match_made: bool,
}

/// Result returned on successful file dump.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDumpResult {
    /// One entry per `FileDump` event emitted.
    pub entries: Vec<FileDumpEntry>,
}

/// Dump the binary content of a file by path or address.
///
/// Calls the upstream `lore::file::dump` in-process and collects
/// `FileDump` events into a typed result.
pub async fn dump(api: &LoreApi, args: FileDumpArgs) -> Result<FileDumpResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status = lore::file::dump(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file dump failed with status {status}"),
        )));
    }

    let entries: Vec<FileDumpEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::FileDump(data) = event {
                Some(FileDumpEntry {
                    address: format!("{}", data.address),
                    flags: data.flags,
                    size_payload: data.size_payload,
                    size_content: data.size_content,
                    match_made: data.match_made != 0,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(FileDumpResult { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_dump_args_serializes() {
        let args = FileDumpArgs {
            address: String::new(),
            path: "src/main.rs".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
    }

    #[test]
    fn file_dump_args_deserializes_with_defaults() {
        let json = r#"{"path": "README.md"}"#;
        let args: FileDumpArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.path, "README.md");
        assert_eq!(args.address, "");
    }

    #[test]
    fn file_dump_args_deserializes_empty() {
        let json = r#"{}"#;
        let args: FileDumpArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.path.is_empty());
        assert!(args.address.is_empty());
    }

    #[test]
    fn file_dump_args_into_lore_conversion() {
        let args = FileDumpArgs {
            address: "abc123".into(),
            path: "test.txt".into(),
        };
        let lore_args = args.into_lore(std::path::Path::new("/repo"));
        assert_eq!(lore_args.address.as_str(), "abc123");
        assert_eq!(lore_args.path.as_str(), "/repo/test.txt");
    }

    #[test]
    fn file_dump_entry_serializes() {
        let entry = FileDumpEntry {
            address: "addr-123".into(),
            flags: 0,
            size_payload: 512,
            size_content: 1024,
            match_made: true,
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("addr-123"));
        assert!(json.contains("512"));
        assert!(json.contains("1024"));
        assert!(json.contains(r#""match_made":true"#));
    }

    #[test]
    fn file_dump_result_serializes() {
        let result = FileDumpResult {
            entries: vec![FileDumpEntry {
                address: "a".into(),
                flags: 1,
                size_payload: 100,
                size_content: 200,
                match_made: false,
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""address":"a""#));
        assert!(json.contains(r#""match_made":false"#));
    }

    #[test]
    fn file_dump_result_empty() {
        let result = FileDumpResult { entries: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }
}
