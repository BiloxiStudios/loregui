//! Integration test for branch merge_resolve_mine operation.
//!
//! Tests the lore-vm::ops::branch::merge_resolve_mine binding args/result
//! construction against a temporary directory.

use lore_vm::api::LoreApi;
use lore_vm::ops::branch::merge_resolve_mine::{
    BranchMergeResolveMineArgs, BranchMergeResolveMineResult,
};
use tempfile::TempDir;

#[test]
fn test_merge_resolve_mine_api_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = BranchMergeResolveMineArgs {
        paths: vec!["src/conflict.rs".to_string()],
    };
    assert_eq!(args.paths.len(), 1);
    assert_eq!(args.paths[0], "src/conflict.rs");
}

#[test]
fn test_merge_resolve_mine_args_empty_paths() {
    let args = BranchMergeResolveMineArgs { paths: vec![] };
    assert!(args.paths.is_empty());

    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: BranchMergeResolveMineArgs =
        serde_json::from_str(&json).expect("should deserialize");
    assert!(deserialized.paths.is_empty());
}

#[test]
fn test_merge_resolve_mine_args_multiple_paths() {
    let args = BranchMergeResolveMineArgs {
        paths: vec![
            "file_a.txt".to_string(),
            "dir/file_b.rs".to_string(),
            "assets/image.png".to_string(),
        ],
    };
    assert_eq!(args.paths.len(), 3);

    let json = serde_json::to_string(&args).expect("should serialize");
    assert!(json.contains("file_a.txt"));
    assert!(json.contains("dir/file_b.rs"));
    assert!(json.contains("assets/image.png"));
}

#[test]
fn test_merge_resolve_mine_result_roundtrip() {
    let result = BranchMergeResolveMineResult {
        resolved_paths: vec!["resolved.rs".to_string(), "also_resolved.txt".to_string()],
        revision: "a1b2c3d4e5f6".to_string(),
    };

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: BranchMergeResolveMineResult =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.resolved_paths, result.resolved_paths);
    assert_eq!(deserialized.revision, result.revision);
}

#[test]
fn test_merge_resolve_mine_result_empty() {
    let result = BranchMergeResolveMineResult {
        resolved_paths: vec![],
        revision: String::new(),
    };

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: BranchMergeResolveMineResult =
        serde_json::from_str(&json).expect("should deserialize");
    assert!(deserialized.resolved_paths.is_empty());
    assert!(deserialized.revision.is_empty());
}
