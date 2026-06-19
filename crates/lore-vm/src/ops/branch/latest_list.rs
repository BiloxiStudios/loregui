//! `branch latest_list` operation — binds `lore::branch::latest_list`.
//!
//! Lists the LATEST revision history for a branch, returning one entry per
//! revision pointer in the branch's latest-chain. Each entry carries the
//! branch identifier and the revision hash.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchLatestListArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`latest_list`].
///
/// Mirrors `LoreBranchLatestListArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchLatestListArgs {
    /// Branch name; empty string uses the current branch.
    #[serde(default)]
    pub branch: String,
    /// Maximum entries to return; `0` uses the upstream default of 30.
    #[serde(default)]
    pub limit: u32,
}

impl BranchLatestListArgs {
    fn into_lore(self) -> LoreBranchLatestListArgs {
        LoreBranchLatestListArgs {
            branch: LoreString::from_str(&self.branch),
            limit: self.limit,
        }
    }
}

/// A single entry in the branch latest-revision history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchLatestListEntry {
    /// Branch identifier.
    pub branch: String,
    /// Revision hash recorded in this history entry.
    pub revision: String,
}

/// Result returned by [`latest_list`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchLatestListResult {
    /// Ordered list of latest-revision entries (newest first).
    pub entries: Vec<BranchLatestListEntry>,
}

/// List the LATEST revision history for a branch.
///
/// Calls the upstream `lore::branch::latest_list` in-process and collects
/// all `BranchLatestListEntry` events into a typed result.
pub async fn latest_list(
    api: &LoreApi,
    args: BranchLatestListArgs,
) -> Result<BranchLatestListResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::latest_list(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch latest_list failed with status {status}"),
        )));
    }

    let entries = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::BranchLatestListEntry(data) = event {
                Some(BranchLatestListEntry {
                    branch: format!("{}", data.branch),
                    revision: format!("{}", data.revision),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(BranchLatestListResult { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_list_args_serializes() {
        let args = BranchLatestListArgs {
            branch: "main".into(),
            limit: 10,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("main"));
        assert!(json.contains("10"));
    }

    #[test]
    fn latest_list_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: BranchLatestListArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.branch, "");
        assert_eq!(args.limit, 0);
    }

    #[test]
    fn latest_list_args_into_lore_conversion() {
        let args = BranchLatestListArgs {
            branch: "feature/test".into(),
            limit: 5,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.branch.as_str(), "feature/test");
        assert_eq!(lore_args.limit, 5);
    }

    #[test]
    fn latest_list_entry_serializes() {
        let entry = BranchLatestListEntry {
            branch: "abc123".into(),
            revision: "def456".into(),
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("abc123"));
        assert!(json.contains("def456"));
    }

    #[test]
    fn latest_list_result_serializes_roundtrip() {
        let result = BranchLatestListResult {
            entries: vec![
                BranchLatestListEntry {
                    branch: "aaa".into(),
                    revision: "r1".into(),
                },
                BranchLatestListEntry {
                    branch: "bbb".into(),
                    revision: "r2".into(),
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: BranchLatestListResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.entries.len(), 2);
        assert_eq!(deserialized.entries[0].branch, "aaa");
        assert_eq!(deserialized.entries[1].revision, "r2");
    }

    #[test]
    fn latest_list_result_empty() {
        let result = BranchLatestListResult { entries: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }
}
