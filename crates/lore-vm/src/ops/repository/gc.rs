//! `repository gc` operation — binds `lore::repository::gc`.
//!
//! Runs garbage collection on the local repository store to reclaim space from
//! unreferenced data. The upstream function takes no arguments and emits only
//! standard events (Log, Error, Complete, End) — no domain-specific result
//! events, so the binding simply checks for success/failure.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::repository::LoreRepositoryGcArgs;
use serde::{Deserialize, Serialize};

/// Result of a successful `gc` call.
///
/// The upstream operation does not emit any domain-specific result events, so
/// this is a simple success marker. The `log_messages` field collects any
/// diagnostic `Log` events emitted during collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GcResult {
    /// Diagnostic log messages emitted during garbage collection.
    pub log_messages: Vec<String>,
}

/// Run garbage collection on the local repository store.
///
/// Calls upstream `lore::repository::gc` in-process. Since the operation emits
/// only standard events, the binding collects any `Log` messages and checks
/// the `Complete` status for success.
pub async fn gc(api: &LoreApi) -> Result<GcResult> {
    let args = LoreRepositoryGcArgs {};
    let (callback, rx) = collect_events();

    let status = lore::repository::gc(api.globals().build(), args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository gc failed with status {status}"),
        )));
    }

    let mut log_messages = Vec::new();
    for event in &stream.events {
        if let lore::interface::LoreEvent::Log(data) = event {
            log_messages.push(data.message.as_str().to_string());
        }
    }

    Ok(GcResult { log_messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = GcResult {
            log_messages: vec!["collected 42 fragments".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("collected 42 fragments"));
    }

    #[test]
    fn empty_result() {
        let result = GcResult::default();
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"log_messages\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"log_messages":["done"]}"#;
        let result: GcResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.log_messages.len(), 1);
        assert_eq!(result.log_messages[0], "done");
    }
}
