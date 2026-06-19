//! `branch metadata_get` operation — binds `lore::branch::metadata_get`.
//!
//! Retrieves metadata from a branch. If `key` is non-empty, returns that
//! single key's value. If `key` is empty, returns all metadata entries.
//! Each entry is delivered as a `LoreEvent::Metadata` event.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchMetadataGetArgs;
use lore::interface::{LoreEvent, LoreMetadata, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_get`].
///
/// Mirrors `LoreBranchMetadataGetArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMetadataGetArgs {
    /// Branch name; empty string uses the current branch.
    #[serde(default)]
    pub branch: String,
    /// Metadata key to fetch; empty string returns all entries.
    #[serde(default)]
    pub key: String,
}

impl BranchMetadataGetArgs {
    fn into_lore(self) -> LoreBranchMetadataGetArgs {
        LoreBranchMetadataGetArgs {
            branch: LoreString::from_str(&self.branch),
            key: LoreString::from_str(&self.key),
        }
    }
}

/// A single metadata key-value pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMetadataEntry {
    /// The metadata key.
    pub key: String,
    /// The metadata value rendered as a string.
    pub value: String,
    /// The type of the metadata value (e.g. "string", "numeric", "boolean",
    /// "hash", "address", "context", "binary").
    pub value_type: String,
}

/// Result of a successful `metadata_get` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMetadataGetResult {
    /// The branch whose metadata was queried.
    pub branch: String,
    /// The retrieved metadata entries.
    pub entries: Vec<BranchMetadataEntry>,
}

fn format_metadata_value(value: &LoreMetadata) -> (String, &'static str) {
    match value {
        LoreMetadata::String(s) => (s.as_str().to_string(), "string"),
        LoreMetadata::Numeric(n) => (n.to_string(), "numeric"),
        LoreMetadata::Boolean(b) => (if *b != 0 { "true" } else { "false" }.into(), "boolean"),
        LoreMetadata::Hash(h) => (format!("{h}"), "hash"),
        LoreMetadata::Address(a) => (format!("{a}"), "address"),
        LoreMetadata::Context(c) => (format!("{c}"), "context"),
        LoreMetadata::Binary(_) => ("<binary>".into(), "binary"),
    }
}

/// Retrieve branch metadata.
///
/// Calls upstream `lore::branch::metadata_get` in-process, collects
/// `Metadata` events, and returns typed key-value entries.
pub async fn metadata_get(
    api: &LoreApi,
    args: BranchMetadataGetArgs,
) -> Result<BranchMetadataGetResult> {
    let branch = args.branch.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::branch::metadata_get(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch metadata_get failed with status {status}"),
        )));
    }

    let mut entries = Vec::new();

    for event in &stream.events {
        if let LoreEvent::Metadata(data) = event {
            let (value, value_type) = format_metadata_value(&data.value);
            entries.push(BranchMetadataEntry {
                key: data.key.as_str().to_string(),
                value,
                value_type: value_type.into(),
            });
        }
    }

    Ok(BranchMetadataGetResult { branch, entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = BranchMetadataGetArgs {
            branch: "main".into(),
            key: "description".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("main"));
        assert!(json.contains("description"));
    }

    #[test]
    fn args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: BranchMetadataGetArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.branch, "");
        assert_eq!(args.key, "");
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = BranchMetadataGetArgs {
            branch: "feature/test".into(),
            key: "owner".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.branch.as_str(), "feature/test");
        assert_eq!(lore_args.key.as_str(), "owner");
    }

    #[test]
    fn result_serializes() {
        let result = BranchMetadataGetResult {
            branch: "main".into(),
            entries: vec![
                BranchMetadataEntry {
                    key: "description".into(),
                    value: "Main branch".into(),
                    value_type: "string".into(),
                },
                BranchMetadataEntry {
                    key: "priority".into(),
                    value: "42".into(),
                    value_type: "numeric".into(),
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("main"));
        assert!(json.contains("description"));
        assert!(json.contains("Main branch"));
        assert!(json.contains("42"));
    }

    #[test]
    fn result_empty_entries() {
        let result = BranchMetadataGetResult {
            branch: "dev".into(),
            entries: vec![],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""entries":[]"#));
    }
}
