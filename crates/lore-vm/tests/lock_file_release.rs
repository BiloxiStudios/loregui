//! Integration test for lock file_release operation.
//!
//! Tests the lore-vm::ops::lock::file_release binding types against
//! serialization round-trips and construction correctness.

use lore_vm::ops::lock::file_release::{FileReleaseArgs, FileReleaseResult};

#[test]
fn test_file_release_args_construction() {
    let args = FileReleaseArgs {
        paths: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
        branch: "main".to_string(),
        owner: "user@example.com".to_string(),
        owner_id: "user-123".to_string(),
    };

    assert_eq!(args.paths.len(), 2);
    assert_eq!(args.paths[0], "src/main.rs");
    assert_eq!(args.paths[1], "Cargo.toml");
    assert_eq!(args.branch, "main");
    assert_eq!(args.owner, "user@example.com");
    assert_eq!(args.owner_id, "user-123");
}

#[test]
fn test_file_release_args_single_path() {
    let args = FileReleaseArgs {
        paths: vec!["README.md".to_string()],
        branch: "develop".to_string(),
        owner: "dev@example.com".to_string(),
        owner_id: "dev-456".to_string(),
    };

    assert_eq!(args.paths.len(), 1);
    assert_eq!(args.paths[0], "README.md");
    assert_eq!(args.branch, "develop");
}

#[test]
fn test_file_release_args_empty_paths() {
    let args = FileReleaseArgs {
        paths: vec![],
        branch: "main".to_string(),
        owner: "user@example.com".to_string(),
        owner_id: "user-123".to_string(),
    };

    assert!(args.paths.is_empty());
    assert_eq!(args.branch, "main");
}

#[test]
fn test_file_release_result_fields() {
    let result = FileReleaseResult {
        released: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
        not_found: false,
    };

    assert_eq!(result.released.len(), 2);
    assert_eq!(result.released[0], "src/main.rs");
    assert_eq!(result.released[1], "Cargo.toml");
    assert!(!result.not_found);
}

#[test]
fn test_file_release_result_with_not_found() {
    let result = FileReleaseResult {
        released: vec!["src/main.rs".to_string()],
        not_found: true,
    };

    assert_eq!(result.released.len(), 1);
    assert!(result.not_found);
}

#[test]
fn test_file_release_result_empty() {
    let result = FileReleaseResult {
        released: vec![],
        not_found: false,
    };

    assert!(result.released.is_empty());
    assert!(!result.not_found);
}

#[test]
fn test_file_release_args_serialization() {
    let args = FileReleaseArgs {
        paths: vec!["a.txt".to_string(), "b.rs".to_string()],
        branch: "feature-branch".to_string(),
        owner: "owner@test.com".to_string(),
        owner_id: "id-789".to_string(),
    };

    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: FileReleaseArgs = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.paths.len(), 2);
    assert_eq!(deserialized.paths[0], "a.txt");
    assert_eq!(deserialized.paths[1], "b.rs");
    assert_eq!(deserialized.branch, "feature-branch");
    assert_eq!(deserialized.owner, "owner@test.com");
    assert_eq!(deserialized.owner_id, "id-789");
}

#[test]
fn test_file_release_result_serialization() {
    let result = FileReleaseResult {
        released: vec!["file1.txt".to_string()],
        not_found: true,
    };

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: FileReleaseResult = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.released.len(), 1);
    assert_eq!(deserialized.released[0], "file1.txt");
    assert!(deserialized.not_found);
}

#[test]
fn test_file_release_args_with_special_characters() {
    let args = FileReleaseArgs {
        paths: vec![
            "path/with spaces/file.txt".to_string(),
            "path/with-unicode/日本語.txt".to_string(),
            "path/with-emoji/🎨.txt".to_string(),
        ],
        branch: "feature/test-branch".to_string(),
        owner: "user with spaces".to_string(),
        owner_id: "id-with-special-chars!@#".to_string(),
    };

    assert_eq!(args.paths.len(), 3);
    assert!(args.paths[0].contains(' '));
    assert!(args.paths[1].contains("日本語"));
    assert!(args.paths[2].contains('🎨'));
}
