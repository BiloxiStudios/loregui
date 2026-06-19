//! `storage upload` operation — binds [`lore::storage::upload::upload`].
//!
//! Pushes one or more locally-stored, not-yet-durable content entries to the
//! remote store attached to an open storage handle. Each item is identified
//! by `(partition, address)` and runs independently; per-item results are
//! collected from `StorageUploadItemComplete` events emitted by the upstream
//! crate.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::upload::LoreStorageUploadArgs;
use serde::{Deserialize, Serialize};

/// One item to upload — the Tauri-friendly counterpart of
/// [`LoreStorageUploadItem`](lore::storage::upload::LoreStorageUploadItem).
///
/// Partition and address are hex strings so they serialise cleanly across
/// the Tauri IPC boundary; the upstream C-repr types are reconstructed via
/// serde deserialisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadItem {
    /// Caller-chosen correlation id echoed back in the result.
    pub id: u64,
    /// Partition of the local content to push (32-char hex string).
    pub partition: String,
    /// Content address to push (`"<hash>"` or `"<hash>-<context>"`).
    pub address: String,
}

/// Arguments for [`upload`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageUploadArgs {
    /// Handle id of an already-open store (from `storage open`).
    /// The store must have been opened with `remote_config`.
    pub handle: u64,
    /// Items (partition + address) to push to the remote store.
    pub items: Vec<UploadItem>,
}

impl StorageUploadArgs {
    /// Convert to the upstream `LoreStorageUploadArgs` via serde round-trip.
    ///
    /// The upstream types (`Partition`, `Address`, `LoreArray`) live in crates
    /// that are not direct dependencies of lore-vm, so we build intermediate
    /// JSON and let their `Deserialize` impls handle hex parsing.
    fn into_lore(self) -> std::result::Result<LoreStorageUploadArgs, LoreError> {
        let items_json: Vec<serde_json::Value> = self
            .items
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "partition": item.partition,
                    "address": item.address,
                })
            })
            .collect();

        let args_json = serde_json::json!({
            "handle": { "handle_id": self.handle },
            "items": items_json,
        });

        serde_json::from_value::<LoreStorageUploadArgs>(args_json)
            .map_err(|e| LoreError::Parse(format!("failed to build LoreStorageUploadArgs: {e}")))
    }
}

/// Per-item result from the upload operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadItemResult {
    /// Correlation id of the item.
    pub id: u64,
    /// The content address as `"<hash>-<context>"`, or empty on failure.
    pub address: String,
    /// `true` when the item was already flagged durable and no upload was
    /// performed.
    pub already_durable: bool,
    /// `true` if the item completed successfully.
    pub ok: bool,
    /// Error code name when `ok` is false; empty on success.
    #[serde(default)]
    pub error: String,
}

/// Result of the overall upload operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageUploadResult {
    /// Per-item outcomes (one entry per input item).
    pub items: Vec<UploadItemResult>,
}

/// Push locally-stored content to the remote store on an open handle.
///
/// Calls upstream [`lore::storage::upload::upload`] in-process and collects
/// the `StorageUploadItemComplete` events to build a typed result.
pub async fn upload(api: &LoreApi, args: StorageUploadArgs) -> Result<StorageUploadResult> {
    let lore_args = args.into_lore()?;

    let (callback, rx) = collect_events();

    let status =
        lore::storage::upload::upload(api.globals().build(), lore_args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage upload failed with status {status}"),
        )));
    }

    let mut results: Vec<UploadItemResult> = Vec::new();
    for event in &stream.events {
        if let LoreEvent::StorageUploadItemComplete(data) = event {
            let error_code_val = data.error_code as i32;
            let ok = error_code_val == 0;
            results.push(UploadItemResult {
                id: data.id,
                address: if ok {
                    format!("{}", data.address)
                } else {
                    String::new()
                },
                already_durable: data.already_durable != 0,
                ok,
                error: if ok {
                    String::new()
                } else {
                    format!("{:?}", data.error_code)
                },
            });
        }
    }

    Ok(StorageUploadResult { items: results })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = StorageUploadResult {
            items: vec![UploadItemResult {
                id: 1,
                address: "abc123".into(),
                already_durable: false,
                ok: true,
                error: String::new(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("abc123"));
        assert!(json.contains("\"ok\":true"));
    }

    #[test]
    fn result_with_already_durable() {
        let result = UploadItemResult {
            id: 42,
            address: "deadbeef".into(),
            already_durable: true,
            ok: true,
            error: String::new(),
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"already_durable\":true"));
    }

    #[test]
    fn result_with_error() {
        let result = UploadItemResult {
            id: 7,
            address: String::new(),
            already_durable: false,
            ok: false,
            error: "AddressNotFound".into(),
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("AddressNotFound"));
    }

    #[test]
    fn empty_result() {
        let result = StorageUploadResult { items: vec![] };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"items\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"items":[{"id":1,"address":"abc","already_durable":false,"ok":true,"error":""}]}"#;
        let result: StorageUploadResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].id, 1);
        assert!(result.items[0].ok);
    }

    #[test]
    fn args_serialises() {
        let args = StorageUploadArgs {
            handle: 99,
            items: vec![UploadItem {
                id: 1,
                partition: "a".repeat(32),
                address: "b".repeat(64),
            }],
        };
        let json = serde_json::to_string(&args).expect("serialise");
        assert!(json.contains("\"handle\":99"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"handle":5,"items":[{"id":1,"partition":"00000000000000000000000000000001","address":"00000000000000000000000000000000000000000000000000000000000000ab"}]}"#;
        let args: StorageUploadArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.handle, 5);
        assert_eq!(args.items.len(), 1);
        assert_eq!(args.items[0].id, 1);
    }

    #[test]
    fn args_converts_to_lore() {
        let args = StorageUploadArgs {
            handle: 42,
            items: vec![UploadItem {
                id: 1,
                partition: "00000000000000000000000000000001".into(),
                address: "00000000000000000000000000000000000000000000000000000000000000ab".into(),
            }],
        };
        let lore_args = args.into_lore().expect("into_lore");
        assert_eq!(lore_args.handle.handle_id, 42);
        assert_eq!(lore_args.items.as_slice().len(), 1);
    }

    #[test]
    fn args_empty_items_converts() {
        let args = StorageUploadArgs {
            handle: 1,
            items: vec![],
        };
        let lore_args = args.into_lore().expect("into_lore");
        assert_eq!(lore_args.items.as_slice().len(), 0);
    }
}
