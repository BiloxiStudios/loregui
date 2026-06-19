//! Integration test for auth login_with_token operation.
//!
//! Tests the lore-vm::ops::auth::login_with_token binding types against
//! serialization round-trips and construction correctness.

use lore_vm::api::LoreApi;
use lore_vm::ops::auth::login_with_token::{LoginWithTokenArgs, LoginWithTokenResult};
use tempfile::TempDir;

#[test]
fn test_login_with_token_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = LoginWithTokenArgs {
        remote_url: "https://lore.example.com".to_string(),
        token: "my-secret-token".to_string(),
        token_type: "Bearer".to_string(),
        auth_url: "ucs-auth://auth.example.com".to_string(),
    };

    assert_eq!(args.remote_url, "https://lore.example.com");
    assert_eq!(args.token, "my-secret-token");
    assert_eq!(args.token_type, "Bearer");
    assert_eq!(args.auth_url, "ucs-auth://auth.example.com");
}

#[test]
fn test_login_with_token_args_default_token_type() {
    let args = LoginWithTokenArgs {
        remote_url: String::new(),
        token: "token123".to_string(),
        token_type: "Bearer".to_string(),
        auth_url: String::new(),
    };
    assert_eq!(args.token_type, "Bearer");
}

#[test]
fn test_login_with_token_args_jwt_token_type() {
    let args = LoginWithTokenArgs {
        remote_url: "https://lore.example.com".to_string(),
        token: "jwt-token".to_string(),
        token_type: "JWT".to_string(),
        auth_url: String::new(),
    };
    assert_eq!(args.token_type, "JWT");
}

#[test]
fn test_login_with_token_args_empty_remote_url() {
    let args = LoginWithTokenArgs {
        remote_url: String::new(),
        token: "my-token".to_string(),
        token_type: "Bearer".to_string(),
        auth_url: "ucs-auth://auth.example.com".to_string(),
    };
    assert_eq!(args.remote_url, "");
    assert_eq!(args.auth_url, "ucs-auth://auth.example.com");
}

#[test]
fn test_login_with_token_result_fields() {
    let result = LoginWithTokenResult {
        user_id: "user-abc-123".to_string(),
        display_name: "Alice Developer".to_string(),
    };

    assert_eq!(result.user_id, "user-abc-123");
    assert_eq!(result.display_name, "Alice Developer");

    let json = serde_json::to_string(&result).expect("should serialize");
    let deserialized: LoginWithTokenResult =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(deserialized.user_id, result.user_id);
    assert_eq!(deserialized.display_name, result.display_name);
}

#[test]
fn test_login_with_token_result_empty_user() {
    let result = LoginWithTokenResult {
        user_id: String::new(),
        display_name: String::new(),
    };
    assert_eq!(result.user_id, "");
    assert_eq!(result.display_name, "");
}

#[test]
fn test_login_with_token_args_serialization() {
    let args = LoginWithTokenArgs {
        remote_url: "https://lore.example.com".to_string(),
        token: "secret-token".to_string(),
        token_type: "Bearer".to_string(),
        auth_url: "ucs-auth://auth.example.com".to_string(),
    };

    let json = serde_json::to_string(&args).expect("should serialize");
    let deserialized: LoginWithTokenArgs = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.remote_url, args.remote_url);
    assert_eq!(deserialized.token, args.token);
    assert_eq!(deserialized.token_type, args.token_type);
    assert_eq!(deserialized.auth_url, args.auth_url);
}

#[test]
fn test_login_with_token_args_with_special_characters() {
    let args = LoginWithTokenArgs {
        remote_url: "https://lore.example.com:8080/path?query=value".to_string(),
        token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test.token".to_string(),
        token_type: "JWT".to_string(),
        auth_url: "ucs-auth://auth.example.com:9000".to_string(),
    };

    assert_eq!(
        args.remote_url,
        "https://lore.example.com:8080/path?query=value"
    );
    assert!(args.token.contains('.'));
    assert_eq!(args.token_type, "JWT");
}
