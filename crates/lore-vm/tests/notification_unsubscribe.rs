//! Integration test for notification unsubscribe operation.
//!
//! Tests the lore-vm::ops::notification::unsubscribe binding.

use lore_vm::ops::notification::unsubscribe::UnsubscribeResult;

#[test]
fn test_unsubscribe_result_serializes() {
    let result = UnsubscribeResult::default();
    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: UnsubscribeResult = serde_json::from_str(&json).expect("should deserialize");
    let _ = deserialized;
}

#[test]
fn test_unsubscribe_result_debug() {
    let result = UnsubscribeResult::default();
    let debug = format!("{:?}", result);
    assert!(debug.contains("UnsubscribeResult"));
}

#[test]
fn test_unsubscribe_result_clone() {
    let result = UnsubscribeResult::default();
    let cloned = result.clone();
    let json_a = serde_json::to_string(&result).unwrap();
    let json_b = serde_json::to_string(&cloned).unwrap();
    assert_eq!(json_a, json_b);
}
