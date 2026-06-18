//! `repository metadata_set` operation — binds `lore::repository::metadata_set`.
//!
//! Sets one or more metadata key-value pairs on the current repository.
//! Each entry is a (key, value, format) triple; the three arrays must be the
//! same length.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreMetadataType, LoreString};
use lore::repository::LoreRepositoryMetadataSetArgs;
use serde::{Deserialize, Serialize};

/// Value format/type for a metadata entry.
///
/// Mirrors [`LoreMetadataType`] from the upstream `lore` crate but uses a
/// Serde-friendly enum so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
/// Mirrors `LoreRepositoryMetadataSetArgs` from the upstream `lore` crate but
/// uses plain `String` / `MetadataFormat` so it serialises cleanly across the
/// Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSetArgs {
    /// Metadata keys to set.
    pub keys: Vec<String>,
    /// Values to set, one per key.
    pub values: Vec<String>,
    /// Value format/type for each key-value pair.
    /// Defaults to `String` for every entry when omitted.
    #[serde(default = "default_formats")]
    pub formats: Vec<MetadataFormat>,
}

fn default_formats() -> Vec<MetadataFormat> {
    Vec::new()
}

impl MetadataSetArgs {
    fn into_lore(self) -> LoreRepositoryMetadataSetArgs {
        let formats: Vec<LoreMetadataType> = if self.formats.is_empty() {
            vec![LoreMetadataType::String; self.keys.len()]
        } else {
            self.formats.into_iter().map(Into::into).collect()
        };

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
pub struct MetadataSetResult {
    /// The keys that were set.
    pub keys: Vec<String>,
    /// The values that were set (echo back for confirmation).
    pub values: Vec<String>,
}

/// Set one or more metadata key-value pairs on the current repository.
///
/// Calls the upstream `lore::repository::metadata_set` in-process and returns
/// a typed result echoing the keys and values that were written.
pub async fn metadata_set(api: &LoreApi, args: MetadataSetArgs) -> Result<MetadataSetResult> {
    if args.keys.len() != args.values.len() {
        return Err(LoreError::Parse(format!(
            "keys.len ({}) != values.len ({})",
            args.keys.len(),
            args.values.len()
        )));
    }
    if !args.formats.is_empty() && args.formats.len() != args.keys.len() {
        return Err(LoreError::Parse(format!(
            "formats.len ({}) != keys.len ({})",
            args.formats.len(),
            args.keys.len()
        )));
    }

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
            || format!("metadata_set failed with status {status}"),
        )));
    }

    Ok(MetadataSetResult { keys, values })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_set_args_construction() {
        let args = MetadataSetArgs {
            keys: vec!["author".into(), "version".into()],
            values: vec!["Alice".into(), "1.0".into()],
            formats: vec![MetadataFormat::String, MetadataFormat::String],
        };

        assert_eq!(args.keys.len(), 2);
        assert_eq!(args.values.len(), 2);
        assert_eq!(args.formats.len(), 2);
        assert_eq!(args.keys[0], "author");
        assert_eq!(args.values[0], "Alice");
        assert_eq!(args.formats[0], MetadataFormat::String);
    }

    #[test]
    fn test_metadata_format_conversion() {
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

    #[test]
    fn test_default_formats_empty() {
        let args: MetadataSetArgs = serde_json::from_str(
            r#"{"keys":["k1"],"values":["v1"]}"#,
        )
        .unwrap();
        assert!(args.formats.is_empty());
    }

    #[test]
    fn test_into_lore_fills_default_formats() {
        let args = MetadataSetArgs {
            keys: vec!["k1".into(), "k2".into()],
            values: vec!["v1".into(), "v2".into()],
            formats: vec![],
        };
        let lore_args = args.into_lore();
        let fmts = lore_args.formats.as_slice();
        assert_eq!(fmts.len(), 2);
        assert_eq!(fmts[0], LoreMetadataType::String);
        assert_eq!(fmts[1], LoreMetadataType::String);
    }

    #[tokio::test]
    async fn test_metadata_set_mismatched_lengths() {
        let api = LoreApi::new(std::path::PathBuf::from("/tmp/nonexistent"));
        let args = MetadataSetArgs {
            keys: vec!["k1".into()],
            values: vec!["v1".into(), "v2".into()],
            formats: vec![],
        };
        let err = metadata_set(&api, args).await.unwrap_err();
        assert!(matches!(err, LoreError::Parse(_)));
    }
}
