//! `repository flush` operation — binds `lore::repository::flush`.
//!
//! Waits for all outstanding asynchronous repository tasks to complete. The
//! upstream function takes no arguments and emits only standard events (Log,
//! Error, Complete, End), so the binding simply checks for success/failure and
//! collects any diagnostic log messages.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::repository::LoreRepositoryFlushArgs;
use serde::{Deserialize, Serialize};

/// Result of a successful `flush` call.
///
/// The upstream operation does not emit any domain-specific result events, so
/// this is a simple success marker. The `log_messages` field collects any
/// diagnostic `Log` events emitted during the flush.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlushResult {
    /// Diagnostic log messages emitted while flushing outstanding tasks.
    pub log_messages: Vec<String>,
}

/// Wait for all outstanding asynchronous repository tasks to complete.
///
/// Calls upstream `lore::repository::flush` in-process. Since the operation
/// emits only standard events, the binding collects any `Log` messages and
/// checks the `Complete` status for success.
pub async fn flush(api: &LoreApi) -> Result<FlushResult> {
    let args = LoreRepositoryFlushArgs {};
    let (callback, rx) = collect_events();

    let status = lore::repository::flush(api.globals().build(), args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("repository flush failed with status {status}"),
        )));
    }

    let mut log_messages = Vec::new();
    for event in &stream.events {
        if let lore::interface::LoreEvent::Log(data) = event {
            log_messages.push(data.message.as_str().to_string());
        }
    }

    Ok(FlushResult { log_messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = FlushResult {
            log_messages: vec!["flushed 3 tasks".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("flushed 3 tasks"));
    }

    #[test]
    fn empty_result() {
        let result = FlushResult::default();
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"log_messages\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"log_messages":["done"]}"#;
        let result: FlushResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.log_messages.len(), 1);
        assert_eq!(result.log_messages[0], "done");
    }
}
