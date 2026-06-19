//! `revision history` operation — binds `lore::revision::history`.
//!
//! Retrieves the revision history for the current branch or a specified
//! revision. Emits one `LoreEvent::RevisionHistoryEntry` per revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionHistoryArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`history`].
///
/// Mirrors `LoreRevisionHistoryArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionHistoryArgs {
    /// Start from this revision; empty for current.
    #[serde(default)]
    pub revision: String,
    /// Restrict to this branch; empty for current.
    #[serde(default)]
    pub branch: String,
    /// Stop at revisions created before this date (Unix timestamp; 0 disables).
    #[serde(default)]
    pub date: u64,
    /// Maximum number of revisions to return; 0 for unlimited.
    #[serde(default)]
    pub length: u32,
    /// Stop when reaching a different branch.
    #[serde(default)]
    pub only_branch: bool,
}

impl RevisionHistoryArgs {
    fn into_lore(self) -> LoreRevisionHistoryArgs {
        LoreRevisionHistoryArgs {
            revision: LoreString::from_str(&self.revision),
            branch: LoreString::from_str(&self.branch),
            date: self.date,
            length: self.length,
            only_branch: u8::from(self.only_branch),
        }
    }
}

/// A single entry in the revision history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionHistoryEntry {
    /// Revision hash signature.
    pub revision: String,
    /// Sequential revision number.
    pub revision_number: u64,
    /// Parent revision hashes (zero hashes are omitted).
    pub parents: Vec<String>,
}

/// Result returned on a successful history query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionHistoryResult {
    /// History entries, newest first.
    pub entries: Vec<RevisionHistoryEntry>,
}

/// Retrieve the revision history for the current branch or a specified revision.
///
/// Calls the upstream `lore::revision::history` in-process and collects
/// `RevisionHistoryEntry` events into a typed result.
pub async fn history(api: &LoreApi, args: RevisionHistoryArgs) -> Result<RevisionHistoryResult> {
    let (callback, rx) = collect_events();

    let status = lore::revision::history(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision history failed with status {status}"),
        )));
    }

    let entries: Vec<RevisionHistoryEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::RevisionHistoryEntry(data) = event {
                let parents: Vec<String> = data
                    .parent
                    .iter()
                    .filter(|h| !h.is_zero())
                    .map(|h| format!("{h}"))
                    .collect();
                Some(RevisionHistoryEntry {
                    revision: format!("{}", data.revision),
                    revision_number: data.revision_number,
                    parents,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(RevisionHistoryResult { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_args_defaults() {
        let json = r#"{}"#;
        let args: RevisionHistoryArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.revision, "");
        assert_eq!(args.branch, "");
        assert_eq!(args.length, 0);
        assert!(!args.only_branch);
    }

    #[test]
    fn history_args_into_lore_conversion() {
        let args = RevisionHistoryArgs {
            revision: "rev1".into(),
            branch: "main".into(),
            date: 0,
            length: 10,
            only_branch: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision.as_str(), "rev1");
        assert_eq!(lore_args.branch.as_str(), "main");
        assert_eq!(lore_args.length, 10);
        assert_eq!(lore_args.only_branch, 1);
    }

    #[test]
    fn history_result_serializes() {
        let result = RevisionHistoryResult {
            entries: vec![
                RevisionHistoryEntry {
                    revision: "r2".into(),
                    revision_number: 2,
                    parents: vec!["r1".into()],
                },
                RevisionHistoryEntry {
                    revision: "r1".into(),
                    revision_number: 1,
                    parents: vec![],
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("r1"));
        assert!(json.contains("r2"));
    }
}
