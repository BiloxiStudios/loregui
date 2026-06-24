//! `storage put` operation — binds [`lore::storage::put::put`].
//!
//! Writes one or more content-addressed buffers to an open storage handle.
//! Each item is hashed and stored independently; per-item results are collected
//! from `StoragePutItemComplete` events emitted by the upstream crate.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreArray, LoreEvent};
use lore::storage::handle::LoreStore;
use lore::storage::put::{LoreStoragePutArgs, LoreStoragePutItem};
use serde::{Deserialize, Serialize};

/// One item to store — the safe, serialisable counterpart of
/// [`LoreStoragePutItem`].
///
/// `data` is a plain byte vector rather than a raw pointer, so it serialises
/// cleanly across the Tauri IPC boundary. Partition and context are hex strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutItem {
    /// Caller-chosen correlation id echoed back in the result.
    pub id: u64,
    /// Target partition as a 32-char hex string.
    pub partition: String,
    /// Dedup context as a 32-char hex string; empty string → zero context.
    #[serde(default)]
    pub context: String,
    /// The bytes to store.
    pub data: Vec<u8>,
    /// Opt into remote upload (default false).
    #[serde(default)]
    pub remote_write: bool,
    /// Tag the fragment for local cache priority (default false).
    #[serde(default)]
    pub local_cache: bool,
    /// Leaf fragment size cap for large buffers; 0 lets the engine choose.
    #[serde(default)]
    pub fixed_size_chunk: u64,
}

/// Arguments for [`put`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePutArgs {
    /// Handle id of an already-open store (from `storage open`).
    pub handle: u64,
    /// Buffers to store.
    pub items: Vec<PutItem>,
}

/// Per-item result from the put operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutItemResult {
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

/// Result of the overall put operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePutResult {
    /// Per-item outcomes (one entry per input item).
    pub items: Vec<PutItemResult>,
}

/// Build a [`LoreStoragePutItem`] from our safe [`PutItem`] plus a pointer
/// into the borrowed data buffer.
///
/// The upstream struct uses FFI types (`Partition`, `Context`, `LoreBytes`)
/// that are not re-exported by the `lore` crate, so we cannot name them here.
/// We construct the item with a struct literal: `Partition`/`Context` are
/// produced by `serde_json::from_value` (their `Deserialize` accepts a hex
/// string) with the target type inferred from the field, and `data` — a
/// `#[repr(C)]` `{ ptr, len }` POD whose `Deserialize` impl rejects because it
/// is a *borrowed view* — is zero-initialised (a valid null/empty view) and
/// then patched to point at the real buffer.
fn build_lore_item(
    item: &PutItem,
    data_ptr: *const u8,
    data_len: usize,
) -> std::result::Result<LoreStoragePutItem, LoreError> {
    let context_hex = if item.context.is_empty() {
        "00000000000000000000000000000000".to_string()
    } else {
        item.context.clone()
    };

    let mut lore_item = LoreStoragePutItem {
        id: item.id,
        partition: serde_json::from_value(serde_json::Value::String(item.partition.clone()))
            .map_err(|e| LoreError::Parse(format!("failed to build put item (partition): {e}")))?,
        context: serde_json::from_value(serde_json::Value::String(context_hex))
            .map_err(|e| LoreError::Parse(format!("failed to build put item (context): {e}")))?,
        // SAFETY: `LoreBytes` is a `#[repr(C)]` `{ *const c_void, usize }` POD;
        // an all-zero value is a valid null/empty view. We overwrite ptr/len
        // immediately below with the caller's live buffer (kept alive by the
        // owner for the duration of the put call).
        data: unsafe { std::mem::zeroed() },
        remote_write: u8::from(item.remote_write),
        local_cache: u8::from(item.local_cache),
        fixed_size_chunk: item.fixed_size_chunk,
    };

    lore_item.data.ptr = data_ptr.cast();
    lore_item.data.len = data_len;

    Ok(lore_item)
}

/// Write one or more content-addressed buffers to an open store.
///
/// Calls upstream [`lore::storage::put::put`] in-process and collects the
/// `StoragePutItemComplete` events to build a typed result.
pub async fn put(api: &LoreApi, args: StoragePutArgs) -> Result<StoragePutResult> {
    // The bytes the `LoreBytes` raw pointers refer to are owned by `args.items`
    // (each `PutItem::data` is an owned `Vec<u8>`). These borrowed slices own
    // nothing — they merely view into `args`, so `args` MUST stay alive and
    // unmoved until the put completes (`Complete` fires). It does: `args` is a
    // by-value parameter that lives for this whole function, past the `.await`s
    // below, so the pointers remain valid for the duration of the call.
    let item_slices: Vec<&[u8]> = args.items.iter().map(|i| i.data.as_slice()).collect();

    let mut lore_items: Vec<LoreStoragePutItem> = Vec::with_capacity(args.items.len());

    for (item, buf) in args.items.iter().zip(item_slices.iter()) {
        lore_items.push(build_lore_item(item, buf.as_ptr(), buf.len())?);
    }

    let lore_args = LoreStoragePutArgs {
        handle: LoreStore {
            handle_id: args.handle,
        },
        items: LoreArray::from_vec(lore_items),
    };

    let (callback, rx) = collect_events();

    let status = lore::storage::put::put(api.globals().build(), lore_args, callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("storage put failed with status {status}"),
        )));
    }

    let mut results: Vec<PutItemResult> = Vec::new();
    for event in &stream.events {
        if let LoreEvent::StoragePutItemComplete(data) = event {
            // LoreErrorCode::None == 0 means success.
            let error_code_val = data.error_code as i32;
            let ok = error_code_val == 0;
            results.push(PutItemResult {
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

    Ok(StoragePutResult { items: results })
}
