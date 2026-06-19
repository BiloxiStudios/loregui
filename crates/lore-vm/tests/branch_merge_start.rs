//! Integration test for branch merge_start operation.
//!
//! Tests the lore-vm::ops::branch::merge_start binding args/result
//! construction against a temporary directory.

use lore_vm::api::LoreApi;
use lore_vm::ops::branch::merge_start::{BranchMergeStartArgs, BranchMergeStartResult};
use tempfile::TempDir;

#[test]
fn test_merge_start_api_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = BranchMergeStartArgs {
        branch: "feature".to_string(),
        message: "merge feature into main".to_string(),
        no_commit: false,
        link: String::new(),
        ignore_links: false,
    };
    assert_eq!(args.branch, "feature");
    assert_eq!(args.message, "merge feature into main");
}

#[test]
fn test_merge_start_args_defaults() {
    let json = r#"{}"#;
    let args: BranchMergeStartArgs = serde_json::from_str(json).expect("should deserialize");
    assert_eq!(args.branch, "");
    assert_eq!(args.message, "");
    assert!(!args.no_commit);
    assert_eq!(args.link, "");
    assert!(!args.ignore_links);
}

#[test]
fn test_merge_start_args_full_roundtrip() {
    let args = BranchMergeStartArgs {
        branch: "dev".to_string(),
        message: "merge dev".to_string(),
        no_commit: true,
        link: "my-link".to_string(),
        ignore_links: true,
    };

    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: BranchMergeStartArgs =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.branch, "dev");
    assert_eq!(deserialized.message, "merge dev");
    assert!(deserialized.no_commit);
    assert_eq!(deserialized.link, "my-link");
    assert!(deserialized.ignore_links);
}

#[test]
fn test_merge_start_result_with_conflicts() {
    let result = BranchMergeStartResult {
        source_branch: "feature".to_string(),
        source_revision: "abc123".to_string(),
        source_revision_number: 5,
        has_conflicts: true,
        conflict_files: vec![
            "src/main.rs".to_string(),
            "README.md".to_string(),
        ],
        merge_revision: "def456".to_string(),
    };

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: BranchMergeStartResult =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.source_branch, "feature");
    assert_eq!(deserialized.source_revision, "abc123");
    assert_eq!(deserialized.source_revision_number, 5);
    assert!(deserialized.has_conflicts);
    assert_eq!(deserialized.conflict_files.len(), 2);
    assert_eq!(deserialized.merge_revision, "def456");
}

#[test]
fn test_merge_start_result_no_conflicts() {
    let result = BranchMergeStartResult {
        source_branch: "main".to_string(),
        source_revision: "fff000".to_string(),
        source_revision_number: 1,
        has_conflicts: false,
        conflict_files: vec![],
        merge_revision: "aaa111".to_string(),
    };

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: BranchMergeStartResult =
        serde_json::from_str(&json).expect("should deserialize");
    assert!(!deserialized.has_conflicts);
    assert!(deserialized.conflict_files.is_empty());
}
