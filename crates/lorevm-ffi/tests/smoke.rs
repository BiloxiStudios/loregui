//! Smoke test for the production C-ABI bridge (SBAI-4081).
//!
//! Calls the `extern "C"` entry points exactly as a C/C++ host (the UE plugin)
//! would — `CString` in, free the returned C string — to prove lore-vm can be
//! driven over the C ABI through the warm-handle lifecycle. We drive a real
//! in-memory lore engine (`in_memory + offline`, the same headless mode the
//! integration harness uses), so the roundtrip exercises the real ops layer via
//! `lore_vm::dispatch`, not a stub.

use std::ffi::{c_char, CStr, CString};

use lorevm_ffi::{
    lorevm_ffi_abi_version, lorevm_ffi_call, lorevm_ffi_close, lorevm_ffi_open,
    lorevm_ffi_string_free, LorevmHandle,
};
use serde_json::Value;

/// Open a warm handle for a repo working dir. Caller closes it.
fn open(request: &Value) -> *mut LorevmHandle {
    let req_c = CString::new(request.to_string()).unwrap();
    // SAFETY: a valid NUL-terminated UTF-8 request string.
    let h = unsafe { lorevm_ffi_open(req_c.as_ptr()) };
    assert!(!h.is_null(), "lorevm_ffi_open returned NULL for {request}");
    h
}

/// Call one op on a warm handle, returning the parsed JSON response.
fn call(handle: *const LorevmHandle, op_id: &str, args: &Value) -> Value {
    let op_c = CString::new(op_id).unwrap();
    let args_c = CString::new(args.to_string()).unwrap();
    // SAFETY: handle is live; both string pointers are valid NUL-terminated
    // UTF-8 for the call's duration; we free the result exactly once below.
    unsafe {
        let out: *mut c_char = lorevm_ffi_call(handle, op_c.as_ptr(), args_c.as_ptr());
        assert!(!out.is_null(), "ffi call returned NULL for `{op_id}`");
        let text = CStr::from_ptr(out).to_str().unwrap().to_owned();
        lorevm_ffi_string_free(out);
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("non-JSON response `{text}`: {e}"))
    }
}

#[test]
fn abi_version_is_exposed() {
    // SAFETY: returns a static NUL-terminated string we must NOT free.
    let v = unsafe { CStr::from_ptr(lorevm_ffi_abi_version()) }
        .to_str()
        .unwrap();
    assert!(v.starts_with("lorevm-ffi/"), "unexpected ABI version: {v}");
}

#[test]
fn warm_handle_create_then_status_then_status_roundtrips() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_string_lossy().to_string();
    // In-memory mode keeps stores in a process-wide cache that persists across
    // sequential calls on the SAME warm handle, so create-then-status round-trips
    // with no server and no on-disk store — mirroring the integration harness.
    let repo_url = format!("lore://localhost/ffi-{}", std::process::id());

    // 1. Open ONE warm handle (one runtime + one LoreApi).
    let handle = open(&serde_json::json!({
        "dir": dir,
        "in_memory": true,
        "offline": true,
        "identity": "ffi-smoke",
    }));

    // 2. Mutating op across the ABI on the warm handle.
    let created = call(
        handle,
        "repository.create",
        &serde_json::json!({ "repository_url": repo_url }),
    );
    assert!(
        created.get("error").is_none(),
        "repository.create errored over FFI: {created}"
    );

    // 3. Two read ops REUSING the same warm handle — the hot path. Both must hit
    //    the same warm in-memory engine the create populated.
    let status1 = call(handle, "repository.status", &serde_json::json!({}));
    assert!(
        status1.get("error").is_none() && status1.is_object(),
        "first repository.status errored/!object over FFI: {status1}"
    );
    let status2 = call(handle, "repository.status", &serde_json::json!({}));
    assert!(
        status2.get("error").is_none() && status2.is_object(),
        "second repository.status errored/!object over FFI: {status2}"
    );

    // 4. Close the warm handle.
    // SAFETY: handle came from lorevm_ffi_open and is closed exactly once.
    unsafe { lorevm_ffi_close(handle) };
}

#[test]
fn unknown_op_returns_structured_error_not_null() {
    let handle = open(&serde_json::json!({ "dir": ".", "offline": true }));
    let resp = call(handle, "nope.nope", &serde_json::json!({}));
    let kind = resp
        .pointer("/error/kind")
        .and_then(Value::as_str)
        .expect("expected {error:{kind}} shape");
    // Unknown ops are caught by lore_vm::dispatch and surface as a LoreError::Parse,
    // whose serde tag is the variant name "Parse".
    assert_eq!(kind, "Parse");
    // SAFETY: closed exactly once.
    unsafe { lorevm_ffi_close(handle) };
}

#[test]
fn a_panic_in_an_op_becomes_a_json_error_not_a_crash() {
    // The `panic-test-hook` feature (enabled for this dev build) compiles a magic
    // op id that panics inside `run_call`. The `catch_unwind` guard in
    // `lorevm_ffi_call` must turn that into an `{"error":{"kind":"panic"}}` JSON
    // response instead of letting the panic unwind across the C ABI (UB).
    let handle = open(&serde_json::json!({ "dir": ".", "offline": true }));
    let resp = call(handle, "__ffi_panic_test__", &serde_json::json!({}));
    let kind = resp
        .pointer("/error/kind")
        .and_then(Value::as_str)
        .expect("expected {error:{kind}} shape after a guarded panic");
    assert_eq!(
        kind, "panic",
        "panic was not converted to a JSON error: {resp}"
    );
    // SAFETY: closed exactly once.
    unsafe { lorevm_ffi_close(handle) };
}

#[test]
fn close_is_null_safe() {
    // SAFETY: NULL is explicitly allowed and is a no-op.
    unsafe { lorevm_ffi_close(std::ptr::null_mut()) };
}
