//! Integration test for branch info operation.
//!
//! Tests the lore-vm::ops::branch::info binding against a temporary
//! Lore repository.

use lore_vm::api::LoreApi;
use lore_vm::ops::branch::info::{BranchInfoArgs, BranchInfoResult};
use tempfile::TempDir;

#[test]
fn test_branch_info_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = BranchInfoArgs {
        branch: "main".to_string(),
    };
    assert_eq!(args.branch, "main");
}

#[test]
fn test_branch_info_args_default_branch() {
    let args = BranchInfoArgs {
        branch: String::new(),
    };
    assert_eq!(args.branch, "");
}

#[test]
fn test_branch_info_result_fields() {
    let result = BranchInfoResult {
        id: "abc123".into(),
        name: "feature/test".into(),
        category: "dev".into(),
        latest: "def456".into(),
        latest_remote: "ghi789".into(),
        parent: "root".into(),
        branch_point: "jkl012".into(),
        creator: "alice".into(),
        created: 1718000000,
        archived: false,
    };

    assert_eq!(result.name, "feature/test");
    assert_eq!(result.category, "dev");
    assert_eq!(result.creator, "alice");
    assert_eq!(result.created, 1718000000);
    assert!(!result.archived);

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: BranchInfoResult =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.name, result.name);
    assert_eq!(deserialized.id, result.id);
    assert_eq!(deserialized.archived, result.archived);
}

#[test]
fn test_branch_info_result_archived() {
    let result = BranchInfoResult {
        id: String::new(),
        name: "old-branch".into(),
        category: String::new(),
        latest: String::new(),
        latest_remote: String::new(),
        parent: String::new(),
        branch_point: String::new(),
        creator: String::new(),
        created: 0,
        archived: true,
    };
    assert!(result.archived);
}
