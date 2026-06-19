//! `auth list` operation — binds `lore::auth::list`.
//!
//! Lists all stored authentication identities across all auth endpoints.
//! Calls [`lore::auth::list`] in-process (no CLI shelling) and collects
//! `LoreEvent::AuthIdentity` events to return typed results.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::auth::LoreAuthListArgs;
use lore::interface::LoreEvent;
use serde::{Deserialize, Serialize};

/// Arguments for [`list`].
///
/// Mirrors `LoreAuthListArgs` from the upstream `lore` crate
/// but uses plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListArgs {
    /// When true, include decrypted cached tokens in each identity entry.
    #[serde(default)]
    pub with_token: bool,
}

impl ListArgs {
    fn into_lore(self) -> LoreAuthListArgs {
        LoreAuthListArgs {
            with_token: u8::from(self.with_token),
        }
    }
}

/// A single stored authentication identity entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthIdentityEntry {
    /// Auth service URL (e.g. `ucs-auth://auth.example.com`).
    pub auth_url: String,
    /// Resource identifier (e.g. `urc-{repository_id}`); empty for auth tokens.
    pub resource: String,
    /// User identity ID.
    pub user_id: String,
    /// Comma-separated list of authorised root domains.
    pub authorized_domains: String,
    /// Token expiry as milliseconds since epoch; 0 if no expiry.
    pub expires: u64,
    /// Decrypted cached token (only populated when `with_token` was set).
    pub token: String,
}

/// Result of a successful `auth list` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthListResult {
    /// Number of stored identities found.
    pub identity_count: u32,
    /// Details of each stored identity.
    pub identities: Vec<AuthIdentityEntry>,
}

/// List all stored authentication identities across all auth endpoints.
///
/// Calls upstream `lore::auth::list` in-process, collects the `AuthIdentity`
/// events emitted for each stored identity, and returns a typed result.
pub async fn list(api: &LoreApi, args: ListArgs) -> Result<AuthListResult> {
    let (callback, rx) = collect_events();

    let status = lore::auth::list(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("auth list failed with status {status}"),
        )));
    }

    let mut identities = Vec::new();
    for event in &stream.events {
        if let LoreEvent::AuthIdentity(data) = event {
            identities.push(AuthIdentityEntry {
                auth_url: data.auth_url.as_str().to_string(),
                resource: data.resource.as_str().to_string(),
                user_id: data.user_id.as_str().to_string(),
                authorized_domains: data.authorized_domains.as_str().to_string(),
                expires: data.expires,
                token: data.token.as_str().to_string(),
            });
        }
    }

    Ok(AuthListResult {
        identity_count: identities.len() as u32,
        identities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = AuthListResult {
            identity_count: 1,
            identities: vec![AuthIdentityEntry {
                auth_url: "ucs-auth://auth.example.com".into(),
                resource: "urc-abc123".into(),
                user_id: "user-42".into(),
                authorized_domains: "example.com, test.com".into(),
                expires: 1719705600000,
                token: "".into(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"identity_count\":1"));
        assert!(json.contains("\"auth_url\":\"ucs-auth://auth.example.com\""));
        assert!(json.contains("\"user_id\":\"user-42\""));
    }

    #[test]
    fn empty_list_result() {
        let result = AuthListResult {
            identity_count: 0,
            identities: vec![],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"identity_count\":0"));
        assert!(json.contains("\"identities\":[]"));
    }

    #[test]
    fn args_into_lore_with_token() {
        let args = ListArgs { with_token: true };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.with_token, 1);
    }

    #[test]
    fn args_into_lore_without_token() {
        let args = ListArgs { with_token: false };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.with_token, 0);
    }

    #[test]
    fn args_default_with_token_is_false() {
        let args: ListArgs = serde_json::from_str("{}").expect("deserialise");
        assert!(!args.with_token);
    }
}
