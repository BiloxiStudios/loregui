//! `revision metadata_get` operation — binds `lore::revision::metadata_get`.
//!
//! Retrieves a single metadata value by key from a revision. If `key` is
//! non-empty, returns that key's value. The result is delivered as a
//! `LoreEvent::Metadata` event.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreMetadata, LoreString};
use lore::revision::LoreRevisionMetadataGetArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_get`].
///
/// Mirrors `LoreRevisionMetadataGetArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionMetadataGetArgs {
    /// Metadata key to look up.
    pub key: String,
    /// Revision to query; empty string for current HEAD.
    #[serde(default)]
    pub revision: String,
}

impl RevisionMetadataGetArgs {
    fn into_lore(self) -> LoreRevisionMetadataGetArgs {
        LoreRevisionMetadataGetArgs {
            key: LoreString::from_str(&self.key),
            revision: LoreString::from_str(&self.revision),
        }
    }
}

/// A single metadata key-value pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEntry {
    /// The metadata key.
    pub key: String,
    /// The metadata value rendered as a string.
    pub value: String,
    /// The type of the metadata value (e.g. "string", "numeric", "boolean",
    /// "hash", "address", "context", "binary").
    pub value_type: String,
}

/// Result of a successful `revision metadata_get` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionMetadataGetResult {
    /// The retrieved metadata entries (typically one for a single-key lookup).
    pub entries: Vec<MetadataEntry>,
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

/// Retrieve metadata for a specific key from a revision.
///
/// Calls upstream `lore::revision::metadata_get` in-process, collects
/// `Metadata` events, and returns typed key-value entries.
pub async fn metadata_get(
    api: &LoreApi,
    args: RevisionMetadataGetArgs,
) -> Result<RevisionMetadataGetResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::revision::metadata_get(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision metadata_get failed with status {status}"),
        )));
    }

    let entries: Vec<MetadataEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::Metadata(data) = event {
                let (value, value_type) = format_metadata_value(&data.value);
                Some(MetadataEntry {
                    key: data.key.as_str().to_string(),
                    value,
                    value_type: value_type.into(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(RevisionMetadataGetResult { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = RevisionMetadataGetArgs {
            key: "change-request".into(),
            revision: "abc123".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("change-request"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn args_deserializes_with_default_revision() {
        let args: RevisionMetadataGetArgs =
            serde_json::from_str(r#"{"key":"my.key"}"#).expect("should deserialize");
        assert_eq!(args.key, "my.key");
        assert_eq!(args.revision, "");
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = RevisionMetadataGetArgs {
            key: "test.key".into(),
            revision: "deadbeef".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.key.as_str(), "test.key");
        assert_eq!(lore_args.revision.as_str(), "deadbeef");
    }

    #[test]
    fn result_serializes() {
        let result = RevisionMetadataGetResult {
            entries: vec![MetadataEntry {
                key: "change-request".into(),
                value: "CR-1234".into(),
                value_type: "string".into(),
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("change-request"));
        assert!(json.contains("CR-1234"));
        assert!(json.contains("string"));
    }

    #[test]
    fn empty_result_serializes() {
        let result = RevisionMetadataGetResult { entries: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }

    #[test]
    fn multiple_entries_serialize() {
        let result = RevisionMetadataGetResult {
            entries: vec![
                MetadataEntry {
                    key: "author".into(),
                    value: "alice".into(),
                    value_type: "string".into(),
                },
                MetadataEntry {
                    key: "priority".into(),
                    value: "42".into(),
                    value_type: "numeric".into(),
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("author"));
        assert!(json.contains("priority"));
        assert!(json.contains("42"));
    }
}
