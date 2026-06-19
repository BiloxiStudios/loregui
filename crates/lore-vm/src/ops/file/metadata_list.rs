//! `file metadata_list` operation — binds `lore::file::metadata_list`.
//!
//! Lists all metadata key/value pairs associated with a file at a given revision.
//! Returns a vector of metadata entries, each containing a key, value, and type.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileMetadataListArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_list`].
///
/// Mirrors `LoreFileMetadataListArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataListArgs {
    /// Path to the file to list metadata for.
    pub path: String,
    /// Revision to list metadata for; empty string uses current revision.
    #[serde(default)]
    pub revision: String,
}

impl MetadataListArgs {
    fn into_lore(self) -> LoreFileMetadataListArgs {
        LoreFileMetadataListArgs {
            path: LoreString::from_str(&self.path),
            revision: LoreString::from_str(&self.revision),
        }
    }
}

/// A single metadata entry returned by [`metadata_list`].
///
/// Represents one key/value pair from file metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEntry {
    /// The metadata key.
    pub key: String,
    /// The metadata value as a string representation.
    pub value: String,
    /// The type of the metadata value.
    #[serde(rename = "type")]
    pub entry_type: MetadataEntryType,
}

/// The type of a metadata entry value.
///
/// These are the runtime types that can be stored in file metadata.
/// Note: This is a superset of `MetadataType` used in `metadata_set`,
/// which only includes Binary/Numeric/String. The lore crate can store
/// additional types like Address, Context, Hash which have specific
/// semantics but serialize to strings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MetadataEntryType {
    /// An address value (content identifier hash).
    Address,
    /// A boolean value.
    Boolean,
    /// A block of raw bytes (base64-encoded in the value field).
    Binary,
    /// A context identifier.
    Context,
    /// A hash value.
    Hash,
    /// An unsigned integer value.
    Numeric,
    /// A string value.
    String,
}

/// Result returned on successful metadata list operation.
///
/// Contains all metadata entries for the requested file at the given revision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataListResult {
    /// All metadata entries found for the file.
    pub entries: Vec<MetadataEntry>,
}

/// List all metadata key/value pairs associated with a file.
///
/// Calls the upstream `lore::file::metadata_list` in-process and collects
/// `Metadata` events to build the result vector.
///
/// # Example
///
/// ```ignore
/// let result = metadata_list(&api, MetadataListArgs {
///     path: "src/main.rs".into(),
///     revision: String::new(), // use current revision
/// }).await?;
/// for entry in result.entries {
///     println!("{}: {} ({:?})", entry.key, entry.value, entry.entry_type);
/// }
/// ```
pub async fn metadata_list(
    api: &LoreApi,
    args: MetadataListArgs,
) -> Result<MetadataListResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::file::metadata_list(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("metadata_list failed with status {status}"),
        )));
    }

    // Extract Metadata events from the stream
    let mut entries = Vec::new();
    for event in &stream.events {
        if let LoreEvent::Metadata(data) = event {
            let key = data.key.as_str().to_string();
            let (value, entry_type) = convert_lore_metadata(&data.value)?;
            entries.push(MetadataEntry {
                key,
                value,
                entry_type,
            });
        }
    }

    Ok(MetadataListResult { entries })
}

/// Convert a `LoreMetadata` value to a string representation and its type.
///
/// This extracts the actual value from the FFI wrapper types and determines
/// the appropriate type tag for the entry.
fn convert_lore_metadata(value: &lore::interface::LoreMetadata) -> Result<(String, MetadataEntryType)> {
    use lore::interface::LoreMetadata;
    match value {
        LoreMetadata::Address(addr) => {
            // Address implements Display, format as "hash-context"
            let value_str = addr.to_string();
            Ok((value_str, MetadataEntryType::Address))
        }
        LoreMetadata::Boolean(b) => {
            // Boolean is stored as u8, convert to string
            let value_str = if *b != 0 { "true" } else { "false" };
            Ok((value_str.to_string(), MetadataEntryType::Boolean))
        }
        LoreMetadata::Binary(bin) => {
            // Binary data - serialize as JSON byte array for the string representation
            // LoreBinary has payload: *const c_void and length: usize
            let bytes = unsafe { std::slice::from_raw_parts(bin.payload.cast::<u8>(), bin.length) };
            let json_bytes = serde_json::to_vec(bytes).map_err(|e| {
                LoreError::Parse(format!("failed to serialize binary metadata: {e}"))
            })?;
            let value_str = String::from_utf8(json_bytes).map_err(|e| {
                LoreError::Parse(format!("failed to convert binary json to string: {e}"))
            })?;
            Ok((value_str, MetadataEntryType::Binary))
        }
        LoreMetadata::Context(ctx) => {
            // Context implements Display
            let value_str = ctx.to_string();
            Ok((value_str, MetadataEntryType::Context))
        }
        LoreMetadata::Hash(hash) => {
            // Hash implements Display
            let value_str = hash.to_string();
            Ok((value_str, MetadataEntryType::Hash))
        }
        LoreMetadata::Numeric(n) => {
            // Numeric is u64, convert to string
            let value_str = n.to_string();
            Ok((value_str, MetadataEntryType::Numeric))
        }
        LoreMetadata::String(s) => {
            // String is direct
            let value_str = s.as_str().to_string();
            Ok((value_str, MetadataEntryType::String))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_list_args_serializes() {
        let args = MetadataListArgs {
            path: "src/main.rs".to_string(),
            revision: "abc123".to_string(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn metadata_list_args_with_empty_revision() {
        let args = MetadataListArgs {
            path: "file.txt".to_string(),
            revision: String::new(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("file.txt"));
        // Empty revision should serialize to empty string
        assert!(json.contains("\"revision\":\"\""));
    }

    #[test]
    fn metadata_entry_serializes() {
        let entry = MetadataEntry {
            key: "author".to_string(),
            value: "alice".to_string(),
            entry_type: MetadataEntryType::String,
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("author"));
        assert!(json.contains("alice"));
        assert!(json.contains("string"));
    }

    #[test]
    fn metadata_entry_type_serializes_lowercase() {
        let json = serde_json::to_string(&MetadataEntryType::String).expect("should serialize");
        assert_eq!(json, r#""string""#);
    }

    #[test]
    fn metadata_entry_numeric_type() {
        let json = serde_json::to_string(&MetadataEntryType::Numeric).expect("should serialize");
        assert_eq!(json, r#""numeric""#);
    }

    #[test]
    fn metadata_entry_boolean_type() {
        let json = serde_json::to_string(&MetadataEntryType::Boolean).expect("should serialize");
        assert_eq!(json, r#""boolean""#);
    }

    #[test]
    fn metadata_entry_address_type() {
        let json = serde_json::to_string(&MetadataEntryType::Address).expect("should serialize");
        assert_eq!(json, r#""address""#);
    }

    #[test]
    fn metadata_entry_context_type() {
        let json = serde_json::to_string(&MetadataEntryType::Context).expect("should serialize");
        assert_eq!(json, r#""context""#);
    }

    #[test]
    fn metadata_entry_hash_type() {
        let json = serde_json::to_string(&MetadataEntryType::Hash).expect("should serialize");
        assert_eq!(json, r#""hash""#);
    }

    #[test]
    fn metadata_entry_binary_type() {
        let json = serde_json::to_string(&MetadataEntryType::Binary).expect("should serialize");
        assert_eq!(json, r#""binary""#);
    }

    #[test]
    fn metadata_list_result_empty() {
        let result = MetadataListResult { entries: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }

    #[test]
    fn metadata_list_result_with_entries() {
        let result = MetadataListResult {
            entries: vec![
                MetadataEntry {
                    key: "author".to_string(),
                    value: "alice".to_string(),
                    entry_type: MetadataEntryType::String,
                },
                MetadataEntry {
                    key: "priority".to_string(),
                    value: "1".to_string(),
                    entry_type: MetadataEntryType::Numeric,
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("author"));
        assert!(json.contains("priority"));
    }
}
