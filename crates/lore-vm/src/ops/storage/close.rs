//! `storage close` operation — binds `lore::storage::close`.
//!
//! Releases a storage handle acquired via `storage open`. The upstream call
//! unregisters the handle, drains any in-flight ops, then spawns a
//! fire-and-forget flush. `Complete` fires once the handle is invalidated;
//! subsequent ops against the same handle return `InvalidArguments`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::close::LoreStorageCloseArgs;
use lore::storage::handle::LoreStore;
use serde::{Deserialize, Serialize};

/// Arguments for [`close`].
///
/// Wraps the upstream `LoreStorageCloseArgs` with a plain `u64` handle so it
/// serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCloseArgs {
    /// Handle ID of the open storage instance to close.
    pub handle: u64,
}

impl StorageCloseArgs {
    fn into_lore(self) -> LoreStorageCloseArgs {
        LoreStorageCloseArgs {
            handle: LoreStore {
                handle_id: self.handle,
            },
        }
    }
}

/// Result of a successful `close` call.
///
/// Close emits only standard lifecycle events, so this is a simple success
/// marker with any diagnostic log messages collected.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageCloseResult {
    /// Diagnostic log messages emitted during the close sequence.
    pub log_messages: Vec<String>,
}

/// Release a storage handle acquired via `storage open`.
///
/// Calls upstream `lore::storage::close::close` in-process. Collects any `Log`
/// messages and checks the `Complete` status for success.
pub async fn close(api: &LoreApi, args: StorageCloseArgs) -> Result<StorageCloseResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::storage::close::close(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage close failed with status {status}"),
        )));
    }

    let mut log_messages = Vec::new();
    for event in &stream.events {
        if let LoreEvent::Log(data) = event {
            log_messages.push(data.message.as_str().to_string());
        }
    }

    Ok(StorageCloseResult { log_messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = StorageCloseResult {
            log_messages: vec!["handle released".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("handle released"));
    }

    #[test]
    fn empty_result() {
        let result = StorageCloseResult::default();
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"log_messages\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"log_messages":["closed"]}"#;
        let result: StorageCloseResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.log_messages.len(), 1);
        assert_eq!(result.log_messages[0], "closed");
    }

    #[test]
    fn args_converts_to_lore() {
        let args = StorageCloseArgs { handle: 42 };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.handle.handle_id, 42);
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"handle":7}"#;
        let args: StorageCloseArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.handle, 7);
    }
}
