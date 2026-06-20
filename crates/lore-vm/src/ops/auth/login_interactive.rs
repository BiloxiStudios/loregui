//! `auth::login_interactive` — authenticate against a remote via browser-based flow.
//!
//! Binds [`lore::auth::login_interactive`] in-process (no CLI shelling).
//! Emits `LoreEvent::AuthUserInfo` on success containing user id + display name,
//! and (in `no_browser` mode) `LoreEvent::AuthUrl` with the login URL.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::auth::LoreAuthLoginInteractiveArgs;
use lore::interface::{LoreEvent, LoreString};
use serde::{Deserialize, Serialize};

/// Arguments for [`login_interactive`].
///
/// Mirrors `LoreAuthLoginInteractiveArgs` from the upstream `lore` crate
/// but uses plain `String`/`bool` so it serialises cleanly across the Tauri
/// boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginInteractiveArgs {
    /// Remote URL; empty resolves from the repository config.
    #[serde(default)]
    pub remote_url: String,
    /// Emit the login URL instead of opening a browser.
    #[serde(default)]
    pub no_browser: bool,
}

impl LoginInteractiveArgs {
    fn into_lore(self) -> LoreAuthLoginInteractiveArgs {
        LoreAuthLoginInteractiveArgs {
            remote_url: LoreString::from_str(&self.remote_url),
            no_browser: u8::from(self.no_browser),
        }
    }
}

/// Result returned on successful interactive login.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginInteractiveResult {
    /// User identity ID.
    pub user_id: String,
    /// Display name.
    pub display_name: String,
    /// Login URL, populated only in `no_browser` mode.
    #[serde(default)]
    pub auth_url: String,
}

/// Authenticate against a remote URL via browser-based interactive login.
///
/// Calls the upstream `lore::auth::login_interactive` in-process and collects
/// the `AuthUserInfo` (and optional `AuthUrl`) events to return a typed result.
pub async fn login_interactive(
    api: &LoreApi,
    args: LoginInteractiveArgs,
) -> Result<LoginInteractiveResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::auth::login_interactive(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("login_interactive failed with status {status}"),
        )));
    }

    let mut auth_url = String::new();
    for event in &stream.events {
        if let LoreEvent::AuthUrl(data) = event {
            auth_url = data.url.as_str().to_string();
        }
    }

    let (user_id, display_name) = stream.auth_user_info().unwrap_or_default();

    Ok(LoginInteractiveResult {
        user_id,
        display_name,
        auth_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_deserialise_defaults() {
        let args: LoginInteractiveArgs = serde_json::from_str("{}").expect("deserialise");
        assert!(args.remote_url.is_empty());
        assert!(!args.no_browser);
    }

    #[test]
    fn args_into_lore_maps_fields() {
        let args = LoginInteractiveArgs {
            remote_url: "https://api.example.com".into(),
            no_browser: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.remote_url.as_str(), "https://api.example.com");
        assert_eq!(lore_args.no_browser, 1);
    }

    #[test]
    fn result_serialises() {
        let result = LoginInteractiveResult {
            user_id: "u-1".into(),
            display_name: "Alice".into(),
            auth_url: String::new(),
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("u-1"));
        assert!(json.contains("Alice"));
    }
}
