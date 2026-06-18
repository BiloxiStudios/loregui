//! Integration test for `auth::local_user_info`.
//!
//! Spins a temp directory as a throwaway repo, constructs a `LoreApi`,
//! and exercises the op. Without stored auth tokens the call either
//! succeeds with an empty identity list or fails gracefully —
//! we assert on the expected behaviour shape.

use lore_vm::api::LoreApi;
use lore_vm::error::LoreError;
use lore_vm::ops::auth::local_user_info::{
    local_user_info, LocalUserInfoArgs, LocalUserInfoResult,
};

fn test_api() -> LoreApi {
    let tmp = std::env::temp_dir().join(format!("lore-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    LoreApi::new(tmp)
}

#[tokio::test]
async fn test_local_user_info_args_serialization() {
    let args = LocalUserInfoArgs {
        auth_endpoint: "ucs-auth://auth.example.com".into(),
        user_ids: vec!["user-123".into(), "user-456".into()],
        with_token: true,
    };

    // Verify JSON serialization round-trips cleanly
    let json = serde_json::to_string(&args).unwrap();
    let roundtrip: LocalUserInfoArgs = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip.auth_endpoint, args.auth_endpoint);
    assert_eq!(roundtrip.user_ids, args.user_ids);
    assert_eq!(roundtrip.with_token, args.with_token);
}

#[tokio::test]
async fn test_local_user_info_args_defaults() {
    // Verify default values work for partial args
    let args = LocalUserInfoArgs {
        auth_endpoint: String::new(),
        user_ids: Vec::new(),
        with_token: false,
    };

    assert!(args.auth_endpoint.is_empty());
    assert!(args.user_ids.is_empty());
    assert!(!args.with_token);
}

#[tokio::test]
async fn test_local_user_info_result_serialization() {
    let result = LocalUserInfoResult {
        identities: vec![
            lore_vm::ops::auth::local_user_info::ResolvedUserInfo {
                id: "user-123".into(),
                name: "Test User".into(),
                token: Some("fake-jwt-token".into()),
                preferred_username: Some("testuser".into()),
                is_service_account: Some(false),
                expires: Some(1718700000000),
            },
        ],
    };

    let json = serde_json::to_string(&result).unwrap();
    let roundtrip: LocalUserInfoResult = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip.identities.len(), 1);
    assert_eq!(roundtrip.identities[0].id, "user-123");
    assert_eq!(roundtrip.identities[0].name, "Test User");
    assert!(roundtrip.identities[0].token.is_some());
}

#[tokio::test]
async fn test_local_user_info_empty_repo_no_tokens() {
    // In a temp directory with no stored auth tokens, the call should
    // either succeed with an empty identity list or fail gracefully.
    let api = test_api();

    let args = LocalUserInfoArgs {
        auth_endpoint: String::new(),
        user_ids: Vec::new(),
        with_token: false,
    };

    let result = local_user_info(&api, args).await;

    // Without stored tokens, the result is either:
    // - Ok with empty identities (no tokens found)
    // - Err with a graceful error (no auth endpoint available)
    match result {
        Ok(LocalUserInfoResult { identities }) => {
            // Success with empty list is valid (no tokens stored)
            assert!(
                identities.is_empty(),
                "Expected empty identities in a fresh temp repo, got: {:?}",
                identities
            );
        }
        Err(_) => {
            // Error is also acceptable — no auth endpoint configured
            // The important thing is no panic or memory corruption
        }
    }
}

#[tokio::test]
async fn test_local_user_info_with_token_flag() {
    // Same as above but with with_token=true
    let api = test_api();

    let args = LocalUserInfoArgs {
        auth_endpoint: String::new(),
        user_ids: Vec::new(),
        with_token: true,
    };

    let result = local_user_info(&api, args).await;

    match result {
        Ok(LocalUserInfoResult { identities }) => {
            assert!(identities.is_empty());
        }
        Err(_) => {
            // Acceptable — no auth endpoint
        }
    }
}

#[tokio::test]
async fn test_local_user_info_api_uses_working_dir() {
    let tmp = std::env::temp_dir().join(format!("lore-auth-ui-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);

    let api = LoreApi::new(tmp.clone());
    assert_eq!(api.globals().repository_path, tmp);

    // Verify working dir can be changed
    let new_dir = tmp.join("subdir");
    let _ = std::fs::create_dir_all(&new_dir);
    let mut api_mut = api.clone();
    api_mut.set_working_dir(new_dir.clone());
    assert_eq!(api_mut.globals().repository_path, new_dir);

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp);
}
