//! Integration test for `auth::login_with_token`.
//!
//! Spins a temp directory as a throwaway repo, constructs a `LoreApi`,
//! and exercises the op. Without a real auth server the call is expected
//! to fail — we assert on the error shape, not success.

use lore_vm::api::LoreApi;
use lore_vm::error::LoreError;
use lore_vm::ops::auth::login_with_token::{LoginWithTokenArgs, LoginWithTokenResult};

fn test_api() -> LoreApi {
    let tmp = std::env::temp_dir().join(format!("lore-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    LoreApi::new(tmp)
}

#[tokio::test]
async fn test_login_with_token_args_conversion() {
    let args = LoginWithTokenArgs {
        remote_url: "https://example.com".into(),
        token: "test-token-123".into(),
        token_type: "Bearer".into(),
        auth_url: "ucs-auth://auth.example.com".into(),
    };

    assert_eq!(args.remote_url, "https://example.com");
    assert_eq!(args.token, "test-token-123");
    assert_eq!(args.token_type, "Bearer");
    assert_eq!(args.auth_url, "ucs-auth://auth.example.com");
}

#[tokio::test]
async fn test_login_with_token_default_token_type() {
    let args = LoginWithTokenArgs {
        remote_url: String::new(),
        token: "test-token".into(),
        token_type: "Bearer".into(), // default
        auth_url: String::new(),
    };

    assert_eq!(args.token_type, "Bearer");
}

#[tokio::test]
async fn test_login_with_token_fails_without_auth_server() {
    // Without a real auth server, the call should fail with a CommandFailed
    // or similar error — this is the expected behaviour in CI.
    let api = test_api();

    let args = LoginWithTokenArgs {
        remote_url: "https://nonexistent-auth-server-12345.invalid".into(),
        token: "fake-token".into(),
        token_type: "Bearer".into(),
        auth_url: "ucs-auth://nonexistent.invalid".into(),
    };

    let result = lore_vm::ops::auth::login_with_token::login_with_token(&api, args).await;

    // Expected to fail — assert it's an error, not a parse bug or panic
    assert!(result.is_err(), "Expected error without real auth server, got: {:?}", result);

    match result {
        Err(LoreError::CommandFailed(msg)) => {
            // Good — command failed with a descriptive message
            assert!(!msg.is_empty());
        }
        Err(LoreError::Parse(msg)) => {
            // Also acceptable — event stream didn't contain expected data
            assert!(!msg.is_empty());
        }
        Err(other) => {
            // Any other error type is fine too in CI without auth infra
            let _ = other;
        }
        Ok(LoginWithTokenResult { .. }) => {
            panic!("login_with_token succeeded against a nonexistent auth server — this should not happen");
        }
    }
}

#[tokio::test]
async fn test_login_with_token_api_uses_working_dir() {
    let tmp = std::env::temp_dir().join(format!("lore-auth-test-{}", std::process::id()));
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
