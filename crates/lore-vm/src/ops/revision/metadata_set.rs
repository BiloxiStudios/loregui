//! `revision metadata_set` operation — binds `lore::revision::metadata_set`.
//!
//! Sets one or more key-value metadata pairs on the current revision (staged or
//! committed). Use `metadata_get` to read keys and `metadata_clear` to remove them.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreMetadataType, LoreString};
use lore::revision::LoreRevisionMetadataSetArgs;
use serde::{Deserialize, Serialize};

/// Metadata value type — mirrors `LoreMetadataType` but serialises cleanly
/// across the Tauri boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetadataFormat {
    Binary,
    Numeric,
    String,
}

impl From<MetadataFormat> for LoreMetadataType {
    fn from(f: MetadataFormat) -> Self {
        match f {
            MetadataFormat::Binary => LoreMetadataType::Binary,
            MetadataFormat::Numeric => LoreMetadataType::Numeric,
            MetadataFormat::String => LoreMetadataType::String,
        }
    }
}

/// Arguments for [`metadata_set`].
///
/// Mirrors `LoreRevisionMetadataSetArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
/// The `keys`, `values`, and `formats` arrays must be parallel (same length).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSetArgs {
    /// Metadata keys to set (e.g., "description", "change_request").
    pub keys: Vec<String>,
    /// Values to set, one per key.
    pub values: Vec<String>,
    /// Value type for each key, one per key. Defaults to String for each
    /// entry if omitted or shorter than `keys`.
    #[serde(default)]
    pub formats: Vec<MetadataFormat>,
}

impl MetadataSetArgs {
    fn into_lore(self) -> LoreRevisionMetadataSetArgs {
        // If formats is shorter than keys, pad with String (the common case).
        let formats: Vec<LoreMetadataType> = self
            .keys
            .iter()
            .enumerate()
            .map(|(i, _)| {
                self.formats
                    .get(i)
                    .copied()
                    .unwrap_or(MetadataFormat::String)
                    .into()
            })
            .collect();

        LoreRevisionMetadataSetArgs {
            keys: lore::interface::LoreArray::from_vec(
                self.keys
                    .into_iter()
                    .map(|k| LoreString::from_str(&k))
                    .collect(),
            ),
            values: lore::interface::LoreArray::from_vec(
                self.values
                    .into_iter()
                    .map(|v| LoreString::from_str(&v))
                    .collect(),
            ),
            formats: lore::interface::LoreArray::from_vec(formats),
        }
    }
}

/// Result returned on successful metadata set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSetResult {
    /// The keys that were set.
    pub keys: Vec<String>,
    /// The values that were set (parallel with `keys`).
    pub values: Vec<String>,
}

/// Set metadata key-value pairs on the current revision.
///
/// Calls the upstream `lore::revision::metadata_set` in-process and returns
/// a typed result indicating which keys were set.
pub async fn metadata_set(api: &LoreApi, args: MetadataSetArgs) -> Result<MetadataSetResult> {
    let keys = args.keys.clone();
    let values = args.values.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::revision::metadata_set(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("metadata_set failed with status {status}"),
        )));
    }

    Ok(MetadataSetResult { keys, values })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_set_args_serializes() {
        let args = MetadataSetArgs {
            keys: vec!["change_request".into(), "reviewed_by".into()],
            values: vec!["CR-1234".into(), "alice".into()],
            formats: vec![MetadataFormat::String, MetadataFormat::String],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("change_request"));
        assert!(json.contains("CR-1234"));
    }

    #[test]
    fn metadata_set_args_into_lore_conversion() {
        let args = MetadataSetArgs {
            keys: vec!["key1".into()],
            values: vec!["value1".into()],
            formats: vec![],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.keys.as_slice().len(), 1);
        assert_eq!(lore_args.values.as_slice().len(), 1);
        // Empty formats should pad to String
        assert_eq!(lore_args.formats.as_slice().len(), 1);
    }

    #[test]
    fn metadata_format_converts_to_lore() {
        assert_eq!(
            LoreMetadataType::from(MetadataFormat::Binary),
            LoreMetadataType::Binary
        );
        assert_eq!(
            LoreMetadataType::from(MetadataFormat::Numeric),
            LoreMetadataType::Numeric
        );
        assert_eq!(
            LoreMetadataType::from(MetadataFormat::String),
            LoreMetadataType::String
        );
    }
}
