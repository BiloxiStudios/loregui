//! Integration test for repository verify_fragment operation.
//!
//! Tests the lore-vm::ops::repository::verify_fragment binding.

use lore_vm::api::LoreApi;
use lore_vm::ops::repository::verify_fragment::{
    verify_fragment, VerifyFragmentArgs, VerifyFragmentResult,
};
use tempfile::TempDir;

#[test]
fn test_verify_fragment_args_construction() {
    let args = VerifyFragmentArgs {
        hash: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
        context: "".to_string(),
        heal: false,
    };

    assert_eq!(args.hash.len(), 64);
    assert!(args.context.is_empty());
    assert!(!args.heal);
}

#[test]
fn test_verify_fragment_args_with_context_and_heal() {
    let args = VerifyFragmentArgs {
        hash: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
        context: "some-context".to_string(),
        heal: true,
    };

    assert!(!args.context.is_empty());
    assert!(args.heal);
}

#[test]
fn test_verify_fragment_args_serde_roundtrip() {
    let args = VerifyFragmentArgs {
        hash: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
        context: "ctx".to_string(),
        heal: true,
    };

    let json = serde_json::to_string(&args).expect("serialize");
    let deser: VerifyFragmentArgs = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deser.hash, args.hash);
    assert_eq!(deser.context, args.context);
    assert_eq!(deser.heal, args.heal);
}

#[test]
fn test_verify_fragment_result_serde() {
    use lore_vm::ops::repository::verify_fragment::{FragmentMatch, VerifyFragmentLocalResult};

    let result = VerifyFragmentResult::Local(VerifyFragmentLocalResult {
        hash: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".into(),
        group_index: 1,
        bucket_index: 42,
        index_path: "/tmp/index".into(),
        entry_count: 100,
        packfile_entry_count: 50,
        match_count: 1,
        matches: vec![FragmentMatch {
            slot: 0,
            index: 0,
            repository: "00000000-0000-0000-0000-000000000001".into(),
            address_hash: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".into(),
            address_context: "00000000000000000000000000000000".into(),
            flags: 0,
            size_payload: 1024,
            size_content: 2048,
            pack_offset: 0,
            pack_file: 0,
            last_access: 1718000000,
        }],
        error: String::new(),
    });

    let json = serde_json::to_string(&result).expect("serialize");
    let deser: VerifyFragmentResult = serde_json::from_str(&json).expect("deserialize");

    match deser {
        VerifyFragmentResult::Local(local) => {
            assert_eq!(local.match_count, 1);
            assert_eq!(local.matches.len(), 1);
            assert_eq!(local.matches[0].size_payload, 1024);
        }
        _ => panic!("expected Local variant"),
    }
}

#[tokio::test]
async fn test_verify_fragment_no_repository() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let api = LoreApi::new(temp_dir.path().to_path_buf());

    let args = VerifyFragmentArgs {
        hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        context: "".to_string(),
        heal: false,
    };

    let result = verify_fragment(&api, args).await;
    assert!(result.is_err(), "should fail without a valid repository");
}
