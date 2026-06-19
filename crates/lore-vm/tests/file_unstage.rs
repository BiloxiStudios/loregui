//! Integration test for file unstage operation.
//!
//! Tests the lore-vm::ops::file::unstage binding types against
//! serialization round-trips and construction correctness.

use lore_vm::api::LoreApi;
use lore_vm::ops::file::unstage::{
    FileUnstageArgs, FileUnstageCounts, FileUnstageEntry, FileUnstageResult,
};
use tempfile::TempDir;

#[test]
fn test_file_unstage_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = FileUnstageArgs {
        paths: vec!["src/main.rs".to_string()],
    };
    assert_eq!(args.paths.len(), 1);
    assert_eq!(args.paths[0], "src/main.rs");
}

#[test]
fn test_file_unstage_args_multiple_paths() {
    let args = FileUnstageArgs {
        paths: vec!["a.txt".into(), "b.txt".into(), "c/d.rs".into()],
    };
    assert_eq!(args.paths.len(), 3);
    assert_eq!(args.paths[0], "a.txt");
    assert_eq!(args.paths[1], "b.txt");
    assert_eq!(args.paths[2], "c/d.rs");
}

#[test]
fn test_file_unstage_args_empty_paths() {
    let args = FileUnstageArgs { paths: vec![] };
    assert!(args.paths.is_empty());
}

#[test]
fn test_file_unstage_entry_fields() {
    let entry = FileUnstageEntry {
        path: "assets/texture.png".into(),
        action: "keep".into(),
    };
    assert_eq!(entry.path, "assets/texture.png");
    assert_eq!(entry.action, "keep");

    let json = serde_json::to_string(&entry).expect("should serialize");
    let deserialized: FileUnstageEntry = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.path, entry.path);
    assert_eq!(deserialized.action, entry.action);
}

#[test]
fn test_file_unstage_counts_fields() {
    let counts = FileUnstageCounts {
        directory_unstaged_count: 2,
        directory_discarded_count: 1,
        file_unstaged_count: 5,
        file_discarded_count: 3,
        total_count: 11,
    };
    assert_eq!(counts.directory_unstaged_count, 2);
    assert_eq!(counts.directory_discarded_count, 1);
    assert_eq!(counts.file_unstaged_count, 5);
    assert_eq!(counts.file_discarded_count, 3);
    assert_eq!(counts.total_count, 11);

    let json = serde_json::to_string(&counts).expect("should serialize");
    let deserialized: FileUnstageCounts = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.total_count, 11);
    assert_eq!(deserialized.file_unstaged_count, 5);
}

#[test]
fn test_file_unstage_result_multiple_entries() {
    let result = FileUnstageResult {
        files: vec![
            FileUnstageEntry {
                path: "a.txt".into(),
                action: "keep".into(),
            },
            FileUnstageEntry {
                path: "b/c.rs".into(),
                action: "delete".into(),
            },
        ],
        counts: FileUnstageCounts {
            directory_unstaged_count: 0,
            directory_discarded_count: 0,
            file_unstaged_count: 1,
            file_discarded_count: 1,
            total_count: 2,
        },
    };

    assert_eq!(result.files.len(), 2);
    assert_eq!(result.files[0].path, "a.txt");
    assert_eq!(result.files[0].action, "keep");
    assert_eq!(result.files[1].path, "b/c.rs");
    assert_eq!(result.files[1].action, "delete");
    assert_eq!(result.counts.total_count, 2);

    let json = serde_json::to_string(&result).expect("should serialize");
    assert!(json.contains("a.txt"));
    assert!(json.contains("b/c.rs"));
}

#[test]
fn test_file_unstage_result_empty() {
    let result = FileUnstageResult {
        files: vec![],
        counts: FileUnstageCounts::default(),
    };
    assert!(result.files.is_empty());
    assert_eq!(result.counts.total_count, 0);

    let json = serde_json::to_string(&result).expect("should serialize");
    assert!(json.contains("[]"));
}

#[test]
fn test_file_unstage_args_serde_round_trip() {
    let args = FileUnstageArgs {
        paths: vec!["x.txt".into(), "y/z.md".into()],
    };
    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: FileUnstageArgs = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.paths, args.paths);
}
