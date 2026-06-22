// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "Dom/JsonObject.h"

struct LorevmHandle;

/**
 * FLorevmFfi — the thin FFI binding layer between the UE source-control plugin
 * and our `lorevm-ffi` cdylib (built from `crates/lorevm-ffi`).
 *
 * Responsibilities (and nothing else — keep this layer thin):
 *  1. Locate + load the shared library (LoadLibrary / dlopen) from the plugin's
 *     Binaries directory or an explicit override path.
 *  2. Resolve the five `extern "C"` symbols into function pointers.
 *  3. Assert the ABI major version matches what this build expects.
 *  4. Open ONE warm handle for a repository working dir (one tokio runtime + one
 *     LoreApi, held for the editor session).
 *  5. Run ops: `Call("<domain>.<op>", argsJson)` -> parsed JSON result, or a
 *     structured error. This is the only place op-id strings + the C ABI live.
 *  6. Close the handle + unload the library on shutdown.
 *
 * Threading: Open/Call/Close must run on a background thread (the source-control
 * worker thread), never the game thread — `lorevm_ffi_call` BLOCKS for the op's
 * duration. The handle itself is Send+Sync on the Rust side, so concurrent calls
 * are sound, but this class serialises Open/Close with a critical section so a
 * status sweep and a shutdown can't race the handle pointer.
 *
 * This is the single seam to swap if we ever re-target Epic's first-party lore
 * provider: the Workers talk to FLorevmFfi, FLorevmFfi talks to the cdylib.
 */
class FLorevmResult
{
public:
	/** True when the op returned a result (no {"error":...} envelope, non-NULL). */
	bool bSuccess = false;

	/** The parsed top-level JSON object of a successful op result. */
	TSharedPtr<FJsonObject> Result;

	/** On failure: the LoreError kind ("CommandFailed", "Parse", "panic", "ffi", ...). */
	FString ErrorKind;

	/** On failure: the human-readable message. */
	FString ErrorMessage;

	static FLorevmResult MakeError(const FString& Kind, const FString& Message)
	{
		FLorevmResult R;
		R.bSuccess = false;
		R.ErrorKind = Kind;
		R.ErrorMessage = Message;
		return R;
	}
};

class FLorevmFfi
{
public:
	FLorevmFfi() = default;
	~FLorevmFfi();

	FLorevmFfi(const FLorevmFfi&) = delete;
	FLorevmFfi& operator=(const FLorevmFfi&) = delete;

	/**
	 * Load the cdylib and resolve its symbols. Tries, in order:
	 *   1. InExplicitLibPath if non-empty.
	 *   2. The LOREVM_FFI_LIB environment variable.
	 *   3. The plugin's Binaries/<Platform>/ directory (platform-specific name).
	 * Returns false (and sets OutError) if the library can't be found/loaded or a
	 * symbol is missing or the ABI major version doesn't match. Idempotent.
	 */
	bool Load(const FString& InExplicitLibPath, FString& OutError);

	/** True once Load() has succeeded and all symbols resolved. */
	bool IsLoaded() const { return bLoaded; }

	/** The "lorevm-ffi/<major>" string reported by the loaded library. */
	FString GetAbiVersion() const { return AbiVersion; }

	/**
	 * Open the warm handle for a repository working dir. One per provider/session.
	 * `bInMemory`/`bOffline` map to the open-request JSON; `Identity` is the
	 * commit identity (may be empty). Returns false + OutError on failure; a
	 * previously-open handle is closed first.
	 */
	bool Open(const FString& WorkingDir, bool bInMemory, bool bOffline, const FString& Identity, FString& OutError);

	/** True when a warm handle is open. */
	bool IsOpen() const;

	/**
	 * Run one op on the warm handle. `OpId` is "<domain>.<op>"; `Args` is the
	 * args object serialised to a JSON string ("{}" for defaults). Parses the
	 * returned JSON into an FLorevmResult: either a Result object or a structured
	 * error. Never returns a raw pointer; always frees the C string.
	 *
	 * MUST be called from a background thread.
	 */
	FLorevmResult Call(const FString& OpId, const FString& Args);

	/** Convenience: build the Args JSON from an FJsonObject, then Call(). */
	FLorevmResult Call(const FString& OpId, const TSharedRef<FJsonObject>& Args);

	/** Close the warm handle (NULL-safe). Library stays loaded. */
	void Close();

	/** Close the handle and unload the library. */
	void Unload();

private:
	/** Resolve one symbol by name; logs + returns false if missing. */
	template <typename FnPtr>
	bool ResolveSymbol(const ANSICHAR* Name, FnPtr& OutPtr);

	/** Platform default library file name (liblorevm_ffi.so / .dylib / lorevm_ffi.dll). */
	static FString DefaultLibFileName();

	/** Candidate path inside the plugin's Binaries/<Platform>/ directory. */
	static FString PluginBinariesCandidate();

	void* LibHandle = nullptr;
	bool bLoaded = false;
	FString AbiVersion;

	// Resolved entry points.
	void* FnOpen = nullptr;        // lorevm_ffi_open_fn
	void* FnCall = nullptr;        // lorevm_ffi_call_fn
	void* FnClose = nullptr;       // lorevm_ffi_close_fn
	void* FnStringFree = nullptr;  // lorevm_ffi_string_free_fn
	void* FnAbiVersion = nullptr;  // lorevm_ffi_abi_version_fn

	// The single warm handle. Guarded by HandleCS for open/close vs. call races.
	LorevmHandle* Handle = nullptr;
	mutable FCriticalSection HandleCS;
};
