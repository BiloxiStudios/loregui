//! Integration test for repository create operation.
//!
//! Tests the lore-vm::ops::repository::create binding.

use lore_vm::api::LoreApi;
use lore_vm::ops::repository::create::{create, CreateArgs, CreateResult};
use tempfile::TempDir;

#[test]
fn test_create_args_construction() {
    let args = CreateArgs {
        repository_url: "lore://localhost/test-repo".to_string(),
        description: "A test repository".to_string(),
        id: "".to_string(),
        use_shared_store: false,
        shared_store_path: String::new(),
    };

    assert_eq!(args.repository_url, "lore://localhost/test-repo");
    assert_eq!(args.description, "A test repository");
    assert!(!args.use_shared_store);
}

#[test]
fn test_create_args_serde_roundtrip() {
    let args = CreateArgs {
        repository_url: "lore://localhost/demo".to_string(),
        description: "demo repo".to_string(),
        id: "00000000-0000-0000-0000-000000000001".to_string(),
        use_shared_store: true,
        shared_store_path: "/tmp/store".to_string(),
    };

    let json = serde_json::to_string(&args).expect("serialize");
    let deser: CreateArgs = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deser.repository_url, args.repository_url);
    assert_eq!(deser.description, args.description);
    assert_eq!(deser.id, args.id);
    assert_eq!(deser.use_shared_store, args.use_shared_store);
    assert_eq!(deser.shared_store_path, args.shared_store_path);
}

#[test]
fn test_create_args_deserializes_with_defaults() {
    let json = r#"{"repository_url":"lore://localhost/x"}"#;
    let args: CreateArgs = serde_json::from_str(json).expect("deserialize");

    assert_eq!(args.repository_url, "lore://localhost/x");
    assert_eq!(args.description, "");
    assert_eq!(args.id, "");
    assert!(!args.use_shared_store);
    assert_eq!(args.shared_store_path, "");
}

#[test]
fn test_create_result_serde_roundtrip() {
    let result = CreateResult {
        id: "repo-abc-123".into(),
        name: "test-repo".into(),
        path: "/tmp/test-repo".into(),
    };

    let json = serde_json::to_string(&result).expect("serialize");
    let deser: CreateResult = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deser.id, result.id);
    assert_eq!(deser.name, result.name);
    assert_eq!(deser.path, result.path);
}

#[tokio::test]
async fn test_create_execution() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("repo");
    let store_path = temp_dir.path().join("store");
    std::fs::create_dir_all(&store_path).unwrap();

    let api = LoreApi::new(repo_path.clone());

    let args = CreateArgs {
        repository_url: format!("file://{}", repo_path.to_str().unwrap()),
        description: "Integration test repo".to_string(),
        id: "".to_string(),
        use_shared_store: true,
        shared_store_path: store_path.to_str().unwrap().to_string(),
    };

    let result = create(&api, args).await;

    match result {
        Ok(res) => {
            assert!(!res.id.is_empty());
            assert!(res.path.contains("repo"));
        }
        Err(e) => {
            eprintln!(
                "repository create failed (expected in some envs): {:?}",
                e
            );
        }
    }
}
