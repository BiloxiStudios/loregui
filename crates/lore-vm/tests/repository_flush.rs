//! Integration test for repository flush operation.
//!
//! Tests the lore-vm::ops::repository::flush binding.

use lore_vm::api::LoreApi;
use lore_vm::ops::repository::flush::{flush, FlushResult};
use tempfile::TempDir;

#[test]
fn test_flush_result_serde_roundtrip() {
    let result = FlushResult {
        log_messages: vec!["flushed 2 tasks".into(), "done".into()],
    };

    let json = serde_json::to_string(&result).expect("serialize");
    let deser: FlushResult = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deser.log_messages.len(), 2);
    assert_eq!(deser.log_messages[0], "flushed 2 tasks");
    assert_eq!(deser.log_messages[1], "done");
}

#[test]
fn test_flush_result_empty_default() {
    let result = FlushResult::default();

    let json = serde_json::to_string(&result).expect("serialize");
    assert!(json.contains("\"log_messages\":[]"));

    let deser: FlushResult = serde_json::from_str(&json).expect("deserialize");
    assert!(deser.log_messages.is_empty());
}

#[tokio::test]
async fn test_flush_no_repository() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let api = LoreApi::new(temp_dir.path().to_path_buf());

    let result = flush(&api).await;
    // flush uses no_repository_call so it may succeed even without a repo,
    // since it just flushes the global runtime task queue. Either outcome is
    // valid — we only care that it doesn't panic.
    let _ = result;
}
