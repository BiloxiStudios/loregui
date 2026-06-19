//! `repository config_get` operation — binds `lore::repository::config_get`.
//!
//! Reads a configuration value from the repository config. The upstream
//! function emits a `LoreEvent::RepositoryConfigGet` event containing the
//! key-value pair.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::repository::LoreRepositoryConfigGetArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`config_get`].
///
/// Mirrors `LoreRepositoryConfigGetArgs` from the upstream `lore` crate
/// but uses plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryConfigGetArgs {
    /// Config key to read (e.g. `remote_url`, `identity`).
    pub key: String,
}

impl RepositoryConfigGetArgs {
    fn into_lore(self) -> LoreRepositoryConfigGetArgs {
        LoreRepositoryConfigGetArgs {
            key: LoreString::from_str(&self.key),
        }
    }
}

/// Result of a successful `config_get` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryConfigGetResult {
    /// The config key that was read.
    pub key: String,
    /// The value associated with the key.
    pub value: String,
}

/// Read a configuration value from the repository.
///
/// Calls upstream `lore::repository::config_get` in-process, collects the
/// `RepositoryConfigGet` event, and returns the key-value pair.
pub async fn config_get(
    api: &LoreApi,
    args: RepositoryConfigGetArgs,
) -> Result<RepositoryConfigGetResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::repository::config_get(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository config_get failed with status {status}"),
        )));
    }

    for event in &stream.events {
        if let LoreEvent::RepositoryConfigGet(data) = event {
            return Ok(RepositoryConfigGetResult {
                key: data.key.as_str().to_string(),
                value: data.value.as_str().to_string(),
            });
        }
    }

    Err(LoreError::Parse(
        "config_get succeeded but no RepositoryConfigGet event emitted".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serializes() {
        let args = RepositoryConfigGetArgs {
            key: "remote_url".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("remote_url"));
    }

    #[test]
    fn args_deserializes() {
        let args: RepositoryConfigGetArgs =
            serde_json::from_str(r#"{"key":"identity"}"#).expect("should deserialize");
        assert_eq!(args.key, "identity");
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = RepositoryConfigGetArgs {
            key: "remote_url".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.key.as_str(), "remote_url");
    }

    #[test]
    fn result_serializes() {
        let result = RepositoryConfigGetResult {
            key: "remote_url".into(),
            value: "https://example.com/repo".into(),
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("remote_url"));
        assert!(json.contains("https://example.com/repo"));
    }

    #[test]
    fn result_deserializes() {
        let json = r#"{"key":"identity","value":"user@example.com"}"#;
        let result: RepositoryConfigGetResult =
            serde_json::from_str(json).expect("should deserialize");
        assert_eq!(result.key, "identity");
        assert_eq!(result.value, "user@example.com");
    }
}
