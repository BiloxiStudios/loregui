//! `file metadata_get` operation — binds `lore::file::metadata_get`.
//!
//! Retrieves a single metadata value for a file by key at a given revision.
//! Returns the key, value, and type of the requested metadata entry.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileMetadataGetArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`metadata_get`].
///
/// Mirrors `LoreFileMetadataGetArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataGetArgs {
    /// Path to the file to get metadata for.
    pub path: String,
    /// Metadata key to retrieve.
    pub key: String,
    /// Revision to get metadata for; empty string uses current revision.
    #[serde(default)]
    pub revision: String,
}

impl MetadataGetArgs {
    fn into_lore(self) -> LoreFileMetadataGetArgs {
        LoreFileMetadataGetArgs {
            path: LoreString::from_str(&self.path),
            key: LoreString::from_str(&self.key),
            revision: LoreString::from_str(&self.revision),
        }
    }
}

/// The type of a metadata value.
///
/// These are the runtime types that can be stored in file metadata.
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

/// Result returned on successful metadata get operation.
///
/// Contains the requested metadata entry for the file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataGetResult {
    /// The metadata key (matches the request).
    pub key: String,
    /// The metadata value as a string representation.
    pub value: String,
    /// The type of the metadata value.
    #[serde(rename = "type")]
    pub entry_type: MetadataEntryType,
}

/// Get a single metadata value for a file by key.
///
/// Calls the upstream `lore::file::metadata_get` in-process and collects
/// the `Metadata` event to return a typed result.
///
/// # Example
///
/// ```ignore
/// let result = metadata_get(&api, MetadataGetArgs {
///     path: "src/main.rs".into(),
///     key: "author".into(),
///     revision: String::new(), // use current revision
/// }).await?;
/// println!("{}: {} ({:?})", result.key, result.value, result.entry_type);
/// ```
pub async fn metadata_get(
    api: &LoreApi,
    args: MetadataGetArgs,
) -> Result<MetadataGetResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::file::metadata_get(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("metadata_get failed with status {status}"),
        )));
    }

    // Extract the single Metadata event from the stream
    let (key, value, entry_type) = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::Metadata(data) = event {
                let key = data.key.as_str().to_string();
                let (value, entry_type) = convert_lore_metadata(&data.value).ok()?;
                Some((key, value, entry_type))
            } else {
                None
            }
        })
        .ok_or_else(|| LoreError::Parse("metadata_get succeeded but no Metadata event emitted".into()))?;

    Ok(MetadataGetResult {
        key,
        value,
        entry_type,
    })
}

/// Convert a `LoreMetadata` value to a string representation and its type.
///
/// This extracts the actual value from the FFI wrapper types and determines
/// the appropriate type tag for the entry.
fn convert_lore_metadata(value: &lore::interface::LoreMetadata) -> Result<(String, MetadataEntryType)> {
    use lore::interface::LoreMetadata;
    match value {
        LoreMetadata::Address(addr) => {
            let value_str = addr.to_string();
            Ok((value_str, MetadataEntryType::Address))
        }
        LoreMetadata::Boolean(b) => {
            let value_str = if *b != 0 { "true" } else { "false" };
            Ok((value_str.to_string(), MetadataEntryType::Boolean))
        }
        LoreMetadata::Binary(bin) => {
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
            let value_str = ctx.to_string();
            Ok((value_str, MetadataEntryType::Context))
        }
        LoreMetadata::Hash(hash) => {
            let value_str = hash.to_string();
            Ok((value_str, MetadataEntryType::Hash))
        }
        LoreMetadata::Numeric(n) => {
            let value_str = n.to_string();
            Ok((value_str, MetadataEntryType::Numeric))
        }
        LoreMetadata::String(s) => {
            let value_str = s.as_str().to_string();
            Ok((value_str, MetadataEntryType::String))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_get_args_serializes() {
        let args = MetadataGetArgs {
            path: "src/main.rs".to_string(),
            key: "author".to_string(),
            revision: String::new(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("author"));
    }

    #[test]
    fn metadata_get_args_with_revision() {
        let args = MetadataGetArgs {
            path: "file.txt".to_string(),
            key: "priority".to_string(),
            revision: "abc123".to_string(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("file.txt"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn metadata_get_args_deserializes_with_defaults() {
        let json = r#"{"path":"README.md","key":"title"}"#;
        let args: MetadataGetArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.path, "README.md");
        assert_eq!(args.key, "title");
        assert_eq!(args.revision, "");
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
    fn metadata_get_result_serializes() {
        let result = MetadataGetResult {
            key: "author".to_string(),
            value: "alice".to_string(),
            entry_type: MetadataEntryType::String,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("author"));
        assert!(json.contains("alice"));
    }
}
