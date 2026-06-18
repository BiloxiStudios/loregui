//! `branch info` operation — binds `lore::branch::info`.
//!
//! Retrieves metadata for a branch including its name, id, category,
//! protection status, parent, creation time, and archive state.
//! Emits `LoreEvent::BranchInfo` on success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchInfoArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`info`].
///
/// Mirrors `LoreBranchInfoArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfoArgs {
    /// Branch name; empty string uses the current branch.
    #[serde(default)]
    pub branch: String,
}

impl BranchInfoArgs {
    fn into_lore(self) -> LoreBranchInfoArgs {
        LoreBranchInfoArgs {
            branch: LoreString::from_str(&self.branch),
        }
    }
}

/// Result returned on successful branch info query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfoResult {
    /// Branch identifier (Context hash).
    pub id: String,
    /// Branch name.
    pub name: String,
    /// Branch category (e.g. "main", "dev").
    pub category: String,
    /// Latest revision hash known locally.
    pub latest: String,
    /// Latest revision hash known on the remote.
    pub latest_remote: String,
    /// Identifier of the parent branch.
    pub parent: String,
    /// Revision hash on the parent branch where this branch was created.
    pub branch_point: String,
    /// User who created the branch.
    pub creator: String,
    /// Creation timestamp (Unix epoch seconds).
    pub created: u64,
    /// Whether the branch has been archived.
    pub archived: bool,
}

/// Retrieve metadata for a branch.
///
/// Calls the upstream `lore::branch::info` in-process and collects
/// the `BranchInfo` event to return a typed result.
pub async fn info(api: &LoreApi, args: BranchInfoArgs) -> Result<BranchInfoResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::info(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch info failed with status {status}"),
        )));
    }

    let data = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::BranchInfo(data) = event {
                Some(data.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::Parse("branch info succeeded but no BranchInfo event emitted".into())
        })?;

    Ok(BranchInfoResult {
        id: format!("{}", data.id),
        name: data.name.as_str().to_string(),
        category: data.category.as_str().to_string(),
        latest: format!("{}", data.latest),
        latest_remote: format!("{}", data.latest_remote),
        parent: format!("{}", data.parent),
        branch_point: format!("{}", data.branch_point),
        creator: data.creator.as_str().to_string(),
        created: data.created,
        archived: data.archived != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_info_args_serializes() {
        let args = BranchInfoArgs {
            branch: "main".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("main"));
    }

    #[test]
    fn branch_info_args_deserializes_with_default() {
        let json = r#"{}"#;
        let args: BranchInfoArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.branch, "");
    }

    #[test]
    fn branch_info_args_into_lore_conversion() {
        let args = BranchInfoArgs {
            branch: "feature/test".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.branch.as_str(), "feature/test");
    }

    #[test]
    fn branch_info_result_serializes() {
        let result = BranchInfoResult {
            id: "abc123".into(),
            name: "main".into(),
            category: "dev".into(),
            latest: "def456".into(),
            latest_remote: "ghi789".into(),
            parent: "root".into(),
            branch_point: "jkl012".into(),
            creator: "alice".into(),
            created: 1718000000,
            archived: false,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("main"));
        assert!(json.contains("abc123"));
        assert!(json.contains("alice"));
        assert!(json.contains("1718000000"));
    }

    #[test]
    fn branch_info_result_archived_true() {
        let result = BranchInfoResult {
            id: String::new(),
            name: "old-feature".into(),
            category: String::new(),
            latest: String::new(),
            latest_remote: String::new(),
            parent: String::new(),
            branch_point: String::new(),
            creator: String::new(),
            created: 0,
            archived: true,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains(r#""archived":true"#));
    }
}
