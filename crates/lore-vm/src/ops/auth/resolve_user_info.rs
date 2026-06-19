//! `auth resolve_user_info` operation — binds `lore::auth::resolve_user_info`.
//!
//! Binds [`lore::auth::resolve_user_info`] in-process (no CLI shelling).
//! Emits [`lore::interface::LoreEvent::AuthUserInfo`] for each resolved user.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::auth::LoreAuthUserInfoArgs;
use lore::interface::{LoreArray, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`resolve_user_info`].
///
/// Mirrors `LoreAuthUserInfoArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveUserInfoArgs {
    /// User IDs to resolve; empty resolves the current user locally.
    #[serde(default)]
    pub user_ids: Vec<String>,
}

impl ResolveUserInfoArgs {
    fn into_lore(self) -> LoreAuthUserInfoArgs {
        LoreAuthUserInfoArgs {
            user_ids: LoreArray::from_vec(
                self.user_ids
                    .into_iter()
                    .map(|id| LoreString::from_str(&id))
                    .collect(),
            ),
        }
    }
}

/// A resolved user info entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedUserInfo {
    /// User identity ID.
    pub user_id: String,
    /// Display name.
    pub display_name: String,
}

/// Result returned on successful user info resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveUserInfoResult {
    /// List of resolved users.
    pub users: Vec<ResolvedUserInfo>,
}

/// Resolves user IDs to display names using the remote authentication service.
///
/// Calls the upstream `lore::auth::resolve_user_info` in-process and collects
/// all `AuthUserInfo` events to return a typed result.
pub async fn resolve_user_info(
    api: &LoreApi,
    args: ResolveUserInfoArgs,
) -> Result<ResolveUserInfoResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::auth::resolve_user_info(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("resolve_user_info failed with status {status}"),
        )));
    }

    let mut users = Vec::new();
    for event in &stream.events {
        if let lore::interface::LoreEvent::AuthUserInfo(data) = event {
            users.push(ResolvedUserInfo {
                user_id: data.id.as_str().into(),
                display_name: data.name.as_str().into(),
            });
        }
    }

    Ok(ResolveUserInfoResult { users })
}
