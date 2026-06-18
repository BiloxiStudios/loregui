//! Integration test for branch metadata_set operation.
//!
//! Tests the lore-vm::ops::branch::metadata_set binding — verifies types,
//! serialisation, and construction against a LoreApi instance.

use lore_vm::api::LoreApi;
use lore_vm::ops::branch::metadata_set::{MetadataFormat, MetadataSetArgs};
use tempfile::TempDir;

#[test]
fn test_branch_metadata_set_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    // Construct args with explicit formats
    let args = MetadataSetArgs {
        branch: "main".to_string(),
        keys: vec!["description".to_string(), "owner".to_string()],
        values: vec!["A test branch".to_string(), "alice".to_string()],
        formats: vec![MetadataFormat::String, MetadataFormat::String],
    };

    assert_eq!(args.branch, "main");
    assert_eq!(args.keys.len(), 2);
    assert_eq!(args.values.len(), 2);
    assert_eq!(args.formats.len(), 2);
    assert_eq!(args.keys[0], "description");
    assert_eq!(args.values[0], "A test branch");
    assert_eq!(args.formats[0], MetadataFormat::String);
}

#[test]
fn test_branch_metadata_set_args_default_formats() {
    // When formats is omitted (empty), into_lore pads with String
    let args = MetadataSetArgs {
        branch: "feature".to_string(),
        keys: vec!["priority".to_string()],
        values: vec!["high".to_string()],
        formats: vec![],
    };

    assert_eq!(args.branch, "feature");
    assert_eq!(args.keys.len(), 1);
    assert_eq!(args.formats.len(), 0); // empty before into_lore
}

#[test]
fn test_branch_metadata_set_serde_roundtrip() {
    let args = MetadataSetArgs {
        branch: "dev".to_string(),
        keys: vec!["tag".to_string()],
        values: vec!["v1.0".to_string()],
        formats: vec![MetadataFormat::String],
    };

    let json = serde_json::to_string(&args).expect("serialize");
    let deser: MetadataSetArgs = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deser.branch, "dev");
    assert_eq!(deser.keys, vec!["tag"]);
    assert_eq!(deser.values, vec!["v1.0"]);
    assert_eq!(deser.formats, vec![MetadataFormat::String]);
}

#[test]
fn test_metadata_format_serde() {
    // Verify rename_all = lowercase works
    let json = serde_json::to_string(&MetadataFormat::Binary).unwrap();
    assert_eq!(json, "\"binary\"");

    let json = serde_json::to_string(&MetadataFormat::Numeric).unwrap();
    assert_eq!(json, "\"numeric\"");

    let json = serde_json::to_string(&MetadataFormat::String).unwrap();
    assert_eq!(json, "\"string\"");

    // Deserialize
    let f: MetadataFormat = serde_json::from_str("\"numeric\"").unwrap();
    assert_eq!(f, MetadataFormat::Numeric);
}
