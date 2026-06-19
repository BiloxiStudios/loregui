//! `service start` operation — binds `lore::service::start`.
//!
//! Starts the Lore service process to manage the current repository. The
//! upstream function takes no arguments and emits only standard events (Log,
//! Error, Complete, End), so the binding simply checks for success/failure and
//! collects any diagnostic log messages.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::service::LoreServiceStartArgs;
use serde::{Deserialize, Serialize};

/// Result of a successful `service start` call.
///
/// The upstream operation does not emit any domain-specific result events, so
/// this is a simple success marker. The `log_messages` field collects any
/// diagnostic `Log` events emitted during service startup.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceStartResult {
    /// Diagnostic log messages emitted while starting the service process.
    pub log_messages: Vec<String>,
}

/// Start the Lore service process to manage the current repository.
///
/// Calls upstream `lore::service::start` in-process. Since the operation
/// emits only standard events, the binding collects any `Log` messages and
/// checks the `Complete` status for success.
pub async fn start(api: &LoreApi) -> Result<ServiceStartResult> {
    let args = LoreServiceStartArgs {};
    let (callback, rx) = collect_events();

    let status = lore::service::start(api.globals().build(), args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("service start failed with status {status}"),
        )));
    }

    let mut log_messages = Vec::new();
    for event in &stream.events {
        if let lore::interface::LoreEvent::Log(data) = event {
            log_messages.push(data.message.as_str().to_string());
        }
    }

    Ok(ServiceStartResult { log_messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = ServiceStartResult {
            log_messages: vec!["service started".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("service started"));
    }

    #[test]
    fn empty_result() {
        let result = ServiceStartResult::default();
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"log_messages\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"log_messages":["starting service"]}"#;
        let result: ServiceStartResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.log_messages.len(), 1);
        assert_eq!(result.log_messages[0], "starting service");
    }
}
