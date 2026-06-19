//! Integration test for revision sync operation.
//!
//! Tests the `lore_vm::ops::revision::sync` binding's type surface and
//! serialisation. A full round-trip (create repo → commit → sync from remote)
//! requires shared-store + remote infrastructure; here we validate
//! construction and JSON round-trips so CI stays green.

use lore_vm::api::LoreApi;
use lore_vm::ops::revision::sync::{
    RevisionSyncArgs, RevisionSyncResult, SyncFileEntry, SyncRevisionInfo,
};
use tempfile::TempDir;

#[test]
fn api_and_args_construct() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = RevisionSyncArgs {
        revision: "abc123".into(),
        forward_changes: true,
        reset: false,
        root_files: vec!["src/main.rs".into()],
        dependency_tags: vec!["tag1".into()],
        dependency_recursive: true,
        dependency_depth_limit: 5,
    };
    assert_eq!(args.revision, "abc123");
    assert!(args.forward_changes);
    assert!(!args.reset);
    assert_eq!(args.root_files.len(), 1);
}

#[test]
fn args_round_trips_through_json() {
    let args = RevisionSyncArgs {
        revision: "rev1".into(),
        forward_changes: true,
        reset: true,
        root_files: vec!["file1.txt".into(), "file2.txt".into()],
        dependency_tags: vec!["tag1".into()],
        dependency_recursive: false,
        dependency_depth_limit: 10,
    };
    let json = serde_json::to_string(&args).expect("serialise");
    let back: RevisionSyncArgs = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.revision, "rev1");
    assert!(back.forward_changes);
    assert!(back.reset);
    assert_eq!(back.root_files, vec!["file1.txt", "file2.txt"]);
    assert_eq!(back.dependency_tags, vec!["tag1"]);
    assert!(!back.dependency_recursive);
    assert_eq!(back.dependency_depth_limit, 10);
}

#[test]
fn result_round_trips_through_json() {
    let result = RevisionSyncResult {
        files: vec![SyncFileEntry {
            path: "src/lib.rs".into(),
            size: 2048,
            action: "Add".into(),
            is_file: true,
        }],
        revisions: vec![SyncRevisionInfo {
            branch: "main".into(),
            revision: "deadbeef".into(),
            revision_number: 7,
            is_merge: false,
            has_conflicts: false,
        }],
        files_updated: 1,
        files_deleted: 0,
    };
    let json = serde_json::to_string(&result).expect("serialise");
    let back: RevisionSyncResult = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.files.len(), 1);
    assert_eq!(back.files[0].path, "src/lib.rs");
    assert_eq!(back.revisions.len(), 1);
    assert_eq!(back.revisions[0].revision, "deadbeef");
    assert_eq!(back.files_updated, 1);
}

#[test]
fn default_args_deserialize_from_empty_json() {
    let json = r#"{}"#;
    let args: RevisionSyncArgs = serde_json::from_str(json).expect("should deserialize");
    assert_eq!(args.revision, "");
    assert!(!args.forward_changes);
    assert!(!args.reset);
    assert!(args.root_files.is_empty());
    assert!(args.dependency_tags.is_empty());
    assert!(!args.dependency_recursive);
    assert_eq!(args.dependency_depth_limit, 0);
}

#[test]
fn empty_result_serializes() {
    let result = RevisionSyncResult::default();
    let json = serde_json::to_string(&result).expect("should serialize");
    let back: RevisionSyncResult = serde_json::from_str(&json).expect("deserialise");
    assert!(back.files.is_empty());
    assert!(back.revisions.is_empty());
    assert_eq!(back.files_updated, 0);
    assert_eq!(back.files_deleted, 0);
}

#[test]
fn merge_result_serializes() {
    let result = RevisionSyncResult {
        files: vec![],
        revisions: vec![SyncRevisionInfo {
            branch: "feature".into(),
            revision: "cafe1234".into(),
            revision_number: 0,
            is_merge: true,
            has_conflicts: true,
        }],
        files_updated: 0,
        files_deleted: 0,
    };
    let json = serde_json::to_string(&result).expect("serialise");
    let back: RevisionSyncResult = serde_json::from_str(&json).expect("deserialise");
    assert!(back.revisions[0].is_merge);
    assert!(back.revisions[0].has_conflicts);
    assert_eq!(back.revisions[0].revision_number, 0);
}
