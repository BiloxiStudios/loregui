//! Integration test for revision info operation.
//!
//! Tests the `lore_vm::ops::revision::info` binding's type surface and
//! serialization round-trips. A full end-to-end test (create repo, commit,
//! query info) requires shared-store infrastructure; here we validate
//! construction and serde so CI stays green.

use lore_vm::api::LoreApi;
use lore_vm::ops::revision::info::{
    RevisionInfoArgs, RevisionInfoData, RevisionInfoDelta, RevisionInfoResult,
    RevisionMetadataEntry,
};
use tempfile::TempDir;

#[test]
fn api_and_args_construct() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = RevisionInfoArgs {
        revision: "rev1".into(),
        delta: true,
        metadata: false,
    };
    assert_eq!(args.revision, "rev1");
    assert!(args.delta);
    assert!(!args.metadata);
}

#[test]
fn args_defaults_from_json() {
    let json = r#"{}"#;
    let args: RevisionInfoArgs = serde_json::from_str(json).expect("should deserialize");
    assert_eq!(args.revision, "");
    assert!(!args.delta);
    assert!(!args.metadata);
}

#[test]
fn args_round_trips_through_json() {
    let args = RevisionInfoArgs {
        revision: "abc123".into(),
        delta: true,
        metadata: true,
    };
    let json = serde_json::to_string(&args).expect("serialise");
    let back: RevisionInfoArgs = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.revision, args.revision);
    assert_eq!(back.delta, args.delta);
    assert_eq!(back.metadata, args.metadata);
}

#[test]
fn result_round_trips_through_json() {
    let result = RevisionInfoResult {
        info: Some(RevisionInfoData {
            repository: "repo-id".into(),
            revision: "rev-hash".into(),
            revision_number: 42,
            parents: vec!["parent-hash".into()],
        }),
        deltas: vec![RevisionInfoDelta {
            path: "src/main.rs".into(),
            size: 1024,
            action: "Add".into(),
            flag_modify: true,
            flag_merged: false,
            flag_file: true,
        }],
        metadata: vec![RevisionMetadataEntry {
            key: "author".into(),
            value: "test-user".into(),
        }],
    };
    let json = serde_json::to_string(&result).expect("serialise");
    let back: RevisionInfoResult = serde_json::from_str(&json).expect("deserialise");
    let info = back.info.expect("should have info");
    assert_eq!(info.repository, "repo-id");
    assert_eq!(info.revision, "rev-hash");
    assert_eq!(info.revision_number, 42);
    assert_eq!(info.parents, vec!["parent-hash"]);
    assert_eq!(back.deltas.len(), 1);
    assert_eq!(back.deltas[0].path, "src/main.rs");
    assert_eq!(back.deltas[0].size, 1024);
    assert!(back.deltas[0].flag_modify);
    assert!(back.deltas[0].flag_file);
    assert_eq!(back.metadata.len(), 1);
    assert_eq!(back.metadata[0].key, "author");
}

#[test]
fn empty_result_serializes() {
    let result = RevisionInfoResult::default();
    let json = serde_json::to_string(&result).expect("serialise");
    let back: RevisionInfoResult = serde_json::from_str(&json).expect("deserialise");
    assert!(back.info.is_none());
    assert!(back.deltas.is_empty());
    assert!(back.metadata.is_empty());
}
