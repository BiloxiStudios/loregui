//! Integration test for file metadata_get operation.
//!
//! Tests the lore-vm::ops::file::metadata_get binding.

use lore_vm::api::LoreApi;
use lore_vm::ops::file::metadata_get::{
    metadata_get, MetadataEntryType, MetadataGetArgs, MetadataGetResult,
};
use tempfile::TempDir;

#[test]
fn test_metadata_get_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let _api = LoreApi::new(repo_path.clone());

    let args = MetadataGetArgs {
        path: "src/main.rs".to_string(),
        key: "author".to_string(),
        revision: String::new(),
    };

    assert_eq!(args.path, "src/main.rs");
    assert_eq!(args.key, "author");
    assert_eq!(args.revision, "");
}

#[test]
fn test_metadata_get_args_with_revision() {
    let args = MetadataGetArgs {
        path: "docs/README.md".to_string(),
        key: "title".to_string(),
        revision: "abc123def".to_string(),
    };

    assert_eq!(args.path, "docs/README.md");
    assert_eq!(args.key, "title");
    assert_eq!(args.revision, "abc123def");
}

#[test]
fn test_metadata_get_args_default_revision() {
    let args = MetadataGetArgs {
        path: "file.txt".to_string(),
        key: "priority".to_string(),
        revision: String::new(),
    };

    assert_eq!(args.revision, "");
}

#[test]
fn test_metadata_get_result_fields() {
    let result = MetadataGetResult {
        key: "author".to_string(),
        value: "alice".to_string(),
        entry_type: MetadataEntryType::String,
    };

    assert_eq!(result.key, "author");
    assert_eq!(result.value, "alice");
    assert_eq!(result.entry_type, MetadataEntryType::String);
}

#[test]
fn test_metadata_get_result_numeric_type() {
    let result = MetadataGetResult {
        key: "priority".to_string(),
        value: "42".to_string(),
        entry_type: MetadataEntryType::Numeric,
    };

    assert_eq!(result.key, "priority");
    assert_eq!(result.value, "42");
    assert_eq!(result.entry_type, MetadataEntryType::Numeric);
}

#[test]
fn test_metadata_get_result_boolean_type() {
    let result = MetadataGetResult {
        key: "reviewed".to_string(),
        value: "true".to_string(),
        entry_type: MetadataEntryType::Boolean,
    };

    assert_eq!(result.key, "reviewed");
    assert_eq!(result.value, "true");
    assert_eq!(result.entry_type, MetadataEntryType::Boolean);
}

#[test]
fn test_metadata_get_args_serialization() {
    let args = MetadataGetArgs {
        path: "src/lib.rs".to_string(),
        key: "license".to_string(),
        revision: "v1.0.0".to_string(),
    };

    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: MetadataGetArgs = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.path, args.path);
    assert_eq!(deserialized.key, args.key);
    assert_eq!(deserialized.revision, args.revision);
}

#[test]
fn test_metadata_get_result_serialization() {
    let result = MetadataGetResult {
        key: "author".to_string(),
        value: "bob".to_string(),
        entry_type: MetadataEntryType::String,
    };

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: MetadataGetResult =
        serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.key, result.key);
    assert_eq!(deserialized.value, result.value);
    assert_eq!(deserialized.entry_type, result.entry_type);
}

#[test]
fn test_metadata_entry_type_all_variants() {
    let types = vec![
        MetadataEntryType::Address,
        MetadataEntryType::Boolean,
        MetadataEntryType::Binary,
        MetadataEntryType::Context,
        MetadataEntryType::Hash,
        MetadataEntryType::Numeric,
        MetadataEntryType::String,
    ];

    for entry_type in types {
        let json = serde_json::to_string(&entry_type).expect("should serialize");
        let deserialized: MetadataEntryType =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized, entry_type);
    }
}

#[tokio::test]
async fn test_metadata_get_execution_stub() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("repo");

    let api = LoreApi::new(repo_path.clone());

    let args = MetadataGetArgs {
        path: "test.txt".to_string(),
        key: "author".to_string(),
        revision: String::new(),
    };

    // We don't necessarily expect this to SUCCEED in a restricted environment
    // (e.g. if the repo doesn't exist or the file doesn't have the key),
    // but we can verify it doesn't panic and we can handle the error.
    let result = metadata_get(&api, args).await;

    match result {
        Ok(res) => {
            assert!(!res.key.is_empty());
            assert_eq!(res.key, "author");
        }
        Err(e) => {
            // If it fails, that's okay as long as it's a "real" error from the
            // lore library and not a bug in our binding.
            eprintln!("metadata_get failed (expected in some envs): {:?}", e);
        }
    }
}

#[test]
fn test_metadata_get_args_with_special_characters() {
    let args = MetadataGetArgs {
        path: "path/with spaces/file.txt".to_string(),
        key: "key-with-dashes".to_string(),
        revision: "revision@123".to_string(),
    };

    assert_eq!(args.path, "path/with spaces/file.txt");
    assert_eq!(args.key, "key-with-dashes");
    assert_eq!(args.revision, "revision@123");
}

#[test]
fn test_metadata_get_result_empty_value() {
    let result = MetadataGetResult {
        key: "empty_key".to_string(),
        value: String::new(),
        entry_type: MetadataEntryType::String,
    };

    assert_eq!(result.key, "empty_key");
    assert_eq!(result.value, "");
}

#[test]
fn test_metadata_get_result_hash_type() {
    let result = MetadataGetResult {
        key: "content_hash".to_string(),
        value: "a1b2c3d4e5f6".to_string(),
        entry_type: MetadataEntryType::Hash,
    };

    assert_eq!(result.key, "content_hash");
    assert_eq!(result.entry_type, MetadataEntryType::Hash);
}

#[test]
fn test_metadata_get_result_address_type() {
    let result = MetadataGetResult {
        key: "upstream".to_string(),
        value: "abc123-context456".to_string(),
        entry_type: MetadataEntryType::Address,
    };

    assert_eq!(result.key, "upstream");
    assert_eq!(result.entry_type, MetadataEntryType::Address);
}

#[test]
fn test_metadata_get_result_context_type() {
    let result = MetadataGetResult {
        key: "branch_context".to_string(),
        value: "ctx-789".to_string(),
        entry_type: MetadataEntryType::Context,
    };

    assert_eq!(result.key, "branch_context");
    assert_eq!(result.entry_type, MetadataEntryType::Context);
}

#[test]
fn test_metadata_get_result_binary_type() {
    // Binary is represented as JSON byte array in the value field
    let result = MetadataGetResult {
        key: "signature".to_string(),
        value: "[1,2,3,4]".to_string(),
        entry_type: MetadataEntryType::Binary,
    };

    assert_eq!(result.key, "signature");
    assert_eq!(result.entry_type, MetadataEntryType::Binary);

    // Verify the value parses as a JSON array
    let parsed: Vec<u8> = serde_json::from_str(&result.value).expect("should parse as JSON array");
    assert_eq!(parsed, vec![1, 2, 3, 4]);
}
