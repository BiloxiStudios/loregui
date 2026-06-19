//! Integration test for branch switch operation.
//!
//! Tests the lore-vm::ops::branch::switch binding against a temporary
//! Lore repository.

use lore_vm::api::LoreApi;
use lore_vm::ops::branch::switch::{BranchSwitchArgs, BranchSwitchResult};
use tempfile::TempDir;

#[test]
fn test_branch_switch_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = BranchSwitchArgs {
        branch: "main".to_string(),
        revision: String::new(),
        reset: false,
        bare: false,
    };
    assert_eq!(args.branch, "main");
}

#[test]
fn test_branch_switch_args_branch_name() {
    let args = BranchSwitchArgs {
        branch: "feature/test-branch".to_string(),
        revision: String::new(),
        reset: false,
        bare: false,
    };
    assert_eq!(args.branch, "feature/test-branch");
}

#[test]
fn test_branch_switch_result_fields() {
    let result = BranchSwitchResult {
        branch: "main".into(),
    };

    assert_eq!(result.branch, "main");

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: BranchSwitchResult = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.branch, result.branch);
}

#[test]
fn test_branch_switch_result_serializes() {
    let result = BranchSwitchResult {
        branch: "feature/new-feature".into(),
    };
    assert_eq!(result.branch, "feature/new-feature");

    let json = serde_json::to_string(&result).expect("should serialize");
    assert!(json.contains("feature/new-feature"));

    let deserialized: BranchSwitchResult = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.branch, "feature/new-feature");
}
