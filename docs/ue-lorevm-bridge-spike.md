# Spike: how a StudioBrain Unreal Engine C++ plugin should drive our lore binding

**Ticket:** SBAI-4079 (spike — design + feasibility, no UE code, no full build)
**Branch:** `spike/ue-lorevm-bridge`
**Status:** complete. FFI feasibility proof builds and its smoke call passes.

---

## 1. Problem

We want a StudioBrain Unreal Engine (UE) editor plugin that surfaces lore VCS
state in the editor — most visibly **Content Browser status overlays** (per-asset
checked-out / locked / modified / up-to-date badges) and a source-control panel
— driving lore through **our** stack, not Epic's public `lore` CLI.

A UE plugin is **C++**. Our lore binding is **Rust**:

```
crates/lore-vm        LoreApi + ops/<domain>/<op>.rs  — binds Epic's `lore` crate in-process,
                      collects its events into typed serde results. The reusable core.
crates/lorevm-cli     the `lorevm` binary: a thin JSON CLI over lore-vm's dispatch.
                      `lorevm <domain>.<op> --dir <repo> --args '<json>'` → JSON on stdout.
                      This is the subprocess surface `lore-mcp` already drives.
```

So the question is: **how does C++ reach lore-vm?** Two viable routes (both go
through *our* stack, neither shells Epic's `lore` CLI):

- **A — Shell `lorevm --json`** as a child process per op (reuses the exact
  surface `lore-mcp` uses today).
- **B — C-ABI FFI**: build lore-vm as a `cdylib` exposing `extern "C"` entry
  points, link it into the UE plugin, call in-process (no subprocess).

This doc assesses both for the UE use case, proves B is feasible with a minimal
crate, and recommends an architecture.

---

## 2. The surface a bridge must expose

Whichever route we pick, the bridge wraps the **same dispatch** that
`lorevm-cli/src/main.rs` already implements:

- Build a `LoreApi` from a `LoreGlobal` (working dir + `in_memory` / `offline` /
  `identity` / `force` / `max_connections`).
- Map a `"<domain>.<op>"` string to the matching `lore_vm::ops::…` async fn.
- Deserialize a JSON args object into that op's `Args` struct (snake_case serde
  keys, all `#[serde(default)]`).
- `await` the op, serialize its typed `Result` to JSON, or return the structured
  `LoreError` as `{"error":{"kind","message"}}`.

The op set today (30 ops across `repository`, `revision`, `branch`, `auth`,
`file`, `lock`) is exactly what a UE plugin needs: `repository.status`,
`lock.file_status` / `lock.file_query` (overlay state), `file.stage` /
`file.unstage` / `lock.file_acquire` / `lock.file_release` (check-out / -in),
`revision.commit` / `branch.*` (submit / streams).

**Key structural fact for this spike:** that dispatch `match` lives **only in the
CLI binary** (`main.rs`), not in `lore-vm` as a library fn. So any second driver
(FFI, or a future in-process host) today has to *re-type* the match. The clean
production fix is to **lift the dispatch into `lore-vm` as a public
`dispatch(op_id, &LoreApi, args_json) -> serde_json::Value` fn** and have both
`lorevm-cli` and the FFI crate call it. The spike crate deliberately re-types a
small subset rather than refactor the CLI, to stay minimal and non-invasive — but
the recommendation below assumes that one-time refactor.

---

## 3. Option A — shell `lorevm --json` from UE

UE spawns the `lorevm` binary per op (UE has `FPlatformProcess` /
`FMonitoredProcess`), writes flags + `--args '<json>'`, reads JSON from stdout,
parses it (UE ships `FJsonSerializer`).

**Pros**
- **Zero new code in our repo.** `lorevm` already exists and is the proven
  `lore-mcp` surface. The bridge is entirely on the UE side.
- **Process isolation.** A panic / crash / memory blow-up in lore or its QUIC
  stack takes down a child process, not the UE editor. For a pre-1.0 dependency
  (`lore` is `0.8.4-nightly`, pinned by exact rev) this is real safety.
- **Trivial threading story.** Run the subprocess on a UE background thread;
  marshal the parsed result back to the game thread. No shared runtime, no
  re-entrancy, no FFI lifetime rules.
- **Process-per-op = no shared mutable state** to corrupt across calls.
- **Cross-platform "for free."** Ship a `lorevm.exe` / `lorevm` per platform;
  UE's process API is already cross-platform. No per-platform linking config.

**Cons**
- **Per-call latency floor.** Each op pays **process spawn + a fresh
  `#[tokio::main(multi_thread)]` runtime spin-up + engine open + teardown.**
  That's milliseconds-to-tens-of-ms of fixed overhead *before* any work. The
  Content Browser refreshes overlays for *many* assets and re-queries on
  navigation/focus; spawning a process per asset (or even per refresh tick)
  is the wrong cost curve.
- **No warm state.** Every call re-opens the repo and re-creates the engine.
  Nothing is cached between calls; a status sweep can't reuse a warm handle.
- **Throughput ceiling.** Batching helps (one `lorevm` call returning many
  assets' status), but you're still bounded by spawn cost and stdout parsing,
  and you can't stream incremental updates — it's one-shot request/response.
- **Binary distribution.** The plugin must locate/ship a `lorevm` binary,
  version-match it, and survive AV/Gatekeeper/quarantine on the child exe
  (Windows Defender slow-write + macOS notarization both bite child binaries).
- **Harder structured errors / progress.** Long-running networked ops
  (`repository.clone`, `branch.push`, `revision.sync`) are opaque until the
  process exits — no progress events, only a final blob.

**Effort:** very low (UE-side only). **Risk:** low. **Fit for hot path:** poor.

---

## 4. Option B — C-ABI FFI (`cdylib`)

Build lore-vm/dispatch as a `cdylib` exposing `extern "C"` entry points; the UE
plugin links the resulting `.dll` / `.dylib` / `.so` and calls in-process.

Minimal proven ABI (see `crates/lorevm-ffi` — this spike):

```c
// op_id  : "<domain>.<op>", UTF-8, NUL-terminated
// request: JSON { "dir", "args", "in_memory", "offline", "identity" }, NUL-terminated
// returns: malloc'd NUL-terminated JSON; caller owns it, frees via the free fn.
//          success → op result; failure → {"error":{"kind","message"}}.
char*       lorevm_ffi_call(const char* op_id, const char* request);
void        lorevm_ffi_string_free(char* s);
const char* lorevm_ffi_abi_version(void);   // static; do NOT free
```

**Pros**
- **No per-call process/runtime cost.** A production version holds **one
  long-lived tokio runtime + a warm `LoreApi`** behind an opaque handle, so a
  status query is "enter runtime, run op, return" — microseconds of overhead,
  not a process spawn. This is the right curve for **high-frequency overlay
  refresh**.
- **Warm state & caching become possible.** A handle can keep the repo open,
  cache lock/status results, and serve the Content Browser's many per-asset
  queries from one engine instance. Can add a batched `status_many` entry point.
- **Streaming / progress is reachable.** With a handle + callback (or a
  poll-a-channel entry point), long ops (`clone`/`push`/`sync`) can report
  progress to the editor UI instead of blocking opaquely.
- **One artifact, tighter coupling.** No child-binary discovery/versioning; the
  `.dll`/`.dylib` ships inside the plugin's `Binaries/` like any other UE
  third-party lib. ABI version fn lets the plugin assert compatibility.

**Cons**
- **Shared address space = shared fate.** A panic or memory error in lore or its
  QUIC/TLS stack can take down the **UE editor**, not just a child process. We
  must `catch_unwind` at every `extern "C"` boundary (a Rust panic unwinding
  across the C ABI is UB) and treat the pre-1.0 `lore` dep with care.
- **Threading discipline is on us.** UE is game-thread-sensitive. The call
  **blocks** for the op's duration, so it must run on a UE background thread
  (`AsyncTask` / `FRunnable`), never the game thread, then marshal results back.
  A warm shared runtime also means the entry points must be `Send`-safe and
  re-entrancy-aware (lore-vm's `LoreApi` is `Clone` and cheap, which helps).
- **Memory ownership rules.** Strings cross the ABI malloc'd by Rust and **must**
  be freed by the matching Rust free fn (not C `free`) — the spike enforces this
  with `CString::into_raw` / `from_raw`. Get this wrong → leak or heap corruption.
- **Cross-platform build/link config.** Need a `cdylib` per platform (Win MSVC
  `.dll` + import lib, macOS `.dylib`, plus codesigning/notarization), wired into
  UE's `*.Build.cs` as a third-party module. More moving parts than shipping a
  binary, though all standard UE third-party-lib practice.
- **The dispatch must be shared, not duplicated.** Worth doing the §2 refactor so
  CLI and FFI can't drift.

**Effort:** moderate (one-time ABI + the §2 dispatch lift + per-platform build).
**Risk:** moderate (in-process fate-sharing, ownership, threading). **Fit for hot
path:** excellent.

---

## 5. Feasibility proof for Option B (this spike)

Added `crates/lorevm-ffi` — a minimal `cdylib` that wraps lore-vm's dispatch
behind the C ABI above. It wires a representative subset of ops (`repository.create`,
`repository.status`, `repository.info`, `branch.list`) — enough to prove a
mutating op, a read op, and a metrics op all cross the boundary cleanly. It is
clearly marked **SPIKE / not a shipped artifact**.

**Build:** `cargo build -p lorevm-ffi` → produces `liblorevm_ffi.so` (and would
produce `.dll` / `.dylib` on Win/Mac). The three `extern "C"` symbols
(`lorevm_ffi_call`, `lorevm_ffi_string_free`, `lorevm_ffi_abi_version`) are
exported (verified with `nm -D`).

**Smoke test** (`crates/lorevm-ffi/tests/smoke.rs`) calls the symbols exactly as
C would — `CString` in, parse + `lorevm_ffi_string_free` the returned C string:

| test | result |
|------|--------|
| `abi_version_is_exposed` | pass — static ABI string readable across ABI |
| `create_then_status_roundtrips_over_the_c_abi` | **pass** — real in-memory engine: `repository.create` then `repository.status`, both driven over the C ABI, no server |
| `unknown_op_returns_structured_error_not_null` | pass — errors come back as `{"error":{"kind":"ffi",…}}`, never NULL |

**Conclusion: Option B is feasible.** lore-vm can be driven over a stable C ABI
with JSON in/out and clean malloc/free ownership. The same `LoreApi` + `ops`
layer the CLI and Tauri app use works unchanged in-process from C.

> Spike shortcuts (intentional, must change for production): a fresh tokio
> runtime is built **per call** (a real bridge holds one long-lived runtime +
> warm `LoreApi` behind an opaque handle); ops are re-typed instead of calling a
> shared `lore_vm::dispatch`; no `catch_unwind` panic guard yet. None of these
> affect the feasibility result — they're the gap between "proven" and "shipped."

---

## 6. Recommendation — **hybrid, FFI-first for the hot path**

Use **both routes against the same dispatch**, picked by call shape:

- **FFI (Option B) for the hot path / interactive state:** Content Browser
  overlay refresh, the source-control panel's live status, lock/checkout state —
  anything queried often or per-asset. Warm handle + one runtime + (later) a
  batched `status_many` and progress callbacks. This is where A's per-call spawn
  cost is unacceptable and B's warm-state model wins.

- **Shell (Option A) for one-shot, low-frequency, or risky ops:** initial
  `repository.clone`, `branch.push`, occasional `revision.commit` submits —
  operations where process isolation (crash containment for the pre-1.0 lore
  networking stack) is worth more than the spawn cost, and where one-shot
  request/response is fine.

**Why hybrid rather than pure B:** the overlay refresh path genuinely needs
in-process latency, but the networked ops are exactly where in-process
fate-sharing with a nightly QUIC stack is scariest. Splitting by call shape gets
the latency where it matters and keeps the editor crash-isolated from the
riskiest ops. If the team wants one mechanism, go **B everywhere** with a robust
`catch_unwind` guard — but start the overlay path on B regardless.

**Prerequisite for either FFI direction:** do the §2 refactor — lift the dispatch
`match` out of `lorevm-cli/src/main.rs` into a public
`lore_vm::dispatch(op_id, &LoreApi, args) -> Value`. Then `lorevm-cli`, the FFI
`cdylib`, and the Tauri commands all share one dispatch and cannot drift.

---

## 7. StudioBrain UE plugin architecture sketch

```
┌─────────────────────────── Unreal Editor (C++) ───────────────────────────┐
│                                                                            │
│  StudioBrain DAM / entity layer  (UE plugin, C++/Slate)                    │
│   • Content Browser overlays, asset → lore-path mapping, entity metadata,  │
│     StudioBrain auth/identity surfaced to the user                         │
│        │ asks "what's the VCS state of these assets?" / "check out"        │
│        ▼                                                                    │
│  ISourceControlProvider impl  ── thin VCS adapter ──────────────┐          │
│   (FStudioBrainSourceControlProvider, modeled on BenVlodgi's    │          │
│    MIT UnrealSourceControl scaffolding: Provider + Operations +  │          │
│    Worker per op + State cache)                                  │          │
│        │ translates UE SCC ops → "<domain>.<op>" + JSON args     │          │
│        ▼                                                          │          │
│  ┌──── Bridge (chosen mechanism) ───────────────────────────┐   │          │
│  │  HOT PATH  → lorevm-ffi cdylib  (in-process C ABI)        │   │          │
│  │              warm handle: 1 runtime + 1 LoreApi           │   │          │
│  │  ONE-SHOT  → spawn `lorevm --json`  (process isolation)   │   │          │
│  └──────────────────────────────────────────────────────────┘   │          │
└───────────────────────────────────│──────────────────────────────┘          │
                                     ▼  (both call the SAME shared dispatch)
        crates/lore-vm  ──  lore_vm::dispatch → LoreApi + ops/<domain>/<op>
                                     ▼
                          Epic's `lore` crate (in-process), pinned by rev
```

**Layering, top to bottom:**

1. **StudioBrain DAM / entity layer (plugin-owned).** Where StudioBrain's own
   value sits: mapping UE assets ↔ lore paths ↔ StudioBrain entities, overlay
   rendering, identity. Knows nothing about the bridge mechanism — it only talks
   to the SCC provider.
2. **Thin VCS adapter — `ISourceControlProvider`.** Implement UE's source-control
   interface so the editor's *native* Content Browser overlays, "Check Out",
   "Submit", "Revert" etc. light up. **Reference BenVlodgi's MIT
   `UnrealSourceControl` scaffolding** for the Provider / per-op Worker / State
   cache shape — but our Workers call our bridge, not git/p4. This adapter is the
   only layer that knows lore op ids + JSON arg shapes; keep it thin.
3. **Bridge.** FFI handle (hot path) + subprocess (one-shot), per §6. Both
   serialize the same `"<domain>.<op>" + {args}` contract.
4. **Shared dispatch → lore-vm.** The single `lore_vm::dispatch` (post-refactor),
   identical to what the CLI and Tauri app drive.

**Re-targeting Epic's official provider later.** Epic is building a first-party
lore source-control provider for UE. Because steps 1–2 are isolated behind
`ISourceControlProvider` and a thin adapter, swapping to Epic's provider later is
a **provider-registration change**, not a rewrite: StudioBrain's DAM/entity layer
keeps talking to `ISourceControlProvider`; we drop our adapter and register
Epic's (or keep ours where StudioBrain needs richer DAM-aware behavior than
Epic's exposes). Keeping the adapter thin and op-id/JSON-shaped is what preserves
this exit.

---

## 8. Shared bridge — the VS Code extension reuses the SAME lorevm

The planned **VS Code extension** reuses the **same `lorevm` JSON surface via
shell (Option A), no FFI.** VS Code extensions are Node/TypeScript and already
spawn child processes idiomatically (`child_process`), and the extension's needs
are one-shot/command-driven, not per-asset hot-path — so the subprocess cost is a
non-issue there. It shells `lorevm <domain>.<op> --args '<json>'` exactly like
`lore-mcp` does.

**Implication:** the JSON `"<domain>.<op>" + {args}` → typed-result contract is
**shared bridge work across three consumers** — `lore-mcp`, the VS Code
extension, and the UE plugin's one-shot path — all over shell, plus the UE hot
path over FFI. The §2 dispatch refactor (one `lore_vm::dispatch`) is therefore
high-leverage: it's the single seam every driver rides on. Get that contract and
the op coverage right once, and every consumer benefits; the only UE-specific net
new work is the `cdylib` ABI + the `ISourceControlProvider` adapter.

---

## 9. Verification (this spike)

- `cargo build -p lorevm-ffi` → `liblorevm_ffi.so` produced; `extern "C"` symbols
  exported.
- `cargo test -p lorevm-ffi` → 3/3 smoke tests pass, including a real in-memory
  create→status roundtrip over the C ABI.
- `cargo fmt --all --check` → clean.
- `cargo check` over the workspace → clean (no existing crate touched beyond the
  one-line `members` addition).

No UE code, no full build, no push — design + feasibility only, per the ticket.
