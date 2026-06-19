//! Integration test for branch list operation.
//!
//! Tests the lore-vm::ops::branch::list binding types
//! against serialisation and construction.

use lore_vm::api::LoreApi;
use lore_vm::ops::branch::list::{BranchListArgs, BranchListEntry, BranchListResult, BranchPoint};
use tempfile::TempDir;

#[test]
fn test_branch_list_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = BranchListArgs { archived: true };
    assert!(args.archived);
}

#[test]
fn test_branch_list_args_default() {
    let json = r#"{}"#;
    let args: BranchListArgs = serde_json::from_str(json).expect("should deserialize");
    assert!(!args.archived);
}

#[test]
fn test_branch_list_result_fields() {
    let result = BranchListResult {
        entries: vec![
            BranchListEntry {
                location: "local".into(),
                id: "abc123".into(),
                name: "main".into(),
                category: "trunk".into(),
                latest: "rev001".into(),
                stack: vec![],
                creator: "alice".into(),
                created: 1718000000,
                is_current: true,
                archived: false,
            },
            BranchListEntry {
                location: "local".into(),
                id: "def456".into(),
                name: "feature/x".into(),
                category: "dev".into(),
                latest: "rev002".into(),
                stack: vec![BranchPoint {
                    branch: "abc123".into(),
                    revision: "rev001".into(),
                }],
                creator: "bob".into(),
                created: 1718100000,
                is_current: false,
                archived: false,
            },
        ],
        count: 2,
    };

    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.entries[0].name, "main");
    assert!(result.entries[0].is_current);
    assert!(!result.entries[0].archived);
    assert_eq!(result.entries[1].name, "feature/x");
    assert!(!result.entries[1].is_current);
    assert_eq!(result.entries[1].stack.len(), 1);
    assert_eq!(result.entries[1].stack[0].branch, "abc123");
    assert_eq!(result.count, 2);

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: BranchListResult =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.entries.len(), result.entries.len());
    assert_eq!(deserialized.entries[0].name, result.entries[0].name);
    assert_eq!(deserialized.entries[1].creator, result.entries[1].creator);
    assert_eq!(deserialized.count, result.count);
}

#[test]
fn test_branch_list_result_empty() {
    let result = BranchListResult {
        entries: vec![],
        count: 0,
    };
    assert!(result.entries.is_empty());
    let json = serde_json::to_string(&result).expect("should serialize");
    assert!(json.contains("[]"));
}

#[test]
fn test_branch_list_archived_entries() {
    let entry = BranchListEntry {
        location: "local".into(),
        id: "ghi789".into(),
        name: "old-feature".into(),
        category: "dev".into(),
        latest: "rev003".into(),
        stack: vec![],
        creator: "carol".into(),
        created: 1718200000,
        is_current: false,
        archived: true,
    };
    assert!(entry.archived);
    let json = serde_json::to_string(&entry).expect("should serialize");
    assert!(json.contains(r#""archived":true"#));
}

#[test]
fn test_branch_point_roundtrip() {
    let bp = BranchPoint {
        branch: "ctx_aaa".into(),
        revision: "rev_bbb".into(),
    };
    let json = serde_json::to_string(&bp).expect("should serialize");
    let deserialized: BranchPoint = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.branch, "ctx_aaa");
    assert_eq!(deserialized.revision, "rev_bbb");
}
