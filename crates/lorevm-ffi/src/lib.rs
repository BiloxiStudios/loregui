//! `lorevm-ffi` — SPIKE (SBAI-4079).
//!
//! A minimal C-ABI surface over [`lore_vm`], proving that an Unreal Engine C++
//! plugin (or any C/C++ host) can drive our lore binding **in-process** over a
//! stable C ABI, instead of shelling out to the `lorevm --json` subprocess that
//! `lore-mcp` uses today.
//!
//! This is a **feasibility proof, not a shipped artifact.** It exposes exactly
//! one call entry point plus a matching free function, and wraps a *subset* of
//! the same `lore_vm::ops` dispatch that `lorevm-cli/src/main.rs` exposes. The
//! point is to validate the ABI shape, the JSON-in/JSON-out contract, the
//! tokio-runtime-per-call lifetime, and the malloc/free ownership rules — NOT
//! to be op-complete. A production bridge would factor the full dispatch match
//! out of `lorevm-cli` into a shared `lore-vm` library fn and call it from both
//! the CLI and here (see docs/ue-lorevm-bridge-spike.md).
//!
//! # ABI contract
//!
//! ```c
//! // op_id  : "<domain>.<op>"  (e.g. "repository.status"), UTF-8, NUL-terminated
//! // request: JSON object, UTF-8, NUL-terminated. Shape:
//! //          { "dir": "<path>", "args": {..}, "in_memory": bool,
//! //            "offline": bool, "identity": "<id>"|null }
//! // returns: malloc'd, NUL-terminated UTF-8 JSON string the caller OWNS and
//! //          MUST release with lorevm_ffi_string_free(). On success it is the
//! //          op's typed result; on failure it is {"error":{"kind","message"}}.
//! //          Returns NULL only on a NUL/invalid-UTF-8 op_id or request pointer.
//! char* lorevm_ffi_call(const char* op_id, const char* request);
//! void  lorevm_ffi_string_free(char* s);
//! const char* lorevm_ffi_abi_version(void);  // static, do NOT free
//! ```
//!
//! # Threading
//!
//! `lorevm_ffi_call` is self-contained: it builds a fresh multi-thread tokio
//! runtime, runs the op to completion, and tears the runtime down before
//! returning. It blocks the calling thread for the op's duration, so a UE host
//! MUST call it from a background thread (e.g. an `AsyncTask`), never the game
//! thread, for anything that can block on I/O or the network. A production
//! bridge would hold one long-lived runtime + `LoreApi` behind an opaque handle
//! rather than spinning a runtime per call; this spike keeps it stateless to
//! stay minimal.

use std::ffi::{c_char, CStr, CString};

use lore_vm::api::LoreApi;
use lore_vm::global::LoreGlobal;
use lore_vm::ops;
use serde::Deserialize;
use serde_json::{json, Value};

/// Bumped if the C ABI shape changes. Lets the UE plugin assert compatibility.
const ABI_VERSION: &str = "lorevm-ffi/0\0";

/// Deserialised `request` envelope.
#[derive(Debug, Deserialize)]
struct Request {
    #[serde(default = "default_dir")]
    dir: String,
    #[serde(default = "empty_obj")]
    args: Value,
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
fn empty_obj() -> Value {
    json!({})
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

/// Invoke `<domain>.<op>` with a JSON request; return a malloc'd JSON response.
///
/// See the module-level docs for the full ABI contract.
///
/// # Safety
/// `op_id` and `request` must be valid, NUL-terminated, UTF-8 C strings (or the
/// call returns NULL). The returned pointer must be released exactly once with
/// [`lorevm_ffi_string_free`].
#[no_mangle]
pub unsafe extern "C" fn lorevm_ffi_call(
    op_id: *const c_char,
    request: *const c_char,
) -> *mut c_char {
    // ---- decode the two C strings ------------------------------------------
    if op_id.is_null() || request.is_null() {
        return std::ptr::null_mut();
    }
    let op_id = match CStr::from_ptr(op_id).to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return std::ptr::null_mut(),
    };
    let request_str = match CStr::from_ptr(request).to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return std::ptr::null_mut(),
    };

    // From here on, every failure is reported as JSON, never NULL.
    let req: Request = match serde_json::from_str(&request_str) {
        Ok(r) => r,
        Err(e) => return respond(&error_json("ffi", &format!("invalid request JSON: {e}"))),
    };

    // ---- build a fresh runtime + API and run the op ------------------------
    // Stateless per-call: minimal for a spike. A real bridge keeps these alive.
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return respond(&error_json("ffi", &format!("failed to start runtime: {e}"))),
    };

    let response = runtime.block_on(async move {
        let mut global = LoreGlobal::new(std::path::PathBuf::from(&req.dir))
            .in_memory(req.in_memory)
            .offline(req.offline);
        if let Some(id) = &req.identity {
            global = global.identity(id.clone());
        }
        let api = LoreApi::from_global(global);
        dispatch(&op_id, &api, req.args).await
    });

    respond(&response)
}

/// Dispatch a `<domain>.<op>` id to the matching `lore_vm::ops` fn and return
/// its result (or a structured error) as a JSON `Value`.
///
/// SPIKE SCOPE: this mirrors `lorevm-cli`'s dispatch but only wires a
/// representative subset of ops — enough to prove a read op, a metrics op, and
/// a mutating op all cross the ABI. The full op set is intentionally omitted;
/// the production design factors the CLI's complete match into a shared fn.
async fn dispatch(op_id: &str, api: &LoreApi, args: Value) -> Value {
    macro_rules! op {
        ($path:path, $args:ty) => {{
            let parsed: $args = match serde_json::from_value(args.clone()) {
                Ok(p) => p,
                Err(e) => {
                    return error_json(
                        "ffi",
                        &format!("could not parse args for `{op_id}`: {e}"),
                    )
                }
            };
            match $path(api, parsed).await {
                Ok(result) => {
                    serde_json::to_value(&result).unwrap_or_else(|e| {
                        error_json("ffi", &format!("failed to serialise result: {e}"))
                    })
                }
                Err(e) => {
                    let v = serde_json::to_value(&e).unwrap_or_else(|_| {
                        json!({ "kind": "client", "message": e.to_string() })
                    });
                    json!({ "error": v })
                }
            }
        }};
    }

    match op_id {
        "repository.status" => op!(
            ops::repository::status::status,
            ops::repository::status::RepositoryStatusArgs
        ),
        "repository.info" => op!(
            ops::repository::info::info,
            ops::repository::info::RepositoryInfoArgs
        ),
        "repository.create" => op!(
            ops::repository::create::create,
            ops::repository::create::CreateArgs
        ),
        "branch.list" => op!(ops::branch::list::list, ops::branch::list::BranchListArgs),
        unknown => error_json(
            "ffi",
            &format!(
                "unknown or unwired op `{unknown}` (spike wires a subset; \
                 production bridge shares lorevm-cli's full dispatch)"
            ),
        ),
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
