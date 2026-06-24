//! `branch archive` operation — binds `lore::branch::archive`.
//!
//! Archives a branch locally and on the remote, preventing further commits.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchArchiveArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchArchiveArgs {
    #[serde(default)]
    pub branch: String,
}

impl BranchArchiveArgs {
    fn into_lore(self) -> LoreBranchArchiveArgs {
        LoreBranchArchiveArgs {
            branch: LoreString::from_str(&self.branch),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchArchiveResult {
    pub branch: String,
}

pub async fn archive(api: &LoreApi, args: BranchArchiveArgs) -> Result<BranchArchiveResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::archive(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch archive failed with status {status}"),
        )));
    }

    let name = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::BranchArchive(data) = event {
                Some(data.name.as_str().to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::CommandFailed(
                "branch archive reported success but emitted no BranchArchive event".into(),
            )
        })?;

    Ok(BranchArchiveResult { branch: name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_args_serializes() {
        let args = BranchArchiveArgs {
            branch: "old-feature".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("old-feature"));
    }

    #[test]
    fn archive_args_deserializes_with_default() {
        let json = r#"{}"#;
        let args: BranchArchiveArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.branch, "");
    }

    #[test]
    fn archive_args_into_lore_conversion() {
        let args = BranchArchiveArgs {
            branch: "feature/test".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.branch.as_str(), "feature/test");
    }

    #[test]
    fn archive_result_serializes() {
        let result = BranchArchiveResult {
            branch: "old-feature".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("old-feature"));
    }
}
