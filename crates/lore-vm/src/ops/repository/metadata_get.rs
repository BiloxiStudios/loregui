//! `repository metadata_get` operation — binds `lore::repository::metadata_get`.
//!
//! Retrieves repository-level metadata. If `key` is non-empty, returns that
//! single key's value. If `key` is empty, returns all metadata entries.
//! Each entry is delivered as a `LoreEvent::Metadata` event.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::repository::LoreRepositoryMetadataGetArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_get`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetadataGetArgs {
    /// Metadata key to fetch; empty string returns all entries.
    #[serde(default)]
    pub key: String,
}

impl RepositoryMetadataGetArgs {
    fn into_lore(self) -> LoreRepositoryMetadataGetArgs {
        LoreRepositoryMetadataGetArgs {
            key: LoreString::from_str(&self.key),
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

/// Result of a successful `metadata_get` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetadataGetResult {
    /// The retrieved metadata entries.
    pub entries: Vec<MetadataEntry>,
}

fn format_metadata_value(value: &lore::interface::LoreMetadata) -> (String, &'static str) {
    use lore::interface::LoreMetadata;
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

/// Retrieve repository metadata.
///
/// Calls upstream `lore::repository::metadata_get` in-process, collects
/// `Metadata` events, and returns typed key-value entries.
pub async fn metadata_get(
    api: &LoreApi,
    args: RepositoryMetadataGetArgs,
) -> Result<RepositoryMetadataGetResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::repository::metadata_get(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository metadata_get failed with status {status}"),
        )));
    }

    let mut entries = Vec::new();

    for event in &stream.events {
        if let LoreEvent::Metadata(data) = event {
            let (value, value_type) = format_metadata_value(&data.value);
            entries.push(MetadataEntry {
                key: data.key.as_str().to_string(),
                value,
                value_type: value_type.into(),
            });
        }
    }

    Ok(RepositoryMetadataGetResult { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = RepositoryMetadataGetArgs {
            key: "my.key".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("my.key"));
    }

    #[test]
    fn args_deserializes_with_default() {
        let args: RepositoryMetadataGetArgs =
            serde_json::from_str("{}").expect("should deserialize");
        assert_eq!(args.key, "");
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = RepositoryMetadataGetArgs {
            key: "test.key".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.key.as_str(), "test.key");
    }

    #[test]
    fn result_serializes() {
        let result = RepositoryMetadataGetResult {
            entries: vec![MetadataEntry {
                key: "version".into(),
                value: "1.0".into(),
                value_type: "string".into(),
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("version"));
        assert!(json.contains("1.0"));
        assert!(json.contains("string"));
    }

    #[test]
    fn empty_result_serializes() {
        let result = RepositoryMetadataGetResult { entries: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }

    #[test]
    fn multiple_entries_serialize() {
        let result = RepositoryMetadataGetResult {
            entries: vec![
                MetadataEntry {
                    key: "k1".into(),
                    value: "v1".into(),
                    value_type: "string".into(),
                },
                MetadataEntry {
                    key: "k2".into(),
                    value: "42".into(),
                    value_type: "numeric".into(),
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("k1"));
        assert!(json.contains("k2"));
        assert!(json.contains("42"));
    }
}
