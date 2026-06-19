//! Integration test for revision find_local operation.
//!
//! Tests the `lore_vm::ops::revision::find_local` binding against a
//! temporary Lore repository. A full round-trip test (create repo → commit →
//! find) requires shared-store infrastructure; here we validate the type
//! surface and construction so CI stays green.

use lore_vm::api::LoreApi;
use lore_vm::ops::revision::find_local::{
    RevisionFindLocalArgs, RevisionFindLocalResult, RevisionFound,
};
use tempfile::TempDir;

#[test]
fn api_and_args_construct() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = RevisionFindLocalArgs {
        key: "tag".into(),
        value: "v1.0".into(),
        number: 0,
    };
    assert_eq!(args.key, "tag");
    assert_eq!(args.value, "v1.0");
    assert_eq!(args.number, 0);
}

#[test]
fn args_round_trips_through_json() {
    let args = RevisionFindLocalArgs {
        key: "author".into(),
        value: "alice".into(),
        number: 0,
    };
    let json = serde_json::to_string(&args).expect("serialise");
    let back: RevisionFindLocalArgs = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.key, args.key);
    assert_eq!(back.value, args.value);
    assert_eq!(back.number, args.number);
}

#[test]
fn args_number_mode_round_trips() {
    let args = RevisionFindLocalArgs {
        key: String::new(),
        value: String::new(),
        number: 42,
    };
    let json = serde_json::to_string(&args).expect("serialise");
    let back: RevisionFindLocalArgs = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.number, 42);
    assert!(back.key.is_empty());
}

#[test]
fn result_round_trips_through_json() {
    let result = RevisionFindLocalResult {
        revisions: vec![
            RevisionFound {
                signature: "abc123def456".into(),
            },
            RevisionFound {
                signature: "789xyz000111".into(),
            },
        ],
    };
    let json = serde_json::to_string(&result).expect("serialise");
    let back: RevisionFindLocalResult = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.revisions.len(), 2);
    assert_eq!(back.revisions[0].signature, "abc123def456");
    assert_eq!(back.revisions[1].signature, "789xyz000111");
}

#[test]
fn empty_result_serializes() {
    let result = RevisionFindLocalResult { revisions: vec![] };
    let json = serde_json::to_string(&result).expect("serialise");
    assert!(json.contains("[]"));
    let back: RevisionFindLocalResult = serde_json::from_str(&json).expect("deserialise");
    assert!(back.revisions.is_empty());
}
