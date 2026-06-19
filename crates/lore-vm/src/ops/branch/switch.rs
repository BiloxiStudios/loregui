//! `branch switch` operation — binds `lore::branch::switch`.
//!
//! Switches the working tree to a different branch.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchSwitchArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`switch`].
///
/// Mirrors `LoreBranchSwitchArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSwitchArgs {
    /// Branch name to switch to.
    pub branch: String,
    /// Optional specific revision to switch to.
    #[serde(default)]
    pub revision: String,
    /// Whether to reset local changes (default: false).
    #[serde(default)]
    pub reset: bool,
    /// Whether to create a bare working tree (default: false).
    #[serde(default)]
    pub bare: bool,
}

impl BranchSwitchArgs {
    fn into_lore(self) -> LoreBranchSwitchArgs {
        LoreBranchSwitchArgs {
            branch: LoreString::from_str(&self.branch),
            revision: LoreString::from_str(&self.revision),
            reset: u8::from(self.reset),
            bare: u8::from(self.bare),
        }
    }
}

/// Result returned on successful branch switch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSwitchResult {
    /// The branch that was switched to.
    pub branch: String,
}

/// Switch the working tree to a different branch.
///
/// Calls the upstream `lore::branch::switch` in-process and collects
/// the `BranchSwitch` event to return a typed result.
pub async fn switch(api: &LoreApi, args: BranchSwitchArgs) -> Result<BranchSwitchResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::switch(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch switch failed with status {status}"),
        )));
    }

    let name = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::BranchSwitchEnd(data) = event {
                Some(data.branch.name.as_str().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    Ok(BranchSwitchResult { branch: name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_args_serializes() {
        let args = BranchSwitchArgs {
            branch: "feature/test".into(),
            revision: String::new(),
            reset: false,
            bare: false,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("feature/test"));
    }

    #[test]
    fn switch_args_into_lore_conversion() {
        let args = BranchSwitchArgs {
            branch: "main".into(),
            revision: String::new(),
            reset: false,
            bare: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.branch.as_str(), "main");
    }

    #[test]
    fn switch_result_serializes() {
        let result = BranchSwitchResult {
            branch: "main".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("main"));
    }
}
