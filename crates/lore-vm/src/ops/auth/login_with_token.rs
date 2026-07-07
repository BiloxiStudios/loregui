//! `auth::login_with_token` — authenticate against a remote using a provided token.
//!
//! Binds [`lore::auth::login_with_token`] in-process (no CLI shelling).
//! Emits `LoreEvent::AuthUserInfo` on success containing user id + display name.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::auth::LoreAuthLoginWithTokenArgs;
use lore::interface::LoreString;
use serde::{Deserialize, Serialize};

/// Arguments for [`login_with_token`].
///
/// Mirrors `LoreAuthLoginWithTokenArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginWithTokenArgs {
    /// Remote URL; empty string falls back to the repository config.
    #[serde(default)]
    pub remote_url: String,
    /// Authentication token (e.g. JWT).
    pub token: String,
    /// Token type (e.g. "Bearer", "JWT").
    #[serde(default = "default_token_type")]
    pub token_type: String,
    /// Auth service URL with scheme (e.g. `ucs-auth://auth.example.com`);
    /// used directly when non-empty, required when no remote URL is available.
    #[serde(default)]
    pub auth_url: String,
}

fn default_token_type() -> String {
    "Bearer".into()
}

impl LoginWithTokenArgs {
    fn into_lore(self) -> LoreAuthLoginWithTokenArgs {
        LoreAuthLoginWithTokenArgs {
            remote_url: LoreString::from_str(&self.remote_url),
            token: LoreString::from_str(&self.token),
            token_type: LoreString::from_str(&self.token_type),
            auth_url: LoreString::from_str(&self.auth_url),
        }
    }
}

/// Result returned on successful token-based login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginWithTokenResult {
    /// User identity ID.
    pub user_id: String,
    /// Display name.
    pub display_name: String,
}

/// Authenticate against a remote URL using a provided token.
///
/// Calls the upstream `lore::auth::login_with_token` in-process and collects
/// the `AuthUserInfo` event to return a typed result.
///
/// **Security (SBAI-4933):** On successful login, the verified `user_id` from
/// the auth service is propagated into the global identity via
/// [`LoreApi::set_identity`], so that all subsequent commit attribution uses
/// the server-verified subject rather than any client-provided identity string.
pub async fn login_with_token(
    api: &LoreApi,
    args: LoginWithTokenArgs,
) -> Result<LoginWithTokenResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::auth::login_with_token(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("login_with_token failed with status {status}"),
        )));
    }

    let (user_id, display_name) = stream.auth_user_info().ok_or_else(|| {
        LoreError::Parse("login succeeded but no AuthUserInfo event emitted".into())
    })?;

    // SBAI-4933: Propagate the server-verified user-id into the global identity
    // so that all subsequent commit attribution uses the authenticated subject.
    api.set_identity(&user_id);

    Ok(LoginWithTokenResult {
        user_id,
        display_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_deserialise_defaults() {
        let args: LoginWithTokenArgs =
            serde_json::from_str(r#"{"token":"abc123"}"#).expect("deserialise");
        assert!(args.remote_url.is_empty());
        assert_eq!(args.token, "abc123");
        assert_eq!(args.token_type, "Bearer");
        assert!(args.auth_url.is_empty());
    }

    #[test]
    fn args_into_lore_maps_fields() {
        let args = LoginWithTokenArgs {
            remote_url: "https://api.example.com".into(),
            token: "tok-42".into(),
            token_type: "JWT".into(),
            auth_url: "ucs-auth://auth.example.com".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.remote_url.as_str(), "https://api.example.com");
        assert_eq!(lore_args.token.as_str(), "tok-42");
        assert_eq!(lore_args.token_type.as_str(), "JWT");
        assert_eq!(lore_args.auth_url.as_str(), "ucs-auth://auth.example.com");
    }

    #[test]
    fn result_serialises() {
        let result = LoginWithTokenResult {
            user_id: "u-1".into(),
            display_name: "Alice".into(),
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("u-1"));
        assert!(json.contains("Alice"));
    }

    #[test]
    fn default_token_type_is_bearer() {
        let args: LoginWithTokenArgs =
            serde_json::from_str(r#"{"token":"x"}"#).expect("deserialise");
        assert_eq!(args.token_type, "Bearer");
    }

    // ── SBAI-4933 regression tests ──────────────────────────────────────
    // These prove that commit attribution uses the server-verified subject,
    // not a client-decoded JWT payload.

    /// Simulates the post-login identity propagation: an API constructed with
    /// a forged client identity has it replaced by the verified user-id.
    #[test]
    fn set_identity_replaces_forged_client_identity() {
        use crate::api::LoreApi;

        // Simulate a frontend that set a forged identity (e.g. from an
        // unsigned, client-decoded JWT payload).
        let api = LoreApi::new(std::path::PathBuf::from("/tmp/test-repo"));
        api.global().set_identity("forged-client-payload");

        // Verify the forged identity is set.
        assert_eq!(api.global().get_identity(), "forged-client-payload");

        // Simulate what login_with_token does after the auth service returns
        // the verified user-id: overwrite with the trusted subject.
        let verified_user_id = "user-verified-by-auth-service";
        api.set_identity(verified_user_id);

        // The global identity is now the verified one, NOT the forged one.
        assert_eq!(api.global().get_identity(), verified_user_id);
        assert_ne!(api.global().get_identity(), "forged-client-payload");
    }

    /// Verify that the identity propagated into LoreGlobalArgs (what the lore
    /// engine receives) reflects the post-login verified identity.
    #[test]
    fn globals_build_uses_verified_identity_after_login() {
        use crate::api::LoreApi;

        let api = LoreApi::new(std::path::PathBuf::from("/tmp/test-repo"));
        api.global().set_identity("forged");

        // Simulate post-login identity propagation.
        api.set_identity("verified-subject");

        // The args built from globals must carry the verified identity.
        let args = api.globals().build();
        assert_eq!(args.identity.as_str(), "verified-subject");
    }
}
