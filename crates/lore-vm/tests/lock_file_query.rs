//! Integration test for lock file_query operation.
//!
//! Tests the lore-vm::ops::lock::file_query binding types against
//! serialization round-trips and construction correctness.

use lore_vm::ops::lock::file_query::{FileQueryArgs, FileQueryResult, LockEntry};

#[test]
fn test_file_query_args_construction() {
    let args = FileQueryArgs {
        branch: "main".to_string(),
        owner: "user@example.com".to_string(),
        path: "src/main.rs".to_string(),
    };

    assert_eq!(args.branch, "main");
    assert_eq!(args.owner, "user@example.com");
    assert_eq!(args.path, "src/main.rs");
}

#[test]
fn test_file_query_args_empty_filters() {
    let args = FileQueryArgs {
        branch: "".to_string(),
        owner: "".to_string(),
        path: "".to_string(),
    };

    assert!(args.branch.is_empty());
    assert!(args.owner.is_empty());
    assert!(args.path.is_empty());
}

#[test]
fn test_file_query_result_with_locks() {
    let result = FileQueryResult {
        count: 2,
        locks: vec![
            LockEntry {
                branch: "abc123".to_string(),
                path: "src/main.rs".to_string(),
                owner: "user-001".to_string(),
                locked_at: 1718700000000,
            },
            LockEntry {
                branch: "abc123".to_string(),
                path: "Cargo.toml".to_string(),
                owner: "user-002".to_string(),
                locked_at: 1718700060000,
            },
        ],
    };

    assert_eq!(result.count, 2);
    assert_eq!(result.locks.len(), 2);
    assert_eq!(result.locks[0].path, "src/main.rs");
    assert_eq!(result.locks[0].owner, "user-001");
    assert_eq!(result.locks[1].path, "Cargo.toml");
    assert_eq!(result.locks[1].owner, "user-002");
}

#[test]
fn test_file_query_result_empty() {
    let result = FileQueryResult {
        count: 0,
        locks: vec![],
    };

    assert_eq!(result.count, 0);
    assert!(result.locks.is_empty());
}

#[test]
fn test_file_query_args_serialization() {
    let args = FileQueryArgs {
        branch: "feature-branch".to_string(),
        owner: "owner@test.com".to_string(),
        path: "assets/texture.png".to_string(),
    };

    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: FileQueryArgs = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.branch, "feature-branch");
    assert_eq!(deserialized.owner, "owner@test.com");
    assert_eq!(deserialized.path, "assets/texture.png");
}

#[test]
fn test_file_query_result_serialization() {
    let result = FileQueryResult {
        count: 1,
        locks: vec![LockEntry {
            branch: "def456".to_string(),
            path: "level/map.umap".to_string(),
            owner: "artist-01".to_string(),
            locked_at: 1718700120000,
        }],
    };

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: FileQueryResult = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.count, 1);
    assert_eq!(deserialized.locks.len(), 1);
    assert_eq!(deserialized.locks[0].path, "level/map.umap");
    assert_eq!(deserialized.locks[0].owner, "artist-01");
    assert_eq!(deserialized.locks[0].locked_at, 1718700120000);
}

#[test]
fn test_lock_entry_fields() {
    let entry = LockEntry {
        branch: "abc-def-123".to_string(),
        path: "Content/Characters/Hero.uasset".to_string(),
        owner: "game-dev-42".to_string(),
        locked_at: 1718700000000,
    };

    assert_eq!(entry.branch, "abc-def-123");
    assert_eq!(entry.path, "Content/Characters/Hero.uasset");
    assert_eq!(entry.owner, "game-dev-42");
    assert_eq!(entry.locked_at, 1718700000000);
}

#[test]
fn test_file_query_args_with_special_characters() {
    let args = FileQueryArgs {
        branch: "feature/lock-support".to_string(),
        owner: "user with spaces".to_string(),
        path: "path/with spaces/日本語.txt".to_string(),
    };

    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: FileQueryArgs = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.branch, "feature/lock-support");
    assert!(deserialized.owner.contains(' '));
    assert!(deserialized.path.contains("日本語"));
}
