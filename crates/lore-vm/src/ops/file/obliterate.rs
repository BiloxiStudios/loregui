//! `file obliterate` operation — binds `lore::file::obliterate`.
//!
//! Permanently removes a file or address from repository history.
//! Emits one `LoreEvent::FileObliterate` per item removed.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileObliterateArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`obliterate`].
///
/// Mirrors `LoreFileObliterateArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileObliterateArgs {
    /// Address of data to obliterate; takes precedence over `path` when non-empty.
    #[serde(default)]
    pub address: String,
    /// Repository path to obliterate; used when `address` is empty.
    #[serde(default)]
    pub path: String,
}

impl FileObliterateArgs {
    fn into_lore(self) -> LoreFileObliterateArgs {
        LoreFileObliterateArgs {
            address: LoreString::from_str(&self.address),
            path: LoreString::from_str(&self.path),
        }
    }
}

/// Entry describing one obliterated item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileObliterateEntry {
    /// Address of the obliterated content.
    pub address: String,
    /// Number of fragments removed.
    pub num_fragments: usize,
    /// Number of payloads removed.
    pub num_payloads: usize,
}

/// Result returned on successful file obliteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileObliterateResult {
    /// One entry per item obliterated.
    pub obliterated: Vec<FileObliterateEntry>,
}

/// Permanently remove a file or address from repository history.
///
/// Calls the upstream `lore::file::obliterate` in-process and collects
/// `FileObliterate` events to return typed results.
pub async fn obliterate(api: &LoreApi, args: FileObliterateArgs) -> Result<FileObliterateResult> {
    let (callback, rx) = collect_events();

    let status = lore::file::obliterate(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file obliterate failed with status {status}"),
        )));
    }

    let obliterated: Vec<FileObliterateEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::FileObliterate(data) = event {
                Some(FileObliterateEntry {
                    address: format!("{}", data.address),
                    num_fragments: data.num_fragments,
                    num_payloads: data.num_payloads,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(FileObliterateResult { obliterated })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_obliterate_args_serializes() {
        let args = FileObliterateArgs {
            address: String::new(),
            path: "foo/bar.txt".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("foo/bar.txt"));
    }

    #[test]
    fn file_obliterate_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: FileObliterateArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.address.is_empty());
        assert!(args.path.is_empty());
    }

    #[test]
    fn file_obliterate_args_deserializes_with_path() {
        let json = r#"{"path":"test.txt"}"#;
        let args: FileObliterateArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.path, "test.txt");
        assert!(args.address.is_empty());
    }

    #[test]
    fn file_obliterate_args_deserializes_with_address() {
        let json = r#"{"address":"abc123"}"#;
        let args: FileObliterateArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.address, "abc123");
        assert!(args.path.is_empty());
    }

    #[test]
    fn file_obliterate_args_into_lore_conversion() {
        let args = FileObliterateArgs {
            address: String::new(),
            path: "hello.md".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.path.as_str(), "hello.md");
        assert!(lore_args.address.is_empty());
    }

    #[test]
    fn file_obliterate_result_serializes() {
        let result = FileObliterateResult {
            obliterated: vec![FileObliterateEntry {
                address: "abc123".into(),
                num_fragments: 3,
                num_payloads: 1,
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123"));
        assert!(json.contains("3"));
    }

    #[test]
    fn file_obliterate_result_empty() {
        let result = FileObliterateResult {
            obliterated: vec![],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""obliterated":[]"#));
    }
}
