//! `auth::logout` — removes stored authentication and authorization tokens.
//!
//! Binds [`lore::auth::logout`] in-process (no CLI shelling).
//! Emits no result events on success; returns `Result<()>`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::auth::LoreAuthLogoutArgs;
use lore::interface::LoreString;
use serde::{Deserialize, Serialize};

/// Arguments for [`logout`].
///
/// Mirrors `LoreAuthLogoutArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoutArgs {
    /// Auth service URL (e.g. `ucs-auth://auth.example.com`);
    /// empty resolves from the current repository's remote config.
    #[serde(default)]
    pub auth_url: String,
    /// Resource ID (e.g. `urc-{id}`);
    /// empty removes all tokens for the auth URL.
    #[serde(default)]
    pub resource: String,
    /// User identity to remove; empty removes all identities
    /// for the auth URL.
    #[serde(default)]
    pub user_id: String,
}

impl LogoutArgs {
    fn into_lore(self) -> LoreAuthLogoutArgs {
        LoreAuthLogoutArgs {
            auth_url: LoreString::from_str(&self.auth_url),
            resource: LoreString::from_str(&self.resource),
            user_id: LoreString::from_str(&self.user_id),
        }
    }
}

/// Removes stored authentication and authorization tokens.
///
/// Calls the upstream `lore::auth::logout` in-process and waits for completion.
///
/// Behavior depends on which arguments are provided:
/// - `auth_url` empty: resolved from the current repository's remote config.
/// - `user_id` empty: removes all identities for the auth URL.
/// - `user_id` set, `resource` empty: removes the user's authentication token
///   and all authorization tokens for the auth URL.
/// - `user_id` set, `resource` set: removes only the specific authorization
///   token for that resource.
pub async fn logout(api: &LoreApi, args: LogoutArgs) -> Result<()> {
    let (callback, rx) = collect_events();

    let status =
        lore::auth::logout(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("logout failed with status {status}"),
        )));
    }

    Ok(())
}
