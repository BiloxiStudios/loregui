//! `revision metadata_list` operation — binds `lore::revision::metadata_list`.
//!
//! Lists revision-level metadata. Emits one `LoreEvent::Metadata` per entry.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreMetadata, LoreString};
use lore::revision::LoreRevisionMetadataListArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_list`].
///
/// Mirrors `LoreRevisionMetadataListArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataListArgs {
    /// Revision hash to query; empty for current HEAD.
    #[serde(default)]
    pub revision: String,
}

impl MetadataListArgs {
    fn into_lore(self) -> LoreRevisionMetadataListArgs {
        LoreRevisionMetadataListArgs {
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

/// Result returned on successful metadata list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataListResult {
    /// The metadata entries for the revision.
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

/// List revision metadata.
///
/// Calls the upstream `lore::revision::metadata_list` in-process and collects
/// `Metadata` events into a typed result.
pub async fn metadata_list(api: &LoreApi, args: MetadataListArgs) -> Result<MetadataListResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::revision::metadata_list(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision metadata_list failed with status {status}"),
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

    Ok(MetadataListResult { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_list_args_defaults() {
        let json = r#"{}"#;
        let args: MetadataListArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.revision, "");
    }

    #[test]
    fn metadata_list_args_into_lore_conversion() {
        let args = MetadataListArgs {
            revision: "abc123".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision.as_str(), "abc123");
    }

    #[test]
    fn metadata_list_result_serializes() {
        let result = MetadataListResult {
            entries: vec![MetadataEntry {
                key: "author".into(),
                value: "test".into(),
                value_type: "string".into(),
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("author"));
        assert!(json.contains("test"));
    }

    #[test]
    fn empty_metadata_list_serializes() {
        let result = MetadataListResult { entries: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }
}
