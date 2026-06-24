//! `branch reset` operation — binds `lore::branch::reset`.
//!
//! Resets the local LATEST pointer of a branch to a specific revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchResetArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`reset`].
///
/// Mirrors `LoreBranchResetArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchResetArgs {
    /// Revision to reset the local LATEST pointer to.
    pub revision: String,
    /// Branch to reset (current branch if empty).
    #[serde(default)]
    pub branch: String,
}

impl BranchResetArgs {
    fn into_lore(self) -> LoreBranchResetArgs {
        LoreBranchResetArgs {
            revision: LoreString::from_str(&self.revision),
            branch: LoreString::from_str(&self.branch),
        }
    }
}

/// Result returned on successful branch reset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchResetResult {
    /// The branch that was reset.
    pub branch: String,
    /// The revision the branch was reset to.
    pub revision: String,
}

/// Reset the local LATEST pointer of a branch to a specific revision.
///
/// Calls the upstream `lore::branch::reset` in-process and collects
/// the `BranchReset` event to return a typed result.
pub async fn reset(api: &LoreApi, args: BranchResetArgs) -> Result<BranchResetResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::reset(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch reset failed with status {status}"),
        )));
    }

    let (name, revision) = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::BranchReset(data) = event {
                Some((data.name.as_str().to_string(), data.revision.to_string()))
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::CommandFailed(
                "branch reset reported success but emitted no BranchReset event".into(),
            )
        })?;

    Ok(BranchResetResult {
        branch: name,
        revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_args_serializes() {
        let args = BranchResetArgs {
            revision: "abc123".into(),
            branch: "feature/test".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("abc123"));
        assert!(json.contains("feature/test"));
    }

    #[test]
    fn reset_args_deserializes_with_default_branch() {
        let json = r#"{"revision":"abc123"}"#;
        let args: BranchResetArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.revision, "abc123");
        assert_eq!(args.branch, "");
    }

    #[test]
    fn reset_args_into_lore_conversion() {
        let args = BranchResetArgs {
            revision: "deadbeef".into(),
            branch: "main".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision.as_str(), "deadbeef");
        assert_eq!(lore_args.branch.as_str(), "main");
    }

    #[test]
    fn reset_result_serializes() {
        let result = BranchResetResult {
            branch: "main".into(),
            revision: "abc123".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("main"));
        assert!(json.contains("abc123"));
    }
}
