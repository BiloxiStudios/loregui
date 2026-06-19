//! `storage put_file` operation — binds [`lore::storage::put_file::put_file`].
//!
//! Reads one or more files from disk and stores their contents at
//! content-addressed locations in an open storage handle. Each item is
//! identified by `(partition, context)` and a filesystem `path`; per-item
//! results are collected from `StoragePutItemComplete` events emitted by the
//! upstream crate (same event type as `storage put`).

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::put_file::LoreStoragePutFileArgs;
use serde::{Deserialize, Serialize};

/// One file to store — the Tauri-friendly counterpart of
/// [`LoreStoragePutFileItem`](lore::storage::put_file::LoreStoragePutFileItem).
///
/// Partition and context are hex strings so they serialise cleanly across the
/// Tauri IPC boundary; the upstream C-repr types are reconstructed via serde
/// deserialisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutFileItem {
    /// Caller-chosen correlation id echoed back in the result.
    pub id: u64,
    /// Target partition as a 32-char hex string.
    pub partition: String,
    /// Dedup context as a 32-char hex string; empty string → zero context.
    #[serde(default)]
    pub context: String,
    /// Filesystem path of the file to read and store.
    pub path: String,
    /// Opt into remote upload (default false).
    #[serde(default)]
    pub remote_write: bool,
    /// Tag the fragment for local cache priority (default false).
    #[serde(default)]
    pub local_cache: bool,
    /// Leaf fragment size cap for large files; 0 lets the engine choose.
    #[serde(default)]
    pub fixed_size_chunk: u64,
}

/// Arguments for [`put_file`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePutFileArgs {
    /// Handle id of an already-open store (from `storage open`).
    pub handle: u64,
    /// Files to store.
    pub items: Vec<PutFileItem>,
}

impl StoragePutFileArgs {
    /// Convert to the upstream `LoreStoragePutFileArgs` via serde round-trip.
    fn into_lore(self) -> std::result::Result<LoreStoragePutFileArgs, LoreError> {
        let items_json: Vec<serde_json::Value> = self
            .items
            .into_iter()
            .map(|item| {
                let context = if item.context.is_empty() {
                    "00000000000000000000000000000000".to_string()
                } else {
                    item.context
                };
                serde_json::json!({
                    "id": item.id,
                    "partition": item.partition,
                    "context": context,
                    "path": item.path,
                    "remote_write": u8::from(item.remote_write),
                    "local_cache": u8::from(item.local_cache),
                    "fixed_size_chunk": item.fixed_size_chunk,
                })
            })
            .collect();

        let args_json = serde_json::json!({
            "handle": { "handle_id": self.handle },
            "items": items_json,
        });

        serde_json::from_value::<LoreStoragePutFileArgs>(args_json)
            .map_err(|e| LoreError::Parse(format!("failed to build LoreStoragePutFileArgs: {e}")))
    }
}

/// Per-item result from the put_file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutFileItemResult {
    /// Correlation id of the item.
    pub id: u64,
    /// The content address as `"<hash>-<context>"`, or empty on failure.
    pub address: String,
    /// `true` if the item was stored successfully.
    pub ok: bool,
    /// Error code name when `ok` is false; empty on success.
    #[serde(default)]
    pub error: String,
}

/// Result of the overall put_file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePutFileResult {
    /// Per-item outcomes (one entry per input item).
    pub items: Vec<PutFileItemResult>,
}

/// Read one or more files from disk into the content-addressed store.
///
/// Calls upstream [`lore::storage::put_file::put_file`] in-process and
/// collects the `StoragePutItemComplete` events to build a typed result.
pub async fn put_file(api: &LoreApi, args: StoragePutFileArgs) -> Result<StoragePutFileResult> {
    let lore_args = args.into_lore()?;

    let (callback, rx) = collect_events();

    let status =
        lore::storage::put_file::put_file(api.globals().build(), lore_args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage put_file failed with status {status}"),
        )));
    }

    let mut results: Vec<PutFileItemResult> = Vec::new();
    for event in &stream.events {
        if let LoreEvent::StoragePutItemComplete(data) = event {
            let error_code_val = data.error_code as i32;
            let ok = error_code_val == 0;
            results.push(PutFileItemResult {
                id: data.id,
                address: if ok {
                    format!("{}", data.address)
                } else {
                    String::new()
                },
                ok,
                error: if ok {
                    String::new()
                } else {
                    format!("{:?}", data.error_code)
                },
            });
        }
    }

    Ok(StoragePutFileResult { items: results })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_serialises_to_json() {
        let result = StoragePutFileResult {
            items: vec![PutFileItemResult {
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
        let result = PutFileItemResult {
            id: 7,
            address: String::new(),
            ok: false,
            error: "InvalidArguments".into(),
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("InvalidArguments"));
    }

    #[test]
    fn empty_result() {
        let result = StoragePutFileResult { items: vec![] };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"items\":[]"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"items":[{"id":1,"address":"abc","ok":true,"error":""}]}"#;
        let result: StoragePutFileResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].id, 1);
        assert!(result.items[0].ok);
    }

    #[test]
    fn args_serialises() {
        let args = StoragePutFileArgs {
            handle: 99,
            items: vec![PutFileItem {
                id: 1,
                partition: "a".repeat(32),
                context: String::new(),
                path: "/tmp/test.bin".into(),
                remote_write: false,
                local_cache: false,
                fixed_size_chunk: 0,
            }],
        };
        let json = serde_json::to_string(&args).expect("serialise");
        assert!(json.contains("\"handle\":99"));
        assert!(json.contains("/tmp/test.bin"));
    }

    #[test]
    fn args_deserialises() {
        let json = r#"{"handle":5,"items":[{"id":1,"partition":"00000000000000000000000000000001","path":"/tmp/f.bin"}]}"#;
        let args: StoragePutFileArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.handle, 5);
        assert_eq!(args.items.len(), 1);
        assert_eq!(args.items[0].id, 1);
        assert_eq!(args.items[0].path, "/tmp/f.bin");
    }

    #[test]
    fn args_converts_to_lore() {
        let args = StoragePutFileArgs {
            handle: 42,
            items: vec![PutFileItem {
                id: 1,
                partition: "00000000000000000000000000000001".into(),
                context: String::new(),
                path: "/tmp/test.bin".into(),
                remote_write: false,
                local_cache: false,
                fixed_size_chunk: 0,
            }],
        };
        let lore_args = args.into_lore().expect("into_lore");
        assert_eq!(lore_args.handle.handle_id, 42);
        assert_eq!(lore_args.items.as_slice().len(), 1);
    }

    #[test]
    fn args_empty_items_converts() {
        let args = StoragePutFileArgs {
            handle: 1,
            items: vec![],
        };
        let lore_args = args.into_lore().expect("into_lore");
        assert_eq!(lore_args.items.as_slice().len(), 0);
    }

    #[test]
    fn args_with_context_converts() {
        let args = StoragePutFileArgs {
            handle: 10,
            items: vec![PutFileItem {
                id: 2,
                partition: "00000000000000000000000000000001".into(),
                context: "ffffffffffffffffffffffffffffffff".into(),
                path: "/tmp/data.txt".into(),
                remote_write: true,
                local_cache: true,
                fixed_size_chunk: 4096,
            }],
        };
        let lore_args = args.into_lore().expect("into_lore");
        assert_eq!(lore_args.items.as_slice().len(), 1);
    }

    #[test]
    fn args_defaults_on_deserialise() {
        let json = r#"{"handle":1,"items":[{"id":0,"partition":"00000000000000000000000000000001","path":"/x"}]}"#;
        let args: StoragePutFileArgs = serde_json::from_str(json).expect("deserialise");
        assert!(!args.items[0].remote_write);
        assert!(!args.items[0].local_cache);
        assert_eq!(args.items[0].fixed_size_chunk, 0);
        assert!(args.items[0].context.is_empty());
    }
}
