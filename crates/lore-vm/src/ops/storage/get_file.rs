//! `storage get_file` operation — binds [`lore::storage::get_file::get_file`].
//!
//! Writes one or more content-addressed payloads to filesystem paths from an
//! open storage handle. Each item is identified by `(partition, address)` and
//! a destination `path`; per-item results are collected from
//! `StorageGetItemComplete` events.
//!
//! Unlike `storage get`, no `GET_HEADER` or `GET_DATA` events are emitted —
//! only the terminal `GET_ITEM_COMPLETE` per item.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::get_file::LoreStorageGetFileArgs;
use serde::{Deserialize, Serialize};

/// One item to retrieve — Tauri-friendly mirror of
/// [`LoreStorageGetFileItem`](lore::storage::get_file::LoreStorageGetFileItem).
///
/// Partition and address are hex strings so they serialise cleanly across the
/// Tauri IPC boundary; the upstream C-repr types are reconstructed via serde
/// deserialisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileItem {
    /// Caller-chosen correlation id echoed back in the result.
    pub id: u64,
    /// Target partition as a 32-char hex string.
    pub partition: String,
    /// Content address as hex (`<hash>` or `<hash>-<context>`).
    pub address: String,
    /// Destination filesystem path to write the content to.
    pub path: String,
    /// Cache fetched fragments back to the local store (default false).
    #[serde(default)]
    pub local_cache: bool,
}

/// Arguments for [`get_file`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageGetFileArgs {
    /// Handle id returned by a prior `storage open` call.
    pub handle: u64,
    /// Items (partition + address + path) to retrieve and write.
    pub items: Vec<GetFileItem>,
}

impl StorageGetFileArgs {
    /// Convert to the upstream `LoreStorageGetFileArgs` via serde round-trip.
    fn into_lore(self) -> std::result::Result<LoreStorageGetFileArgs, LoreError> {
        let items_json: Vec<serde_json::Value> = self
            .items
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "partition": item.partition,
                    "address": item.address,
                    "path": item.path,
                    "local_cache": u8::from(item.local_cache),
                })
            })
            .collect();

        let args_json = serde_json::json!({
            "handle": { "handle_id": self.handle },
            "items": items_json,
        });

        serde_json::from_value::<LoreStorageGetFileArgs>(args_json)
            .map_err(|e| LoreError::Parse(format!("failed to build LoreStorageGetFileArgs: {e}")))
    }
}

/// Per-item result from the get_file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileItemResult {
    /// Correlation id of the item.
    pub id: u64,
    /// The content address as hex, or empty on failure.
    pub address: String,
    /// `true` if the item was written successfully.
    pub ok: bool,
    /// Error code name when `ok` is false; empty on success.
    #[serde(default)]
    pub error: String,
}

/// Result of the overall get_file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageGetFileResult {
    /// Per-item outcomes (one entry per input item).
    pub items: Vec<GetFileItemResult>,
}

/// Write one or more content-addressed payloads to filesystem paths.
///
/// Calls upstream [`lore::storage::get_file::get_file`] in-process and
/// collects the `StorageGetItemComplete` events to build a typed result.
pub async fn get_file(api: &LoreApi, args: StorageGetFileArgs) -> Result<StorageGetFileResult> {
    let lore_args = args.into_lore()?;

    let (callback, rx) = collect_events();

    let status =
        lore::storage::get_file::get_file(api.globals().build(), lore_args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage get_file failed with status {status}"),
        )));
    }

    let mut results: Vec<GetFileItemResult> = Vec::new();
    for event in &stream.events {
        if let LoreEvent::StorageGetItemComplete(data) = event {
            let code_str = format!("{:?}", data.error_code);
            let ok = code_str == "None";
            results.push(GetFileItemResult {
                id: data.id,
                address: if ok {
                    format!("{}", data.address)
                } else {
                    String::new()
                },
                ok,
                error: if ok { String::new() } else { code_str },
            });
        }
    }

    Ok(StorageGetFileResult { items: results })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = StorageGetFileResult {
            items: vec![GetFileItemResult {
                id: 1,
                address: "abc123".into(),
                ok: true,
                error: String::new(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("abc123"));
        assert!(json.contains("\"ok\":true"));
    }

    #[test]
    fn result_with_error() {
        let result = GetFileItemResult {
            id: 7,
            address: String::new(),
            ok: false,
            error: "AddressNotFound".into(),
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("AddressNotFound"));
    }

    #[test]
    fn empty_result() {
        let result = StorageGetFileResult { items: vec![] };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"items\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"items":[{"id":1,"address":"abc","ok":true,"error":""}]}"#;
        let result: StorageGetFileResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].id, 1);
        assert!(result.items[0].ok);
    }

    #[test]
    fn args_serialises() {
        let args = StorageGetFileArgs {
            handle: 99,
            items: vec![GetFileItem {
                id: 1,
                partition: "a".repeat(32),
                address: "b".repeat(64),
                path: "/tmp/test.bin".into(),
                local_cache: false,
            }],
        };
        let json = serde_json::to_string(&args).expect("serialise");
        assert!(json.contains("\"handle\":99"));
        assert!(json.contains("/tmp/test.bin"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"handle":5,"items":[{"id":1,"partition":"00000000000000000000000000000001","address":"abcdef","path":"/tmp/f.bin"}]}"#;
        let args: StorageGetFileArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.handle, 5);
        assert_eq!(args.items.len(), 1);
        assert_eq!(args.items[0].id, 1);
        assert_eq!(args.items[0].path, "/tmp/f.bin");
    }

    #[test]
    fn args_converts_to_lore() {
        let args = StorageGetFileArgs {
            handle: 42,
            items: vec![GetFileItem {
                id: 1,
                partition: "00000000000000000000000000000001".into(),
                address: "a".repeat(64),
                path: "/tmp/test.bin".into(),
                local_cache: false,
            }],
        };
        let lore_args = args.into_lore().expect("into_lore");
        assert_eq!(lore_args.handle.handle_id, 42);
        assert_eq!(lore_args.items.as_slice().len(), 1);
    }

    #[test]
    fn args_empty_items_converts() {
        let args = StorageGetFileArgs {
            handle: 1,
            items: vec![],
        };
        let lore_args = args.into_lore().expect("into_lore");
        assert_eq!(lore_args.items.as_slice().len(), 0);
    }

    #[test]
    fn args_defaults_on_deserialise() {
        let json = r#"{"handle":1,"items":[{"id":0,"partition":"00000000000000000000000000000001","address":"ff","path":"/x"}]}"#;
        let args: StorageGetFileArgs = serde_json::from_str(json).expect("deserialise");
        assert!(!args.items[0].local_cache);
    }
}
