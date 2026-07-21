//! `storage mutable_compare_and_swap` operation — binds
//! [`lore::storage::mutable_compare_and_swap::mutable_compare_and_swap`].
//!
//! Conditionally swaps a mutable key's value when its current value matches
//! `expected` (or the key is absent when `expected` is the null/default hash).
//! Returns the value the key held before the swap. Each item targets local or
//! remote mutable store (via `remote` / `globals.remote` + handle with
//! `remote_config`). Each item resolves to one terminal
//! `StorageMutableCompareAndSwapItemComplete` carrying
//! `{id, previous, error_code}`; the swap took effect when
//! `previous == expected` (and `error_code == None`).

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::mutable_compare_and_swap::LoreStorageMutableCompareAndSwapArgs;
use serde::{Deserialize, Serialize};

const HASH_ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn normalize_hash(hex: &str) -> String {
    if hex.is_empty() {
        HASH_ZERO.to_string()
    } else {
        hex.to_string()
    }
}

/// One CAS request — safe serialisable counterpart of
/// [`lore::storage::mutable_compare_and_swap::LoreStorageMutableCompareAndSwapItem`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableCompareAndSwapItem {
    /// Caller-chosen correlation id echoed back in the complete event.
    pub id: u64,
    /// Partition as a 32-char hex string (zero partition is rejected upstream).
    pub partition: String,
    /// Key as a 64-char hex hash.
    pub key: String,
    /// Expected current value (empty / zero-hash matches an absent key).
    #[serde(default)]
    pub expected: String,
    /// Value to store when the swap takes effect; empty / zero-hash removes the key.
    #[serde(default)]
    pub value: String,
    /// Upstream `KeyType` camelCase name (default `"untyped"`).
    #[serde(default = "default_key_type")]
    pub key_type: String,
}

fn default_key_type() -> String {
    "untyped".into()
}

/// Arguments for [`mutable_compare_and_swap`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMutableCompareAndSwapArgs {
    /// Handle id of an already-open store (from `storage open`).
    pub handle: u64,
    /// Swaps to perform.
    pub items: Vec<MutableCompareAndSwapItem>,
    /// When `true`, route to the remote mutable store (`globals.remote=1`).
    /// Requires a handle opened with `remote_config`.
    #[serde(default)]
    pub remote: bool,
}

impl StorageMutableCompareAndSwapArgs {
    fn into_lore(self) -> std::result::Result<LoreStorageMutableCompareAndSwapArgs, LoreError> {
        let items_json: Vec<serde_json::Value> = self
            .items
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "partition": item.partition,
                    "key": normalize_hash(&item.key),
                    "expected": normalize_hash(&item.expected),
                    "value": normalize_hash(&item.value),
                    "key_type": item.key_type,
                })
            })
            .collect();

        let args_json = serde_json::json!({
            "handle": { "handle_id": self.handle },
            "items": items_json,
        });

        serde_json::from_value::<LoreStorageMutableCompareAndSwapArgs>(args_json).map_err(|e| {
            LoreError::Parse(format!(
                "failed to build LoreStorageMutableCompareAndSwapArgs: {e}"
            ))
        })
    }
}

/// Per-item result from `mutable_compare_and_swap`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableCompareAndSwapItemResult {
    /// Correlation id of the item.
    pub id: u64,
    /// Value the key held before the swap (hex); empty on hard failure.
    pub previous: String,
    /// `true` when the op completed with `error_code == None` (swap may or may not have applied).
    pub ok: bool,
    /// `true` when the swap took effect (`previous == expected` and `ok`).
    pub swapped: bool,
    /// Error code name when `ok` is false; empty on success.
    #[serde(default)]
    pub error: String,
}

/// Result of a `mutable_compare_and_swap` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMutableCompareAndSwapResult {
    /// Per-item outcomes.
    pub items: Vec<MutableCompareAndSwapItemResult>,
}

fn globals_for(api: &LoreApi, remote: bool) -> crate::global::LoreGlobal {
    let mut g = api.globals();
    if remote {
        g = g.remote(true).offline(false).local(false);
    }
    g
}

/// Conditionally swap one or more mutable key values.
///
/// Calls upstream
/// [`lore::storage::mutable_compare_and_swap::mutable_compare_and_swap`]
/// in-process. On success (`ok`), `swapped` is true iff `previous == expected`.
pub async fn mutable_compare_and_swap(
    api: &LoreApi,
    args: StorageMutableCompareAndSwapArgs,
) -> Result<StorageMutableCompareAndSwapResult> {
    // Capture expected values for swapped computation (into_lore consumes args).
    let remote = args.remote;
    let expected_by_id: std::collections::HashMap<u64, String> = args
        .items
        .iter()
        .map(|i| (i.id, normalize_hash(&i.expected)))
        .collect();
    let lore_args = args.into_lore()?;
    let (callback, rx) = collect_events();

    let status = lore::storage::mutable_compare_and_swap::mutable_compare_and_swap(
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
        if let LoreEvent::StorageMutableCompareAndSwapItemComplete(data) = event {
            let code = format!("{:?}", data.error_code);
            let ok = code == "None";
            let previous = if ok {
                format!("{}", data.previous)
            } else {
                String::new()
            };
            let expected = expected_by_id
                .get(&data.id)
                .cloned()
                .unwrap_or_else(|| HASH_ZERO.to_string());
            let swapped = ok && previous == expected;
            items.push(MutableCompareAndSwapItemResult {
                id: data.id,
                previous,
                ok,
                swapped,
                error: if ok { String::new() } else { code },
            });
        }
    }

    // Call-level rejections emit no item events; per-item outcomes always return.
    if items.is_empty() && !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage mutable_compare_and_swap failed with status {status}"),
        )));
    }

    Ok(StorageMutableCompareAndSwapResult { items })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_defaults_empty_expected_and_value() {
        let item: MutableCompareAndSwapItem = serde_json::from_str(
            r#"{"id":1,"partition":"00000000000000000000000000000001","key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        )
        .expect("deserialise");
        assert!(item.expected.is_empty());
        assert!(item.value.is_empty());
        assert_eq!(item.key_type, "untyped");
    }

    #[test]
    fn args_round_trip() {
        let args = StorageMutableCompareAndSwapArgs {
            handle: 3,
            remote: true,
            items: vec![MutableCompareAndSwapItem {
                id: 1,
                partition: "00000000000000000000000000000001".into(),
                key: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into(),
                expected: String::new(),
                value: "1111111111111111111111111111111111111111111111111111111111111111".into(),
                key_type: "untyped".into(),
            }],
        };
        let json = serde_json::to_string(&args).expect("serialise");
        let back: StorageMutableCompareAndSwapArgs =
            serde_json::from_str(&json).expect("deserialise");
        assert!(back.remote);
        assert_eq!(
            back.items[0].value.chars().filter(|c| *c == '1').count(),
            64
        );
    }

    #[test]
    fn into_lore_normalises_empty_expected() {
        let args = StorageMutableCompareAndSwapArgs {
            handle: 1,
            remote: false,
            items: vec![MutableCompareAndSwapItem {
                id: 9,
                partition: "00000000000000000000000000000001".into(),
                key: "2222222222222222222222222222222222222222222222222222222222222222".into(),
                expected: String::new(),
                value: "3333333333333333333333333333333333333333333333333333333333333333".into(),
                key_type: "untyped".into(),
            }],
        };
        let lore = args.into_lore().expect("into_lore");
        assert_eq!(format!("{}", lore.items.as_slice()[0].expected), HASH_ZERO);
    }

    #[test]
    fn result_serialises_swapped_flag() {
        let result = StorageMutableCompareAndSwapResult {
            items: vec![MutableCompareAndSwapItemResult {
                id: 1,
                previous: HASH_ZERO.into(),
                ok: true,
                swapped: true,
                error: String::new(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"swapped\":true"));
    }
}
