//! `branch merge_into` operation — binds `lore::branch::merge_into`.
//!
//! Merges the current branch's staged changes into a target branch and
//! auto-commits if conflict-free. Emits `BranchMergeIntoRevision` with the
//! resulting revision hash on success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchMergeIntoArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeIntoArgs {
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub branch_id: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub link: String,
    #[serde(default)]
    pub ignore_links: bool,
}

impl BranchMergeIntoArgs {
    fn into_lore(self) -> LoreBranchMergeIntoArgs {
        use std::str::FromStr;
        LoreBranchMergeIntoArgs {
            branch: LoreString::from_str(&self.branch),
            branch_id: lore::interface::Context::from_str(&self.branch_id).unwrap_or_default(),
            message: LoreString::from_str(&self.message),
            link: LoreString::from_str(&self.link),
            ignore_links: u8::from(self.ignore_links),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeIntoResult {
    pub revision: String,
    pub revision_number: u64,
}

pub async fn merge_into(api: &LoreApi, args: BranchMergeIntoArgs) -> Result<BranchMergeIntoResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::merge_into(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch merge_into failed with status {status}"),
        )));
    }

    let (revision, revision_number) = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::BranchMergeIntoRevision(data) = event {
                Some((format!("{}", data.revision), data.revision_number))
            } else {
                None
            }
        })
        .unwrap_or_default();

    Ok(BranchMergeIntoResult {
        revision,
        revision_number,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_into_args_serializes() {
        let args = BranchMergeIntoArgs {
            branch: "main".into(),
            branch_id: String::new(),
            message: "merge feature into main".into(),
            link: String::new(),
            ignore_links: false,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("main"));
        assert!(json.contains("merge feature into main"));
    }

    #[test]
    fn merge_into_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: BranchMergeIntoArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.branch, "");
        assert_eq!(args.message, "");
        assert!(!args.ignore_links);
    }

    #[test]
    fn merge_into_args_into_lore_conversion() {
        let args = BranchMergeIntoArgs {
            branch: "target-branch".into(),
            branch_id: String::new(),
            message: "my merge message".into(),
            link: String::new(),
            ignore_links: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.branch.as_str(), "target-branch");
        assert_eq!(lore_args.message.as_str(), "my merge message");
        assert_eq!(lore_args.ignore_links, 1);
    }

    #[test]
    fn merge_into_args_ignore_links_false() {
        let args = BranchMergeIntoArgs {
            branch: "main".into(),
            branch_id: String::new(),
            message: String::new(),
            link: String::new(),
            ignore_links: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.ignore_links, 0);
    }

    #[test]
    fn merge_into_result_serializes() {
        let result = BranchMergeIntoResult {
            revision: "abc123def456".into(),
            revision_number: 42,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123def456"));
        assert!(json.contains("42"));
    }
}
