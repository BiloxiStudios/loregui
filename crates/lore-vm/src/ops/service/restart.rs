//! `service restart` operation — composite stop → start.
//!
//! Performs an idempotent restart of the Lore service process by stopping the
//! current service and then starting it again. Since the upstream `lore` crate
//! has no dedicated `restart` function, this binding orchestrates
//! `lore::service::stop` followed by `lore::service::start`.
//!
//! **Credential safety:** No launch configuration, credentials, or S3 secrets
//! are reconstructed or exposed. The restart uses the same in-process
//! `LoreApi` handle that was used for the original start, so the upstream
//! engine retains its existing configuration. This prevents credential
//! leakage into `HostStatus`, frontend state, or localStorage.
//!
//! **Failure semantics:** If the stop phase fails, restart is aborted and the
//! error is returned immediately. If stop succeeds but start fails, the error
//! is returned and the service remains stopped (fail-closed). Log messages
//! from both phases are collected separately in the result.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::service::LoreServiceStopArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`restart`].
///
/// Mirrors `LoreServiceStopArgs` for the stop phase; the start phase takes no
/// arguments (it resumes the service with its existing configuration).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceRestartArgs {
    /// When `true`, restart the service for all repositories rather than just
    /// the current one.
    #[serde(default)]
    pub all: bool,
}

impl ServiceRestartArgs {
    fn into_lore_stop(self) -> LoreServiceStopArgs {
        LoreServiceStopArgs {
            all: u8::from(self.all),
        }
    }
}

/// Result of a successful `service restart` call.
///
/// Contains log messages from both the stop and start phases so the caller
/// can diagnose issues in either direction of the restart cycle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceRestartResult {
    /// Diagnostic log messages emitted while stopping the service process.
    pub stop_log_messages: Vec<String>,
    /// Diagnostic log messages emitted while starting the service process.
    pub start_log_messages: Vec<String>,
}

fn collect_logs(events: &[lore::interface::LoreEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| {
            if let lore::interface::LoreEvent::Log(data) = e {
                Some(data.message.as_str().to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Restart the Lore service process (stop → start) for the current or all
/// repositories.
///
/// Calls upstream `lore::service::stop` then `lore::service::start`
/// in-process. Both phases collect `Log` events and check `Complete` status.
///
/// **Fail-closed:** if stop succeeds but start fails, the service is left
/// stopped and the start error is returned. The caller is responsible for
/// deciding whether to retry or report the stopped state.
pub async fn restart(api: &LoreApi, args: ServiceRestartArgs) -> Result<ServiceRestartResult> {
    // ── Phase 1: stop ──────────────────────────────────────────────────
    let (callback, rx) = collect_events();
    let stop_status = lore::service::stop(
        api.globals().build(),
        args.clone().into_lore_stop(),
        callback,
    )
    .await;

    let stop_stream = rx.await.map_err(|e| {
        LoreError::CommandFailed(format!("restart stop phase: event stream cancelled: {e}"))
    })?;

    if !stop_stream.is_ok() {
        return Err(LoreError::CommandFailed(stop_stream.error.unwrap_or_else(
            || format!("restart stop phase failed with status {stop_status}"),
        )));
    }
    let stop_log_messages = collect_logs(&stop_stream.events);

    // ── Phase 2: start ─────────────────────────────────────────────────
    let (callback, rx) = collect_events();
    let start_args = lore::service::LoreServiceStartArgs {};
    let start_status = lore::service::start(api.globals().build(), start_args, callback).await;

    let start_stream = rx.await.map_err(|e| {
        LoreError::CommandFailed(format!("restart start phase: event stream cancelled: {e}"))
    })?;

    if !start_stream.is_ok() {
        // Fail-closed: stop succeeded but start failed. Service is now
        // stopped. Return the start error so the caller can decide on retry.
        return Err(LoreError::CommandFailed(start_stream.error.unwrap_or_else(
            || format!("restart start phase failed with status {start_status}"),
        )));
    }
    let start_log_messages = collect_logs(&start_stream.events);

    Ok(ServiceRestartResult {
        stop_log_messages,
        start_log_messages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = ServiceRestartResult {
            stop_log_messages: vec!["service stopped".into()],
            start_log_messages: vec!["service started".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("service stopped"));
        assert!(json.contains("service started"));
    }

    #[test]
    fn empty_result() {
        let result = ServiceRestartResult::default();
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"stop_log_messages\":[]"));
        assert!(json.contains("\"start_log_messages\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"stop_log_messages":["stopping"],"start_log_messages":["starting"]}"#;
        let result: ServiceRestartResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.stop_log_messages.len(), 1);
        assert_eq!(result.start_log_messages.len(), 1);
        assert_eq!(result.stop_log_messages[0], "stopping");
        assert_eq!(result.start_log_messages[0], "starting");
    }

    #[test]
    fn args_default_all_false() {
        let args = ServiceRestartArgs::default();
        assert!(!args.all);
        let lore_args = args.into_lore_stop();
        assert_eq!(lore_args.all, 0);
    }

    #[test]
    fn args_all_true_converts() {
        let args = ServiceRestartArgs { all: true };
        let lore_args = args.into_lore_stop();
        assert_eq!(lore_args.all, 1);
    }

    #[test]
    fn args_deserialises_without_all() {
        let json = r#"{}"#;
        let args: ServiceRestartArgs = serde_json::from_str(json).expect("deserialise");
        assert!(!args.all);
    }

    #[test]
    fn args_deserialises_with_all() {
        let json = r#"{"all":true}"#;
        let args: ServiceRestartArgs = serde_json::from_str(json).expect("deserialise");
        assert!(args.all);
    }

    #[test]
    fn collect_logs_empty_on_no_events() {
        let logs = collect_logs(&[]);
        assert!(logs.is_empty());
    }
}
