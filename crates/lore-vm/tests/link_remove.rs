//! Integration test for link remove operation.
//!
//! Tests the lore-vm::ops::link::remove binding types against
//! serialization and construction contracts.

use lore_vm::api::LoreApi;
use lore_vm::ops::link::remove::{RemoveArgs, RemoveResult};
use tempfile::TempDir;

#[test]
fn test_link_remove_api_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);
}

#[test]
fn test_remove_args_fields() {
    let args = RemoveArgs {
        link_path: "deps/characters".into(),
    };

    assert_eq!(args.link_path, "deps/characters");

    let json = serde_json::to_string(&args).expect("should serialize");
    assert!(json.contains("\"link_path\":\"deps/characters\""));
}

#[test]
fn test_remove_args_deserialization() {
    let json = r#"{"link_path":"deps/world"}"#;
    let args: RemoveArgs = serde_json::from_str(json).expect("should deserialize");
    assert_eq!(args.link_path, "deps/world");
}

#[test]
fn test_remove_result_serialization() {
    let result = RemoveResult {
        link_path: "deps/characters".into(),
    };

    assert_eq!(result.link_path, "deps/characters");

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: RemoveResult = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.link_path, "deps/characters");
}

#[test]
fn test_remove_result_various_paths() {
    for path in &["deps/a", "linked/b/c", "external"] {
        let result = RemoveResult {
            link_path: path.to_string(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        let de: RemoveResult = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(de.link_path, *path);
    }
}
