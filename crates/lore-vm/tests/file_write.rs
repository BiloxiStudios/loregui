//! Integration test for file write operation.
//!
//! Tests the lore-vm::ops::file::write binding types against
//! serialization round-trips and construction correctness.

use lore_vm::api::LoreApi;
use lore_vm::ops::file::write::{FileWriteArgs, FileWriteResult};
use tempfile::TempDir;

#[test]
fn test_file_write_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = FileWriteArgs {
        address: String::new(),
        path: "src/main.rs".to_string(),
        revision: "abc123".to_string(),
        output: "/tmp/out.rs".to_string(),
    };
    assert_eq!(args.path, "src/main.rs");
    assert_eq!(args.output, "/tmp/out.rs");
}

#[test]
fn test_file_write_args_with_address() {
    let args = FileWriteArgs {
        address: "addr-deadbeef".into(),
        path: String::new(),
        revision: String::new(),
        output: "/tmp/content.bin".into(),
    };
    assert_eq!(args.address, "addr-deadbeef");
    assert!(args.path.is_empty());
    assert!(args.revision.is_empty());
    assert_eq!(args.output, "/tmp/content.bin");
}

#[test]
fn test_file_write_args_serde_round_trip() {
    let args = FileWriteArgs {
        address: String::new(),
        path: "assets/texture.png".into(),
        revision: "rev42".into(),
        output: "/home/user/texture.png".into(),
    };
    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: FileWriteArgs = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.path, "assets/texture.png");
    assert_eq!(deserialized.revision, "rev42");
    assert_eq!(deserialized.output, "/home/user/texture.png");
}

#[test]
fn test_file_write_result_fields() {
    let result = FileWriteResult {
        path: "/tmp/written.bin".into(),
    };
    assert_eq!(result.path, "/tmp/written.bin");

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: FileWriteResult = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.path, result.path);
}

#[test]
fn test_file_write_result_empty_path() {
    let result = FileWriteResult {
        path: String::new(),
    };
    let json = serde_json::to_string(&result).expect("should serialize");
    assert!(json.contains(r#""path":"""#));
}

#[test]
fn test_file_write_args_defaults() {
    let json = r#"{"output": "/tmp/file.bin"}"#;
    let args: FileWriteArgs = serde_json::from_str(json).expect("should deserialize");
    assert!(args.address.is_empty());
    assert!(args.path.is_empty());
    assert!(args.revision.is_empty());
    assert_eq!(args.output, "/tmp/file.bin");
}
