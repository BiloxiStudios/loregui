//! Integration test for file info operation.
//!
//! Tests the lore-vm::ops::file::info binding types against
//! serialization round-trips and construction correctness.

use lore_vm::api::LoreApi;
use lore_vm::ops::file::info::{FileInfoArgs, FileInfoEntry, FileInfoResult};
use tempfile::TempDir;

#[test]
fn test_file_info_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = FileInfoArgs {
        paths: vec!["src/main.rs".to_string()],
        revision: String::new(),
        local: false,
        filtered: false,
    };
    assert_eq!(args.paths.len(), 1);
    assert_eq!(args.paths[0], "src/main.rs");
}

#[test]
fn test_file_info_args_multiple_paths() {
    let args = FileInfoArgs {
        paths: vec!["a.txt".into(), "b.txt".into(), "c/d.rs".into()],
        revision: "abc123".into(),
        local: true,
        filtered: true,
    };
    assert_eq!(args.paths.len(), 3);
    assert_eq!(args.revision, "abc123");
    assert!(args.local);
    assert!(args.filtered);
}

#[test]
fn test_file_info_args_empty_paths() {
    let args = FileInfoArgs {
        paths: vec![],
        revision: String::new(),
        local: false,
        filtered: false,
    };
    assert!(args.paths.is_empty());
}

#[test]
fn test_file_info_entry_fields() {
    let entry = FileInfoEntry {
        path: "assets/texture.png".into(),
        context: "ctx-123".into(),
        hash: "hash-456".into(),
        is_file: true,
        is_dir: false,
        flag_modified: true,
        flag_deleted: false,
        flag_added: false,
        flag_conflict: false,
        mode: 0o100644,
        size: 4096,
        local_size: 4200,
        local_hash: "hash-local".into(),
        filter_size: 3800,
    };

    assert_eq!(entry.path, "assets/texture.png");
    assert!(entry.is_file);
    assert!(!entry.is_dir);
    assert!(entry.flag_modified);
    assert!(!entry.flag_deleted);
    assert_eq!(entry.size, 4096);
    assert_eq!(entry.local_size, 4200);
    assert_eq!(entry.mode, 0o100644);

    let json = serde_json::to_string(&entry).expect("should serialize");
    let deserialized: FileInfoEntry = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.path, entry.path);
    assert_eq!(deserialized.hash, entry.hash);
    assert_eq!(deserialized.is_file, entry.is_file);
    assert_eq!(deserialized.size, entry.size);
}

#[test]
fn test_file_info_result_multiple_entries() {
    let result = FileInfoResult {
        entries: vec![
            FileInfoEntry {
                path: "a.txt".into(),
                context: "c1".into(),
                hash: "h1".into(),
                is_file: true,
                is_dir: false,
                flag_modified: false,
                flag_deleted: false,
                flag_added: true,
                flag_conflict: false,
                mode: 0,
                size: 100,
                local_size: 100,
                local_hash: String::new(),
                filter_size: 0,
            },
            FileInfoEntry {
                path: "dir/".into(),
                context: "c2".into(),
                hash: "h2".into(),
                is_file: false,
                is_dir: true,
                flag_modified: false,
                flag_deleted: false,
                flag_added: false,
                flag_conflict: false,
                mode: 0,
                size: 0,
                local_size: 0,
                local_hash: String::new(),
                filter_size: 0,
            },
        ],
    };

    assert_eq!(result.entries.len(), 2);
    assert!(result.entries[0].is_file);
    assert!(result.entries[1].is_dir);

    let json = serde_json::to_string(&result).expect("should serialize");
    assert!(json.contains("a.txt"));
    assert!(json.contains("dir/"));
}

#[test]
fn test_file_info_result_empty() {
    let result = FileInfoResult { entries: vec![] };
    assert!(result.entries.is_empty());
    let json = serde_json::to_string(&result).expect("should serialize");
    assert!(json.contains("[]"));
}
