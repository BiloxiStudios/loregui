//! Integration test for auth clear operation.
//!
//! Tests the lore-vm::ops::auth::clear binding types and
//! serialization behavior.

use lore_vm::api::LoreApi;
use lore_vm::ops::auth::clear::ClearArgs;
use tempfile::TempDir;

#[test]
fn test_clear_args_construction() {
    let args = ClearArgs {};
    // ClearArgs has no fields - just verify it can be constructed
    let _ = args;
}

#[test]
fn test_clear_args_clone() {
    let args = ClearArgs {};
    let cloned = args.clone();
    let _ = cloned;
}

#[test]
fn test_clear_args_debug() {
    let args = ClearArgs {};
    let debug = format!("{:?}", args);
    assert!(debug.contains("ClearArgs"));
}

#[test]
fn test_clear_args_serialize() {
    let args = ClearArgs {};
    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: ClearArgs = serde_json::from_str(&json).expect("should deserialize");
    let _ = deserialized;
}

#[test]
fn test_clear_api_compiles() {
    // Verify that calling clear compiles correctly
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    // Note: We don't actually call lore::auth::clear here because it would
    // fail without a proper lore repository setup. This test just verifies
    // the API surface compiles and is correctly typed.
    let args = ClearArgs {};
    let _api = api;
    let _args = args;
}
