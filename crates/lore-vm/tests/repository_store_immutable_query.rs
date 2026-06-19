//! Integration test for repository store_immutable_query operation.
//!
//! Tests the lore-vm::ops::repository::store_immutable_query binding.

use lore_vm::api::LoreApi;
use lore_vm::ops::repository::store_immutable_query::{
    store_immutable_query, StoreImmutableQueryArgs, StoreImmutableQueryEntry,
    StoreImmutableQueryResult,
};
use tempfile::TempDir;

#[test]
fn test_args_construction() {
    let args = StoreImmutableQueryArgs {
        address: "abcdef0123456789".to_string(),
        recurse: true,
    };

    assert_eq!(args.address, "abcdef0123456789");
    assert!(args.recurse);
}

#[test]
fn test_args_construction_no_recurse() {
    let args = StoreImmutableQueryArgs {
        address: "deadbeef".to_string(),
        recurse: false,
    };

    assert_eq!(args.address, "deadbeef");
    assert!(!args.recurse);
}

#[test]
fn test_args_serde_roundtrip() {
    let args = StoreImmutableQueryArgs {
        address: "abc123def456".to_string(),
        recurse: true,
    };

    let json = serde_json::to_string(&args).expect("serialize");
    let deser: StoreImmutableQueryArgs = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deser.address, args.address);
    assert_eq!(deser.recurse, args.recurse);
}

#[test]
fn test_result_serde_roundtrip() {
    let result = StoreImmutableQueryResult {
        entries: vec![
            StoreImmutableQueryEntry {
                address: "abc123".into(),
                remote: false,
                status: 0,
                status_label: "stored".into(),
                payload: true,
                subfragment: false,
                flags: 0x1,
                payload_size: 1024,
                content_size: 2048,
            },
            StoreImmutableQueryEntry {
                address: "def456".into(),
                remote: true,
                status: 3,
                status_label: "not_found".into(),
                payload: false,
                subfragment: true,
                flags: 0,
                payload_size: 0,
                content_size: 0,
            },
        ],
    };

    let json = serde_json::to_string(&result).expect("serialize");
    let deser: StoreImmutableQueryResult = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deser.entries.len(), 2);
    assert_eq!(deser.entries[0].address, "abc123");
    assert_eq!(deser.entries[0].status, 0);
    assert!(deser.entries[0].payload);
    assert!(!deser.entries[0].subfragment);
    assert_eq!(deser.entries[1].address, "def456");
    assert!(deser.entries[1].remote);
    assert_eq!(deser.entries[1].status, 3);
    assert!(deser.entries[1].subfragment);
}

#[tokio::test]
async fn test_store_immutable_query_no_repository() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let api = LoreApi::new(temp_dir.path().to_path_buf());

    let args = StoreImmutableQueryArgs {
        address: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        recurse: false,
    };

    let result = store_immutable_query(&api, args).await;
    assert!(result.is_err(), "should fail without a valid repository");
}
