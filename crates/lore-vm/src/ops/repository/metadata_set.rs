//! `repository metadata_set` operation — binds `lore::repository::metadata_set`.
//!
//! Sets one or more metadata key-value pairs on the current repository.
//! Use `metadata_get` to read keys and `metadata_clear` to remove them.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreMetadataType, LoreString};
use lore::repository::LoreRepositoryMetadataSetArgs;
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
/// The `keys`, `values`, and `formats` arrays must be parallel (same length).
/// If `formats` is shorter than `keys`, missing entries default to `String`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetadataSetArgs {
    /// Metadata keys to set.
    pub keys: Vec<String>,
    /// Values to set, one per key.
    pub values: Vec<String>,
    /// Value format/type for each key-value pair.
    #[serde(default)]
    pub formats: Vec<MetadataFormat>,
}

impl RepositoryMetadataSetArgs {
    fn into_lore(self) -> LoreRepositoryMetadataSetArgs {
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

        LoreRepositoryMetadataSetArgs {
            keys: LoreArray::from_vec(
                self.keys
                    .into_iter()
                    .map(|k| LoreString::from_str(&k))
                    .collect(),
            ),
            values: LoreArray::from_vec(
                self.values
                    .into_iter()
                    .map(|v| LoreString::from_str(&v))
                    .collect(),
            ),
            formats: LoreArray::from_vec(formats),
        }
    }
}

/// Result returned on successful metadata set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetadataSetResult {
    /// The keys that were set.
    pub keys: Vec<String>,
    /// The values that were set (parallel with `keys`).
    pub values: Vec<String>,
}

/// Set one or more metadata key-value pairs on the current repository.
///
/// Calls the upstream `lore::repository::metadata_set` in-process and returns
/// a typed result indicating which keys were set.
pub async fn metadata_set(
    api: &LoreApi,
    args: RepositoryMetadataSetArgs,
) -> Result<RepositoryMetadataSetResult> {
    let keys = args.keys.clone();
    let values = args.values.clone();

    let (callback, rx) = collect_events();

    let status =
        lore::repository::metadata_set(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository metadata_set failed with status {status}"),
        )));
    }

    Ok(RepositoryMetadataSetResult { keys, values })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = RepositoryMetadataSetArgs {
            keys: vec!["description".into()],
            values: vec!["test repo".into()],
            formats: vec![MetadataFormat::String],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("description"));
        assert!(json.contains("test repo"));
    }

    #[test]
    fn args_deserializes_with_default_formats() {
        let json = r#"{"keys":["k"],"values":["v"]}"#;
        let args: RepositoryMetadataSetArgs =
            serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.keys, vec!["k"]);
        assert_eq!(args.values, vec!["v"]);
        assert!(args.formats.is_empty());
    }

    #[test]
    fn args_into_lore_pads_formats() {
        let args = RepositoryMetadataSetArgs {
            keys: vec!["a".into(), "b".into()],
            values: vec!["1".into(), "2".into()],
            formats: vec![],
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.keys.as_slice().len(), 2);
        assert_eq!(lore_args.values.as_slice().len(), 2);
        assert_eq!(lore_args.formats.as_slice().len(), 2);
        assert_eq!(lore_args.formats.as_slice()[0], LoreMetadataType::String);
    }

    #[test]
    fn metadata_format_serde() {
        assert_eq!(
            serde_json::to_string(&MetadataFormat::Binary).unwrap(),
            "\"binary\""
        );
        assert_eq!(
            serde_json::to_string(&MetadataFormat::Numeric).unwrap(),
            "\"numeric\""
        );
        assert_eq!(
            serde_json::to_string(&MetadataFormat::String).unwrap(),
            "\"string\""
        );

        let f: MetadataFormat = serde_json::from_str("\"numeric\"").unwrap();
        assert_eq!(f, MetadataFormat::Numeric);
    }

    #[test]
    fn result_serializes() {
        let result = RepositoryMetadataSetResult {
            keys: vec!["version".into()],
            values: vec!["1.0".into()],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("version"));
        assert!(json.contains("1.0"));
    }

    #[test]
    fn serde_roundtrip() {
        let args = RepositoryMetadataSetArgs {
            keys: vec!["tag".into(), "owner".into()],
            values: vec!["v2".into(), "alice".into()],
            formats: vec![MetadataFormat::String, MetadataFormat::String],
        };
        let json = serde_json::to_string(&args).expect("serialize");
        let deser: RepositoryMetadataSetArgs =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.keys, vec!["tag", "owner"]);
        assert_eq!(deser.values, vec!["v2", "alice"]);
        assert_eq!(
            deser.formats,
            vec![MetadataFormat::String, MetadataFormat::String]
        );
    }
}
