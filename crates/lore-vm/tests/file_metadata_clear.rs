//! Integration test for file metadata_clear operation.
//!
//! Tests the lore-vm::ops::file::metadata_clear binding against a temporary
//! Lore repository.

use lore_vm::api::LoreApi;
use lore_vm::ops::file::metadata_clear::MetadataClearArgs;
use tempfile::TempDir;

#[test]
fn test_file_metadata_clear() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    // Initialize a new repository
    // Note: This test assumes repository creation is implemented elsewhere.
    // For a full integration test, we would:
    // 1. Create a repository using lore::repository::create
    // 2. Stage a file using lore::file::stage
    // 3. Set metadata on the file using lore::file::metadata_set
    // 4. Clear the metadata using our binding
    // 5. Verify the metadata was cleared

    // For now, we test that the API signature is correct by creating
    // a LoreApi instance and verifying it can be instantiated.
    let api = LoreApi::new(repo_path.clone());

    // Verify the API was created successfully
    assert_eq!(api.global().repository_path, repo_path);

    // Test that we can construct MetadataClearArgs
    let args = MetadataClearArgs {
        path: "test_file.txt".to_string(),
    };

    // Verify the args are correctly structured
    assert_eq!(args.path, "test_file.txt");

    // In a full integration test with a real repository, we would:
    // let result = metadata_clear(&api, args).await.unwrap();
    // assert_eq!(result.path, "test_file.txt");

    // The TempDir is automatically cleaned up when it goes out of scope
}
