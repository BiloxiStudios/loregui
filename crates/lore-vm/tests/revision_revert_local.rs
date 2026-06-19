//! Integration test for revision revert_local operation.
//!
//! Tests the `lore_vm::ops::revision::revert_local` binding against a
//! temporary Lore repository.  A full round-trip test (create repo → commit →
//! revert) requires shared-store infrastructure; here we validate the type
//! surface and construction so CI stays green.

use lore_vm::api::LoreApi;
use lore_vm::ops::revision::revert_local::{
    RevertConflictFile, RevertLocalArgs, RevertLocalResult,
};
use tempfile::TempDir;

#[test]
fn api_and_args_construct() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = RevertLocalArgs {
        revision: "abc123".into(),
        message: "Revert bad change".into(),
        no_commit: false,
    };
    assert_eq!(args.revision, "abc123");
    assert_eq!(args.message, "Revert bad change");
    assert!(!args.no_commit);
}

#[test]
fn args_round_trips_through_json() {
    let args = RevertLocalArgs {
        revision: "deadbeef".into(),
        message: "undo".into(),
        no_commit: true,
    };
    let json = serde_json::to_string(&args).expect("serialise");
    let back: RevertLocalArgs = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.revision, args.revision);
    assert_eq!(back.message, args.message);
    assert_eq!(back.no_commit, args.no_commit);
}

#[test]
fn result_round_trips_through_json() {
    let result = RevertLocalResult {
        has_conflicts: true,
        conflict_files: vec![
            RevertConflictFile {
                path: "src/main.rs".into(),
            },
            RevertConflictFile {
                path: "README.md".into(),
            },
        ],
        committed_revision: None,
    };
    let json = serde_json::to_string(&result).expect("serialise");
    let back: RevertLocalResult = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.has_conflicts, result.has_conflicts);
    assert_eq!(back.conflict_files.len(), 2);
    assert_eq!(back.conflict_files[0].path, "src/main.rs");
    assert!(back.committed_revision.is_none());
}

#[test]
fn result_with_committed_revision() {
    let result = RevertLocalResult {
        has_conflicts: false,
        conflict_files: vec![],
        committed_revision: Some("abc123def456".into()),
    };
    let json = serde_json::to_string(&result).expect("serialise");
    let back: RevertLocalResult = serde_json::from_str(&json).expect("deserialise");
    assert!(!back.has_conflicts);
    assert!(back.conflict_files.is_empty());
    assert_eq!(back.committed_revision.as_deref(), Some("abc123def456"));
}

#[test]
fn args_defaults_apply() {
    let json = r#"{"revision":"abc"}"#;
    let args: RevertLocalArgs = serde_json::from_str(json).expect("deserialise");
    assert_eq!(args.revision, "abc");
    assert_eq!(args.message, "");
    assert!(!args.no_commit);
}
