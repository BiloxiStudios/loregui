//! `revision find_local` operation — binds `lore::revision::find_local`.
//!
//! Finds revisions matching a metadata key/value pair or revision number,
//! searching only the local repository (no remote dispatch).
//! Emits `LoreEvent::RevisionFind` for each matching revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionFindArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`find_local`].
///
/// Mirrors `LoreRevisionFindArgs` from the upstream `lore` crate but uses
/// plain `String` / `u64` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionFindLocalArgs {
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

impl RevisionFindLocalArgs {
    fn into_lore(self) -> LoreRevisionFindArgs {
        LoreRevisionFindArgs {
            key: LoreString::from_str(&self.key),
            value: LoreString::from_str(&self.value),
            number: self.number,
        }
    }
}

/// A revision found by the local search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionFound {
    /// The hash signature of the found revision.
    pub signature: String,
}

/// Result returned on a successful local find.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionFindLocalResult {
    /// All revisions matching the search criteria.
    pub revisions: Vec<RevisionFound>,
}

/// Find revisions matching metadata or number, searching only the local repo.
///
/// Calls the upstream `lore::revision::find_local` in-process and collects
/// `RevisionFind` events into a typed result.
pub async fn find_local(
    api: &LoreApi,
    args: RevisionFindLocalArgs,
) -> Result<RevisionFindLocalResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::revision::find_local(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision find_local failed with status {status}"),
        )));
    }

    let revisions: Vec<RevisionFound> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::RevisionFind(data) = event {
                Some(RevisionFound {
                    signature: format!("{}", data.signature),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(RevisionFindLocalResult { revisions })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_local_args_defaults() {
        let json = r#"{}"#;
        let args: RevisionFindLocalArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.key, "");
        assert_eq!(args.value, "");
        assert_eq!(args.number, 0);
    }

    #[test]
    fn find_local_args_with_key_value() {
        let json = r#"{"key": "tag", "value": "release-1.0"}"#;
        let args: RevisionFindLocalArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.key, "tag");
        assert_eq!(args.value, "release-1.0");
        assert_eq!(args.number, 0);
    }

    #[test]
    fn find_local_args_with_number() {
        let json = r#"{"number": 42}"#;
        let args: RevisionFindLocalArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.key, "");
        assert_eq!(args.number, 42);
    }

    #[test]
    fn find_local_args_into_lore_conversion() {
        let args = RevisionFindLocalArgs {
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
    fn find_local_args_into_lore_number() {
        let args = RevisionFindLocalArgs {
            key: String::new(),
            value: String::new(),
            number: 7,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.key.as_str(), "");
        assert_eq!(lore_args.number, 7);
    }

    #[test]
    fn find_local_result_serializes() {
        let result = RevisionFindLocalResult {
            revisions: vec![
                RevisionFound {
                    signature: "abc123def456".into(),
                },
                RevisionFound {
                    signature: "789012xyz345".into(),
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123def456"));
        assert!(json.contains("789012xyz345"));
    }

    #[test]
    fn find_local_result_empty() {
        let result = RevisionFindLocalResult { revisions: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }
}
