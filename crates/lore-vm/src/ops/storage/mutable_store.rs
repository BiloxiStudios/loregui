//! `storage mutable_store` operation — binds [`lore::storage::mutable_store::mutable_store`].
//!
//! Writes one or more mutable key-value pairs on an open storage handle. Each
//! item targets the local or remote mutable store (selected via `remote` /
//! `globals.remote` + a handle opened with `remote_config`). Storing the null
//! value (`Hash::default()` / empty or all-zero hex) removes the key. Each item
//! resolves to one terminal `StorageMutableStoreItemComplete` carrying
//! `{id, error_code}`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::mutable_store::LoreStorageMutableStoreArgs;
use serde::{Deserialize, Serialize};

/// Zero / default 32-byte hash as 64 hex chars — store of this value removes the key.
const HASH_ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// One mutable store write — safe serialisable counterpart of
/// [`lore::storage::mutable_store::LoreStorageMutableStoreItem`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableStoreItem {
    /// Caller-chosen correlation id echoed back in the complete event.
    pub id: u64,
    /// Partition as a 32-char hex string (zero partition is rejected upstream).
    pub partition: String,
    /// Key as a 64-char hex hash.
    pub key: String,
    /// Value as a 64-char hex hash. Empty or all-zero removes the key.
    #[serde(default)]
    pub value: String,
    /// Upstream `KeyType` camelCase name (default `"untyped"`).
    #[serde(default = "default_key_type")]
    pub key_type: String,
}

fn default_key_type() -> String {
    "untyped".into()
}

fn normalize_hash(hex: &str) -> String {
    if hex.is_empty() {
        HASH_ZERO.to_string()
    } else {
        hex.to_string()
    }
}

/// Arguments for [`mutable_store`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMutableStoreArgs {
    /// Handle id of an already-open store (from `storage open`).
    pub handle: u64,
    /// Key-value pairs to write.
    pub items: Vec<MutableStoreItem>,
    /// When `true`, route to the remote mutable store (`globals.remote=1`).
    /// Requires a handle opened with `remote_config`.
    #[serde(default)]
    pub remote: bool,
}

impl StorageMutableStoreArgs {
    fn into_lore(self) -> std::result::Result<LoreStorageMutableStoreArgs, LoreError> {
        let items_json: Vec<serde_json::Value> = self
            .items
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "partition": item.partition,
                    "key": normalize_hash(&item.key),
                    "value": normalize_hash(&item.value),
                    "key_type": item.key_type,
                })
            })
            .collect();

        let args_json = serde_json::json!({
            "handle": { "handle_id": self.handle },
            "items": items_json,
        });

        serde_json::from_value::<LoreStorageMutableStoreArgs>(args_json).map_err(|e| {
            LoreError::Parse(format!("failed to build LoreStorageMutableStoreArgs: {e}"))
        })
    }
}

/// Per-item result from `mutable_store`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableStoreItemResult {
    /// Correlation id of the item.
    pub id: u64,
    /// `true` when `error_code == None`.
    pub ok: bool,
    /// Error code name when `ok` is false; empty on success.
    #[serde(default)]
    pub error: String,
}

/// Result of a `mutable_store` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMutableStoreResult {
    /// Per-item outcomes (one entry per input item that emitted a complete event).
    pub items: Vec<MutableStoreItemResult>,
}

fn globals_for(api: &LoreApi, remote: bool) -> crate::global::LoreGlobal {
    let mut g = api.globals();
    if remote {
        // Remote routing contradicts offline/local; clear them for this call.
        g = g.remote(true).offline(false).local(false);
    }
    g
}

/// Write one or more mutable key-value pairs on an open store.
///
/// Calls upstream [`lore::storage::mutable_store::mutable_store`] in-process and
/// collects `StorageMutableStoreItemComplete` events into a typed result.
pub async fn mutable_store(
    api: &LoreApi,
    args: StorageMutableStoreArgs,
) -> Result<StorageMutableStoreResult> {
    let remote = args.remote;
    let lore_args = args.into_lore()?;
    let (callback, rx) = collect_events();

    let status = lore::storage::mutable_store::mutable_store(
        globals_for(api, remote).build(),
        lore_args,
        callback,
    )
    .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    let mut items = Vec::new();
    for event in &stream.events {
        if let LoreEvent::StorageMutableStoreItemComplete(data) = event {
            let code = format!("{:?}", data.error_code);
            let ok = code == "None";
            items.push(MutableStoreItemResult {
                id: data.id,
                ok,
                error: if ok { String::new() } else { code },
            });
        }
    }

    // Call-level rejections (e.g. remote without config) emit no item events.
    // Per-item failures still return a typed result so callers can inspect codes.
    if items.is_empty() && !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage mutable_store failed with status {status}"),
        )));
    }

    Ok(StorageMutableStoreResult { items })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_defaults_empty_value_and_untyped() {
        let item: MutableStoreItem = serde_json::from_str(
            r#"{"id":1,"partition":"00000000000000000000000000000001","key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        )
        .expect("deserialise");
        assert!(item.value.is_empty());
        assert_eq!(item.key_type, "untyped");
    }

    #[test]
    fn args_round_trip() {
        let args = StorageMutableStoreArgs {
            handle: 9,
            remote: true,
            items: vec![MutableStoreItem {
                id: 1,
                partition: "00000000000000000000000000000001".into(),
                key: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
                value: String::new(),
                key_type: "branchLatestPointer".into(),
            }],
        };
        let json = serde_json::to_string(&args).expect("serialise");
        let back: StorageMutableStoreArgs = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.handle, 9);
        assert!(back.remote);
        assert_eq!(back.items.len(), 1);
        assert_eq!(back.items[0].key_type, "branchLatestPointer");
        assert!(back.items[0].value.is_empty());
    }

    #[test]
    fn into_lore_normalises_empty_value_to_zero_hash() {
        let args = StorageMutableStoreArgs {
            handle: 1,
            remote: false,
            items: vec![MutableStoreItem {
                id: 3,
                partition: "00000000000000000000000000000001".into(),
                key: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".into(),
                value: String::new(),
                key_type: "untyped".into(),
            }],
        };
        let lore = args.into_lore().expect("into_lore");
        assert_eq!(lore.handle.handle_id, 1);
        assert_eq!(lore.items.as_slice().len(), 1);
        let item = &lore.items.as_slice()[0];
        assert_eq!(item.id, 3);
        assert_eq!(format!("{}", item.value), HASH_ZERO);
    }

    #[test]
    fn into_lore_preserves_all_a_key_hex() {
        let key = "a".repeat(64);
        let args = StorageMutableStoreArgs {
            handle: 1,
            remote: false,
            items: vec![MutableStoreItem {
                id: 1,
                partition: "00000000000000000000000000000001".into(),
                key: key.clone(),
                value: "b".repeat(64),
                key_type: "branchLatestPointer".into(),
            }],
        };
        let lore = args.into_lore().expect("into_lore");
        assert_eq!(format!("{}", lore.items.as_slice()[0].key), key);
        assert_eq!(
            format!("{}", lore.items.as_slice()[0].value),
            "b".repeat(64)
        );
    }

    #[test]
    fn result_serialises() {
        let result = StorageMutableStoreResult {
            items: vec![
                MutableStoreItemResult {
                    id: 1,
                    ok: true,
                    error: String::new(),
                },
                MutableStoreItemResult {
                    id: 2,
                    ok: false,
                    error: "InvalidArguments".into(),
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("InvalidArguments"));
    }
}
