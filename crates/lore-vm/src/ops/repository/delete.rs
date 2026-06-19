//! `repository delete` operation — binds `lore::repository::delete`.
//!
//! Deletes a remote repository by URL. The upstream function emits only standard
//! events (Log, Error, Complete), so the binding checks success/failure and
//! collects any diagnostic log messages.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreString;
use lore::repository::LoreRepositoryDeleteArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`delete`].
///
/// Mirrors `LoreRepositoryDeleteArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteArgs {
    /// URL of the remote repository to delete (e.g. `lore://host/repo`).
    pub repository_url: String,
}

impl DeleteArgs {
    fn into_lore(self) -> LoreRepositoryDeleteArgs {
        LoreRepositoryDeleteArgs {
            repository_url: LoreString::from_str(&self.repository_url),
        }
    }
}

/// Result of a successful `delete` call.
///
/// The upstream operation does not emit any domain-specific result events, so
/// this is a simple success marker. The `log_messages` field collects any
/// diagnostic `Log` events emitted during deletion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeleteResult {
    /// Diagnostic log messages emitted while deleting the repository.
    pub log_messages: Vec<String>,
}

/// Delete a remote repository.
///
/// Calls upstream `lore::repository::delete` in-process. Since the operation
/// emits only standard events, the binding collects any `Log` messages and
/// checks the `Complete` status for success.
pub async fn delete(api: &LoreApi, args: DeleteArgs) -> Result<DeleteResult> {
    let (callback, rx) = collect_events();

    let status = lore::repository::delete(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository delete failed with status {status}"),
        )));
    }

    let mut log_messages = Vec::new();
    for event in &stream.events {
        if let lore::interface::LoreEvent::Log(data) = event {
            log_messages.push(data.message.as_str().to_string());
        }
    }

    Ok(DeleteResult { log_messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises() {
        let args = DeleteArgs {
            repository_url: "lore://host/repo".into(),
        };
        let json = serde_json::to_string(&args).expect("serialise");
        assert!(json.contains("lore://host/repo"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"repository_url":"lore://host/demo"}"#;
        let args: DeleteArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.repository_url, "lore://host/demo");
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = DeleteArgs {
            repository_url: "lore://host/repo".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.repository_url.as_str(), "lore://host/repo");
    }

    #[test]
    fn result_serialises_to_json() {
        let result = DeleteResult {
            log_messages: vec!["repository deleted".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("repository deleted"));
    }

    #[test]
    fn empty_result() {
        let result = DeleteResult::default();
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"log_messages\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"log_messages":["done"]}"#;
        let result: DeleteResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.log_messages.len(), 1);
        assert_eq!(result.log_messages[0], "done");
    }
}
