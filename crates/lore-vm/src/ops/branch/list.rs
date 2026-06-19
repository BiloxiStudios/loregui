//! `branch list` operation — binds `lore::branch::list`.
//!
//! Lists all branches in the repository, returning one entry per branch.
//! Each entry carries the branch id, name, category, latest revision,
//! creator, creation time, and flags for current/archived status.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchListArgs;
use lore::interface::LoreEvent;
use serde::{Deserialize, Serialize};

/// Arguments for [`list`].
///
/// Mirrors `LoreBranchListArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchListArgs {
    /// Whether to include archived branches in the listing.
    #[serde(default)]
    pub archived: bool,
}

impl BranchListArgs {
    fn into_lore(self) -> LoreBranchListArgs {
        LoreBranchListArgs {
            archived: if self.archived { 1 } else { 0 },
        }
    }
}

/// A branch-point entry in the ancestry stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchPoint {
    /// Branch identifier (hex context hash).
    pub branch: String,
    /// Revision hash on that branch.
    pub revision: String,
}

/// A single branch entry returned by [`list`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchListEntry {
    /// Where this branch is located ("local" or "remote").
    pub location: String,
    /// Branch identifier (hex context hash).
    pub id: String,
    /// Branch name.
    pub name: String,
    /// Branch category (e.g. "main", "dev").
    pub category: String,
    /// Latest revision hash the branch points at.
    pub latest: String,
    /// Stack of branch points this branch was created from.
    pub stack: Vec<BranchPoint>,
    /// User who created the branch.
    pub creator: String,
    /// Creation timestamp (Unix epoch seconds).
    pub created: u64,
    /// Whether this is the currently checked-out branch.
    pub is_current: bool,
    /// Whether the branch has been archived.
    pub archived: bool,
}

/// Result returned by [`list`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchListResult {
    /// All branches found in the repository.
    pub entries: Vec<BranchListEntry>,
    /// Total number of branches reported by the end event.
    pub count: u64,
}

/// List all branches in the repository.
///
/// Calls the upstream `lore::branch::list` in-process and collects
/// all `BranchListEntry` events into a typed result.
pub async fn list(api: &LoreApi, args: BranchListArgs) -> Result<BranchListResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::list(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch list failed with status {status}"),
        )));
    }

    let entries: Vec<BranchListEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::BranchListEntry(data) = event {
                let stack = data
                    .stack
                    .as_slice()
                    .iter()
                    .map(|bp| BranchPoint {
                        branch: format!("{}", bp.branch),
                        revision: format!("{}", bp.revision),
                    })
                    .collect();

                Some(BranchListEntry {
                    location: format!("{}", data.location),
                    id: format!("{}", data.id),
                    name: data.name.as_str().to_string(),
                    category: data.category.as_str().to_string(),
                    latest: format!("{}", data.latest),
                    stack,
                    creator: data.creator.as_str().to_string(),
                    created: data.created,
                    is_current: data.is_current != 0,
                    archived: data.archived != 0,
                })
            } else {
                None
            }
        })
        .collect();

    let count = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::BranchListEnd(data) = event {
                Some(data.count)
            } else {
                None
            }
        })
        .unwrap_or(entries.len() as u64);

    Ok(BranchListResult { entries, count })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_args_serializes() {
        let args = BranchListArgs { archived: true };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("true"));
    }

    #[test]
    fn list_args_deserializes_with_default() {
        let json = r#"{}"#;
        let args: BranchListArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(!args.archived);
    }

    #[test]
    fn list_args_into_lore_archived_true() {
        let args = BranchListArgs { archived: true };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.archived, 1);
    }

    #[test]
    fn list_args_into_lore_archived_false() {
        let args = BranchListArgs { archived: false };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.archived, 0);
    }

    #[test]
    fn list_entry_serializes() {
        let entry = BranchListEntry {
            location: "local".into(),
            id: "abc123".into(),
            name: "main".into(),
            category: "trunk".into(),
            latest: "def456".into(),
            stack: vec![BranchPoint {
                branch: "root".into(),
                revision: "ghi789".into(),
            }],
            creator: "alice".into(),
            created: 1718000000,
            is_current: true,
            archived: false,
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("main"));
        assert!(json.contains("abc123"));
        assert!(json.contains("alice"));
        assert!(json.contains(r#""is_current":true"#));
    }

    #[test]
    fn list_result_serializes_roundtrip() {
        let result = BranchListResult {
            entries: vec![
                BranchListEntry {
                    location: "local".into(),
                    id: "aaa".into(),
                    name: "main".into(),
                    category: "trunk".into(),
                    latest: "r1".into(),
                    stack: vec![],
                    creator: "bob".into(),
                    created: 100,
                    is_current: true,
                    archived: false,
                },
                BranchListEntry {
                    location: "local".into(),
                    id: "bbb".into(),
                    name: "feature/x".into(),
                    category: "dev".into(),
                    latest: "r2".into(),
                    stack: vec![BranchPoint {
                        branch: "aaa".into(),
                        revision: "r1".into(),
                    }],
                    creator: "carol".into(),
                    created: 200,
                    is_current: false,
                    archived: false,
                },
            ],
            count: 2,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: BranchListResult =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.entries.len(), 2);
        assert_eq!(deserialized.entries[0].name, "main");
        assert_eq!(deserialized.entries[1].name, "feature/x");
        assert_eq!(deserialized.count, 2);
    }

    #[test]
    fn list_result_empty() {
        let result = BranchListResult {
            entries: vec![],
            count: 0,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("[]"));
    }

    #[test]
    fn branch_point_serializes() {
        let bp = BranchPoint {
            branch: "ctx123".into(),
            revision: "rev456".into(),
        };
        let json = serde_json::to_string(&bp).expect("should serialize");
        let deserialized: BranchPoint = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.branch, "ctx123");
        assert_eq!(deserialized.revision, "rev456");
    }
}
