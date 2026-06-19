//! Integration test for lock file_acquire_as_owner operation.
//!
//! Tests the lore-vm::ops::lock::file_acquire_as_owner binding types against
//! serialization round-trips and construction correctness.

use lore_vm::ops::lock::file_acquire_as_owner::{FileAcquireAsOwnerArgs, FileAcquireAsOwnerResult};

#[test]
fn test_file_acquire_as_owner_args_construction() {
    let args = FileAcquireAsOwnerArgs {
        paths: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
        branch: "main".to_string(),
        owner: "user-123".to_string(),
    };

    assert_eq!(args.paths.len(), 2);
    assert_eq!(args.paths[0], "src/main.rs");
    assert_eq!(args.paths[1], "Cargo.toml");
    assert_eq!(args.branch, "main");
    assert_eq!(args.owner, "user-123");
}

#[test]
fn test_file_acquire_as_owner_args_single_path() {
    let args = FileAcquireAsOwnerArgs {
        paths: vec!["README.md".to_string()],
        branch: "develop".to_string(),
        owner: "admin".to_string(),
    };

    assert_eq!(args.paths.len(), 1);
    assert_eq!(args.paths[0], "README.md");
    assert_eq!(args.branch, "develop");
    assert_eq!(args.owner, "admin");
}

#[test]
fn test_file_acquire_as_owner_args_empty_paths() {
    let args = FileAcquireAsOwnerArgs {
        paths: vec![],
        branch: "main".to_string(),
        owner: "user-456".to_string(),
    };

    assert!(args.paths.is_empty());
    assert_eq!(args.branch, "main");
    assert_eq!(args.owner, "user-456");
}

#[test]
fn test_file_acquire_as_owner_result_fields() {
    let result = FileAcquireAsOwnerResult {
        acquired: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
        ignored: vec!["README.md".to_string()],
    };

    assert_eq!(result.acquired.len(), 2);
    assert_eq!(result.acquired[0], "src/main.rs");
    assert_eq!(result.acquired[1], "Cargo.toml");
    assert_eq!(result.ignored.len(), 1);
    assert_eq!(result.ignored[0], "README.md");
}

#[test]
fn test_file_acquire_as_owner_result_empty() {
    let result = FileAcquireAsOwnerResult {
        acquired: vec![],
        ignored: vec![],
    };

    assert!(result.acquired.is_empty());
    assert!(result.ignored.is_empty());
}

#[test]
fn test_file_acquire_as_owner_args_serialization() {
    let args = FileAcquireAsOwnerArgs {
        paths: vec!["a.txt".to_string(), "b.rs".to_string()],
        branch: "feature-branch".to_string(),
        owner: "team-lead".to_string(),
    };

    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: FileAcquireAsOwnerArgs =
        serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.paths.len(), 2);
    assert_eq!(deserialized.paths[0], "a.txt");
    assert_eq!(deserialized.paths[1], "b.rs");
    assert_eq!(deserialized.branch, "feature-branch");
    assert_eq!(deserialized.owner, "team-lead");
}

#[test]
fn test_file_acquire_as_owner_result_serialization() {
    let result = FileAcquireAsOwnerResult {
        acquired: vec!["file1.txt".to_string()],
        ignored: vec!["file2.txt".to_string()],
    };

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: FileAcquireAsOwnerResult =
        serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.acquired.len(), 1);
    assert_eq!(deserialized.acquired[0], "file1.txt");
    assert_eq!(deserialized.ignored.len(), 1);
    assert_eq!(deserialized.ignored[0], "file2.txt");
}

#[test]
fn test_file_acquire_as_owner_args_with_special_characters() {
    let args = FileAcquireAsOwnerArgs {
        paths: vec![
            "path/with spaces/file.txt".to_string(),
            "path/with-unicode/\u{65E5}\u{672C}\u{8A9E}.txt".to_string(),
            "path/with-emoji/\u{1F3A8}.txt".to_string(),
        ],
        branch: "feature/test-branch".to_string(),
        owner: "user@example.com".to_string(),
    };

    assert_eq!(args.paths.len(), 3);
    assert!(args.paths[0].contains(' '));
    assert!(args.paths[1].contains('\u{65E5}'));
    assert!(args.paths[2].contains('\u{1F3A8}'));
    assert_eq!(args.owner, "user@example.com");
}
