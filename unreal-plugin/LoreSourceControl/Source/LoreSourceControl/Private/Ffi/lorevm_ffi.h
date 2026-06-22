/*
 * lorevm_ffi.h — C ABI for the lorevm-ffi cdylib (SBAI-4081 / SBAI-4086).
 *
 * This header is the hand-written C mirror of the `extern "C"` surface exported
 * by `crates/lorevm-ffi/src/lib.rs` in the LoreGUI repo. It is loaded at runtime
 * by `LorevmFfi.cpp` via dlopen/LoadLibrary — we do NOT link an import lib at
 * build time, so the editor still starts if the shared library is missing (the
 * provider just reports "unavailable").
 *
 * Keep this in lockstep with the Rust source. The canonical contract lives in
 * the module-level doc comment of `crates/lorevm-ffi/src/lib.rs`; this header
 * reproduces it. The exported symbols (verified with `nm -D liblorevm_ffi.so`)
 * are exactly:
 *
 *     lorevm_ffi_open
 *     lorevm_ffi_call
 *     lorevm_ffi_close
 *     lorevm_ffi_string_free
 *     lorevm_ffi_abi_version
 *
 * ABI version: the cdylib reports "lorevm-ffi/1". `LorevmFfi` asserts the major
 * component matches LOREVM_FFI_ABI_MAJOR_EXPECTED below before using a handle.
 */

#ifndef LOREVM_FFI_H
#define LOREVM_FFI_H

#ifdef __cplusplus
extern "C" {
#endif

/*
 * The ABI major version this header was written against. The cdylib's
 * lorevm_ffi_abi_version() returns "lorevm-ffi/<major>"; the loader compares the
 * <major> token against this and refuses to drive a handle on a mismatch.
 */
#define LOREVM_FFI_ABI_MAJOR_EXPECTED 1

/*
 * Opaque warm handle: one tokio runtime + one LoreApi behind the C boundary.
 * Created by lorevm_ffi_open, used by lorevm_ffi_call, freed by lorevm_ffi_close.
 */
typedef struct LorevmHandle LorevmHandle;

/*
 * Function-pointer typedefs for runtime (dlopen) binding. The loader resolves
 * each symbol into one of these.
 */

/*
 * open: build a warm runtime + LoreApi for a repo working dir.
 *   request: JSON { "dir": "<path>", "in_memory": bool, "offline": bool,
 *                   "identity": "<id>"|null }, UTF-8, NUL-terminated.
 *   returns: opaque handle, or NULL on a NUL / invalid-UTF-8 request or a
 *            runtime-build failure. Free exactly once with lorevm_ffi_close().
 */
typedef LorevmHandle* (*lorevm_ffi_open_fn)(const char* request);

/*
 * call: run one op on a warm handle. THE HOT PATH.
 *   op_id : "<domain>.<op>" (e.g. "repository.status"), UTF-8, NUL-terminated.
 *   args  : JSON object deserialised into the op's Args, UTF-8, NUL-terminated.
 *   returns: malloc'd, NUL-terminated UTF-8 JSON the caller OWNS and MUST
 *            release with lorevm_ffi_string_free(). Success -> the op's typed
 *            result; failure (op error, bad args, unknown op, or a caught panic)
 *            -> {"error":{"kind","message"}}. NULL only on a NUL / invalid-UTF-8
 *            handle/op_id/args pointer.
 *
 * Blocks the calling thread for the op's duration. MUST be called from a
 * background thread, never the UE game thread.
 */
typedef char* (*lorevm_ffi_call_fn)(const LorevmHandle* handle,
                                    const char* op_id,
                                    const char* args);

/*
 * close: tear down a handle from lorevm_ffi_open(). NULL-safe; never double-close.
 */
typedef void (*lorevm_ffi_close_fn)(LorevmHandle* handle);

/*
 * string_free: release a string returned by lorevm_ffi_call(). NUL-safe. Must be
 * used instead of C free() — the buffer was allocated by Rust's CString.
 */
typedef void (*lorevm_ffi_string_free_fn)(char* s);

/*
 * abi_version: static "lorevm-ffi/<major>" string. Do NOT free.
 */
typedef const char* (*lorevm_ffi_abi_version_fn)(void);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* LOREVM_FFI_H */
