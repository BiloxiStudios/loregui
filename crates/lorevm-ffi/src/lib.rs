//! `lorevm-ffi` — the production C-ABI bridge over [`lore_vm`] (SBAI-4081).
//!
//! A stable C ABI over our lore binding, so an Unreal Engine C++ plugin (or any
//! C/C++ host) can drive lore **in-process** over FFI instead of shelling out to
//! the `lorevm --json` subprocess that `lore-mcp` uses. It is one of the two
//! deliberate **external-driver** consumers of the canonical
//! [`lore_vm::dispatch`] seam (the other is the `lorevm` CLI). The GUI/Tauri app
//! binds the same `lore-vm` ops **in-process** as typed commands and never goes
//! through this bridge — see `CLAUDE.md` ("One binding, two consumption modes").
//!
//! This crate no longer re-types any ops: every call routes through the single
//! shared `lore_vm::dispatch`, so the FFI surface cannot drift from the CLI.
//!
//! # Design: a warm, long-lived handle
//!
//! The hot path (a UE Content Browser refreshing per-asset overlay state) calls
//! often, so we do **not** spin a tokio runtime per call. Instead a host:
//!
//! 1. `lorevm_ffi_open(request_json)` once — builds **one** multi-thread tokio
//!    runtime and **one** warm [`LoreApi`] for a working dir, returning an opaque
//!    handle.
//! 2. `lorevm_ffi_call(handle, op_id, args_json)` many times — reuses the warm
//!    runtime + api; this is the cheap hot path.
//! 3. `lorevm_ffi_close(handle)` once — tears down the runtime.
//!
//! # ABI contract
//!
//! ```c
//! typedef struct LorevmHandle LorevmHandle;
//!
//! // open: build a warm runtime + LoreApi for a repo.
//! //   request: JSON { "dir": "<path>", "in_memory": bool, "offline": bool,
//! //                   "identity": "<id>"|null }, UTF-8, NUL-terminated.
//! //   returns: opaque handle, or NULL on a NUL/invalid-UTF-8 request or a
//! //            runtime-build failure. Free with lorevm_ffi_close().
//! LorevmHandle* lorevm_ffi_open(const char* request);
//!
//! // call: run one op on a warm handle. THE HOT PATH.
//! //   op_id : "<domain>.<op>"  (e.g. "repository.status"), UTF-8, NUL-terminated.
//! //   args  : JSON object deserialised into the op's Args, UTF-8, NUL-terminated.
//! //   returns: malloc'd, NUL-terminated UTF-8 JSON the caller OWNS and MUST
//! //            release with lorevm_ffi_string_free(). Success → the op's typed
//! //            result; failure (op error, bad args, unknown op, or a caught
//! //            panic) → {"error":{"kind","message"}}. NULL only on a
//! //            NUL/invalid-UTF-8 handle/op_id/args pointer.
//! char* lorevm_ffi_call(const LorevmHandle* handle, const char* op_id, const char* args);
//!
//! // close: tear down a handle from lorevm_ffi_open(). NULL-safe; never double-close.
//! void lorevm_ffi_close(LorevmHandle* handle);
//!
//! void        lorevm_ffi_string_free(char* s);
//! const char* lorevm_ffi_abi_version(void);  // static, do NOT free
//! ```
//!
//! # Safety guarantees
//!
//! - **No panic unwinds across the ABI.** Every `extern "C"` entry that runs Rust
//!   logic wraps it in [`std::panic::catch_unwind`]; a panic in lore or its QUIC
//!   stack becomes an `{"error":{"kind":"panic",…}}` JSON response (or NULL where
//!   no response buffer can be returned), never UB.
//! - **String ownership.** Strings cross the ABI malloc'd by Rust via
//!   `CString::into_raw` and MUST be freed by [`lorevm_ffi_string_free`] (which
//!   calls `CString::from_raw`), never by C `free`.
//! - **Threading.** `lorevm_ffi_call` blocks the calling thread for the op's
//!   duration (it `block_on`s the op on the handle's runtime). A UE host MUST call
//!   it from a background thread, never the game thread. The handle is `Send` +
//!   `Sync`, so it is safe to *share* a handle pointer across threads and to make
//!   concurrent calls on it.
//!
//!   However, each call is **not** a single atomic operation against the store: it
//!   runs `lore_vm::dispatch(...).await` and then `lore_vm::finalize(...).await`,
//!   and `finalize` is a *global, store-wide* flush (it drains every outstanding
//!   guarded task for the repo, not just this op's). If two mutating calls on the
//!   same handle (or on two handles rooted at the same working dir) ran
//!   concurrently, their `dispatch`/`finalize` phases could interleave — one
//!   call's flush could drain the other's half-written state, corrupting the
//!   store. Concurrent *mutating* calls against one repo are therefore **NOT**
//!   sound and MUST be serialised.
//!
//!   This crate enforces that internally: every `lorevm_ffi_call` takes a
//!   per-handle async mutex around the whole `dispatch` + `finalize` critical
//!   section, so calls on one handle are serialised end-to-end and the
//!   non-atomic dispatch→finalize sequence can never interleave. (Two *separate*
//!   handles on the *same* dir still share the on-disk store and are not mutually
//!   serialised — drive one repo through one handle.) Read-only calls are
//!   serialised too; that is a deliberate simplicity/safety trade since
//!   `finalize` is store-wide and cannot cheaply be classified as a no-op here.

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};

use lore_vm::global::LoreGlobal;
use lore_vm::LoreApi;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::runtime::Runtime;

/// Bumped if the C ABI shape changes. Lets the UE plugin assert compatibility.
/// `1` introduces the warm-handle API (open/call/close) over the spike's `0`.
const ABI_VERSION: &str = "lorevm-ffi/1\0";

/// A warm, long-lived bridge handle: one tokio runtime + one [`LoreApi`].
///
/// Opaque to C. Created by [`lorevm_ffi_open`], used by [`lorevm_ffi_call`],
/// destroyed by [`lorevm_ffi_close`].
pub struct LorevmHandle {
    runtime: Runtime,
    api: LoreApi,
    /// Serialises the `dispatch` + `finalize` critical section across concurrent
    /// `lorevm_ffi_call`s on this handle. `finalize` is a store-wide flush, so
    /// two calls' phases must never interleave — see the module "Threading"
    /// docs. An async mutex (not `std::sync::Mutex`) so the guard can be held
    /// across the `.await`s.
    call_lock: tokio::sync::Mutex<()>,
}

/// Deserialised `lorevm_ffi_open` request: how to build the warm [`LoreApi`].
#[derive(Debug, Deserialize)]
struct OpenRequest {
    #[serde(default = "default_dir")]
    dir: String,
    #[serde(default)]
    in_memory: bool,
    #[serde(default)]
    offline: bool,
    #[serde(default)]
    identity: Option<String>,
}

fn default_dir() -> String {
    ".".to_string()
}

/// Return the ABI version string (static, NUL-terminated, do NOT free).
///
/// # Safety
/// The returned pointer is valid for the lifetime of the loaded library.
#[no_mangle]
pub extern "C" fn lorevm_ffi_abi_version() -> *const c_char {
    ABI_VERSION.as_ptr() as *const c_char
}

/// Free a string previously returned by [`lorevm_ffi_call`].
///
/// # Safety
/// `s` must be a pointer returned by [`lorevm_ffi_call`] (or NULL). Passing any
/// other pointer, or freeing twice, is undefined behaviour.
#[no_mangle]
pub unsafe extern "C" fn lorevm_ffi_string_free(s: *mut c_char) {
    if !s.is_null() {
        // Reclaim the CString we leaked in `respond` and drop it.
        drop(CString::from_raw(s));
    }
}

/// Open a warm bridge handle: build one tokio runtime + one [`LoreApi`].
///
/// Call once per repo working dir, then drive many ops through
/// [`lorevm_ffi_call`] on the returned handle, and finally [`lorevm_ffi_close`].
///
/// # Safety
/// `request` must be a valid, NUL-terminated, UTF-8 C string (or this returns
/// NULL). The returned handle must be freed exactly once with
/// [`lorevm_ffi_close`].
#[no_mangle]
pub unsafe extern "C" fn lorevm_ffi_open(request: *const c_char) -> *mut LorevmHandle {
    if request.is_null() {
        return std::ptr::null_mut();
    }
    let request_str = match CStr::from_ptr(request).to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return std::ptr::null_mut(),
    };

    // A panic while building the runtime/api must not cross the ABI.
    let built = catch_unwind(AssertUnwindSafe(|| build_handle(&request_str)));
    match built {
        Ok(Some(handle)) => Box::into_raw(Box::new(handle)),
        // None: bad request JSON or runtime build failure. Ok(None) and a caught
        // panic both surface as NULL — open has no response buffer to return an
        // error through; the contract documents NULL as "could not open".
        Ok(None) | Err(_) => std::ptr::null_mut(),
    }
}

/// Build a warm handle from an open-request JSON string. `None` on bad JSON or a
/// runtime-build failure.
fn build_handle(request_str: &str) -> Option<LorevmHandle> {
    let req: OpenRequest = serde_json::from_str(request_str).ok()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .ok()?;
    let mut global = LoreGlobal::new(std::path::PathBuf::from(&req.dir))
        .in_memory(req.in_memory)
        .offline(req.offline);
    if let Some(id) = &req.identity {
        global = global.identity(id.clone());
    }
    let api = LoreApi::from_global(global);
    Some(LorevmHandle {
        runtime,
        api,
        call_lock: tokio::sync::Mutex::new(()),
    })
}

/// Invoke `<domain>.<op>` with JSON args on a warm `handle`; return a malloc'd
/// JSON response. **The hot path** — reuses the handle's runtime + warm api.
///
/// See the module-level docs for the full ABI contract.
///
/// # Safety
/// `handle` must be a live pointer from [`lorevm_ffi_open`] (not yet closed).
/// `op_id` and `args` must be valid, NUL-terminated, UTF-8 C strings (or the
/// call returns NULL). The returned pointer must be released exactly once with
/// [`lorevm_ffi_string_free`].
#[no_mangle]
pub unsafe extern "C" fn lorevm_ffi_call(
    handle: *const LorevmHandle,
    op_id: *const c_char,
    args: *const c_char,
) -> *mut c_char {
    // ---- decode the inputs -------------------------------------------------
    if handle.is_null() || op_id.is_null() || args.is_null() {
        return std::ptr::null_mut();
    }
    let op_id = match CStr::from_ptr(op_id).to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return std::ptr::null_mut(),
    };
    let args_str = match CStr::from_ptr(args).to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return std::ptr::null_mut(),
    };
    let handle: &LorevmHandle = &*handle;

    // ---- run the op, guarded against panics --------------------------------
    // From here every failure (bad args JSON, op error, unknown op, or a panic)
    // is reported as a JSON value, never NULL. catch_unwind stops a Rust panic
    // (e.g. from the pre-1.0 lore/QUIC stack) from unwinding across the C ABI.
    let response = catch_unwind(AssertUnwindSafe(|| run_call(handle, &op_id, &args_str)))
        .unwrap_or_else(|payload| error_json("panic", &panic_message(payload.as_ref())));

    respond(&response)
}

/// Parse `args`, route through the canonical [`lore_vm::dispatch`] on the
/// handle's warm runtime, and return the op's result (or a structured error) as
/// a JSON `Value`.
fn run_call(handle: &LorevmHandle, op_id: &str, args_str: &str) -> Value {
    // Test-only hook: a magic op id that panics, so the smoke test can prove the
    // `catch_unwind` guard in `lorevm_ffi_call` turns a panic into a JSON error
    // instead of unwinding across the C ABI. Compiled out of normal builds.
    #[cfg(feature = "panic-test-hook")]
    if op_id == "__ffi_panic_test__" {
        panic!("induced panic for FFI catch_unwind test");
    }

    let args: Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return error_json("ffi", &format!("invalid args JSON: {e}")),
    };

    // Block on the handle's long-lived runtime — no per-call runtime spin-up.
    // SBAI-4080: the lore engine defers its end-of-command store flush to a
    // spawned background task. A UE host may make subsequent reads through a
    // *different* handle (or a separate process / the CLI), so we drain the
    // engine's outstanding tasks synchronously after each call to guarantee the
    // mutable-store write (e.g. the staged-revision anchor) is durable before we
    // return — the same durability contract the `lorevm` CLI enforces per
    // process. See `lore_vm::finalize`.
    //
    // `dispatch` then `finalize` is NOT atomic, and `finalize` is a store-wide
    // flush, so two concurrent calls on this handle must not interleave their
    // phases. Hold the per-handle async mutex across both awaits to serialise the
    // whole critical section. Acquired *inside* `block_on` so the future enters
    // the runtime context; the guard is an async mutex and so may be held across
    // the `.await`s.
    let result = handle.runtime.block_on(async {
        let _guard = handle.call_lock.lock().await;
        let r = lore_vm::dispatch(&handle.api, op_id, args).await;
        lore_vm::finalize(&handle.api).await;
        r
    });

    match result {
        Ok(value) => value,
        Err(e) => {
            // Structured LoreError → {"error":{kind,message}}.
            let v = serde_json::to_value(&e)
                .unwrap_or_else(|_| json!({ "kind": "client", "message": e.to_string() }));
            json!({ "error": v })
        }
    }
}

/// Close a handle from [`lorevm_ffi_open`], tearing down its runtime.
///
/// NULL-safe. Must be called exactly once per successful `open`; never call it
/// twice on the same handle.
///
/// # Safety
/// `handle` must be a pointer returned by [`lorevm_ffi_open`] (or NULL) that has
/// not already been closed.
#[no_mangle]
pub unsafe extern "C" fn lorevm_ffi_close(handle: *mut LorevmHandle) {
    if handle.is_null() {
        return;
    }
    // Reclaim the Box and drop it (and its runtime). Guard the drop: a panic in
    // runtime teardown must not cross the ABI.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        drop(Box::from_raw(handle));
    }));
}

/// Best-effort extraction of a panic payload's message.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic in lore-vm op (payload not a string)".to_string()
    }
}

/// Build a `{"error":{"kind","message"}}` JSON value.
fn error_json(kind: &str, message: &str) -> Value {
    json!({ "error": { "kind": kind, "message": message } })
}

/// Serialise a JSON value to a malloc'd, NUL-terminated C string and hand
/// ownership to the caller. Returns NULL only if the value somehow contains an
/// interior NUL (it cannot for JSON text, but we stay defensive).
fn respond(value: &Value) -> *mut c_char {
    let text = serde_json::to_string(value).unwrap_or_else(|_| {
        "{\"error\":{\"kind\":\"ffi\",\"message\":\"unserialisable response\"}}".to_string()
    });
    match CString::new(text) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}
