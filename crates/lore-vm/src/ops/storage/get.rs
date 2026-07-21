//! `storage get` operation — binds `lore::storage::get`.
//!
//! Reads one or more content-addressed buffers from an open store handle.
//! Emits per-item `StorageGetHeader`, `StorageGetData`, and
//! `StorageGetItemComplete` events; this binding collects them into a typed
//! [`StorageGetResult`] with one [`StorageGetItemResult`] per requested item.
//!
//! Unlike the generic [`collect_events`](crate::collect::collect_events)
//! helper, this op uses a specialised callback that copies `LoreBytes`
//! payloads during the callback invocation — the raw-pointer view is only
//! valid for that window, so the generic clone-based collector cannot safely
//! capture binary data.

use crate::api::LoreApi;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreEventCallback};
use lore::storage::get::LoreStorageGetArgs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// A single item to retrieve — Tauri-friendly mirror of [`LoreStorageGetItem`].
///
/// Hex-encoded strings for partition and address cross the serialisation
/// boundary cleanly; the upstream C-repr types are reconstructed via serde
/// deserialisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetItem {
    /// Caller-chosen correlation id echoed back in every event for this item.
    pub id: u64,
    /// Hex-encoded partition (32 hex chars / 16 bytes).
    pub partition: String,
    /// Hex-encoded content address (`<hash>` or `<hash>-<context>`).
    pub address: String,
    /// When `true`, emit one `GET_DATA` per leaf fragment instead of a single
    /// reassembled buffer.
    #[serde(default)]
    pub streaming: bool,
    /// Cache fetched bytes back to the local store.
    #[serde(default)]
    pub local_cache: bool,
}

/// Arguments for [`get`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageGetArgs {
    /// Handle id returned by a prior `storage open` call.
    pub handle: u64,
    /// Items (partition + address) to read.
    pub items: Vec<GetItem>,
}

impl StorageGetArgs {
    /// Convert to the upstream `LoreStorageGetArgs` via serde round-trip.
    ///
    /// The upstream types (`Partition`, `Address`, `LoreArray`) live in crates
    /// that are not direct dependencies of lore-vm, so we build the
    /// intermediate JSON and let their `Deserialize` impls handle hex parsing.
    fn into_lore(self) -> std::result::Result<LoreStorageGetArgs, LoreError> {
        let items_json: Vec<serde_json::Value> = self
            .items
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "partition": item.partition,
                    "address": item.address,
                    "streaming": u8::from(item.streaming),
                    "local_cache": u8::from(item.local_cache),
                })
            })
            .collect();

        let args_json = serde_json::json!({
            "handle": { "handle_id": self.handle },
            "items": items_json,
        });

        serde_json::from_value::<LoreStorageGetArgs>(args_json)
            .map_err(|e| LoreError::Parse(format!("failed to build LoreStorageGetArgs: {e}")))
    }
}

/// Per-item result returned from a `storage get` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageGetItemResult {
    /// Correlation id (matches the request item's `id`).
    pub id: u64,
    /// Content address (hex).
    pub address: String,
    /// Total content size in bytes (from the header event).
    pub size: u64,
    /// The retrieved payload as raw bytes.
    ///
    /// Serialised as a JSON array of numbers by default; callers that need a
    /// compact wire format should base64-encode on the frontend side.
    /// In streaming mode, fragments are concatenated in offset order.
    pub data: Vec<u8>,
    /// Whether this item completed successfully.
    pub ok: bool,
    /// Error code name when `ok == false` (e.g. `"AddressNotFound"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of a `storage get` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageGetResult {
    /// Per-item results, one for each requested item.
    pub items: Vec<StorageGetItemResult>,
}

/// Intermediate per-item state accumulated during the callback.
#[derive(Default)]
struct ItemAccum {
    address: String,
    size: u64,
    data: Vec<u8>,
    ok: bool,
    error: Option<String>,
    seen_header: bool,
}

/// Collected state from the storage-get callback.
#[derive(Default)]
struct GetCollector {
    items: HashMap<u64, ItemAccum>,
    order: Vec<u64>,
    status: Option<i32>,
    call_error: Option<String>,
}

/// Read one or more content-addressed buffers from an open store.
///
/// Calls the upstream `lore::storage::get` in-process with a specialised
/// callback that copies binary payloads during the callback window (the
/// `LoreBytes` raw-pointer view is only valid for that duration).
pub async fn get(api: &LoreApi, args: StorageGetArgs) -> Result<StorageGetResult> {
    let lore_args = args.into_lore()?;

    let collector: Arc<Mutex<GetCollector>> = Arc::new(Mutex::new(GetCollector::default()));
    let (tx, rx) = oneshot::channel::<()>();
    let tx: Arc<Mutex<Option<oneshot::Sender<()>>>> = Arc::new(Mutex::new(Some(tx)));

    let cb_collector = collector.clone();
    let cb_tx = tx.clone();

    let callback: LoreEventCallback = Some(Box::new(move |event: &LoreEvent| {
        let mut c = cb_collector.lock().unwrap();
        match event {
            LoreEvent::StorageGetHeader(h) => {
                let accum = c.items.entry(h.id).or_default();
                accum.address = format!("{}", h.address);
                accum.size = h.size_content;
                accum.seen_header = true;
                if !c.order.contains(&h.id) {
                    c.order.push(h.id);
                }
            }
            LoreEvent::StorageGetData(d) => {
                let accum = c.items.entry(d.id).or_default();
                // SAFETY: we are inside the callback — the LoreBytes pointer
                // is valid for this invocation. Copy the bytes now.
                let bytes_slice = unsafe { d.bytes.as_slice() };
                let end = d.offset as usize + bytes_slice.len();
                if accum.data.len() < end {
                    accum.data.resize(end, 0);
                }
                accum.data[d.offset as usize..end].copy_from_slice(bytes_slice);
            }
            LoreEvent::StorageGetItemComplete(ic) => {
                // LoreErrorCode::None == 0 in the repr(C) enum; use the
                // Debug representation to classify without importing the type.
                let code_str = format!("{:?}", ic.error_code);
                let ok = code_str == "None";
                let error = if ok { None } else { Some(code_str) };
                let accum = c.items.entry(ic.id).or_default();
                accum.ok = ok;
                accum.error = error;
                // A failed item must not surface a partially-reassembled buffer:
                // any GET_DATA fragments accumulated before the failure are
                // incomplete/meaningless, so drop them. Callers see empty `data`
                // alongside `ok == false`.
                if !ok {
                    accum.data.clear();
                }
                if !accum.seen_header {
                    accum.address = format!("{}", ic.address);
                }
                if !c.order.contains(&ic.id) {
                    c.order.push(ic.id);
                }
            }
            LoreEvent::Error(e) => {
                c.call_error = Some(e.error_inner.as_str().to_string());
            }
            LoreEvent::Complete(d) => {
                c.status = Some(d.status);
                // lore v0.8.5+: terminal failure detail rides `Complete` (no
                // `Error` event) — recover the engine's message.
                if d.status != 0 && c.call_error.is_none() {
                    let message = d.error.message.as_str();
                    if !message.is_empty() {
                        c.call_error = Some(message.to_string());
                    }
                }
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

    let status = lore::storage::get::get(api.globals().build(), lore_args, callback).await;

    // Wait for the terminal event to fire.
    let _ = rx.await;

    let c = collector.lock().unwrap();

    if c.status != Some(0) || c.call_error.is_some() {
        return Err(LoreError::CommandFailed(
            c.call_error
                .clone()
                .unwrap_or_else(|| format!("storage get failed with status {status}")),
        ));
    }

    let items = c
        .order
        .iter()
        .map(|id| {
            let accum = c.items.get(id).expect("order contains only inserted ids");
            StorageGetItemResult {
                id: *id,
                address: accum.address.clone(),
                size: accum.size,
                data: accum.data.clone(),
                ok: accum.ok,
                error: accum.error.clone(),
            }
        })
        .collect();

    Ok(StorageGetResult { items })
}

/// Backwards-compatible command symbol for existing Tauri/frontend wiring.
///
/// The domain-relative API is [`get`]. The GUI command surface still invokes
/// `storage_get`, so keep this delegating wrapper until generated command
/// references are renamed in one coordinated pass.
pub async fn storage_get(api: &LoreApi, args: StorageGetArgs) -> Result<StorageGetResult> {
    get(api, args).await
}
