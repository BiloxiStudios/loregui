//! `storage get_metadata` operation — binds `lore::storage::get_metadata`.
//!
//! Fetches fragment metadata (flags, payload size, content size) for one or
//! more content-addressed items without transferring payload bytes.
//!
//! Each item emits a single `StorageGetMetadataItemComplete` event carrying
//! `{id, address, fragment, error_code}`.  On success `error_code` is `None`
//! and the `Fragment` carries the resolved metadata; on miss `error_code` is
//! `AddressNotFound` and the fragment is default/zeroed.
//!
//! Unlike `storage get`, no binary data is returned — the generic oneshot-
//! collector pattern suffices (no need for the specialised `LoreBytes`-copying
//! callback that `get` requires).

use crate::api::LoreApi;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreEventCallback};
use lore::storage::get_metadata::LoreStorageGetMetadataArgs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// A single item to look up — Tauri-friendly mirror of `LoreStorageGetMetadataItem`.
///
/// Hex-encoded strings for partition and address cross the serialisation
/// boundary cleanly; the upstream C-repr types are reconstructed via serde
/// deserialisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMetadataItem {
    /// Caller-chosen correlation id echoed back in the completion event.
    pub id: u64,
    /// Hex-encoded partition (32 hex chars / 16 bytes).
    pub partition: String,
    /// Hex-encoded content address (`<hash>` or `<hash>-<context>`).
    pub address: String,
}

/// Arguments for [`get_metadata`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageGetMetadataArgs {
    /// Handle id returned by a prior `storage open` call.
    pub handle: u64,
    /// Items (partition + address) to look up.
    pub items: Vec<GetMetadataItem>,
}

impl StorageGetMetadataArgs {
    /// Convert to the upstream `LoreStorageGetMetadataArgs` via serde round-trip.
    ///
    /// The upstream types (`Partition`, `Address`, `LoreArray`) live in crates
    /// that are not direct dependencies of lore-vm, so we build the
    /// intermediate JSON and let their `Deserialize` impls handle hex parsing.
    fn into_lore(self) -> std::result::Result<LoreStorageGetMetadataArgs, LoreError> {
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

        serde_json::from_value::<LoreStorageGetMetadataArgs>(args_json).map_err(|e| {
            LoreError::Parse(format!("failed to build LoreStorageGetMetadataArgs: {e}"))
        })
    }
}

/// Fragment metadata for a single item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentMetadata {
    /// Fragment flags (bitfield).
    pub flags: u32,
    /// Payload size in bytes (on-disk, possibly compressed/chunked).
    pub size_payload: u32,
    /// Content size in bytes (uncompressed, reassembled).
    pub size_content: u64,
}

/// Per-item result from a `storage get_metadata` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageGetMetadataItemResult {
    /// Correlation id (matches the request item's `id`).
    pub id: u64,
    /// Content address (hex).
    pub address: String,
    /// Fragment metadata; present when the item resolved successfully.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment: Option<FragmentMetadata>,
    /// Whether this item completed successfully.
    pub ok: bool,
    /// Error code name when `ok == false` (e.g. `"AddressNotFound"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of a `storage get_metadata` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageGetMetadataResult {
    /// Per-item results, one for each requested item.
    pub items: Vec<StorageGetMetadataItemResult>,
}

/// Intermediate per-item state accumulated during the callback.
#[derive(Default)]
struct ItemAccum {
    address: String,
    flags: u32,
    size_payload: u32,
    size_content: u64,
    ok: bool,
    error: Option<String>,
}

/// Collected state from the storage-get-metadata callback.
#[derive(Default)]
struct GetMetadataCollector {
    items: HashMap<u64, ItemAccum>,
    order: Vec<u64>,
    status: Option<i32>,
    call_error: Option<String>,
}

/// Fetch fragment metadata for one or more content-addressed items.
///
/// Calls the upstream `lore::storage::get_metadata::get_metadata` in-process
/// with a callback that captures per-item `StorageGetMetadataItemComplete`
/// events. No binary payload is transferred — only fragment metadata.
pub async fn get_metadata(
    api: &LoreApi,
    args: StorageGetMetadataArgs,
) -> Result<StorageGetMetadataResult> {
    let lore_args = args.into_lore()?;

    let collector: Arc<Mutex<GetMetadataCollector>> =
        Arc::new(Mutex::new(GetMetadataCollector::default()));
    let (tx, rx) = oneshot::channel::<()>();
    let tx: Arc<Mutex<Option<oneshot::Sender<()>>>> = Arc::new(Mutex::new(Some(tx)));

    let cb_collector = collector.clone();
    let cb_tx = tx.clone();

    let callback: LoreEventCallback = Some(Box::new(move |event: &LoreEvent| {
        let mut c = cb_collector.lock().unwrap();
        match event {
            LoreEvent::StorageGetMetadataItemComplete(d) => {
                // LoreErrorCode::None == 0 in the repr(C) enum; use the
                // Debug representation to classify without importing the type.
                let code_str = format!("{:?}", d.error_code);
                let ok = code_str == "None";
                let error = if ok { None } else { Some(code_str) };

                let accum = c.items.entry(d.id).or_default();
                accum.address = format!("{}", d.address);
                accum.flags = d.fragment.flags;
                accum.size_payload = d.fragment.size_payload;
                accum.size_content = d.fragment.size_content;
                accum.ok = ok;
                accum.error = error;
                if !c.order.contains(&d.id) {
                    c.order.push(d.id);
                }
            }
            LoreEvent::Error(e) => {
                c.call_error = Some(e.error_inner.as_str().to_string());
            }
            LoreEvent::Complete(d) => {
                c.status = Some(d.status);
            }
            _ => {}
        }

        let done = matches!(event, LoreEvent::Complete(_) | LoreEvent::Error(_));
        if done {
            drop(c);
            if let Some(sender) = cb_tx.lock().unwrap().take() {
                let _ = sender.send(());
            }
        }
    }));

    let status =
        lore::storage::get_metadata::get_metadata(api.globals().build(), lore_args, callback).await;

    // Wait for the terminal event to fire.
    let _ = rx.await;

    let c = collector.lock().unwrap();

    if c.status != Some(0) || c.call_error.is_some() {
        return Err(LoreError::CommandFailed(
            c.call_error
                .clone()
                .unwrap_or_else(|| format!("storage get_metadata failed with status {status}")),
        ));
    }

    let items = c
        .order
        .iter()
        .map(|id| {
            let accum = c.items.get(id).expect("order contains only inserted ids");
            let fragment = if accum.ok {
                Some(FragmentMetadata {
                    flags: accum.flags,
                    size_payload: accum.size_payload,
                    size_content: accum.size_content,
                })
            } else {
                None
            };
            StorageGetMetadataItemResult {
                id: *id,
                address: accum.address.clone(),
                fragment,
                ok: accum.ok,
                error: accum.error.clone(),
            }
        })
        .collect();

    Ok(StorageGetMetadataResult { items })
}

/// Backwards-compatible command symbol for existing Tauri/frontend wiring.
///
/// The domain-relative API is [`get_metadata`]. The GUI command surface still
/// invokes `storage_get_metadata`, so keep this delegating wrapper until the
/// generated command references are renamed in one coordinated pass.
pub async fn storage_get_metadata(
    api: &LoreApi,
    args: StorageGetMetadataArgs,
) -> Result<StorageGetMetadataResult> {
    get_metadata(api, args).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_serialises_round_trip() {
        let args = StorageGetMetadataArgs {
            handle: 42,
            items: vec![GetMetadataItem {
                id: 1,
                partition: "a".repeat(32),
                address: "b".repeat(64),
            }],
        };
        let json = serde_json::to_string(&args).expect("serialise");
        let back: StorageGetMetadataArgs = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.handle, 42);
        assert_eq!(back.items.len(), 1);
        assert_eq!(back.items[0].id, 1);
    }

    #[test]
    fn result_serialises_success_item() {
        let result = StorageGetMetadataResult {
            items: vec![StorageGetMetadataItemResult {
                id: 1,
                address: "abc123".into(),
                fragment: Some(FragmentMetadata {
                    flags: 0,
                    size_payload: 1024,
                    size_content: 2048,
                }),
                ok: true,
                error: None,
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("\"size_content\":2048"));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn result_serialises_error_item() {
        let result = StorageGetMetadataResult {
            items: vec![StorageGetMetadataItemResult {
                id: 2,
                address: String::new(),
                fragment: None,
                ok: false,
                error: Some("AddressNotFound".into()),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("AddressNotFound"));
        assert!(!json.contains("\"fragment\""));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{"items":[{"id":1,"address":"aa","fragment":{"flags":3,"size_payload":512,"size_content":1024},"ok":true}]}"#;
        let result: StorageGetMetadataResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].ok);
        let frag = result.items[0].fragment.as_ref().unwrap();
        assert_eq!(frag.flags, 3);
        assert_eq!(frag.size_payload, 512);
        assert_eq!(frag.size_content, 1024);
    }

    #[test]
    fn empty_result() {
        let result = StorageGetMetadataResult { items: vec![] };
        let json = serde_json::to_string(&result).expect("serialise");
        assert_eq!(json, r#"{"items":[]}"#);
    }

    #[test]
    fn fragment_metadata_serialises() {
        let frag = FragmentMetadata {
            flags: 7,
            size_payload: 100,
            size_content: 200,
        };
        let json = serde_json::to_string(&frag).expect("serialise");
        assert!(json.contains("\"flags\":7"));
        assert!(json.contains("\"size_payload\":100"));
        assert!(json.contains("\"size_content\":200"));
    }

    #[test]
    fn args_deserialises_minimal() {
        let json = r#"{"handle":5,"items":[]}"#;
        let args: StorageGetMetadataArgs = serde_json::from_str(json).expect("deserialise");
        assert_eq!(args.handle, 5);
        assert!(args.items.is_empty());
    }
}
