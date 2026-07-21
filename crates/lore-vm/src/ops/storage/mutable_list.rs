//! `storage mutable_list` operation — binds [`lore::storage::mutable_list::mutable_list`].
//!
//! Lists a partition's mutable key-value pairs of a given type from the handle's
//! **local** mutable store only. A remote-targeted call (`remote=true` /
//! `globals.remote`, or a remote-bound handle) is rejected with
//! `INVALID_ARGUMENTS` and the message
//! `"mutable_list is only supported on the local store"`.
//!
//! Each found pair is emitted as a `StorageMutableListEntry` event
//! `{id, key, value}`, followed by one terminal
//! `StorageMutableListItemComplete` event `{id, error_code}` for the item.
//! A default/zero partition lists across every partition the caller can access.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::LoreEvent;
use lore::storage::mutable_list::LoreStorageMutableListArgs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One mutable list request — safe serialisable counterpart of
/// [`lore::storage::mutable_list::LoreStorageMutableListItem`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableListItem {
    /// Caller-chosen correlation id echoed on every entry and the terminal event.
    pub id: u64,
    /// Partition as a 32-char hex string; zero/default lists every accessible partition.
    #[serde(default)]
    pub partition: String,
    /// Upstream `KeyType` camelCase name (default `"untyped"`).
    #[serde(default = "default_key_type")]
    pub key_type: String,
}

fn default_key_type() -> String {
    "untyped".into()
}

/// Arguments for [`mutable_list`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMutableListArgs {
    /// Handle id of an already-open store (from `storage open`).
    pub handle: u64,
    /// Listings to perform.
    pub items: Vec<MutableListItem>,
    /// When `true`, force remote routing — upstream **rejects** this with
    /// `InvalidArguments` ("mutable_list is only supported on the local store").
    #[serde(default)]
    pub remote: bool,
}

impl StorageMutableListArgs {
    fn into_lore(self) -> std::result::Result<LoreStorageMutableListArgs, LoreError> {
        let items_json: Vec<serde_json::Value> = self
            .items
            .into_iter()
            .map(|item| {
                let partition = if item.partition.is_empty() {
                    "00000000000000000000000000000000".to_string()
                } else {
                    item.partition
                };
                serde_json::json!({
                    "id": item.id,
                    "partition": partition,
                    "key_type": item.key_type,
                })
            })
            .collect();

        let args_json = serde_json::json!({
            "handle": { "handle_id": self.handle },
            "items": items_json,
        });

        serde_json::from_value::<LoreStorageMutableListArgs>(args_json).map_err(|e| {
            LoreError::Parse(format!("failed to build LoreStorageMutableListArgs: {e}"))
        })
    }
}

/// One listed key-value pair.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MutableListEntry {
    /// Key as 64-char hex.
    pub key: String,
    /// Value as 64-char hex.
    pub value: String,
}

/// Per-item result from `mutable_list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableListItemResult {
    /// Correlation id of the item.
    pub id: u64,
    /// Entries streamed for this item before the terminal complete event.
    pub entries: Vec<MutableListEntry>,
    /// `true` when `error_code == None`.
    pub ok: bool,
    /// Error code name when `ok` is false; empty on success.
    #[serde(default)]
    pub error: String,
}

/// Result of a `mutable_list` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMutableListResult {
    /// Per-item outcomes (entries + terminal status).
    pub items: Vec<MutableListItemResult>,
}

fn globals_for(api: &LoreApi, remote: bool) -> crate::global::LoreGlobal {
    let mut g = api.globals();
    if remote {
        g = g.remote(true).offline(false).local(false);
    }
    g
}

/// List mutable key-value pairs for one or more `(partition, key_type)` items.
///
/// Local-only. Remote-targeted calls fail the whole op with
/// `CommandFailed("…mutable_list is only supported on the local store…")`.
pub async fn mutable_list(
    api: &LoreApi,
    args: StorageMutableListArgs,
) -> Result<StorageMutableListResult> {
    let remote = args.remote;
    let lore_args = args.into_lore()?;
    let (callback, rx) = collect_events();

    let status = lore::storage::mutable_list::mutable_list(
        globals_for(api, remote).build(),
        lore_args,
        callback,
    )
    .await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    // Accumulate entries by id, then fold terminal complete status.
    let mut entries_by_id: HashMap<u64, Vec<MutableListEntry>> = HashMap::new();
    let mut order: Vec<u64> = Vec::new();
    let mut status_by_id: HashMap<u64, (bool, String)> = HashMap::new();

    for event in &stream.events {
        match event {
            LoreEvent::StorageMutableListEntry(data) => {
                let entry = MutableListEntry {
                    key: format!("{}", data.key),
                    value: format!("{}", data.value),
                };
                if !order.contains(&data.id) {
                    order.push(data.id);
                }
                entries_by_id.entry(data.id).or_default().push(entry);
            }
            LoreEvent::StorageMutableListItemComplete(data) => {
                let code = format!("{:?}", data.error_code);
                let ok = code == "None";
                if !order.contains(&data.id) {
                    order.push(data.id);
                }
                status_by_id.insert(data.id, (ok, if ok { String::new() } else { code }));
            }
            _ => {}
        }
    }

    let items: Vec<MutableListItemResult> = order
        .into_iter()
        .map(|id| {
            let entries = entries_by_id.remove(&id).unwrap_or_default();
            let (ok, error) = status_by_id.remove(&id).unwrap_or((true, String::new()));
            MutableListItemResult {
                id,
                entries,
                ok,
                error,
            }
        })
        .collect();

    // Remote-targeted list is rejected up front with no item events.
    if items.is_empty() && !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage mutable_list failed with status {status}"),
        )));
    }

    Ok(StorageMutableListResult { items })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_defaults() {
        let item: MutableListItem = serde_json::from_str(r#"{"id":1}"#).expect("deserialise");
        assert!(item.partition.is_empty());
        assert_eq!(item.key_type, "untyped");
    }

    #[test]
    fn args_round_trip() {
        let args = StorageMutableListArgs {
            handle: 5,
            remote: true,
            items: vec![MutableListItem {
                id: 1,
                partition: "00000000000000000000000000000001".into(),
                key_type: "branchLatestPointer".into(),
            }],
        };
        let json = serde_json::to_string(&args).expect("serialise");
        let back: StorageMutableListArgs = serde_json::from_str(&json).expect("deserialise");
        assert!(back.remote);
        assert_eq!(back.items[0].key_type, "branchLatestPointer");
    }

    #[test]
    fn into_lore_zero_partition_for_empty() {
        let args = StorageMutableListArgs {
            handle: 1,
            remote: false,
            items: vec![MutableListItem {
                id: 1,
                partition: String::new(),
                key_type: "untyped".into(),
            }],
        };
        let lore = args.into_lore().expect("into_lore");
        assert_eq!(lore.items.as_slice().len(), 1);
        assert_eq!(
            format!("{}", lore.items.as_slice()[0].partition),
            "00000000000000000000000000000000"
        );
    }

    #[test]
    fn result_serialises_entries() {
        let result = StorageMutableListResult {
            items: vec![MutableListItemResult {
                id: 1,
                entries: vec![MutableListEntry {
                    key: "aa".repeat(32),
                    value: "bb".repeat(32),
                }],
                ok: true,
                error: String::new(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"entries\""));
        assert!(json.contains("\"ok\":true"));
    }
}
