//! Integration test for link list operation.
//!
//! Tests the lore-vm::ops::link::list binding against a temporary
//! Lore repository.

use lore_vm::api::LoreApi;
use lore_vm::ops::link::list::{LinkEntry, LinkListResult};
use tempfile::TempDir;

#[test]
fn test_link_list_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);
}

#[test]
fn test_link_entry_fields() {
    let entry = LinkEntry {
        link_path: "deps/characters".into(),
        link: "city-of-brains".into(),
        source_node: "root".into(),
        link_node: "nodes/1".into(),
    };

    assert_eq!(entry.link_path, "deps/characters");
    assert_eq!(entry.link, "city-of-brains");
    assert_eq!(entry.source_node, "root");
    assert_eq!(entry.link_node, "nodes/1");

    let json = serde_json::to_string(&entry).expect("should serialize");
    assert!(json.contains("\"link_path\":\"deps/characters\""));
    assert!(json.contains("\"link\":\"city-of-brains\""));
}

#[test]
fn test_link_list_result_serialization() {
    let result = LinkListResult {
        link_count: 2,
        links: vec![
            LinkEntry {
                link_path: "deps/characters".into(),
                link: "city-of-brains".into(),
                source_node: "root".into(),
                link_node: "nodes/1".into(),
            },
            LinkEntry {
                link_path: "deps/world".into(),
                link: "world-builder".into(),
                source_node: "main".into(),
                link_node: "nodes/2".into(),
            },
        ],
    };

    assert_eq!(result.link_count, 2);
    assert_eq!(result.links.len(), 2);

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: LinkListResult = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.link_count, 2);
    assert_eq!(deserialized.links[0].link_path, "deps/characters");
    assert_eq!(deserialized.links[1].link, "world-builder");
}

#[test]
fn test_empty_link_list_result() {
    let result = LinkListResult {
        link_count: 0,
        links: vec![],
    };

    assert_eq!(result.link_count, 0);
    assert!(result.links.is_empty());

    let json = serde_json::to_string(&result).expect("should serialize");
    assert!(json.contains("\"link_count\":0"));
    assert!(json.contains("\"links\":[]"));
}
