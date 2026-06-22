//! SPIKE smoke test (SBAI-4079).
//!
//! Calls the `extern "C"` entry points exactly as a C/C++ host (the UE plugin)
//! would — passing C strings in and freeing the returned C string — to prove
//! lore-vm can be driven over the C ABI. We drive a real in-memory lore engine
//! (`in_memory + offline`, the same headless mode the integration harness uses)
//! so the roundtrip exercises the actual ops layer, not a stub.

use std::ffi::{c_char, CStr, CString};

use lorevm_ffi::{lorevm_ffi_abi_version, lorevm_ffi_call, lorevm_ffi_string_free};
use serde_json::Value;

/// Call across the ABI the way C would, returning the parsed JSON response.
fn call(op_id: &str, request: &Value) -> Value {
    let op_c = CString::new(op_id).unwrap();
    let req_c = CString::new(request.to_string()).unwrap();
    // SAFETY: both pointers are valid NUL-terminated UTF-8 for the call's
    // duration; we free the result exactly once below.
    unsafe {
        let out: *mut c_char = lorevm_ffi_call(op_c.as_ptr(), req_c.as_ptr());
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
fn create_then_status_roundtrips_over_the_c_abi() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_string_lossy().to_string();
    // In-memory mode keeps stores in a process-wide cache that persists across
    // sequential library calls, so create-then-status round-trips with no server
    // and no on-disk store — mirroring lore-vm's integration_roundtrip harness.
    let repo_url = format!("lore://localhost/spike-{}", std::process::id());

    // 1. Mutating op across the ABI: create an in-memory repo.
    let created = call(
        "repository.create",
        &serde_json::json!({
            "dir": dir,
            "in_memory": true,
            "offline": true,
            "identity": "spike-ffi",
            "args": { "repository_url": repo_url }
        }),
    );
    assert!(
        created.get("error").is_none(),
        "repository.create errored over FFI: {created}"
    );

    // 2. Read op across the ABI against the same in-memory engine.
    let status = call(
        "repository.status",
        &serde_json::json!({
            "dir": dir,
            "in_memory": true,
            "offline": true,
            "identity": "spike-ffi",
            "args": {}
        }),
    );
    assert!(
        status.get("error").is_none(),
        "repository.status errored over FFI: {status}"
    );
    // A successful status result is a JSON object (the typed RepoStatus shape).
    assert!(status.is_object(), "status was not a JSON object: {status}");
}

#[test]
fn unknown_op_returns_structured_error_not_null() {
    let resp = call("nope.nope", &serde_json::json!({ "dir": ".", "args": {} }));
    let kind = resp
        .pointer("/error/kind")
        .and_then(Value::as_str)
        .expect("expected {error:{kind}} shape");
    assert_eq!(kind, "ffi");
}
