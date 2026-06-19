//! `storage flush` operation — binds `lore::storage::flush`.
//!
//! Flushes pending writes through an open storage handle. Disk-backed stores
//! call fsync on both immutable and mutable stores; in-memory stores no-op.
//! The binding collects any `Log` messages and checks the `Complete` status
//! for success.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::flush::LoreStorageFlushArgs;
use lore::storage::handle::LoreStore;
use serde::{Deserialize, Serialize};

/// Arguments for [`flush`].
///
/// Wraps the upstream `LoreStorageFlushArgs` with a plain `u64` handle so it
/// serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageFlushArgs {
    /// Handle ID of the open storage instance to flush.
    pub handle: u64,
}

impl StorageFlushArgs {
    fn into_lore(self) -> LoreStorageFlushArgs {
        LoreStorageFlushArgs {
            handle: LoreStore {
                handle_id: self.handle,
            },
        }
    }
}

/// Result of a successful `flush` call.
///
/// The upstream operation emits only standard events, so this is a simple
/// success marker with any diagnostic log messages collected.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageFlushResult {
    /// Diagnostic log messages emitted while flushing pending writes.
    pub log_messages: Vec<String>,
}

/// Flush pending writes through an open storage handle.
///
/// Calls upstream `lore::storage::flush` in-process. Collects any `Log`
/// messages and checks the `Complete` status for success.
pub async fn flush(api: &LoreApi, args: StorageFlushArgs) -> Result<StorageFlushResult> {
    let (callback, rx) = collect_events();

    let status =
        lore::storage::flush::flush(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage flush failed with status {status}"),
        )));
    }

    let mut log_messages = Vec::new();
    for event in &stream.events {
        if let LoreEvent::Log(data) = event {
            log_messages.push(data.message.as_str().to_string());
        }
    }

    Ok(StorageFlushResult { log_messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = StorageFlushResult {
            log_messages: vec!["flushed pending writes".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("flushed pending writes"));
    }

    #[test]
    fn empty_result() {
        let result = StorageFlushResult::default();
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"log_messages\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"log_messages":["done"]}"#;
        let result: StorageFlushResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.log_messages.len(), 1);
        assert_eq!(result.log_messages[0], "done");
    }

    #[test]
    fn args_converts_to_lore() {
        let args = StorageFlushArgs { handle: 42 };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.handle.handle_id, 42);
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"handle":7}"#;
        let args: StorageFlushArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.handle, 7);
    }
}
