//! `branch merge_abort` operation — binds `lore::branch::merge_abort`.
//!
//! Aborts an in-progress branch merge, reverting the working directory to its
//! pre-merge state. Emits `BranchMergeAbortBegin` with the staged and current
//! revision hashes, and `BranchMergeAbortEnd` on completion.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchMergeAbortArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeAbortArgs {
    #[serde(default)]
    pub link: String,
    #[serde(default)]
    pub ignore_links: bool,
}

impl BranchMergeAbortArgs {
    fn into_lore(self) -> LoreBranchMergeAbortArgs {
        LoreBranchMergeAbortArgs {
            link: LoreString::from_str(&self.link),
            ignore_links: u8::from(self.ignore_links),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeAbortResult {
    pub staged_revision: String,
    pub current_revision: String,
}

pub async fn merge_abort(
    api: &LoreApi,
    args: BranchMergeAbortArgs,
) -> Result<BranchMergeAbortResult> {
    let (callback, rx) = collect_events();

    let status = lore::branch::merge_abort(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch merge_abort failed with status {status}"),
        )));
    }

    let (staged_revision, current_revision) = stream
        .events
        .iter()
        .find_map(|event| {
            if let LoreEvent::BranchMergeAbortBegin(data) = event {
                Some((
                    format!("{}", data.state_staged_revision),
                    format!("{}", data.state_current_revision),
                ))
            } else {
                None
            }
        })
        .ok_or_else(|| {
            LoreError::CommandFailed(
                "branch merge_abort reported success but emitted no \
                 BranchMergeAbortBegin event"
                    .into(),
            )
        })?;

    Ok(BranchMergeAbortResult {
        staged_revision,
        current_revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_abort_args_serializes() {
        let args = BranchMergeAbortArgs {
            link: "my-link".into(),
            ignore_links: true,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("my-link"));
        assert!(json.contains("true"));
    }

    #[test]
    fn merge_abort_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: BranchMergeAbortArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.link, "");
        assert!(!args.ignore_links);
    }

    #[test]
    fn merge_abort_args_into_lore_conversion() {
        let args = BranchMergeAbortArgs {
            link: "some-link".into(),
            ignore_links: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.link.as_str(), "some-link");
        assert_eq!(lore_args.ignore_links, 1);
    }

    #[test]
    fn merge_abort_args_ignore_links_false() {
        let args = BranchMergeAbortArgs {
            link: String::new(),
            ignore_links: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.ignore_links, 0);
    }

    #[test]
    fn merge_abort_result_serializes() {
        let result = BranchMergeAbortResult {
            staged_revision: "abc123".into(),
            current_revision: "def456".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("abc123"));
        assert!(json.contains("def456"));
    }

    #[test]
    fn merge_abort_result_empty() {
        let result = BranchMergeAbortResult {
            staged_revision: String::new(),
            current_revision: String::new(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("staged_revision"));
        assert!(json.contains("current_revision"));
    }
}
