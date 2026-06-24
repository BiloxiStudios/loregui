//! `branch protect` operation — binds `lore::branch::protect`.
//!
//! Applies write protection to a branch, preventing direct commits.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchProtectArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchProtectArgs {
    #[serde(default)]
    pub branch: String,
}

impl BranchProtectArgs {
    fn into_lore(self) -> LoreBranchProtectArgs {
        LoreBranchProtectArgs {
            branch: LoreString::from_str(&self.branch),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchProtectResult {
    pub branch: String,
}

pub async fn protect(api: &LoreApi, args: BranchProtectArgs) -> Result<BranchProtectResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::protect(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch protect failed with status {status}"),
        )));
    }

    let name = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::BranchProtect(data) = event {
                Some(data.name.as_str().to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::CommandFailed(
                "branch protect reported success but emitted no BranchProtect event".into(),
            )
        })?;

    Ok(BranchProtectResult { branch: name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protect_args_serializes() {
        let args = BranchProtectArgs {
            branch: "main".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("main"));
    }

    #[test]
    fn protect_args_deserializes_with_default() {
        let json = r#"{}"#;
        let args: BranchProtectArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.branch, "");
    }

    #[test]
    fn protect_args_into_lore_conversion() {
        let args = BranchProtectArgs {
            branch: "feature/test".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.branch.as_str(), "feature/test");
    }

    #[test]
    fn protect_result_serializes() {
        let result = BranchProtectResult {
            branch: "main".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("main"));
    }
}
