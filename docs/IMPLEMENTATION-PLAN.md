# LoreGUI — Implementation Plan (Full-Parity, Pipeline-Driven)

**Status:** active · **Owner:** Brandon F. (integration manager: Claude main loop) · **Created:** 2026-06-18
**Repo:** `BiloxiStudios/loregui` (public, MIT) · canonical checkout on **BRAINZ** at `/srv/studiobrain-dev/loregui`
**Tracking:** Jira `SBAI` — Epic *LoreGUI: full-parity desktop GUI for Lore* → Stories per domain → 1 subtask per API operation
**Upstream:** Epic Games `lore` @ `6022734a6fa5d3f02e6f92c619624dbfbf655186` (pinned)

---

## 1. Goal

Ship a cross-platform desktop GUI for Epic's [Lore](https://github.com/EpicGames/lore) VCS that drives the **complete native API surface (~124 operations across 14 domains)** — not a CLI wrapper. A community-awareness release with a Windows installer is the day-one target; full parity is delivered through the agent pipeline against the ticket tree below.

Non-negotiables from the directive:
- **API-first.** Bind the `lore` Rust crate directly, in-process. The CLI is just another consumer; we do **not** shell out to it.
- **Full parity.** Every operation gets a binding, GUI affordance, and test.
- **Public MIT**, on BiloxiStudios, hosted on BRAINZ.
- Plan committed to the repo **and** mirrored under `/srv/studiobrain-dev/plans/loregui/`.

## 2. Why we bind the `lore` crate (not the C FFI, not the CLI)

The crate named **`lore`** (workspace member `lore/`) is the real API library. The C FFI (`lore-capi/lore.h`, 10k lines, cbindgen-generated) and the `lore` CLI binary (crate `lore-client`) are both thin consumers of it. Each domain is a module exposing one `pub async fn` per operation:

```rust
// lore/src/repository.rs (representative)
pub async fn status(
    global: LoreGlobalArgs,
    args: LoreRepositoryStatusArgs,
    callback: LoreEventCallbackConfig,   // streams LoreEvent values; terminates on Complete
) -> Result<(), LoreError> { ... }
```

Because our GUI is Rust (Tauri), we depend on `lore` as a Cargo git dependency and call these async fns directly — no marshaling, no subprocess, no C ABI. Events are collected by an adapter (§4) into typed view-model results.

## 3. Architecture

```
loregui/ (monorepo)
├── crates/lore-vm/        Reusable, GUI-agnostic core. Binds the `lore` crate.
│                          → also consumable by StudioBrain's desktop app later (model-manager pattern).
├── src-tauri/             Tauri v2 shell. One #[tauri::command] per lore-vm operation.
├── frontend/              The GUI (Vite + React + TS). Per-domain views + a command palette exposing ALL ops.
├── website/               Marketing landing (Next.js) — loregui.com. (Shipped.)
├── docs/                  This plan + per-domain design notes + API parity matrix.
└── .github/workflows/     CI: cargo check/test + Windows installer build (tauri-action, windows-latest).
```

### Layering
1. **`lore` crate** (upstream) — async ops + event callbacks.
2. **`lore-vm`** (ours) — for each op: a typed `Args` struct, an event-collector that turns the callback stream into a typed `Result`/`Vec<Event>`, and an async method on a `LoreApi` facade. **One file per operation** (see §5). This is the unit of parallel work.
3. **`src-tauri/commands/`** — one thin `#[tauri::command]` per op, forwarding to `lore-vm`. **One file per domain.**
4. **`frontend/`** — per-domain panels + a universal command palette (`Ctrl-K`) that can invoke any of the 124 ops with a generated form from the op's args schema.

> The earlier CLI-adapter scaffold is **removed**. `lore-vm` is reframed around the in-process `lore` crate binding. The `LoreBackend` trait is retained only as the `LoreApi` facade boundary.

## 4. The binding pattern (uniform across all 124 ops — agents follow this verbatim)

Every operation is implemented identically, which is what makes 124 parallel agents tractable and merge-safe:

```rust
// crates/lore-vm/src/ops/repository/status.rs   (one file per op)
use crate::collect::collect_events;          // shared event-stream → Vec<LoreEvent> + Result
use crate::model::RepoStatus;                // shared typed view-model

pub async fn status(api: &LoreApi, args: StatusArgs) -> Result<RepoStatus> {
    let (cb, rx) = collect_events();
    lore::repository::status(api.global(&args), args.into_lore(), cb).await?;
    RepoStatus::from_events(rx.events())     // map domain events → view-model
}
```

Shared infrastructure (built **once**, before fan-out, by the integration manager):
- `collect.rs` — the event-callback → typed-stream collector (`Complete`/`Error`/`Progress` handling, cancellation).
- `model.rs` — view-model types (already drafted: `RepoStatus`, `Branch`, `Revision`, `FileChange`, …); extended per domain.
- `global.rs` — `LoreGlobalArgs` builder (repository_path, identity, offline, force, parallelism limits).
- `api.rs` — `LoreApi` facade holding the open repo/working-dir + global-arg defaults.

A subtask is "done" when: op file compiles, its `#[tauri::command]` exists and is registered, a GUI affordance can invoke it, and a test exercises it against a throwaway local repo + shared-store (see §7).

## 5. Merge-conflict avoidance (critical for 124 parallel agents)

**One operation = one new file in each layer.** No two operation-tickets edit the same file. The only shared, append-mostly files are the registries, which are owned by the integration manager and updated at merge time:
- `crates/lore-vm/src/ops/<domain>/mod.rs` — `pub mod <op>;` lines (append-only; trivial conflicts).
- `src-tauri/src/lib.rs` `generate_handler![...]` — command registration (integration-manager-owned; agents propose, manager applies in merge order).
- `frontend` route/command-palette registry — generated from a manifest (agents add a manifest entry, not hand-edit a switch).

Agents must **never** reformat or touch files outside their op. PRs that do are bounced.

## 6. Ticket tree (Jira SBAI)

**Epic:** *LoreGUI — full-parity desktop GUI for Lore.*

**Stories (one per domain)** with subtask counts = operations to bind:

| Story (domain) | Ops | Notes |
|---|---:|---|
| Foundation & infra | — | crate binding, `collect`/`global`/`api`, CI, scaffolding. **Blocks all others.** Manager-owned. |
| Auth / session | 7 | login_interactive, login_with_token, user_info, local_user_info, list, logout, clear |
| Repository | 21 | clone, info, dump, create, create_with_metadata, delete, release, flush, gc, list, status, verify_state, verify_fragment, store_immutable_query, metadata_get/set/clear, instance_list, instance_prune, update_path, config_get |
| Branch | 22 | create, info, list, switch, push, diff, reset, archive, protect, unprotect, merge_start/into/resolve(_mine/_theirs)/unresolve/restart/abort, metadata_get/set/clear |
| Revision / commit | 31 | commit, amend, info, history, diff, find, sync, restore, revert(+resolve/unresolve/restart/abort variants), metadata_* |
| File / staging | 20 | stage(+move/merge), unstage, dirty(+move/copy), reset(+to_last_merged), obliterate, info, history, diff, write, hash, dump, metadata_*, dependency_add/remove/list |
| Locking | 5 | acquire, status, query, release (+ ignore/not_found events) |
| Link | 5 | add, remove, update, list (+ staged) |
| Layer | 4 | add, remove, list (+ staged) |
| Storage / fragment | 11 | open, close, flush, put, put_file, get, get_file, get_metadata, copy, obliterate, upload |
| Shared store | 3 | create, info, set_use_automatically |
| Service | 2 | start, stop |
| Notification | 2 | subscribe, unsubscribe |
| Dependency | 3 | add, remove, list (file-dep graph) |
| **Total** | **~124** | revision_tree (13 args/events) and cherry-pick/bisect are **stubbed/deferred** — args+events exist upstream but no exported fn yet; tracked as a spike. |

### Upstream Gaps (Lore)

The following Lore features are missing from upstream and are tracked as gaps:
- **Notification cursor/replay** ([SBAI-4135](https://studiobrain.atlassian.net/browse/SBAI-4135))
- **Chunked/range blob read** ([SBAI-4136](https://studiobrain.atlassian.net/browse/SBAI-4136))
- **TreeNode size/mtime/content-type** ([SBAI-4137](https://studiobrain.atlassian.net/browse/SBAI-4137))
- **Non-interactive service token issuance** ([SBAI-4138](https://studiobrain.atlassian.net/browse/SBAI-4138))

**Subtask template (every op ticket):**
> **SBAI-XXXX — [domain] op `<name>`**
> Implement `lore-vm::ops::<domain>::<name>` per the binding pattern (§4), add `#[tauri::command]`, add GUI affordance (panel action or command-palette entry), add integration test. One file per layer; do not touch registries (manager merges). Repo: `BiloxiStudios/loregui`. Branch `SBAI-XXXX-<domain>-<name>`. PR title `SBAI-XXXX: <domain> <name>`. Acceptance: compiles, command registered via manifest entry, test green against local repo+shared-store, no files outside the op touched.

## 7. Testing

- **Per-op integration test:** spin a temp dir, `repository create` + `shared-store create`, exercise the op, assert on collected events / resulting `status`. Helper `test_repo()` in `lore-vm` test support (manager-built before fan-out).
- **CI gates:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test -p lore-vm`, `cargo check -p loregui`, `npm --prefix frontend run build`. All green before merge.
- **Windows smoke:** the windows-build job must produce a launchable installer artifact.

## 8. CI / Windows build (the "shippable today" mechanism)

GitHub Actions, `windows-latest` runner, `tauri-apps/tauri-action` → NSIS `.exe` + MSI. A tagged release publishes the installer to GitHub Releases (public, since repo is public), which the website's "Download for Windows" button targets. Linux/macOS jobs added for cross-platform but Windows is the release gate today. (Cross-compiling Tauri from Linux is impractical; CI on a real Windows runner is the supported path.)

## 9. Pipeline workflow & integration-manager role

1. **Manager (me)** lands the Foundation Story first: crate dependency pinned, `collect`/`global`/`api`/`model`/test-support, the `ops/<domain>/mod.rs` skeletons, CI, and a stub file per op (so each agent has a precise file to fill). Until Foundation merges, op-tickets are `blocked`.
2. **Pipeline agents** claim op subtasks (per BrainMon claim protocol), implement against the stub, open one PR per ticket.
3. **Manager** reviews each PR: enforces one-file-per-op, runs CI, applies registry updates in a controlled merge order, resolves the only-expected (append) conflicts, merges. Invalid/incomplete work is bounced back with specifics or finished by the manager.
4. Domain Stories close when all their op subtasks merge + the domain's GUI panel is wired.
5. Release: tag → CI Windows installer → website links updated.

**Repo routing:** these SBAI tickets target `BiloxiStudios/loregui`, **not** the StudioBrain repos. The pipeline routing table must map this Epic's children to `loregui`. (Manager to confirm/add before mass dispatch.)

## 10. Milestones (today)

- **M0 Foundation** (manager): repo live, scaffold API-first, crate bound, CI green, one-file-per-op stubs generated, Epic+Stories+subtasks created. ← unblocks everyone
- **M1 Core loop** (priority subtasks): repository.status, file.stage/unstage, revision.commit, branch.push, revision.sync, branch.create/switch/list, revision.history, lock.acquire/release — wired to real GUI. This is the demoable, shippable Windows build.
- **M2 Fan-out**: remaining ops via pipeline, merged continuously.
- **M3 Parity**: all domains green; revision_tree/cherry-pick/bisect spike resolved.

## 11. Risks

- **Pre-1.0 upstream churn.** `lore` is 0.x and `publish = false`. We pin a git rev; bumping is a deliberate, manager-owned PR. API may break.
- **`lore` builds on Windows CI.** It targets Windows (winresource, windows-sys) so this should hold, but the first CI run is the proof. Has integration crates (AWS/HashiCorp) — keep default features minimal.
- **124 parallel PRs** → merge throughput is the bottleneck, not authoring. The one-file-per-op rule and manifest-driven registries are the mitigation; manager serializes only the registry merges.
- **`edition = "2024"`** upstream → our crates must use a new enough toolchain.
- **Trademark/positioning.** Community project; clear "not affiliated with Epic" disclaimer (done on website, add to repo README).
