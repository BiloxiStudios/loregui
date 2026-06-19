//! `storage open` operation — binds `lore::storage::open`.
//!
//! Acquires a handle to a content-addressed store, either disk-backed (when
//! `repository_path` is set) or fully in-memory (when `in_memory` is true).
//! The binding collects the `StorageOpened` event to return the opaque handle
//! ID that subsequent storage ops (`get`, `put`, `flush`, `close`, …) require.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::storage::open::{LoreStorageOpenArgs, LoreStorageRemoteConfig};
use serde::{Deserialize, Serialize};

/// Arguments for [`open`].
///
/// Wraps the upstream `LoreStorageOpenArgs` with plain Rust types so it
/// serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageOpenArgs {
    /// Path to an existing lore repository. Must be empty when `in_memory` is
    /// true.
    #[serde(default)]
    pub repository_path: String,
    /// Open a fresh in-memory store. `repository_path` must be empty when set.
    #[serde(default)]
    pub in_memory: bool,
    /// Optional remote endpoint URL for ops that consult a peer service.
    /// When non-empty, `has_remote_config` is implicitly set.
    #[serde(default)]
    pub remote_url: String,
    /// Soft cap on total immutable-store bytes (compactor target). `0` selects
    /// the upstream default.
    #[serde(default)]
    pub cache_target_bytes: u64,
    /// Soft cap on immutable-store fragment count (evictor target). `0` selects
    /// the upstream default.
    #[serde(default)]
    pub cache_target_fragments: u64,
}

impl StorageOpenArgs {
    fn into_lore(self) -> LoreStorageOpenArgs {
        let has_remote = !self.remote_url.is_empty();
        LoreStorageOpenArgs {
            repository_path: LoreString::from_str(&self.repository_path),
            in_memory: u8::from(self.in_memory),
            remote_config: LoreStorageRemoteConfig {
                remote_url: LoreString::from_str(&self.remote_url),
            },
            has_remote_config: u8::from(has_remote),
            cache_target_bytes: self.cache_target_bytes,
            cache_target_fragments: self.cache_target_fragments,
        }
    }
}

/// Result of a successful `open` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageOpenResult {
    /// Opaque handle ID for the opened store. Pass this to subsequent storage
    /// operations (`get`, `put`, `flush`, `close`, …).
    pub handle: u64,
    /// Diagnostic log messages emitted during open.
    pub log_messages: Vec<String>,
}

/// Open a content-addressed store and return its handle.
///
/// Calls upstream `lore::storage::open::open` in-process. Collects the
/// `StorageOpened` event to extract the handle ID and any `Log` messages.
pub async fn open(api: &LoreApi, args: StorageOpenArgs) -> Result<StorageOpenResult> {
    let (callback, rx) = collect_events();

    let status = lore::storage::open::open(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage open failed with status {status}"),
        )));
    }

    // Extract the handle from the StorageOpened event.
    let mut handle = None;
    let mut log_messages = Vec::new();
    for event in &stream.events {
        match event {
            LoreEvent::StorageOpened(data) => {
                handle = Some(data.handle_id);
            }
            LoreEvent::Log(data) => {
                log_messages.push(data.message.as_str().to_string());
            }
            _ => {}
        }
    }

    let handle = handle.ok_or_else(|| {
        LoreError::Parse("storage open succeeded but no StorageOpened event emitted".into())
    })?;

    Ok(StorageOpenResult {
        handle,
        log_messages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = StorageOpenResult {
            handle: 42,
            log_messages: vec!["opened store".into()],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"handle\":42"));
        assert!(json.contains("opened store"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"handle":7,"log_messages":["ok"]}"#;
        let result: StorageOpenResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.handle, 7);
        assert_eq!(result.log_messages, vec!["ok"]);
    }

    #[test]
    fn args_converts_to_lore_disk_backed() {
        let args = StorageOpenArgs {
            repository_path: "/tmp/repo".into(),
            in_memory: false,
            remote_url: String::new(),
            cache_target_bytes: 1024,
            cache_target_fragments: 0,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.repository_path.as_str(), "/tmp/repo");
        assert_eq!(lore_args.in_memory, 0);
        assert_eq!(lore_args.has_remote_config, 0);
        assert_eq!(lore_args.cache_target_bytes, 1024);
        assert_eq!(lore_args.cache_target_fragments, 0);
    }

    #[test]
    fn args_converts_to_lore_in_memory() {
        let args = StorageOpenArgs {
            repository_path: String::new(),
            in_memory: true,
            remote_url: String::new(),
            cache_target_bytes: 0,
            cache_target_fragments: 0,
        };
        let lore_args = args.into_lore();
        assert!(lore_args.repository_path.as_str().is_empty());
        assert_eq!(lore_args.in_memory, 1);
        assert_eq!(lore_args.has_remote_config, 0);
    }

    #[test]
    fn args_with_remote_sets_has_remote_config() {
        let args = StorageOpenArgs {
            repository_path: String::new(),
            in_memory: true,
            remote_url: "https://store.example.com".into(),
            cache_target_bytes: 0,
            cache_target_fragments: 0,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.has_remote_config, 1);
        assert_eq!(
            lore_args.remote_config.remote_url.as_str(),
            "https://store.example.com"
        );
    }

    #[test]
    fn args_deserialises_defaults() {
        let json = r#"{}"#;
        let args: StorageOpenArgs = serde_json::from_str(json).expect("deserialise");
        assert!(args.repository_path.is_empty());
        assert!(!args.in_memory);
        assert!(args.remote_url.is_empty());
        assert_eq!(args.cache_target_bytes, 0);
        assert_eq!(args.cache_target_fragments, 0);
    }

    #[test]
    fn args_deserialises_full() {
        let json = r#"{"repository_path":"/repo","in_memory":false,"remote_url":"https://x","cache_target_bytes":512,"cache_target_fragments":16}"#;
        let args: StorageOpenArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.repository_path, "/repo");
        assert!(!args.in_memory);
        assert_eq!(args.remote_url, "https://x");
        assert_eq!(args.cache_target_bytes, 512);
        assert_eq!(args.cache_target_fragments, 16);
    }
}
