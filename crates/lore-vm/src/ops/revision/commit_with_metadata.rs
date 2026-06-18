//! `revision commit_with_metadata` operation — binds `lore::revision::commit_with_metadata`.
//!
//! Creates a new revision from staged files, attaching key-value metadata entries
//! to the revision. Each metadata entry has a key, value, and format type
//! (Binary, Numeric, or String).
//!
//! Emits `LoreEvent::RevisionCommitRevision` on success containing the repository,
//! branch, and revision identifiers.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreEvent, LoreString};
use lore::revision::LoreRevisionCommitWithMetadataArgs;
use serde::{Deserialize, Serialize};

/// Metadata format type — mirrors `lore::interface::LoreMetadataType` with
/// serde-friendly naming for the Tauri boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetadataFormat {
    Binary,
    Numeric,
    String,
}

impl MetadataFormat {
    fn into_lore(self) -> lore::interface::LoreMetadataType {
        match self {
            MetadataFormat::Binary => lore::interface::LoreMetadataType::Binary,
            MetadataFormat::Numeric => lore::interface::LoreMetadataType::Numeric,
            MetadataFormat::String => lore::interface::LoreMetadataType::String,
        }
    }
}

/// Arguments for [`commit_with_metadata`].
///
/// Mirrors `LoreRevisionCommitWithMetadataArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitWithMetadataArgs {
    /// Commit message describing the revision.
    pub message: String,
    /// Metadata keys (parallel array with `values` and `formats`).
    #[serde(default)]
    pub keys: Vec<String>,
    /// Metadata values (parallel array with `keys` and `formats`).
    #[serde(default)]
    pub values: Vec<String>,
    /// Metadata format types (parallel array with `keys` and `values`).
    #[serde(default)]
    pub formats: Vec<MetadataFormat>,
}

impl CommitWithMetadataArgs {
    fn into_lore(self) -> LoreRevisionCommitWithMetadataArgs {
        LoreRevisionCommitWithMetadataArgs {
            message: LoreString::from_str(&self.message),
            keys: LoreArray::from_vec(
                self.keys.iter().map(|k| LoreString::from_str(k)).collect(),
            ),
            values: LoreArray::from_vec(
                self.values.iter().map(|v| LoreString::from_str(v)).collect(),
            ),
            formats: LoreArray::from_vec(
                self.formats.iter().map(|f| f.into_lore()).collect(),
            ),
        }
    }
}

/// Result returned on successful commit with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitWithMetadataResult {
    /// BLAKE3 hash signature of the newly created revision.
    pub revision: String,
    /// Sequential revision number on the branch.
    pub revision_number: u64,
    /// Branch identifier the revision was committed on.
    pub branch: String,
}

/// Commit staged files as a new revision with attached metadata key-value pairs.
///
/// Calls the upstream `lore::revision::commit_with_metadata` in-process and
/// collects the `RevisionCommitRevision` event to return a typed result.
pub async fn commit_with_metadata(
    api: &LoreApi,
    args: CommitWithMetadataArgs,
) -> Result<CommitWithMetadataResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::revision::commit_with_metadata(api.globals().build(), args.into_lore(), callback)
            .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("commit_with_metadata failed with status {status}"),
        )));
    }

    let data = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::RevisionCommitRevision(data) = event {
                Some(data.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::Parse(
                "commit_with_metadata succeeded but no RevisionCommitRevision event emitted".into(),
            )
        })?;

    Ok(CommitWithMetadataResult {
        revision: format!("{}", data.revision),
        revision_number: data.revision_number,
        branch: format!("{}", data.branch),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = CommitWithMetadataArgs {
            message: "Initial commit".into(),
            keys: vec!["author".into(), "ticket".into()],
            values: vec!["alice".into(), "SBAI-3750".into()],
            formats: vec![MetadataFormat::String, MetadataFormat::String],
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("Initial commit"));
        assert!(json.contains("author"));
        assert!(json.contains("SBAI-3750"));
    }

    #[test]
    fn args_deserializes_with_defaults() {
        let json = r#"{"message":"test"}"#;
        let args: CommitWithMetadataArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.message, "test");
        assert!(args.keys.is_empty());
        assert!(args.values.is_empty());
        assert!(args.formats.is_empty());
    }

    #[test]
    fn args_full_roundtrip() {
        let args = CommitWithMetadataArgs {
            message: "Add feature".into(),
            keys: vec!["priority".into()],
            values: vec!["42".into()],
            formats: vec![MetadataFormat::Numeric],
        };
        let json = serde_json::to_string(&args).expect("serialize");
        let back: CommitWithMetadataArgs = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.message, "Add feature");
        assert_eq!(back.keys, vec!["priority"]);
        assert_eq!(back.values, vec!["42"]);
        assert_eq!(back.formats, vec![MetadataFormat::Numeric]);
    }

    #[test]
    fn metadata_format_serde() {
        assert_eq!(
            serde_json::to_string(&MetadataFormat::Binary).unwrap(),
            r#""binary""#
        );
        assert_eq!(
            serde_json::to_string(&MetadataFormat::Numeric).unwrap(),
            r#""numeric""#
        );
        assert_eq!(
            serde_json::to_string(&MetadataFormat::String).unwrap(),
            r#""string""#
        );

        let back: MetadataFormat = serde_json::from_str(r#""numeric""#).unwrap();
        assert_eq!(back, MetadataFormat::Numeric);
    }

    #[test]
    fn result_serializes() {
        let result = CommitWithMetadataResult {
            revision: "abc123def456".into(),
            revision_number: 7,
            branch: "main-ctx-hash".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123def456"));
        assert!(json.contains("7"));
        assert!(json.contains("main-ctx-hash"));
    }
}
