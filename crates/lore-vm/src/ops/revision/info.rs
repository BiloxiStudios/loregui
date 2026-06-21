//! `revision info` operation — binds `lore::revision::info`.
//!
//! Retrieves metadata and file-change information for a revision.
//! Emits `RevisionInfo`, optionally `RevisionInfoDelta` (per-file changes),
//! and optionally `Metadata` (key/value pairs).

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreMetadata, LoreString};
use lore::revision::LoreRevisionInfoArgs;
use serde::{Deserialize, Serialize};

/// Metadata key under which a revision's commit message is stored.
pub const METADATA_KEY_MESSAGE: &str = "message";
/// Metadata key under which a revision's commit Unix timestamp is stored.
pub const METADATA_KEY_TIMESTAMP: &str = "timestamp";
/// Metadata key under which a revision's creating author is stored.
pub const METADATA_KEY_CREATED_BY: &str = "created-by";
/// Metadata key under which a revision's committing author is stored.
pub const METADATA_KEY_COMMITTED_BY: &str = "committed-by";

/// Render a metadata value as a plain display string.
///
/// String values are returned verbatim (no surrounding JSON quotes) and numeric
/// values as their decimal form; richer value kinds fall back to their JSON
/// representation. This is what callers want for human-facing fields such as the
/// commit message, author, and timestamp.
fn metadata_display(value: &LoreMetadata) -> String {
    match value {
        LoreMetadata::String(s) => s.as_str().to_string(),
        LoreMetadata::Numeric(n) => n.to_string(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

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

impl RevisionInfoResult {
    /// Look up a metadata value by key, returning its display string.
    pub fn metadata_value(&self, key: &str) -> Option<&str> {
        self.metadata
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.value.as_str())
    }

    /// The revision's commit message, if present in metadata.
    pub fn message(&self) -> Option<&str> {
        self.metadata_value(METADATA_KEY_MESSAGE)
    }

    /// The revision's commit Unix timestamp as a string, if present.
    pub fn timestamp(&self) -> Option<&str> {
        self.metadata_value(METADATA_KEY_TIMESTAMP)
    }

    /// The revision's author: prefers the creating author, falling back to the
    /// committing author.
    pub fn author(&self) -> Option<&str> {
        self.metadata_value(METADATA_KEY_CREATED_BY)
            .or_else(|| self.metadata_value(METADATA_KEY_COMMITTED_BY))
    }
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
                    value: metadata_display(&data.value),
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

    #[test]
    fn metadata_accessors_resolve_commit_fields() {
        let result = RevisionInfoResult {
            info: None,
            deltas: vec![],
            metadata: vec![
                RevisionMetadataEntry {
                    key: METADATA_KEY_MESSAGE.into(),
                    value: "initial commit".into(),
                },
                RevisionMetadataEntry {
                    key: METADATA_KEY_TIMESTAMP.into(),
                    value: "1700000000".into(),
                },
                RevisionMetadataEntry {
                    key: METADATA_KEY_CREATED_BY.into(),
                    value: "alice".into(),
                },
            ],
        };
        assert_eq!(result.message(), Some("initial commit"));
        assert_eq!(result.timestamp(), Some("1700000000"));
        assert_eq!(result.author(), Some("alice"));
        assert_eq!(result.metadata_value("nonexistent"), None);
    }

    #[test]
    fn author_falls_back_to_committed_by() {
        let result = RevisionInfoResult {
            info: None,
            deltas: vec![],
            metadata: vec![RevisionMetadataEntry {
                key: METADATA_KEY_COMMITTED_BY.into(),
                value: "bob".into(),
            }],
        };
        assert_eq!(result.author(), Some("bob"));
    }

    #[test]
    fn metadata_display_unwraps_string_and_numeric() {
        use lore::interface::{LoreMetadata, LoreString};
        assert_eq!(
            metadata_display(&LoreMetadata::String(LoreString::from("hello"))),
            "hello"
        );
        assert_eq!(metadata_display(&LoreMetadata::Numeric(42)), "42");
    }
}
