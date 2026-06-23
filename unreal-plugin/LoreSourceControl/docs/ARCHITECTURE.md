# Architecture — Lore Source Control plugin (SBAI-4086)

## Layers

```
1. Unreal Editor revision-control system
      │  ISourceControlProvider / ISourceControlOperation / ISourceControlState
2. FLoreSourceControlProvider              (the thin VCS adapter)
      │  per-op Workers (ILoreSourceControlWorker)
      │  FLoreSourceControlCommand          (IQueuedWork; runs a worker on a thread)
3. FLorevmFfi                              (the thin FFI binding layer)
      │  loads liblorevm_ffi.{so,dylib,dll}; one warm handle (open/call/close)
      │  C ABI: lorevm_ffi_open/_call/_close/_string_free/_abi_version
4. crates/lorevm-ffi (cdylib)              (SBAI-4081 — the production C bridge)
      │  lore_vm::dispatch(&api, "<domain>.<op>", argsJson) -> Value
5. crates/lore-vm  →  Epic's `lore` crate  (in-process, pinned by rev)
```

Only layers 2–3 know the lore op-id + JSON contract. Layer 1 is pure UE. Layer 4
is the shared dispatch every external driver (CLI, FFI, VS Code ext) rides, so the
FFI surface can't drift from the CLI.

## The FFI binding (FLorevmFfi)

`Ffi/LorevmFfi.{h,cpp}` is the entire bridge to our stack. Its job:

1. **Load** the cdylib at runtime via `FPlatformProcess::GetDllHandle` (dlopen /
   LoadLibrary) — *not* a link-time import lib, so the editor still starts if the
   library is missing (the provider just reports "unavailable").
2. **Resolve** the five `extern "C"` symbols into function pointers
   (`FPlatformProcess::GetDllExport`).
3. **Assert** the ABI major version (`lorevm_ffi_abi_version()` →
   `"lorevm-ffi/1"`) matches `LOREVM_FFI_ABI_MAJOR_EXPECTED`.
4. **Open** ONE warm handle for the repo working dir
   (`lorevm_ffi_open({"dir","in_memory","offline","identity"})`), held for the
   editor session.
5. **Call** ops: `Call("<domain>.<op>", argsJson)` →
   `lorevm_ffi_call(handle, op_id, args)` → parse the returned JSON with UE's
   `FJsonSerializer` into `FLorevmResult` (a result object, or a structured
   `{error:{kind,message}}`), then **free the Rust string with
   `lorevm_ffi_string_free`** (never C `free` — the buffer is a Rust `CString`).
6. **Close** the handle + unload the library on shutdown.

### Response contract

`lorevm_ffi_call` returns malloc'd JSON the caller owns:

- **Success** → the op's typed result object (e.g. `repository.status` →
  `{revision, files:[...], count}`; `lock.file_status` → `{locks:[{path,owner,locked_at}]}`).
- **Failure** → `{"error":{"kind","message"}}`. `kind` is the serialized
  `LoreError` variant (`CommandFailed`, `Parse`, `Auth`, …) or `"panic"` / `"ffi"`
  for boundary failures. `FLorevmFfi::Call` surfaces this as
  `FLorevmResult{ bSuccess=false, ErrorKind, ErrorMessage }`.
- **NULL** only on a NUL / invalid-UTF-8 pointer (shouldn't happen given our
  UTF-8 conversions); treated as an `ffi` error, never a crash.

## Operations mapping (UE op → lore op-id)

| UE operation | Worker | lore op-id(s) | Status |
|--------------|--------|---------------|--------|
| `Connect`      | `FLoreConnectWorker`      | `repository.info` (fallback `repository.status`) | ✓ wired |
| `UpdateStatus` | `FLoreUpdateStatusWorker` | `repository.status` + `lock.file_status` | ✓ wired + lock owner |
| `CheckOut`     | `FLoreCheckOutWorker`     | `lock.file_acquire` | ✓ wired; SBAI-4044 tray stub present |
| `MarkForAdd`   | `FLoreMarkForAddWorker`   | `file.stage` | ✓ wired |
| `Delete`       | `FLoreDeleteWorker`       | `file.stage` | ✓ wired |
| `CheckIn`      | `FLoreCheckInWorker`      | `revision.commit` (+ `branch.push`) | ✓ wired |
| `Revert`       | `FLoreRevertWorker`       | `lock.file_release` (+ `file.unstage`) | ✓ wired |
| `Sync`         | `FLoreSyncWorker`         | `revision.sync` | ✓ wired |
| `GetHistory`   | `FLoreHistoryWorker`      | `file.history` | ✓ wired (UE-BUILD-PENDING: GetHistory op name) |

Every op-id above is in `lore_vm::supported_ops()` (see
`crates/lore-vm/src/dispatch.rs`). Args are repo-relative paths + the branch
(lock ops are branch-scoped); `LoreSourceControlUtils::ToRepoRelative` maps UE's
absolute filenames to lore paths.

## Settings surface (two complementary layers)

The plugin exposes settings through two mechanism that merge at Init() time:

### 1. `ULoreSourceControlDeveloperSettings` (project-wide)

`Source/LoreSourceControl/Public/LoreSourceControlDeveloperSettings.h`

A `UDeveloperSettings` subclass persisted in `DefaultSourceControlSettings.ini`.
Shows up in **Edit → Project Settings → Plugins → Lore Source Control**.
Fields: `LoreVmBinaryPath`, `ServerUrl` (default `http://localhost:17171`),
`Identity`, `bUseInMemory`, `bOffline`.
Registered with `ISettingsModule` in `FLoreSourceControlModule::StartupModule`.

### 2. `FLoreSourceControlSettings` (per-developer)

`Source/LoreSourceControl/Private/LoreSourceControlSettings.h`

Plain thread-safe struct, persisted in `SourceControlSettings.ini` (the editor's
standard per-developer source-control ini). The same fields as above, read/written
via `GConfig`.

### Merge rules at Init()

`FLoreSourceControlProvider::Init()` merges both:
- Per-developer values take priority over project CDO defaults when non-empty.
- `bInMemory` / `bOffline` are OR'd: either setting enables the mode.

## Overlay state mapping (lore result → icon)

`FLoreSourceControlState` carries two orthogonal axes:

- **WorkingCopyState** from `repository.status` file actions (add/delete/move/copy
  → Added/Deleted/Modified; `conflict` flag → Conflicted).
- **LockState** from `lock.file_status` owner vs. our `Identity`
  (`owner == identity` → LockedByMe, else LockedByOther; none → NotLocked).

`GetIcon()` resolves these to the engine's standard `RevisionControl.*` brushes
(priority: lock state, then out-of-date, then local modification), so badges look
native and re-theme with the editor:

| State | Icon | Meaning |
|-------|------|---------|
| LockedByMe | `RevisionControl.CheckedOut` | checked out by me |
| LockedByOther | `RevisionControl.CheckedOutByOtherUser` | checked out by other (+ owner in tooltip) |
| bNewerVersionOnServer | `RevisionControl.NotAtHeadRevision` | out of date |
| Added | `RevisionControl.OpenForAdd` | staged add |
| Modified | `RevisionControl.CheckedOut` | locally modified |
| Deleted | `RevisionControl.MarkedForDelete` | staged delete |
| Conflicted | `RevisionControl.Conflicted` | merge conflict |

## Revision history (`FLoreSourceControlRevision`)

`FLoreHistoryWorker::Execute()` calls `file.history` (lore op-id) once per
requested file. The `file.history` result JSON shape:

```json
{ "entries": [
    { "path": "...", "repository": "...", "revision": "<hash>",
      "revision_number": 42, "parents": ["<hash>"], "address": "...",
      "size": 1024, "action": "keep|add|delete|move|copy" }
] }
```

`FLoreSourceControlRevision` implements `ISourceControlRevision`. The `History`
array on `FLoreSourceControlState` is populated by `FLoreHistoryWorker::UpdateStates`
(game thread). History accessors (`GetHistorySize`, `GetHistoryItem`,
`FindHistoryRevision`, `GetCurrentRevision`) are now wired to this array.

**UE-BUILD-PENDING (history enrichment):** `file.history` does not return
timestamps or commit messages. A follow-up should call `revision.info` per hash
to populate `FLoreSourceControlRevision::Timestamp` and `Description`.

## SBAI-4044 lock-request → tray-message seam

`FLoreCheckOutWorker::Execute()` contains a clearly marked `// SBAI-4044 TODO`
comment after a successful `lock.file_acquire` call. The stub describes the
`FLoreNotificationBridge::Get().NotifyLockAcquired(...)` call site where the
cross-app notification to the LoreGUI desktop tray should fire. The bridge class
itself does not exist yet; the comment marks the seam so the tray work (SBAI-4044)
has a clear integration point.

## Threading + ownership rules

- `lorevm_ffi_call` **blocks** for the op's duration. Workers run inside
  `FLoreSourceControlCommand::DoThreadedWork` on the source-control thread pool —
  **never the game thread**. `Execute(...Synchronous)` runs inline only for
  explicitly-synchronous requests (e.g. `GetState(ForceUpdate)`).
- The Rust handle is `Send + Sync`; `FLorevmFfi` guards the *handle pointer* with
  a critical section only so a concurrent `Close()`/shutdown can't free it
  mid-call. Each call enters the shared tokio runtime.
- Strings returned by `lorevm_ffi_call` are owned by the caller and freed with
  `lorevm_ffi_string_free`. `FLorevmFfi::Call` always frees before returning.
- Worker results marshal back to the game thread through the worker's `States`
  vector (or `HistoryMap` for history); `UpdateStates()` (game thread, called from
  `Tick`/`ReturnResults`) writes them into the provider's `StateCache`, then
  `OnSourceControlStateChanged` broadcasts so overlays refresh.

## Swappable design

The plugin is deliberately two pieces: the `ISourceControlProvider` adapter and
the `FLorevmFfi` bridge. To adopt Epic's future first-party lore provider, drop
the adapter and register Epic's — the editor keeps talking to
`ISourceControlProvider`. Keeping the adapter thin and op-id/JSON-shaped is what
preserves that exit. See `docs/ue-lorevm-bridge-spike.md` (LoreGUI repo) §7.

## Coverage summary (feat/ue-plugin-advance — SBAI-4079)

| Feature | Status |
|---------|--------|
| UDeveloperSettings panel (Project Settings) | ✓ `LoreSourceControlDeveloperSettings.h/.cpp` |
| Settings merge (per-dev + project CDO) | ✓ `LoreSourceControlProvider::Init()` |
| ISettingsModule registration | ✓ `FLoreSourceControlModule::RegisterSettings()` |
| UpdateStatus (status + lock owner) | ✓ pre-existing, unchanged |
| GetIcon / overlay badges | ✓ pre-existing, unchanged |
| FLoreHistoryWorker (`file.history`) | ✓ fully wired, states flushed |
| FLoreSourceControlRevision | ✓ `ISourceControlRevision` impl |
| History accessors on FLoreSourceControlState | ✓ wired to History[] |
| SBAI-4044 tray stub (lock → tray notification) | ✓ stub + comment in CheckOutWorker |

## Out of scope (future layers)

- `revision.info` per-hash to populate revision Timestamp + Description (UE-BUILD-PENDING)
- Rich Slate settings widget (MakeSettingsWidget placeholder retained)
- Changelists, cross-branch lock visibility, history pagination
- `FLoreNotificationBridge` (SBAI-4044 cross-app tray notification, stub seam present)
- `GetHistory` as a named UE operation (currently invoked directly; confirm on-device)
