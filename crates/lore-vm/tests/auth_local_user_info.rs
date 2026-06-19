//! Integration test for auth local_user_info operation.

use lore_vm::api::LoreApi;
use lore_vm::ops::auth::local_user_info::{
    LocalUserInfo, LocalUserInfoArgs, LocalUserInfoResult, LocalUserTokenInfo,
};
use tempfile::TempDir;

#[test]
fn test_local_user_info_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = LocalUserInfoArgs {
        auth_endpoint: "ucs-auth://auth.example.com".to_string(),
        user_ids: vec!["user1".to_string(), "user2".to_string()],
        with_token: false,
    };
    assert_eq!(args.auth_endpoint, "ucs-auth://auth.example.com");
    assert_eq!(args.user_ids.len(), 2);
    assert_eq!(args.user_ids[0], "user1");
    assert!(!args.with_token);
}

#[test]
fn test_local_user_info_args_defaults() {
    let args = LocalUserInfoArgs {
        auth_endpoint: String::new(),
        user_ids: vec![],
        with_token: false,
    };
    assert_eq!(args.auth_endpoint, "");
    assert!(args.user_ids.is_empty());
    assert!(!args.with_token);
}

#[test]
fn test_local_user_info_args_with_token() {
    let args = LocalUserInfoArgs {
        auth_endpoint: "ucs-auth://auth.example.com".to_string(),
        user_ids: vec!["user1".to_string()],
        with_token: true,
    };
    assert!(args.with_token);
}

#[test]
fn test_local_user_info_args_serialization() {
    let args = LocalUserInfoArgs {
        auth_endpoint: "ucs-auth://auth.example.com".to_string(),
        user_ids: vec!["user1".to_string()],
        with_token: true,
    };
    let json = serde_json::to_string(&args).unwrap();
    let deserialized: LocalUserInfoArgs = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.auth_endpoint, args.auth_endpoint);
    assert_eq!(deserialized.user_ids, args.user_ids);
    assert_eq!(deserialized.with_token, args.with_token);
}

#[test]
fn test_local_user_info_result_serialization() {
    let result = LocalUserInfoResult {
        users: vec![LocalUserInfo {
            user_id: "user1".into(),
            display_name: "User One".into(),
        }],
        tokens: vec![],
    };
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: LocalUserInfoResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.users.len(), 1);
    assert_eq!(deserialized.users[0].user_id, "user1");
    assert_eq!(deserialized.users[0].display_name, "User One");
    assert!(deserialized.tokens.is_empty());
}

#[test]
fn test_local_user_token_info_serialization() {
    let token_info = LocalUserTokenInfo {
        user_id: "user1".into(),
        display_name: "User One".into(),
        token: "eyJhbGciOiJSUzI1NiJ9.test".into(),
        preferred_username: "userone".into(),
        is_service_account: false,
        expires: 1750000000000,
    };
    let json = serde_json::to_string(&token_info).unwrap();
    let deserialized: LocalUserTokenInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.user_id, "user1");
    assert_eq!(deserialized.display_name, "User One");
    assert_eq!(deserialized.token, "eyJhbGciOiJSUzI1NiJ9.test");
    assert_eq!(deserialized.preferred_username, "userone");
    assert!(!deserialized.is_service_account);
    assert_eq!(deserialized.expires, 1750000000000);
}

#[test]
fn test_local_user_info_result_with_tokens() {
    let result = LocalUserInfoResult {
        users: vec![LocalUserInfo {
            user_id: "user2".into(),
            display_name: "User Two".into(),
        }],
        tokens: vec![LocalUserTokenInfo {
            user_id: "user1".into(),
            display_name: "User One".into(),
            token: "jwt-token-string".into(),
            preferred_username: "userone".into(),
            is_service_account: true,
            expires: 0,
        }],
    };
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: LocalUserInfoResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.users.len(), 1);
    assert_eq!(deserialized.tokens.len(), 1);
    assert!(deserialized.tokens[0].is_service_account);
    assert_eq!(deserialized.tokens[0].expires, 0);
}

#[test]
fn test_local_user_info_args_json_deserialization_with_defaults() {
    let json = r#"{"auth_endpoint":"ucs-auth://example.com"}"#;
    let args: LocalUserInfoArgs = serde_json::from_str(json).unwrap();
    assert_eq!(args.auth_endpoint, "ucs-auth://example.com");
    assert!(args.user_ids.is_empty());
    assert!(!args.with_token);
}
