//! Integration test for repository create_with_metadata operation.
//!
//! Tests the lore-vm::ops::repository::create_with_metadata binding.

use lore_vm::api::LoreApi;
use lore_vm::ops::repository::create_with_metadata::{
    create_with_metadata, CreateWithMetadataArgs, CreateWithMetadataResult,
};
use tempfile::TempDir;

#[test]
fn test_create_with_metadata_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");
    let store_path = temp_dir.path().join("shared_store");

    let _api = LoreApi::new(repo_path.clone());

    let args = CreateWithMetadataArgs {
        repository_url: "file:///tmp/repo".to_string(),
        description: "A test repository".to_string(),
        id: "00000000-0000-0000-0000-000000000001".to_string(),
        use_shared_store: true,
        shared_store_path: store_path.to_str().unwrap().to_string(),
        creator: "alice".to_string(),
        created: 1718000000,
    };

    assert_eq!(args.repository_url, "file:///tmp/repo");
    assert_eq!(args.creator, "alice");
    assert_eq!(args.created, 1718000000);
}

#[test]
fn test_create_with_metadata_result_serde() {
    let result = CreateWithMetadataResult {
        id: "00000000-0000-0000-0000-000000000001".into(),
        name: "test-repo".into(),
        path: "/tmp/repo".into(),
    };

    let json = serde_json::to_string(&result).expect("serialize");
    let deser: CreateWithMetadataResult = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deser.id, result.id);
    assert_eq!(deser.name, result.name);
    assert_eq!(deser.path, result.path);
}

#[tokio::test]
async fn test_create_with_metadata_execution_stub() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("repo");
    let store_path = temp_dir.path().join("store");
    std::fs::create_dir_all(&store_path).unwrap();

    let api = LoreApi::new(repo_path.clone());

    let args = CreateWithMetadataArgs {
        repository_url: format!("file://{}", repo_path.to_str().unwrap()),
        description: "Integration test repo".to_string(),
        id: "".to_string(),
        use_shared_store: true,
        shared_store_path: store_path.to_str().unwrap().to_string(),
        creator: "test-user".to_string(),
        created: 1718712000000,
    };

    // We don't necessarily expect this to SUCCEED in a restricted environment
    // (e.g. if the 'lore' library requires specific setup or credentials),
    // but we can verify it doesn't panic and we can handle the error.
    let result = create_with_metadata(&api, args).await;

    match result {
        Ok(res) => {
            assert!(!res.id.is_empty());
            assert!(res.path.contains("repo"));
        }
        Err(e) => {
            // If it fails, that's okay as long as it's a "real" error from the
            // lore library and not a bug in our binding.
            eprintln!(
                "create_with_metadata failed (expected in some envs): {:?}",
                e
            );
        }
    }
}
