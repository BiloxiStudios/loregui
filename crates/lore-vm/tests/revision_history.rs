//! Integration test for revision history operation.
//!
//! Tests the `lore_vm::ops::revision::history` binding's type surface and
//! serialization round-trips. A full end-to-end test (create repo, commit,
//! query history) requires shared-store infrastructure; here we validate
//! construction and serde so CI stays green.

use lore_vm::api::LoreApi;
use lore_vm::ops::revision::history::{
    RevisionHistoryArgs, RevisionHistoryEntry, RevisionHistoryResult,
};
use tempfile::TempDir;

#[test]
fn api_and_args_construct() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = RevisionHistoryArgs {
        revision: "rev1".into(),
        branch: "main".into(),
        date: 0,
        length: 50,
        only_branch: true,
    };
    assert_eq!(args.revision, "rev1");
    assert_eq!(args.branch, "main");
    assert_eq!(args.length, 50);
    assert!(args.only_branch);
}

#[test]
fn args_defaults_from_json() {
    let json = r#"{}"#;
    let args: RevisionHistoryArgs = serde_json::from_str(json).expect("should deserialize");
    assert_eq!(args.revision, "");
    assert_eq!(args.branch, "");
    assert_eq!(args.date, 0);
    assert_eq!(args.length, 0);
    assert!(!args.only_branch);
}

#[test]
fn args_round_trips_through_json() {
    let args = RevisionHistoryArgs {
        revision: "abc123".into(),
        branch: "feature".into(),
        date: 1700000000,
        length: 100,
        only_branch: true,
    };
    let json = serde_json::to_string(&args).expect("serialise");
    let back: RevisionHistoryArgs = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.revision, args.revision);
    assert_eq!(back.branch, args.branch);
    assert_eq!(back.date, args.date);
    assert_eq!(back.length, args.length);
    assert_eq!(back.only_branch, args.only_branch);
}

#[test]
fn result_round_trips_through_json() {
    let result = RevisionHistoryResult {
        entries: vec![
            RevisionHistoryEntry {
                revision: "r3".into(),
                revision_number: 3,
                parents: vec!["r2".into()],
            },
            RevisionHistoryEntry {
                revision: "r2".into(),
                revision_number: 2,
                parents: vec!["r1".into()],
            },
            RevisionHistoryEntry {
                revision: "r1".into(),
                revision_number: 1,
                parents: vec![],
            },
        ],
    };
    let json = serde_json::to_string(&result).expect("serialise");
    let back: RevisionHistoryResult = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.entries.len(), 3);
    assert_eq!(back.entries[0].revision, "r3");
    assert_eq!(back.entries[0].revision_number, 3);
    assert_eq!(back.entries[0].parents, vec!["r2"]);
    assert_eq!(back.entries[2].parents.len(), 0);
}

#[test]
fn empty_result_serializes() {
    let result = RevisionHistoryResult { entries: vec![] };
    let json = serde_json::to_string(&result).expect("serialise");
    assert!(json.contains("[]"));
    let back: RevisionHistoryResult = serde_json::from_str(&json).expect("deserialise");
    assert!(back.entries.is_empty());
}
