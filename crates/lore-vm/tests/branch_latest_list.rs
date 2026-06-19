//! Integration test for branch latest_list operation.
//!
//! Tests the lore-vm::ops::branch::latest_list binding types
//! against serialisation and construction.

use lore_vm::api::LoreApi;
use lore_vm::ops::branch::latest_list::{
    BranchLatestListArgs, BranchLatestListEntry, BranchLatestListResult,
};
use tempfile::TempDir;

#[test]
fn test_branch_latest_list_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = BranchLatestListArgs {
        branch: "main".to_string(),
        limit: 10,
    };
    assert_eq!(args.branch, "main");
    assert_eq!(args.limit, 10);
}

#[test]
fn test_branch_latest_list_args_default_branch() {
    let args = BranchLatestListArgs {
        branch: String::new(),
        limit: 0,
    };
    assert_eq!(args.branch, "");
    assert_eq!(args.limit, 0);
}

#[test]
fn test_branch_latest_list_result_fields() {
    let result = BranchLatestListResult {
        entries: vec![
            BranchLatestListEntry {
                branch: "abc123".into(),
                revision: "rev001".into(),
            },
            BranchLatestListEntry {
                branch: "abc123".into(),
                revision: "rev002".into(),
            },
        ],
    };

    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.entries[0].branch, "abc123");
    assert_eq!(result.entries[0].revision, "rev001");
    assert_eq!(result.entries[1].revision, "rev002");

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: BranchLatestListResult =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.entries.len(), result.entries.len());
    assert_eq!(deserialized.entries[0].branch, result.entries[0].branch);
    assert_eq!(deserialized.entries[1].revision, result.entries[1].revision);
}

#[test]
fn test_branch_latest_list_result_empty() {
    let result = BranchLatestListResult { entries: vec![] };
    assert!(result.entries.is_empty());
    let json = serde_json::to_string(&result).expect("should serialize");
    assert!(json.contains("[]"));
}
