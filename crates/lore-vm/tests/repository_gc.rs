//! Integration test for repository gc operation.
//!
//! Tests the lore-vm::ops::repository::gc binding.

use lore_vm::api::LoreApi;
use lore_vm::ops::repository::gc::{gc, GcResult};
use tempfile::TempDir;

#[test]
fn test_gc_result_serde_roundtrip() {
    let result = GcResult {
        log_messages: vec!["collected 5 fragments".into(), "done".into()],
    };

    let json = serde_json::to_string(&result).expect("serialize");
    let deser: GcResult = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deser.log_messages.len(), 2);
    assert_eq!(deser.log_messages[0], "collected 5 fragments");
    assert_eq!(deser.log_messages[1], "done");
}

#[test]
fn test_gc_result_empty_default() {
    let result = GcResult::default();

    let json = serde_json::to_string(&result).expect("serialize");
    assert!(json.contains("\"log_messages\":[]"));

    let deser: GcResult = serde_json::from_str(&json).expect("deserialize");
    assert!(deser.log_messages.is_empty());
}

#[tokio::test]
async fn test_gc_no_repository() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let api = LoreApi::new(temp_dir.path().to_path_buf());

    let result = gc(&api).await;
    assert!(result.is_err(), "gc should fail without a valid repository");
}
