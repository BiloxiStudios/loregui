//! `file metadata_set` operation — binds `lore::file::metadata_set`.
//!
//! Sets metadata key/value pairs on one or more files in the repository.
//! Each path can have multiple metadata entries. The operation accepts
//! parallel arrays of paths, keys, values, and formats along with an entries
//! array specifying how many metadata entries apply to each path.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileMetadataSetArgs;
use lore::interface::{LoreArray, LoreMetadataType, LoreString};
use serde::{Deserialize, Serialize};

/// Type identifier for metadata values.
///
/// Corresponds to `lore::interface::LoreMetadataType`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MetadataType {
    /// A block of raw bytes.
    Binary = 0,
    /// An unsigned integer value.
    Numeric = 1,
    /// A string value.
    String = 2,
}

impl From<MetadataType> for LoreMetadataType {
    fn from(value: MetadataType) -> Self {
        match value {
            MetadataType::Binary => LoreMetadataType::Binary,
            MetadataType::Numeric => LoreMetadataType::Numeric,
            MetadataType::String => LoreMetadataType::String,
        }
    }
}

impl From<LoreMetadataType> for MetadataType {
    fn from(value: LoreMetadataType) -> Self {
        match value {
            LoreMetadataType::Binary => MetadataType::Binary,
            LoreMetadataType::Numeric => MetadataType::Numeric,
            LoreMetadataType::String => MetadataType::String,
        }
    }
}

/// Arguments for [`metadata_set`].
///
/// Mirrors `LoreFileMetadataSetArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
///
/// # Arguments
///
/// - `paths`: Array of file paths to set metadata on
/// - `keys`: Array of metadata keys (flat across all paths)
/// - `values`: Array of metadata values (flat across all paths)
/// - `formats`: Array of value types (flat across all paths)
/// - `entries`: Array where each element is the count of metadata entries for the corresponding path
///
/// The `keys`, `values`, and `formats` arrays are flat - all entries for all paths
/// are concatenated. The `entries` array describes how to partition them: the first
/// `entries[0]` key/value/format triples belong to `paths[0]`, the next `entries[1]`
/// belong to `paths[1]`, and so on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSetArgs {
    /// Array of file paths to set metadata on.
    pub paths: Vec<String>,
    /// Array of metadata keys (flat across all paths).
    pub keys: Vec<String>,
    /// Array of metadata values (flat across all paths).
    pub values: Vec<String>,
    /// Array of value types (flat across all paths).
    #[serde(default)]
    pub formats: Vec<MetadataType>,
    /// Array where each element is the count of metadata entries for the corresponding path.
    pub entries: Vec<u32>,
}

impl MetadataSetArgs {
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileMetadataSetArgs {
        LoreFileMetadataSetArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
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
            formats: LoreArray::from_vec(self.formats.into_iter().map(|t| t.into()).collect()),
            entries: LoreArray::from_vec(self.entries),
        }
    }
}

/// Result returned on successful metadata set operation.
///
/// Since `lore::file::metadata_set` emits only standard events (Log, Error, Complete),
/// this result type is minimal - success is indicated by the absence of an error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSetResult {
    /// Number of files processed.
    pub files_processed: u32,
    /// Total number of metadata entries set.
    pub entries_set: u32,
}

/// Set metadata key/value pairs on one or more files.
///
/// Calls the upstream `lore::file::metadata_set` in-process and collects
/// standard events to determine success/failure.
///
/// # Example
///
/// ```ignore
/// let result = metadata_set(&api, MetadataSetArgs {
///     paths: vec!["file1.txt".into(), "file2.txt".into()],
///     keys: vec!["author".into(), "priority".into(), "reviewed".into()],
///     values: vec!["alice".into(), "1".into(), "true".into()],
///     formats: vec![MetadataType::String, MetadataType::Numeric, MetadataType::String],
///     entries: vec![2, 1], // file1 has 2 entries, file2 has 1
/// }).await?;
/// ```
pub async fn metadata_set(api: &LoreApi, args: MetadataSetArgs) -> Result<MetadataSetResult> {
    // Calculate totals before consuming args
    let files_processed = args.paths.len() as u32;
    let entries_set: u32 = args.entries.iter().sum();

    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status =
        lore::file::metadata_set(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("metadata_set failed with status {status}"),
        )));
    }

    Ok(MetadataSetResult {
        files_processed,
        entries_set,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_set_args_serializes() {
        let args = MetadataSetArgs {
            paths: vec!["file1.txt".into(), "file2.txt".into()],
            keys: vec!["author".into(), "priority".into()],
            values: vec!["alice".into(), "1".into()],
            formats: vec![MetadataType::String, MetadataType::Numeric],
            entries: vec![1, 1],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("file1.txt"));
        assert!(json.contains("author"));
    }

    #[test]
    fn metadata_set_args_deserializes_with_defaults() {
        let json = r#"{"paths":[],"keys":[],"values":[],"entries":[]}"#;
        let args: MetadataSetArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
        assert!(args.keys.is_empty());
        assert!(args.formats.is_empty());
    }

    #[test]
    fn metadata_type_serializes_lowercase() {
        let json = serde_json::to_string(&MetadataType::String).expect("should serialize");
        assert_eq!(json, r#""string""#);
    }

    #[test]
    fn metadata_type_from_lore() {
        assert_eq!(
            MetadataType::from(LoreMetadataType::Binary),
            MetadataType::Binary
        );
        assert_eq!(
            MetadataType::from(LoreMetadataType::Numeric),
            MetadataType::Numeric
        );
        assert_eq!(
            MetadataType::from(LoreMetadataType::String),
            MetadataType::String
        );
    }

    #[test]
    fn metadata_type_into_lore() {
        assert_eq!(
            LoreMetadataType::from(MetadataType::Binary),
            LoreMetadataType::Binary
        );
        assert_eq!(
            LoreMetadataType::from(MetadataType::Numeric),
            LoreMetadataType::Numeric
        );
        assert_eq!(
            LoreMetadataType::from(MetadataType::String),
            LoreMetadataType::String
        );
    }

    #[test]
    fn metadata_set_result_serializes() {
        let result = MetadataSetResult {
            files_processed: 2,
            entries_set: 5,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("2"));
        assert!(json.contains("5"));
    }

    #[test]
    fn metadata_set_args_with_multiple_entries_per_path() {
        let args = MetadataSetArgs {
            paths: vec!["file1.txt".into(), "file2.txt".into()],
            keys: vec!["author".into(), "priority".into(), "reviewed".into()],
            values: vec!["alice".into(), "1".into(), "true".into()],
            formats: vec![
                MetadataType::String,
                MetadataType::Numeric,
                MetadataType::String,
            ],
            entries: vec![2, 1], // file1 has author+priority, file2 has reviewed
        };
        assert_eq!(args.paths.len(), 2);
        assert_eq!(args.keys.len(), 3);
        assert_eq!(args.entries.len(), 2);
        assert_eq!(args.entries.iter().sum::<u32>(), 3);
    }
}
