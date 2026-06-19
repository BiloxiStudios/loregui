//! `auth::local_user_info` — resolve user identities from locally cached JWTs.
//!
//! Binds [`lore::auth::local_user_info`] in-process (no CLI shelling).
//! Emits [`lore::interface::LoreEvent::AuthUserInfo`] for resolved users
//! and optionally [`lore::interface::LoreEvent::AuthUserToken`] when
//! `with_token` is set and a cached token is available.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::auth::LoreAuthLocalUserInfoArgs;
use lore::interface::{LoreArray, LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`local_user_info`].
///
/// Mirrors `LoreAuthLocalUserInfoArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalUserInfoArgs {
    /// Auth service endpoint URL; empty resolves from the repository remote config.
    #[serde(default)]
    pub auth_endpoint: String,
    /// User IDs to resolve; empty resolves the current user.
    #[serde(default)]
    pub user_ids: Vec<String>,
    /// When true, emit token details for identities with a locally cached token.
    #[serde(default)]
    pub with_token: bool,
}

impl LocalUserInfoArgs {
    fn into_lore(self) -> LoreAuthLocalUserInfoArgs {
        LoreAuthLocalUserInfoArgs {
            auth_endpoint: LoreString::from_str(&self.auth_endpoint),
            user_ids: LoreArray::from_vec(
                self.user_ids
                    .into_iter()
                    .map(|id| LoreString::from_str(&id))
                    .collect(),
            ),
            with_token: u8::from(self.with_token),
        }
    }
}

/// A resolved local user entry (without token details).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalUserInfo {
    pub user_id: String,
    pub display_name: String,
}

/// A resolved local user entry with cached token details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalUserTokenInfo {
    pub user_id: String,
    pub display_name: String,
    pub token: String,
    pub preferred_username: String,
    pub is_service_account: bool,
    pub expires: u64,
}

/// Result returned on successful local user info resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalUserInfoResult {
    pub users: Vec<LocalUserInfo>,
    pub tokens: Vec<LocalUserTokenInfo>,
}

/// Resolve user identities from locally cached JWT tokens.
///
/// Calls the upstream `lore::auth::local_user_info` in-process and collects
/// `AuthUserInfo` and `AuthUserToken` events to return a typed result.
pub async fn local_user_info(
    api: &LoreApi,
    args: LocalUserInfoArgs,
) -> Result<LocalUserInfoResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::auth::local_user_info(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("local_user_info failed with status {status}"),
        )));
    }

    let mut users = Vec::new();
    let mut tokens = Vec::new();

    for event in &stream.events {
        match event {
            LoreEvent::AuthUserInfo(data) => {
                users.push(LocalUserInfo {
                    user_id: data.id.as_str().into(),
                    display_name: data.name.as_str().into(),
                });
            }
            LoreEvent::AuthUserToken(data) => {
                tokens.push(LocalUserTokenInfo {
                    user_id: data.id.as_str().into(),
                    display_name: data.name.as_str().into(),
                    token: data.token.as_str().into(),
                    preferred_username: data.preferred_username.as_str().into(),
                    is_service_account: data.flag_service_account != 0,
                    expires: data.expires,
                });
            }
            _ => {}
        }
    }

    Ok(LocalUserInfoResult { users, tokens })
}
