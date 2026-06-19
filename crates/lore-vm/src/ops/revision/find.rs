//! `revision find` operation — binds `lore::revision::find`.
//!
//! Finds revisions matching a metadata key/value pair or revision number,
//! searching the local repository and any connected remote shared stores.
//! Emits `LoreEvent::RevisionFind` for each matching revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionFindArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`find`].
///
/// Mirrors `LoreRevisionFindArgs` from the upstream `lore` crate but uses
/// plain `String` / `u64` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionFindArgs {
    /// Metadata key to search for; non-empty selects key/value search.
    #[serde(default)]
    pub key: String,
    /// Metadata value to match against `key`.
    #[serde(default)]
    pub value: String,
    /// Revision number to search for when `key` is empty; 0 disables.
    #[serde(default)]
    pub number: u64,
}

impl RevisionFindArgs {
    fn into_lore(self) -> LoreRevisionFindArgs {
        LoreRevisionFindArgs {
            key: LoreString::from_str(&self.key),
            value: LoreString::from_str(&self.value),
            number: self.number,
        }
    }
}

/// A revision found by the search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionFindEntry {
    /// The hash signature of the found revision.
    pub signature: String,
}

/// Result returned on a successful find.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionFindResult {
    /// All revisions matching the search criteria.
    pub revisions: Vec<RevisionFindEntry>,
}

/// Find revisions matching metadata or number, searching local and remote stores.
///
/// Calls the upstream `lore::revision::find` in-process and collects
/// `RevisionFind` events into a typed result.
pub async fn find(api: &LoreApi, args: RevisionFindArgs) -> Result<RevisionFindResult> {
    let (callback, rx) = collect_events();

    let status = lore::revision::find(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision find failed with status {status}"),
        )));
    }

    let revisions: Vec<RevisionFindEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::RevisionFind(data) = event {
                Some(RevisionFindEntry {
                    signature: format!("{}", data.signature),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(RevisionFindResult { revisions })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_args_defaults() {
        let json = r#"{}"#;
        let args: RevisionFindArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.key, "");
        assert_eq!(args.value, "");
        assert_eq!(args.number, 0);
    }

    #[test]
    fn find_args_with_key_value() {
        let json = r#"{"key": "tag", "value": "release-1.0"}"#;
        let args: RevisionFindArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.key, "tag");
        assert_eq!(args.value, "release-1.0");
        assert_eq!(args.number, 0);
    }

    #[test]
    fn find_args_with_number() {
        let json = r#"{"number": 42}"#;
        let args: RevisionFindArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.key, "");
        assert_eq!(args.number, 42);
    }

    #[test]
    fn find_args_into_lore_conversion() {
        let args = RevisionFindArgs {
            key: "author".into(),
            value: "alice".into(),
            number: 0,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.key.as_str(), "author");
        assert_eq!(lore_args.value.as_str(), "alice");
        assert_eq!(lore_args.number, 0);
    }

    #[test]
    fn find_args_into_lore_number() {
        let args = RevisionFindArgs {
            key: String::new(),
            value: String::new(),
            number: 7,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.key.as_str(), "");
        assert_eq!(lore_args.number, 7);
    }

    #[test]
    fn find_result_serializes() {
        let result = RevisionFindResult {
            revisions: vec![
                RevisionFindEntry {
                    signature: "abc123def456".into(),
                },
                RevisionFindEntry {
                    signature: "789012xyz345".into(),
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123def456"));
        assert!(json.contains("789012xyz345"));
    }

    #[test]
    fn find_result_empty() {
        let result = RevisionFindResult { revisions: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }
}
