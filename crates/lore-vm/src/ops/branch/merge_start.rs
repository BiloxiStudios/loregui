//! `branch merge_start` operation — binds `lore::branch::merge_start`.
//!
//! Begins merging a source branch into the current branch, auto-committing
//! if there are no conflicts. Emits `BranchMergeStartBegin` with source
//! branch/revision info, `BranchMergeConflictFile` per conflict, and
//! `BranchMergeStartEnd` with sync stats and a conflict flag.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::branch::LoreBranchMergeStartArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeStartArgs {
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub no_commit: bool,
    #[serde(default)]
    pub link: String,
    #[serde(default)]
    pub ignore_links: bool,
}

impl BranchMergeStartArgs {
    fn into_lore(self) -> LoreBranchMergeStartArgs {
        LoreBranchMergeStartArgs {
            branch: LoreString::from_str(&self.branch),
            message: LoreString::from_str(&self.message),
            no_commit: u8::from(self.no_commit),
            link: LoreString::from_str(&self.link),
            ignore_links: u8::from(self.ignore_links),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMergeStartResult {
    pub source_branch: String,
    pub source_revision: String,
    pub source_revision_number: u64,
    pub has_conflicts: bool,
    pub conflict_files: Vec<String>,
    pub merge_revision: String,
}

pub async fn merge_start(
    api: &LoreApi,
    args: BranchMergeStartArgs,
) -> Result<BranchMergeStartResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::branch::merge_start(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("branch merge_start failed with status {status}"),
        )));
    }

    let mut source_branch = String::new();
    let mut source_revision = String::new();
    let mut source_revision_number = 0u64;
    let mut has_conflicts = false;
    let mut conflict_files = Vec::new();
    let mut merge_revision = String::new();

    for event in &stream.events {
        match event {
            LoreEvent::BranchMergeStartBegin(data) => {
                source_branch = format!("{}", data.branch);
                source_revision = format!("{}", data.revision);
                source_revision_number = data.revision_number;
            }
            LoreEvent::BranchMergeStartEnd(data) => {
                has_conflicts = data.has_conflicts != 0;
                merge_revision = format!("{}", data.signature);
            }
            LoreEvent::BranchMergeConflictFile(data) => {
                conflict_files.push(data.path.as_str().to_string());
            }
            _ => {}
        }
    }

    Ok(BranchMergeStartResult {
        source_branch,
        source_revision,
        source_revision_number,
        has_conflicts,
        conflict_files,
        merge_revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_start_args_serializes() {
        let args = BranchMergeStartArgs {
            branch: "feature".into(),
            message: "merge feature".into(),
            no_commit: false,
            link: String::new(),
            ignore_links: false,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("feature"));
        assert!(json.contains("merge feature"));
    }

    #[test]
    fn merge_start_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: BranchMergeStartArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.branch, "");
        assert_eq!(args.message, "");
        assert!(!args.no_commit);
        assert!(!args.ignore_links);
    }

    #[test]
    fn merge_start_args_into_lore_conversion() {
        let args = BranchMergeStartArgs {
            branch: "dev".into(),
            message: "merge dev".into(),
            no_commit: true,
            link: "my-link".into(),
            ignore_links: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.branch.as_str(), "dev");
        assert_eq!(lore_args.message.as_str(), "merge dev");
        assert_eq!(lore_args.no_commit, 1);
        assert_eq!(lore_args.link.as_str(), "my-link");
        assert_eq!(lore_args.ignore_links, 1);
    }

    #[test]
    fn merge_start_args_no_commit_false() {
        let args = BranchMergeStartArgs {
            branch: "main".into(),
            message: String::new(),
            no_commit: false,
            link: String::new(),
            ignore_links: false,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.no_commit, 0);
        assert_eq!(lore_args.ignore_links, 0);
    }

    #[test]
    fn merge_start_result_serializes() {
        let result = BranchMergeStartResult {
            source_branch: "feature".into(),
            source_revision: "abc123".into(),
            source_revision_number: 5,
            has_conflicts: true,
            conflict_files: vec!["src/main.rs".into()],
            merge_revision: "def456".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("feature"));
        assert!(json.contains("abc123"));
        assert!(json.contains("5"));
        assert!(json.contains("true"));
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("def456"));
    }

    #[test]
    fn merge_start_result_no_conflicts() {
        let result = BranchMergeStartResult {
            source_branch: "dev".into(),
            source_revision: "aaa".into(),
            source_revision_number: 1,
            has_conflicts: false,
            conflict_files: vec![],
            merge_revision: "bbb".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("\"has_conflicts\":false"));
        assert!(json.contains("\"conflict_files\":[]"));
    }
}
