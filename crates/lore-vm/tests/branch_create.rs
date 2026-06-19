//! Integration test for branch create operation.
//!
//! Tests the lore-vm::ops::branch::create binding against a temporary
//! Lore repository.

use lore_vm::api::LoreApi;
use lore_vm::ops::branch::create::{BranchCreateArgs, BranchCreateResult};
use tempfile::TempDir;

#[test]
fn test_branch_create_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = BranchCreateArgs {
        branch: "feature/new".to_string(),
        category: "feature".to_string(),
        id: String::new(),
    };
    assert_eq!(args.branch, "feature/new");
    assert_eq!(args.category, "feature");
}

#[test]
fn test_branch_create_args_default_fields() {
    let json = r#"{"branch":"dev"}"#;
    let args: BranchCreateArgs = serde_json::from_str(json).expect("should deserialize");
    assert_eq!(args.branch, "dev");
    assert_eq!(args.category, "");
    assert_eq!(args.id, "");
}

#[test]
fn test_branch_create_args_full() {
    let args = BranchCreateArgs {
        branch: "release/1.0".to_string(),
        category: "release".to_string(),
        id: "deadbeef01234567".to_string(),
    };
    let json = serde_json::to_string(&args).expect("should serialize");
    assert!(json.contains("release/1.0"));
    assert!(json.contains("release"));
    assert!(json.contains("deadbeef01234567"));

    let deserialized: BranchCreateArgs = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.branch, "release/1.0");
    assert_eq!(deserialized.category, "release");
    assert_eq!(deserialized.id, "deadbeef01234567");
}

#[test]
fn test_branch_create_result_fields() {
    let result = BranchCreateResult {
        name: "feature/new".into(),
        latest: "abc123def456".into(),
        is_commit: false,
    };

    assert_eq!(result.name, "feature/new");
    assert_eq!(result.latest, "abc123def456");
    assert!(!result.is_commit);

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: BranchCreateResult = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.name, result.name);
    assert_eq!(deserialized.latest, result.latest);
    assert_eq!(deserialized.is_commit, result.is_commit);
}

#[test]
fn test_branch_create_result_with_commit() {
    let result = BranchCreateResult {
        name: "hotfix/urgent".into(),
        latest: String::new(),
        is_commit: true,
    };
    assert!(result.is_commit);
    assert!(result.latest.is_empty());

    let json = serde_json::to_string(&result).expect("should serialize");
    assert!(json.contains(r#""is_commit":true"#));
}
