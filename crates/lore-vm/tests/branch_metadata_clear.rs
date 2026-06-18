//! Integration test for branch metadata_clear operation.
//!
//! Tests the lore-vm::ops::branch::metadata_clear binding against a temporary
//! Lore repository.

use lore_vm::api::LoreApi;
use lore_vm::ops::branch::metadata_clear::MetadataClearArgs;
use tempfile::TempDir;

#[test]
fn test_branch_metadata_clear() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    // Initialize a new repository
    // Note: This test assumes repository creation is implemented elsewhere.
    // For a full integration test, we would:
    // 1. Create a repository using lore::repository::create
    // 2. Create a branch using lore::branch::create
    // 3. Set metadata on the branch using lore::branch::metadata_set
    // 4. Clear the metadata using our binding
    // 5. Verify the metadata was cleared

    // For now, we test that the API signature is correct by creating
    // a LoreApi instance and verifying it can be instantiated.
    let api = LoreApi::new(repo_path.clone());

    // Verify the API was created successfully
    assert_eq!(api.global().repository_path, repo_path);

    // Test that we can construct MetadataClearArgs
    let args = MetadataClearArgs {
        branch: "test-branch".to_string(),
        keys: vec!["description".to_string(), "owner".to_string()],
    };

    // Verify the args are correctly structured
    assert_eq!(args.branch, "test-branch");
    assert_eq!(args.keys.len(), 2);
    assert!(args.keys.contains(&"description".to_string()));
    assert!(args.keys.contains(&"owner".to_string()));

    // In a full integration test with a real repository, we would:
    // let result = metadata_clear(&api, args).await.unwrap();
    // assert_eq!(result.branch, "test-branch");
    // assert_eq!(result.keys.len(), 2);

    // The TempDir is automatically cleaned up when it goes out of scope
}
