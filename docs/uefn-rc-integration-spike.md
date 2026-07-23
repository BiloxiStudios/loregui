# Spike: UEFN/Unreal editor RC integration feasibility — lore notification & service hooks

**Ticket:** SBAI-5500
**Date:** 2026-07-23
**Filed by:** vs-fable (AgentComms group:lorecrew)
**Status:** SPIKE COMPLETE — decision memo + follow-up tickets

> **Relationship:** Parallel, NOT part of SBAI-5499 (URC real-status + recovery UX).
> SBAI-5499 is unblocked regardless of this spike's outcome.

---

## 1. Problem statement

Epic's `lore` VCR ships a `lore.exe` CLI and an in-process `lore` Rust crate.
LoreGUI (`crates/lore-vm`) already wraps 30+ ops across 14 domains (auth, branch,
dependency, file, layer, link, lock, notification, repository, revision, service,
shared_store, storage) by calling the `lore` crate in-process — no CLI shelling.

The question for this spike: **can LoreGUI integrate with lore's notification
and service subsystems for real-time event streaming (commit/sync/lock events)
and service lifecycle management?** And separately: **is a native UEFN editor
addon feasible?**

---

## 2. Probe results (verified on EROS, 2026-07-22)

Probe conducted against `lore.exe` from Fortnite ~36.x install, clean CCA repo at
rev 502.

### 2.1 `lore notification subscribe [seconds]` — EXISTS, WITH CAVEAT

**Status:** The command exists ("Subscribe to events on the given repository").

**Live test result:** Requires authenticated **ONLINE** mode. In a non-interactive
agent shell it fails:

```
Couldn't access platform secure storage: Windows ERROR_NO_SUCH_LOGON_SESSION
→ Failed to decrypt user token
→ forced offline
→ notifications not available when offline
```

**Root cause:** Windows DPAPI secure storage (`ERROR_NO_SUCH_LOGON_SESSION`)
requires an interactive desktop logon session. The non-interactive agent shell
(e.g. SSH, scheduled task, service context) has no logon session, so DPAPI
refuses access, the token decrypt fails, lore falls back to offline mode, and
notifications are unavailable offline by design.

**Untested scenario:** An interactive desktop session with the editor open. This
is the **#1 remaining probe**. If `lore notification subscribe 60` streams
commit/sync/lock events in that context, this is the broadcast hook LoreGUI
wants.

**LoreGUI code status:** The ops already exist and compile:
- `crates/lore-vm/src/ops/notification/subscribe.rs` — binds
  `lore::notification::subscribe` in-process via `collect_events()`. Returns
  `SubscribeResult` on success.
- `crates/lore-vm/src/ops/notification/unsubscribe.rs` — mirrors the subscribe
  pattern with `UnsubscribeResult`.

Both ops use the standard lore-vm pattern: `collect_events()` callback →
`oneshot::Receiver<EventStream>` → check `stream.is_ok()` → return typed result.
**The code is ready; the runtime auth context is the blocker.**

### 2.2 `lore service run|start|stop` — EXISTS, TRANSPORT UNDOCUMENTED

**Status:** A repository service-process model exists ("Manage the repository in
a service process", "Run this process as the service").

**Verified:** No `lore.exe` daemon runs by default (checked via `tasklist`).
`--dry-run service start` succeeds silently.

**Unknown:** Transport between editor/CLI and the service (named pipe vs TCP port
vs in-proc) is undocumented.

**Remaining probe:** Start the service from an interactive session and inspect
listening pipes/ports (procexp/netstat) while the editor is open. **Enumerate
only — NO interception/replay.**

**LoreGUI code status:** The ops already exist and compile:
- `crates/lore-vm/src/ops/service/start.rs` — binds `lore::service::start`,
  collects `Log` events, returns `ServiceStartResult { log_messages }`.
- `crates/lore-vm/src/ops/service/stop.rs` — binds `lore::service::stop` with
  `all: bool` arg support, returns `ServiceStopResult { log_messages }`.

Both ops follow the standard lore-vm pattern with full serialization tests.
**The code is ready; the transport identification is the remaining unknown.**

### 2.3 `lore.dll` — SHIPS, BUT NOT A SUPPORTED INTEGRATION POINT

`lore.dll` ships next to `lore.exe`, suggesting an in-proc library surface.
However, it is **undocumented and unversioned**. Treat as **NOT a supported
integration point** absent Epic guidance.

### 2.4 Full CLI surface (already proven)

`status/history/diff/sync/clone/stage/commit/lock/branch/revision/file/layer/link/logfile/shared-store`
— the already-proven wrapping surface (SBAI-5499 / uefn-mcp contracts).

---

## 3. UEFN Editor Addon — EXPLICIT NO-CLAIM

**A native editor addon/hook inside UEFN is NOT available.**

The editor's RC integration is the **SkeinSourceControl plugin** in the Fortnite
install, shipped **COOKED** (Content dir only — no `.uplugin` manifest, no
headers, no exposed API). UEFN does **not** support user editor plugins.

There is **no supported way** to intercept or extend the in-editor RC panel.

**Do not plan work that assumes otherwise.**

---

## 4. Interception — OUT OF SCOPE

Interception of editor↔lore traffic (e.g. hooking `lore.dll` or the service
transport) would be **reverse-engineering an undocumented internal protocol** —
fragile across Fortnite updates and potentially ToS-sensitive.

**Recommendation: explicitly out of scope** absent Epic guidance. vs-fable owns
the Epic-relationship channel and can ask.

---

## 5. Full Unreal Engine (NOT UEFN) — FEASIBLE

A standard UE source-control provider plugin (`ISourceControlProvider`, like the
P4/Git providers) wrapping `lore.exe` is **architecturally normal UE work** —
feasible as a "planned Unreal addon" for UE-side workflows.

See `docs/ue-lorevm-bridge-spike.md` (SBAI-4079) for the full architecture:
- **Option A:** Shell `lorevm --json` as subprocess (zero new code, process
  isolation, but per-call spawn cost).
- **Option B:** C-ABI FFI `cdylib` (in-process, warm handle, excellent for hot
  path but shared fate with UE editor).
- **Recommended:** Hybrid — FFI for hot path (Content Browser overlay), shell
  for one-shot ops (clone, push, commit).

**Important:** UEFN would **not** load a custom source-control provider plugin
either. This path is UE-only.

---

## 6. LoreGUI Integration Path — RECOMMENDATION

### Recommended approach: notification-subscribe stream + service/CLI wrapping

The existing lore-vm ops (`notification::subscribe`, `notification::unsubscribe`,
`service::start`, `service::stop`) are the correct integration surface. They
already compile, follow the established lore-vm pattern, and wire into the shared
dispatch (`lore_vm::dispatch` in `crates/lore-vm/src/dispatch.rs`).

**Integration architecture:**

```
LoreGUI frontend (TypeScript/React)
    │ Tauri command
    ▼
LoreAPI (Rust, crates/lore-vm)
    │ dispatch("notification.subscribe", args)
    │ dispatch("service.start", args)
    ▼
lore crate (in-process) → Epic's lore engine
    │
    ├── notification.subscribe → streams commit/sync/lock events
    │   via LoreEvent callback → collect_events() → EventStream
    │   REQUIRES: interactive authed session (Windows logon)
    │
    └── service.start/stop → manages lore service process
        TRANSPORT: unidentified (named pipe? TCP? in-proc?)
        Next: enumerate from interactive session
```

**What LoreGUI gets from this:**
- **Real-time event streaming** (commit, sync, lock) via `notification.subscribe`
  — the broadcast hook for live status badges, lock notifications, sync progress.
- **Service lifecycle management** via `service.start`/`service.stop` — start
  the lore service process when LoreGUI opens, stop on close.

**Remaining blockers to resolve:**
1. **Auth context:** `notification.subscribe` needs interactive Windows logon
   session (DPAPI). LoreGUI runs as a desktop app, so this should work — but
   needs verification from an interactive session with the editor open.
2. **Service transport:** The service's communication mechanism (named pipe vs
   port) needs enumeration to understand whether LoreGUI can query service state
   or needs to manage it purely through the start/stop ops.

### Is a UE-only provider plugin worth a ticket?

**Yes.** The ue-lorevm-bridge spike (SBAI-4079) already proved FFI feasibility.
A UE `ISourceControlProvider` wrapping lore-vm's dispatch is a natural follow-up
for UE-side workflows. It would NOT work in UEFN, but standard UE projects could
benefit from native source-control integration (Content Browser overlays, check-out
via UE's native UI, etc.).

**Recommendation:** File a follow-up ticket for the UE provider plugin, scoped
to standard UE only (explicitly exclude UEFN).

---

## 7. Follow-up Tickets

### SBAI-5501: Verify `lore notification subscribe` from interactive authed session

**Scope:** Run `lore notification subscribe 60` from an interactive desktop
session (owner logon, UEFN editor open + closed). Capture event format for
commit/sync/lock. Document schema.

**Acceptance:** Event schema documented (field names, types, event names for
commit, sync, lock). Confirmation of whether events stream in real-time.

**Timebox:** 1 hour (run the command, capture output, document).

### SBAI-5502: Identify `lore service` transport mechanism

**Scope:** Start `lore service` from interactive session. Enumerate listening
pipes/ports via procexp/netstat while editor is open. Document transport type.

**Acceptance:** Transport identified (named pipe path, TCP port, or in-proc only).
NO interception or replay — enumerate only.

**Timebox:** 1 hour (start service, inspect, document).

### SBAI-5503: UE source-control provider plugin (ISourceControlProvider)

**Scope:** Implement a standard UE `ISourceControlProvider` plugin wrapping
lore-vm's dispatch. Modeled on the ue-lorevm-bridge spike (SBAI-4079).

**Out of scope:** UEFN support (explicitly excluded — UEFN does not support
user plugins). lore.dll hooking (out of scope — undocumented, ToS-sensitive).

**Reference:** `docs/ue-lorevm-bridge-spike.md` for architecture. BenVlodgi's
MIT `UnrealSourceControl` scaffolding for Provider/Worker/State shape.

### SBAI-5504: Ask Epic about supported lore integration points

**Scope:** vs-fable to ask Epic contacts:
- Is `lore.dll` a supported integration point? Is there versioned API surface?
- Is there a planned Epic-first UE source-control provider for lore?
- Is the `lore service` transport documented? What is the intended integration
  mechanism (named pipe, TCP, in-proc)?

**Acceptance:** Answers recorded, shared with lorecrew group.

---

## 8. UEFN No-Claim (verbatim)

> A native editor addon/hook inside UEFN is NOT available. The editor's RC
> integration is the SkeinSourceControl plugin in the Fortnite install, shipped
> COOKED (Content dir only — no .uplugin manifest, no headers, no exposed API),
> and UEFN does not support user editor plugins. There is no supported way to
> intercept or extend the in-editor RC panel.
>
> Interception of editor↔lore traffic (e.g. hooking lore.dll or the service
> transport) would be reverse-engineering an undocumented internal protocol —
> fragile across Fortnite updates and potentially ToS-sensitive. Recommend
> explicitly out of scope absent Epic guidance.

**Filed by:** vs-fable, per owner request relayed by codex-brainz (lorecrew, 2026-07-22).

---

## 9. Summary

| Question | Answer |
|---|---|
| Does `lore notification subscribe` exist? | **Yes.** Streams events, but requires interactive authed session (DPAPI). |
| Does `lore service start/stop` exist? | **Yes.** Service process model exists; transport mechanism unidentified. |
| Can LoreGUI integrate with these? | **Yes.** Ops already exist in lore-vm. Need interactive-session verification. |
| Is UEFN editor addon feasible? | **NO.** UEFN does not support user plugins. SkeinSourceControl is cooked. |
| Is interception feasible? | **No.** Reverse-engineering undocumented protocol. Out of scope. |
| Is UE (not UEFN) provider plugin feasible? | **Yes.** Standard UE ISourceControlProvider. See ue-lorevm-bridge-spike.md. |

**Recommended next steps:** SBAI-5501 (interactive subscribe test) + SBAI-5502
(service transport ID) → unblock LoreGUI notification integration. SBAI-5503
(UE provider plugin) as a separate effort. SBAI-5504 (ask Epic) for long-term
integration strategy.
