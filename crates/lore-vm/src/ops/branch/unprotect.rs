//! `branch unprotect` operation — binds `lore::branch::unprotect`.
//!
//! Removes write protection from a branch, re-allowing direct commits.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchUnprotectArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchUnprotectArgs {
    #[serde(default)]
    pub branch: String,
}

impl BranchUnprotectArgs {
    fn into_lore(self) -> LoreBranchUnprotectArgs {
        LoreBranchUnprotectArgs {
            branch: LoreString::from_str(&self.branch),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchUnprotectResult {
    pub branch: String,
}

pub async fn unprotect(api: &LoreApi, args: BranchUnprotectArgs) -> Result<BranchUnprotectResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::unprotect(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch unprotect failed with status {status}"),
        )));
    }

    let name = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::BranchUnprotect(data) = event {
                Some(data.name.as_str().to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::CommandFailed(
                "branch unprotect reported success but emitted no BranchUnprotect event".into(),
            )
        })?;

    Ok(BranchUnprotectResult { branch: name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unprotect_args_serializes() {
        let args = BranchUnprotectArgs {
            branch: "main".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("main"));
    }

    #[test]
    fn unprotect_args_deserializes_with_default() {
        let json = r#"{}"#;
        let args: BranchUnprotectArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.branch, "");
    }

    #[test]
    fn unprotect_args_into_lore_conversion() {
        let args = BranchUnprotectArgs {
            branch: "feature/test".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.branch.as_str(), "feature/test");
    }

    #[test]
    fn unprotect_result_serializes() {
        let result = BranchUnprotectResult {
            branch: "main".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("main"));
    }
}
