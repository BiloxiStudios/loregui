//! `storage copy` operation — binds [`lore::storage::copy::copy`].
//!
//! Copies content between `(partition, context)` tuples in the same open store.
//! Each item relocates content from `(source_partition, source_address)` to
//! `(target_partition, source_address.hash, target_context)`, preserving the
//! content hash. Per-item results are collected from `StorageCopyItemComplete`
//! events emitted by the upstream crate.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::copy::LoreStorageCopyArgs;
use serde::{Deserialize, Serialize};

/// One copy item — the safe, serialisable counterpart of [`LoreStorageCopyItem`].
///
/// Hex-encoded strings for partitions, address, and context cross the Tauri
/// boundary cleanly; the upstream C-repr types are reconstructed via serde
/// round-trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyItem {
    /// Caller-chosen correlation id echoed back in `StorageCopyItemComplete`.
    pub id: u64,
    /// Source partition as a 32-char hex string.
    pub source_partition: String,
    /// Target partition as a 32-char hex string.
    pub target_partition: String,
    /// Source content address as hex (`<hash>` or `<hash>-<context>`).
    pub source_address: String,
    /// Dedup context for the destination address as a 32-char hex string;
    /// empty string falls back to zero context.
    #[serde(default)]
    pub target_context: String,
}

/// Arguments for [`copy`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCopyArgs {
    /// Handle id returned by a prior `storage open` call.
    pub handle: u64,
    /// Copy requests; each runs independently and emits its own completion event.
    pub items: Vec<CopyItem>,
}

impl StorageCopyArgs {
    /// Convert to the upstream `LoreStorageCopyArgs` via serde round-trip.
    ///
    /// The upstream types (`Partition`, `Address`, `Context`, `LoreArray`) live
    /// in crates that are not direct dependencies of lore-vm, so we build the
    /// intermediate JSON and let their `Deserialize` impls handle hex parsing.
    fn into_lore(self) -> std::result::Result<LoreStorageCopyArgs, LoreError> {
        let items_json: Vec<serde_json::Value> = self
            .items
            .into_iter()
            .map(|item| {
                let target_context = if item.target_context.is_empty() {
                    "00000000000000000000000000000000".to_string()
                } else {
                    item.target_context
                };
                serde_json::json!({
                    "id": item.id,
                    "source_partition": item.source_partition,
                    "target_partition": item.target_partition,
                    "source_address": item.source_address,
                    "target_context": target_context,
                })
            })
            .collect();

        let args_json = serde_json::json!({
            "handle": { "handle_id": self.handle },
            "items": items_json,
        });

        serde_json::from_value::<LoreStorageCopyArgs>(args_json)
            .map_err(|e| LoreError::Parse(format!("failed to build LoreStorageCopyArgs: {e}")))
    }
}

/// Per-item result from the copy operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyItemResult {
    /// Correlation id of the item.
    pub id: u64,
    /// Source partition (hex), echoed back from the completion event.
    pub source_partition: String,
    /// Target partition (hex), echoed back from the completion event.
    pub target_partition: String,
    /// Source address (hex), or empty on failure.
    pub source_address: String,
    /// Target context (hex), echoed back from the completion event.
    pub target_context: String,
    /// `true` if the item was copied successfully.
    pub ok: bool,
    /// Error code name when `ok` is false; empty on success.
    #[serde(default)]
    pub error: String,
}

/// Result of the overall copy operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCopyResult {
    /// Per-item outcomes (one entry per input item).
    pub items: Vec<CopyItemResult>,
}

/// Copy content between partitions in an open store.
///
/// Calls upstream [`lore::storage::copy::copy`] in-process and collects the
/// `StorageCopyItemComplete` events to build a typed result.
pub async fn copy(api: &LoreApi, args: StorageCopyArgs) -> Result<StorageCopyResult> {
    let lore_args = args.into_lore()?;

    let (callback, rx) = collect_events();

    let status =
        lore::storage::copy::copy(api.globals().build(), lore_args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage copy failed with status {status}"),
        )));
    }

    let mut results: Vec<CopyItemResult> = Vec::new();
    for event in &stream.events {
        if let LoreEvent::StorageCopyItemComplete(data) = event {
            let error_code_val = data.error_code as i32;
            let ok = error_code_val == 0;
            results.push(CopyItemResult {
                id: data.id,
                source_partition: format!("{}", data.source_partition),
                target_partition: format!("{}", data.target_partition),
                source_address: if ok {
                    format!("{}", data.source_address)
                } else {
                    String::new()
                },
                target_context: format!("{}", data.target_context),
                ok,
                error: if ok {
                    String::new()
                } else {
                    format!("{:?}", data.error_code)
                },
            });
        }
    }

    Ok(StorageCopyResult { items: results })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = StorageCopyResult {
            items: vec![CopyItemResult {
                id: 1,
                source_partition: "aa".repeat(16),
                target_partition: "bb".repeat(16),
                source_address: "cc".repeat(16),
                target_context: "dd".repeat(16),
                ok: true,
                error: String::new(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains(&"aa".repeat(16)));
    }

    #[test]
    fn empty_result() {
        let result = StorageCopyResult { items: vec![] };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"items\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"items":[{"id":1,"source_partition":"aa","target_partition":"bb","source_address":"cc","target_context":"dd","ok":true,"error":""}]}"#;
        let result: StorageCopyResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].ok);
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"handle":42,"items":[{"id":1,"source_partition":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","target_partition":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","source_address":"cccccccccccccccccccccccccccccccc","target_context":"dddddddddddddddddddddddddddddddd"}]}"#;
        let args: StorageCopyArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.handle, 42);
        assert_eq!(args.items.len(), 1);
        assert_eq!(args.items[0].id, 1);
    }

    /// Address format is `<64 hex hash>-<32 hex context>`.
    fn test_address() -> String {
        format!("{}-{}", "aa".repeat(32), "bb".repeat(16))
    }

    #[test]
    fn args_into_lore_builds_successfully() {
        let args = StorageCopyArgs {
            handle: 7,
            items: vec![CopyItem {
                id: 1,
                source_partition: "aa".repeat(16),
                target_partition: "bb".repeat(16),
                source_address: test_address(),
                target_context: "".into(),
            }],
        };
        // Should not panic — validates the serde round-trip builds correctly
        let lore_args = args.into_lore().expect("into_lore");
        assert_eq!(lore_args.handle.handle_id, 7);
    }

    #[test]
    fn copy_item_default_context() {
        let item = CopyItem {
            id: 0,
            source_partition: "aa".repeat(16),
            target_partition: "bb".repeat(16),
            source_address: test_address(),
            target_context: String::new(),
        };
        // Empty target_context should be replaced with zero context in into_lore
        let args = StorageCopyArgs {
            handle: 1,
            items: vec![item],
        };
        assert!(args.into_lore().is_ok());
    }
}
