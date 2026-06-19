//! `revision info` operation — binds `lore::revision::info`.
//!
//! Retrieves metadata and file-change information for a revision.
//! Emits `RevisionInfo`, optionally `RevisionInfoDelta` (per-file changes),
//! and optionally `Metadata` (key/value pairs).

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionInfoArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`info`].
///
/// Mirrors `LoreRevisionInfoArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionInfoArgs {
    /// Revision to get info for; empty for current.
    #[serde(default)]
    pub revision: String,
    /// Include delta (per-file changes) against parent.
    #[serde(default)]
    pub delta: bool,
    /// Include metadata entries.
    #[serde(default)]
    pub metadata: bool,
}

impl RevisionInfoArgs {
    fn into_lore(self) -> LoreRevisionInfoArgs {
        LoreRevisionInfoArgs {
            revision: LoreString::from_str(&self.revision),
            delta: u8::from(self.delta),
            metadata: u8::from(self.metadata),
        }
    }
}

/// Core revision information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionInfoData {
    /// Repository identifier.
    pub repository: String,
    /// Revision hash signature.
    pub revision: String,
    /// Sequential revision number.
    pub revision_number: u64,
    /// Parent revision hashes (zero hashes are omitted).
    pub parents: Vec<String>,
}

/// Per-file change between a revision and its parent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionInfoDelta {
    /// File path relative to the repository root.
    pub path: String,
    /// File size in bytes.
    pub size: u64,
    /// Action applied to the file.
    pub action: String,
    /// Whether the file content was modified.
    pub flag_modify: bool,
    /// Whether the change came from a merge.
    pub flag_merged: bool,
    /// Whether the entry is a file (not a directory).
    pub flag_file: bool,
}

/// A metadata key/value pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionMetadataEntry {
    /// Metadata key.
    pub key: String,
    /// Metadata value as a display string.
    pub value: String,
}

/// Result returned on a successful revision info query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionInfoResult {
    /// Core revision information (populated from `RevisionInfo` event).
    pub info: Option<RevisionInfoData>,
    /// Per-file deltas (populated when `delta=true`).
    pub deltas: Vec<RevisionInfoDelta>,
    /// Metadata entries (populated when `metadata=true`).
    pub metadata: Vec<RevisionMetadataEntry>,
}

/// Retrieve metadata and file information for a revision.
///
/// Calls the upstream `lore::revision::info` in-process and collects
/// events into a typed result.
pub async fn info(api: &LoreApi, args: RevisionInfoArgs) -> Result<RevisionInfoResult> {
    let (callback, rx) = collect_events();

    let status = lore::revision::info(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision info failed with status {status}"),
        )));
    }

    let mut result = RevisionInfoResult::default();

    for event in &stream.events {
        match event {
            LoreEvent::RevisionInfo(data) => {
                let parents: Vec<String> = data
                    .parent
                    .iter()
                    .filter(|h| !h.is_zero())
                    .map(|h| format!("{h}"))
                    .collect();
                result.info = Some(RevisionInfoData {
                    repository: format!("{}", data.repository),
                    revision: format!("{}", data.revision),
                    revision_number: data.revision_number,
                    parents,
                });
            }
            LoreEvent::RevisionInfoDelta(data) => {
                result.deltas.push(RevisionInfoDelta {
                    path: data.path.as_str().to_string(),
                    size: data.size,
                    action: format!("{:?}", data.action),
                    flag_modify: data.flag_modify != 0,
                    flag_merged: data.flag_merged != 0,
                    flag_file: data.flag_file != 0,
                });
            }
            LoreEvent::Metadata(data) => {
                result.metadata.push(RevisionMetadataEntry {
                    key: data.key.as_str().to_string(),
                    value: serde_json::to_string(&data.value).unwrap_or_default(),
                });
            }
            _ => {}
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_args_defaults() {
        let json = r#"{}"#;
        let args: RevisionInfoArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.revision, "");
        assert!(!args.delta);
        assert!(!args.metadata);
    }

    #[test]
    fn info_args_into_lore_conversion() {
        let args = RevisionInfoArgs {
            revision: "rev1".into(),
            delta: true,
            metadata: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision.as_str(), "rev1");
        assert_eq!(lore_args.delta, 1);
        assert_eq!(lore_args.metadata, 0);
    }

    #[test]
    fn info_result_serializes() {
        let result = RevisionInfoResult {
            info: Some(RevisionInfoData {
                repository: "repo1".into(),
                revision: "rev1".into(),
                revision_number: 1,
                parents: vec![],
            }),
            deltas: vec![RevisionInfoDelta {
                path: "file.txt".into(),
                size: 100,
                action: "Add".into(),
                flag_modify: false,
                flag_merged: false,
                flag_file: true,
            }],
            metadata: vec![RevisionMetadataEntry {
                key: "author".into(),
                value: "test".into(),
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("rev1"));
        assert!(json.contains("file.txt"));
        assert!(json.contains("author"));
    }

    #[test]
    fn info_result_default_is_empty() {
        let result = RevisionInfoResult::default();
        assert!(result.info.is_none());
        assert!(result.deltas.is_empty());
        assert!(result.metadata.is_empty());
    }
}
