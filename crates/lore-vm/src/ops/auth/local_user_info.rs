//! `auth::local_user_info` — resolve user identities from locally stored JWT
//! tokens. Does not require a repository context or network access.
//!
//! Binds [`lore::auth::local_user_info`] in-process (no CLI shelling).
//! Emits `LoreEvent::AuthUserInfo` (or `LoreEvent::AuthUserToken` when
//! `with_token` is set) for each resolved identity.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::auth::LoreAuthLocalUserInfoArgs;
use lore::interface::{LoreArray, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`local_user_info`].
///
/// Mirrors `LoreAuthLocalUserInfoArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalUserInfoArgs {
    /// Auth service remote URL; empty resolves from the repository's remote
    /// environment.
    #[serde(default)]
    pub auth_endpoint: String,
    /// User identities to resolve; empty resolves the current user.
    #[serde(default)]
    pub user_ids: Vec<String>,
    /// Emit cached token details for identities with a local token.
    #[serde(default)]
    pub with_token: bool,
}

impl LocalUserInfoArgs {
    fn into_lore(self) -> LoreAuthLocalUserInfoArgs {
        LoreAuthLocalUserInfoArgs {
            auth_endpoint: LoreString::from_str(&self.auth_endpoint),
            user_ids: LoreArray::from_iter(
                self.user_ids
                    .into_iter()
                    .map(|s| LoreString::from_str(&s)),
            ),
            with_token: u8::from(self.with_token),
        }
    }
}

/// A single resolved identity from [`local_user_info`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedUserInfo {
    /// User identity (sub claim or supplied ID).
    pub id: String,
    /// Display name (preferred_username > name > id).
    pub name: String,
    /// Cached token string — only populated when `with_token` was requested
    /// and a local token exists for this identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Preferred username from the token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    /// Non-zero (true) if the identity is a service account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_service_account: Option<bool>,
    /// Expiry time in milliseconds since UNIX epoch, or 0 if unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<u64>,
}

/// Result returned on successful local user info resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalUserInfoResult {
    /// Resolved identities (may be empty if no identities were found).
    pub identities: Vec<ResolvedUserInfo>,
}

/// Resolve user identities from locally stored JWT tokens.
///
/// Does not require a repository context or network access. Decodes locally
/// cached JWT tokens to extract display names. When `user_ids` is empty,
/// resolves the current user's identity.
///
/// When `with_token` is set, the `token` field in each identity is populated
/// with the decrypted cached token.
pub async fn local_user_info(
    api: &LoreApi,
    args: LocalUserInfoArgs,
) -> Result<LocalUserInfoResult> {
    let (callback, rx) = collect_events();

    let status = lore::auth::local_user_info(
        api.globals().build(),
        args.into_lore(),
        callback,
    )
    .await;

    let stream = rx.await.map_err(|e| {
        LoreError::CommandFailed(format!("event stream cancelled: {e}"))
    })?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(
            stream
                .error
                .unwrap_or_else(|| format!("local_user_info failed with status {status}")),
        ));
    }

    let identities = if args.with_token {
        stream
            .auth_user_token_events()
            .into_iter()
            .map(
                |(id, name, token, preferred_username, is_service_account, expires)| {
                    ResolvedUserInfo {
                        id,
                        name,
                        token: Some(token),
                        preferred_username: Some(preferred_username),
                        is_service_account: Some(is_service_account),
                        expires: Some(expires),
                    }
                },
            )
            .collect()
    } else {
        stream
            .auth_events()
            .into_iter()
            .map(|(id, name, _token)| ResolvedUserInfo {
                id,
                name,
                token: None,
                preferred_username: None,
                is_service_account: None,
                expires: None,
            })
            .collect()
    };

    Ok(LocalUserInfoResult { identities })
}
