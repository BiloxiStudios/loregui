//! `branch create` operation — binds `lore::branch::create`.
//!
//! Creates a new branch with the given name and category. Emits
//! `LoreEvent::BranchCreate` carrying the new branch name and latest revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchCreateArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`create`].
///
/// Mirrors `LoreBranchCreateArgs` from the upstream `lore` crate but uses plain
/// `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchCreateArgs {
    /// Name of the branch to create.
    pub branch: String,
    /// Category of the branch.
    #[serde(default)]
    pub category: String,
    /// Optional explicit branch ID (hex-encoded 16-byte context).
    #[serde(default)]
    pub id: String,
}

impl BranchCreateArgs {
    fn into_lore(self) -> LoreBranchCreateArgs {
        LoreBranchCreateArgs {
            branch: LoreString::from_str(&self.branch),
            category: LoreString::from_str(&self.category),
            id: LoreString::from_str(&self.id),
        }
    }
}

/// Result returned on a successful branch creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchCreateResult {
    /// Name of the created branch.
    pub name: String,
    /// Latest revision the new branch points at (empty when unset).
    pub latest: String,
    /// True when creating the branch also produced a new commit.
    pub is_commit: bool,
}

/// Create a new branch.
///
/// Calls the upstream `lore::branch::create` in-process and collects the
/// `BranchCreate` event to return a typed result.
pub async fn create(api: &LoreApi, args: BranchCreateArgs) -> Result<BranchCreateResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::create(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch create failed with status {status}"),
        )));
    }

    let data = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::BranchCreate(data) = event {
                Some(data.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::Parse("branch created successfully but no BranchCreate event emitted".into())
        })?;

    let latest = if data.latest.is_zero() {
        String::new()
    } else {
        format!("{}", data.latest)
    };

    Ok(BranchCreateResult {
        name: data.name.as_str().to_string(),
        latest,
        is_commit: data.is_commit != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_args_serializes() {
        let args = BranchCreateArgs {
            branch: "feature/x".into(),
            category: "feature".into(),
            id: String::new(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("feature/x"));
        assert!(json.contains("feature"));
    }

    #[test]
    fn create_args_deserializes_with_defaults() {
        let json = r#"{"branch":"dev"}"#;
        let args: BranchCreateArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.branch, "dev");
        assert_eq!(args.category, "");
        assert_eq!(args.id, "");
    }

    #[test]
    fn create_args_into_lore_conversion() {
        let args = BranchCreateArgs {
            branch: "main".into(),
            category: "trunk".into(),
            id: "deadbeef".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.branch.as_str(), "main");
        assert_eq!(lore_args.category.as_str(), "trunk");
        assert_eq!(lore_args.id.as_str(), "deadbeef");
    }

    #[test]
    fn create_result_serializes() {
        let result = BranchCreateResult {
            name: "feature/x".into(),
            latest: "abc123".into(),
            is_commit: false,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("feature/x"));
        assert!(json.contains("abc123"));
    }
}
