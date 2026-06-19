//! `service stop` operation — binds `lore::service::stop`.
//!
//! Stops the Lore service process for the current repository (or all
//! repositories when `all` is set). The upstream function emits only standard
//! events (Log, Error, Complete, End), so the binding collects diagnostic log
//! messages and checks the `Complete` status for success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::service::LoreServiceStopArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`stop`].
///
/// Mirrors `LoreServiceStopArgs` from the upstream `lore` crate but uses
/// idiomatic Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceStopArgs {
    /// When `true`, stop the service for all repositories rather than just
    /// the current one.
    #[serde(default)]
    pub all: bool,
}

impl ServiceStopArgs {
    fn into_lore(self) -> LoreServiceStopArgs {
        LoreServiceStopArgs {
            all: u8::from(self.all),
        }
    }
}

/// Result of a successful `service stop` call.
///
/// The upstream operation does not emit any domain-specific result events, so
/// this is a simple success marker. The `log_messages` field collects any
/// diagnostic `Log` events emitted while stopping the service.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceStopResult {
    /// Diagnostic log messages emitted while stopping the service process.
    pub log_messages: Vec<String>,
}

/// Stop the Lore service process for the current or all repositories.
///
/// Calls upstream `lore::service::stop` in-process. Since the operation
/// emits only standard events, the binding collects any `Log` messages and
/// checks the `Complete` status for success.
pub async fn stop(api: &LoreApi, args: ServiceStopArgs) -> Result<ServiceStopResult> {
    let (callback, rx) = collect_events();

    let status = lore::service::stop(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("service stop failed with status {status}"),
        )));
    }

    let mut log_messages = Vec::new();
    for event in &stream.events {
        if let lore::interface::LoreEvent::Log(data) = event {
            log_messages.push(data.message.as_str().to_string());
        }
    }

    Ok(ServiceStopResult { log_messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = ServiceStopResult {
            log_messages: vec!["service stopped".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("service stopped"));
    }

    #[test]
    fn empty_result() {
        let result = ServiceStopResult::default();
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"log_messages\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"log_messages":["stopping service"]}"#;
        let result: ServiceStopResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.log_messages.len(), 1);
        assert_eq!(result.log_messages[0], "stopping service");
    }

    #[test]
    fn args_default_all_false() {
        let args = ServiceStopArgs::default();
        assert!(!args.all);
        let lore_args = args.into_lore();
        assert_eq!(lore_args.all, 0);
    }

    #[test]
    fn args_all_true_converts() {
        let args = ServiceStopArgs { all: true };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.all, 1);
    }

    #[test]
    fn args_deserialises_without_all() {
        let json = r#"{}"#;
        let args: ServiceStopArgs = serde_json::from_str(json).expect("deserialise");
        assert!(!args.all);
    }

    #[test]
    fn args_deserialises_with_all() {
        let json = r#"{"all":true}"#;
        let args: ServiceStopArgs = serde_json::from_str(json).expect("deserialise");
        assert!(args.all);
    }
}
