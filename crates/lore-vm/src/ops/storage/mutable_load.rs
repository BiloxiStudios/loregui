//! `storage mutable_load` operation — binds [`lore::storage::mutable_load::mutable_load`].
//!
//! Reads one or more mutable keys from an open storage handle. Each item targets
//! the local or remote mutable store (via `remote` / `globals.remote` + a handle
//! opened with `remote_config`). Each item resolves to one terminal
//! `StorageMutableLoadItemComplete` carrying `{id, value, error_code}`; a key
//! with no stored value reports `error_code == AddressNotFound`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::mutable_load::LoreStorageMutableLoadArgs;
use serde::{Deserialize, Serialize};

const HASH_ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn normalize_hash(hex: &str) -> String {
    if hex.is_empty() {
        HASH_ZERO.to_string()
    } else {
        hex.to_string()
    }
}

/// One mutable load request — safe serialisable counterpart of
/// [`lore::storage::mutable_load::LoreStorageMutableLoadItem`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableLoadItem {
    /// Caller-chosen correlation id echoed back in the complete event.
    pub id: u64,
    /// Partition as a 32-char hex string (zero partition is rejected upstream).
    pub partition: String,
    /// Key as a 64-char hex hash.
    pub key: String,
    /// Upstream `KeyType` camelCase name (default `"untyped"`).
    #[serde(default = "default_key_type")]
    pub key_type: String,
}

fn default_key_type() -> String {
    "untyped".into()
}

/// Arguments for [`mutable_load`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMutableLoadArgs {
    /// Handle id of an already-open store (from `storage open`).
    pub handle: u64,
    /// Keys to read.
    pub items: Vec<MutableLoadItem>,
    /// When `true`, route to the remote mutable store (`globals.remote=1`).
    /// Requires a handle opened with `remote_config`.
    #[serde(default)]
    pub remote: bool,
}

impl StorageMutableLoadArgs {
    fn into_lore(self) -> std::result::Result<LoreStorageMutableLoadArgs, LoreError> {
        let items_json: Vec<serde_json::Value> = self
            .items
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "partition": item.partition,
                    "key": normalize_hash(&item.key),
                    "key_type": item.key_type,
                })
            })
            .collect();

        let args_json = serde_json::json!({
            "handle": { "handle_id": self.handle },
            "items": items_json,
        });

        serde_json::from_value::<LoreStorageMutableLoadArgs>(args_json).map_err(|e| {
            LoreError::Parse(format!("failed to build LoreStorageMutableLoadArgs: {e}"))
        })
    }
}

/// Per-item result from `mutable_load`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableLoadItemResult {
    /// Correlation id of the item.
    pub id: u64,
    /// Loaded value as 64-char hex; empty when not found or on error.
    pub value: String,
    /// `true` when `error_code == None`.
    pub ok: bool,
    /// Error code name when `ok` is false (e.g. `"AddressNotFound"`); empty on success.
    #[serde(default)]
    pub error: String,
}

/// Result of a `mutable_load` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMutableLoadResult {
    /// Per-item outcomes.
    pub items: Vec<MutableLoadItemResult>,
}

fn globals_for(api: &LoreApi, remote: bool) -> crate::global::LoreGlobal {
    let mut g = api.globals();
    if remote {
        g = g.remote(true).offline(false).local(false);
    }
    g
}

/// Read one or more mutable key values from an open store.
///
/// Calls upstream [`lore::storage::mutable_load::mutable_load`] in-process and
/// collects `StorageMutableLoadItemComplete` events into a typed result.
/// Absent keys surface as `ok=false` with `error="AddressNotFound"`.
pub async fn mutable_load(
    api: &LoreApi,
    args: StorageMutableLoadArgs,
) -> Result<StorageMutableLoadResult> {
    let remote = args.remote;
    let lore_args = args.into_lore()?;
    let (callback, rx) = collect_events();

    let status = lore::storage::mutable_load::mutable_load(
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
        if let LoreEvent::StorageMutableLoadItemComplete(data) = event {
            let code = format!("{:?}", data.error_code);
            let ok = code == "None";
            items.push(MutableLoadItemResult {
                id: data.id,
                value: if ok {
                    format!("{}", data.value)
                } else {
                    String::new()
                },
                ok,
                error: if ok { String::new() } else { code },
            });
        }
    }

    // Absent keys report per-item AddressNotFound (overall status may be non-zero).
    // Call-level rejections emit no item events and surface as CommandFailed.
    if items.is_empty() && !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage mutable_load failed with status {status}"),
        )));
    }

    Ok(StorageMutableLoadResult { items })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_defaults_untyped() {
        let item: MutableLoadItem = serde_json::from_str(
            r#"{"id":1,"partition":"00000000000000000000000000000001","key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        )
        .expect("deserialise");
        assert_eq!(item.key_type, "untyped");
    }

    #[test]
    fn args_round_trip() {
        let args = StorageMutableLoadArgs {
            handle: 4,
            remote: false,
            items: vec![MutableLoadItem {
                id: 7,
                partition: "00000000000000000000000000000001".into(),
                key: "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".into(),
                key_type: "branchId".into(),
            }],
        };
        let json = serde_json::to_string(&args).expect("serialise");
        let back: StorageMutableLoadArgs = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.handle, 4);
        assert_eq!(back.items[0].key_type, "branchId");
    }

    #[test]
    fn into_lore_builds_upstream_args() {
        let args = StorageMutableLoadArgs {
            handle: 2,
            remote: false,
            items: vec![MutableLoadItem {
                id: 1,
                partition: "00000000000000000000000000000001".into(),
                key: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".into(),
                key_type: "untyped".into(),
            }],
        };
        let lore = args.into_lore().expect("into_lore");
        assert_eq!(lore.handle.handle_id, 2);
        assert_eq!(lore.items.as_slice().len(), 1);
    }

    #[test]
    fn result_serialises_address_not_found() {
        let result = StorageMutableLoadResult {
            items: vec![MutableLoadItemResult {
                id: 1,
                value: String::new(),
                ok: false,
                error: "AddressNotFound".into(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("AddressNotFound"));
        assert!(json.contains("\"ok\":false"));
    }
}
