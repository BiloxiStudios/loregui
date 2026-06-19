//! Integration test for file dirty_move operation.
//!
//! Tests the lore-vm::ops::file::dirty_move binding types against
//! serialization round-trips and construction correctness.

use lore_vm::api::LoreApi;
use lore_vm::ops::file::dirty_move::{FileDirtyMoveArgs, FileDirtyMoveResult};
use tempfile::TempDir;

#[test]
fn test_file_dirty_move_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = FileDirtyMoveArgs {
        from_path: "src/main.rs".to_string(),
        to_path: "src/app.rs".to_string(),
    };
    assert_eq!(args.from_path, "src/main.rs");
    assert_eq!(args.to_path, "src/app.rs");
}

#[test]
fn test_file_dirty_move_args_serde_round_trip() {
    let args = FileDirtyMoveArgs {
        from_path: "assets/model.obj".into(),
        to_path: "assets/renamed_model.obj".into(),
    };
    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: FileDirtyMoveArgs = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.from_path, args.from_path);
    assert_eq!(deserialized.to_path, args.to_path);
}

#[test]
fn test_file_dirty_move_result_construction() {
    let result = FileDirtyMoveResult {
        from_path: "docs/readme.md".into(),
        to_path: "docs/README.md".into(),
    };
    assert_eq!(result.from_path, "docs/readme.md");
    assert_eq!(result.to_path, "docs/README.md");
}

#[test]
fn test_file_dirty_move_result_serde_round_trip() {
    let result = FileDirtyMoveResult {
        from_path: "src/old_module.rs".into(),
        to_path: "src/new_module.rs".into(),
    };
    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: FileDirtyMoveResult =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.from_path, result.from_path);
    assert_eq!(deserialized.to_path, result.to_path);
}

#[test]
fn test_file_dirty_move_args_empty_paths() {
    let args = FileDirtyMoveArgs {
        from_path: String::new(),
        to_path: String::new(),
    };
    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: FileDirtyMoveArgs = serde_json::from_str(&json).expect("should deserialize");
    assert!(deserialized.from_path.is_empty());
    assert!(deserialized.to_path.is_empty());
}

#[test]
fn test_file_dirty_move_args_nested_paths() {
    let args = FileDirtyMoveArgs {
        from_path: "deeply/nested/path/to/file.txt".into(),
        to_path: "another/deep/path/file.txt".into(),
    };
    let json = serde_json::to_string(&args).expect("should serialize");
    assert!(json.contains("deeply/nested/path/to/file.txt"));
    assert!(json.contains("another/deep/path/file.txt"));
}
