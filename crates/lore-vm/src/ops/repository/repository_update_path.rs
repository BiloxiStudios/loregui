//! `repository repository_update_path` operation — binds `lore::repository::repository_update_path`.
//!
//! Updates the recorded filesystem path for the current repository instance to
//! match the actual working directory. This is useful after moving a repository
//! on disk — the instance table stores the original path, and this operation
//! refreshes it to the current location.
//!
//! The upstream function takes no domain-specific arguments and emits only
//! standard events (Log, Error, Complete) — no domain-specific result events,
//! so the binding simply checks for success/failure and collects log messages.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::repository::LoreRepositoryUpdatePathArgs;
use serde::{Deserialize, Serialize};

/// Result of a successful `repository_update_path` call.
///
/// The upstream operation does not emit domain-specific result events, so this
/// is a simple success marker. The `log_messages` field collects any diagnostic
/// `Log` events emitted during the update.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepositoryUpdatePathResult {
    /// Diagnostic log messages emitted during the path update.
    pub log_messages: Vec<String>,
}

/// Update the recorded filesystem path for the current repository instance.
///
/// Calls upstream `lore::repository::repository_update_path` in-process. Since
/// the operation emits only standard events, the binding collects any `Log`
/// messages and checks the `Complete` status for success.
pub async fn repository_update_path(api: &LoreApi) -> Result<RepositoryUpdatePathResult> {
    let args = LoreRepositoryUpdatePathArgs {};
    let (callback, rx) = collect_events();

    let status =
        lore::repository::repository_update_path(api.globals().build(), args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository_update_path failed with status {status}"),
        )));
    }

    let mut log_messages = Vec::new();
    for event in &stream.events {
        if let lore::interface::LoreEvent::Log(data) = event {
            log_messages.push(data.message.as_str().to_string());
        }
    }

    Ok(RepositoryUpdatePathResult { log_messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = RepositoryUpdatePathResult {
            log_messages: vec!["path updated".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("path updated"));
    }

    #[test]
    fn empty_result() {
        let result = RepositoryUpdatePathResult::default();
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"log_messages\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"log_messages":["done"]}"#;
        let result: RepositoryUpdatePathResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.log_messages.len(), 1);
        assert_eq!(result.log_messages[0], "done");
    }
}
