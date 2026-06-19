//! Integration test for notification subscribe operation.
//!
//! Tests the lore-vm::ops::notification::subscribe binding.

use lore_vm::ops::notification::subscribe::SubscribeResult;

#[test]
fn test_subscribe_result_serializes() {
    let result = SubscribeResult::default();
    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: SubscribeResult = serde_json::from_str(&json).expect("should deserialize");
    let _ = deserialized;
}

#[test]
fn test_subscribe_result_debug() {
    let result = SubscribeResult::default();
    let debug = format!("{:?}", result);
    assert!(debug.contains("SubscribeResult"));
}

#[test]
fn test_subscribe_result_clone() {
    let result = SubscribeResult::default();
    let cloned = result.clone();
    let json_a = serde_json::to_string(&result).unwrap();
    let json_b = serde_json::to_string(&cloned).unwrap();
    assert_eq!(json_a, json_b);
}
