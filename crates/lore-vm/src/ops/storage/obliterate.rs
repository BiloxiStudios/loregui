//! `storage obliterate` operation — binds `lore::storage::obliterate`.
//!
//! Permanently deletes content at `(partition, address)` from an open storage
//! handle. Runs local and remote obliteration in parallel (when configured).
//! The operation is idempotent: an address not present on a side reports
//! success for that side.
//!
//! Collects `StorageObliterateItemComplete` events — one per requested item —
//! to produce a typed per-item result.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::obliterate::LoreStorageObliterateArgs;
use serde::{Deserialize, Serialize};

/// One item to obliterate — Tauri-friendly mirror of
/// [`LoreStorageObliterateItem`].
///
/// Partition and address are hex-encoded strings so they serialise cleanly
/// across the Tauri IPC boundary; the upstream C-repr types are reconstructed
/// via serde round-trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObliterateItem {
    /// Caller-chosen correlation id echoed back in the result.
    pub id: u64,
    /// Hex-encoded partition (32 hex chars / 16 bytes).
    pub partition: String,
    /// Hex-encoded content address (`<hash>` or `<hash>-<context>`).
    pub address: String,
}

/// Arguments for [`obliterate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageObliterateArgs {
    /// Handle id returned by a prior `storage open` call.
    pub handle: u64,
    /// Items (partition + address) to obliterate.
    pub items: Vec<ObliterateItem>,
}

impl StorageObliterateArgs {
    /// Convert to the upstream `LoreStorageObliterateArgs` via serde round-trip.
    ///
    /// `Partition`, `Address`, and `LoreArray` live in crates that are not
    /// direct dependencies of lore-vm, so we build intermediate JSON and let
    /// their `Deserialize` impls handle hex parsing.
    fn into_lore(self) -> std::result::Result<LoreStorageObliterateArgs, LoreError> {
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

        serde_json::from_value::<LoreStorageObliterateArgs>(args_json).map_err(|e| {
            LoreError::Parse(format!("failed to build LoreStorageObliterateArgs: {e}"))
        })
    }
}

/// Per-item result from the obliterate operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObliterateItemResult {
    /// Correlation id (matches the request item's `id`).
    pub id: u64,
    /// Content address (hex); empty on failure.
    pub address: String,
    /// `true` when the local side completed without error.
    pub local_success: bool,
    /// `true` when the remote side completed without error.
    pub remote_success: bool,
    /// `true` when the local side was skipped (e.g. offline/remote-only mode).
    pub local_skipped: bool,
    /// `true` when the remote side was skipped (e.g. no remote configured).
    pub remote_skipped: bool,
    /// Whether this item completed without error on all active sides.
    pub ok: bool,
    /// Error code name when `ok == false`; empty on success.
    #[serde(default)]
    pub error: String,
}

/// Result of the overall obliterate operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageObliterateResult {
    /// Per-item outcomes (one entry per input item).
    pub items: Vec<ObliterateItemResult>,
}

/// Delete one or more `(partition, address)` entries from an open store.
///
/// Calls the upstream `lore::storage::obliterate` in-process and collects
/// `StorageObliterateItemComplete` events to build a typed result.
pub async fn obliterate(
    api: &LoreApi,
    args: StorageObliterateArgs,
) -> Result<StorageObliterateResult> {
    let lore_args = args.into_lore()?;

    let (callback, rx) = collect_events();

    let status =
        lore::storage::obliterate::obliterate(api.globals().build(), lore_args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage obliterate failed with status {status}"),
        )));
    }

    let mut results: Vec<ObliterateItemResult> = Vec::new();
    for event in &stream.events {
        if let LoreEvent::StorageObliterateItemComplete(data) = event {
            let error_code_str = format!("{:?}", data.error_code);
            let ok = error_code_str == "None";
            results.push(ObliterateItemResult {
                id: data.id,
                address: if ok {
                    format!("{}", data.address)
                } else {
                    String::new()
                },
                local_success: data.local_success != 0,
                remote_success: data.remote_success != 0,
                local_skipped: data.local_skipped != 0,
                remote_skipped: data.remote_skipped != 0,
                ok,
                error: if ok { String::new() } else { error_code_str },
            });
        }
    }

    Ok(StorageObliterateResult { items: results })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises_to_json() {
        let args = StorageObliterateArgs {
            handle: 42,
            items: vec![ObliterateItem {
                id: 1,
                partition: "a".repeat(32),
                address: "b".repeat(64),
            }],
        };
        let json = serde_json::to_string(&args).expect("serialise");
        assert!(json.contains("\"handle\":42"));
        assert!(json.contains(&"a".repeat(32)));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"handle":7,"items":[{"id":1,"partition":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","address":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}]}"#;
        let args: StorageObliterateArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.handle, 7);
        assert_eq!(args.items.len(), 1);
        assert_eq!(args.items[0].id, 1);
    }

    #[test]
    fn args_deserialises_empty_items() {
        let json = r#"{"handle":1,"items":[]}"#;
        let args: StorageObliterateArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.handle, 1);
        assert!(args.items.is_empty());
    }

    #[test]
    fn item_serialises() {
        let item = ObliterateItem {
            id: 99,
            partition: "abc".into(),
            address: "def".into(),
        };
        let json = serde_json::to_string(&item).expect("serialise");
        assert!(json.contains("\"id\":99"));
        assert!(json.contains("\"partition\":\"abc\""));
        assert!(json.contains("\"address\":\"def\""));
    }

    #[test]
    fn result_serialises() {
        let result = StorageObliterateResult {
            items: vec![ObliterateItemResult {
                id: 1,
                address: "abc123".into(),
                local_success: true,
                remote_success: false,
                local_skipped: false,
                remote_skipped: true,
                ok: true,
                error: String::new(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("abc123"));
        assert!(json.contains("\"local_success\":true"));
        assert!(json.contains("\"remote_skipped\":true"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"items":[{"id":1,"address":"abc","local_success":true,"remote_success":true,"local_skipped":false,"remote_skipped":false,"ok":true,"error":""}]}"#;
        let result: StorageObliterateResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].ok);
        assert!(result.items[0].local_success);
    }

    #[test]
    fn result_empty_items() {
        let result = StorageObliterateResult { items: vec![] };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains(r#""items":[]"#));
    }

    #[test]
    fn result_with_error() {
        let result = StorageObliterateResult {
            items: vec![ObliterateItemResult {
                id: 5,
                address: String::new(),
                local_success: false,
                remote_success: false,
                local_skipped: false,
                remote_skipped: false,
                ok: false,
                error: "InvalidArguments".into(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("InvalidArguments"));
        assert!(json.contains("\"ok\":false"));
    }
}
