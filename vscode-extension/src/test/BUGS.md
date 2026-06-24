# Lore VS Code extension — bug catalog (surfaced by the E2E harness)

This is the running catalog the `src/test/suite/*.test.js` E2E harness either
guards against or surfaced while being built. Reproduce any of these with the
checked-in `lorevm` CLI; the harness exercises the real extension + a real
`.lore` repo headless (`xvfb-run -a npm test`).

The bug tests live in `suite/knownBugs.test.ts`. BUG#1/#2 are `test.skip`
(pending) so CI stays green on current behavior while keeping the bug visible;
flip `.skip` → `test` to make them live regression guards once fixed.

---

## BUG #1 — `file.stage` silently no-ops on repo-relative paths  (P0) — FIXED (SBAI-4080)

**FIXED in the engine.** `FileStageArgs::into_lore` in
`crates/lore-vm/src/ops/file/stage.rs` now resolves every relative path against
the repository root (`--dir` / `api.globals().repository_path`) before handing it
to `lore::file::stage`; absolute paths pass through unchanged. Fixes the CLI,
MCP, VS Code extension, and UE plugin at once. The now-LIVE `knownBugs.test.ts`
BUG#1/#2 guards (flipped from `test.skip` → `test`) plus engine unit tests
(`stage_args_resolves_relative_paths_against_repo_root`,
`stage_args_passes_absolute_paths_through`) guard against regression.

**The dominant manual bug (original report).** The extension stages files using **repo-relative**
paths (`extension.ts` → `resolveResourceTargets()` → `path.relative(repoRoot, uri)`).
The engine's `file.stage` only stages when given **absolute** paths; with a
relative path it returns `{"files": [], "revision": ""}` — a silent success that
stages **nothing**.

Reproduce:
```sh
REPO=$(mktemp -d); lorevm repository.create --dir "$REPO" --offline \
  --args '{"repository_url":"lore://localhost/x"}'
echo hi > "$REPO/rel.txt"

# Relative path — what the extension sends. Stages NOTHING:
lorevm file.stage --dir "$REPO" --offline --args '{"paths":["rel.txt"],"scan":true}'
#   => {"files": [], "revision": ""}

# Absolute path — actually stages:
lorevm file.stage --dir "$REPO" --offline --args "{\"paths\":[\"$REPO/rel.txt\"],\"scan\":true}"
#   => {"files":[{"action":"add","path":"rel.txt",...}], "revision":"<hash>"}
```

**User-visible effect:** clicking "+" (stage) in the VS Code SCM view appears to
do nothing, and a subsequent commit fails with **"Nothing staged for commit"**.

**Fix options (pick one):**
- **Engine:** `file.stage` should resolve relative `paths` against `--dir`
  (the repo root) before handing them to the lore stage call. This is the
  correct fix — every external driver (CLI, MCP, VS Code, UE) sends repo-relative
  paths today and silently mis-stages.
- **Extension (workaround):** have `resolveResourceTargets()` emit absolute
  `uri.fsPath` paths instead of `path.relative(...)`. Cheap, unblocks the UI, but
  leaves the trap for every other driver.

Guard: `knownBugs.test.ts` → BUG#1 (pending).

---

## BUG #2 — UI stage→commit flow fails end to end (consequence of #1)  (P0)

The exact extension flow — `file.stage(['relative.txt'])` then
`revision.commit` — fails because #1 staged nothing:
```sh
lorevm file.stage   --dir "$REPO" --offline --args '{"paths":["a.txt"],"scan":true}'
lorevm revision.commit --dir "$REPO" --offline --args '{"message":"x"}'
#   => {"error":{"kind":"CommandFailed","message":"Nothing staged for commit"}}
```
Fixing #1 fixes #2. Guard: `knownBugs.test.ts` → BUG#2 (pending). The harness's
own `seedWorkspace.ts` works around #1 by staging **absolute** paths to build its
committed baseline — proof the absolute path stages and commits cleanly.

---

## BUG #3 — cross-process flush (SBAI-4080): now WORKING, guard added

Historically, `stage` in one `lorevm` process was invisible to a separate
`commit` process (the deferred mutable-store flush was aborted on runtime drop),
yielding "Nothing staged for commit". The SBAI-4080 fix (`finalize()` →
`repository.flush`, `crates/lore-vm/src/dispatch.rs`) drains the flush
synchronously and **works** when stage is given absolute paths: a separate-process
`stage(abs) → commit → status` round trip persists and leaves a clean tree, and a
second modify→stage→commit cycle also persists.

> **Stale-binary caveat (found during this work):** the checked-in
> `vscode-extension/bin/lorevm` and `target/release/lorevm` on this machine were
> built ~3 min BEFORE the SBAI-4080 fix commit `c4f72c1`, so they do **not**
> contain the flush fix. The harness rebuilds via `cargo build --release -p
> lorevm-cli` (CI does this) and points `LOREVM_BIN` at the fresh binary. **Action:
> rebuild + re-bundle `bin/lorevm` before the next Marketplace publish**, or
> shipped users get the pre-fix flush bug. Guard: `knownBugs.test.ts` → BUG#3
> (active, passing).

---

## BUG #5 — SCM view empty after an EDITOR edit (real-flow SCM bug)  (P0) — FIXED (SBAI-4080, vscode 0.2.3)

**FIXED in the extension.** The 0.2.1 flush + 0.2.2 relative-path fixes were
validated against a CLEAN CLI-created scratch repo and missed how a real user
drives the extension: they **edit a tracked file in the VS Code editor and save**,
not via the CLI.

Two engine facts make the difference:
- `repository.status` only reports editor-edited working-tree changes (modified
  tracked + untracked) when **`scan = true`**. The extension already polls
  `status { scan: true }`, so the engine side was fine — the gap was entirely in
  WHEN the extension refreshed.
- The SCM groups are only rebuilt on `refresh()`, and `refresh()` was driven
  **only** by an OS `FileSystemWatcher`. That watcher does NOT reliably fire for
  in-editor saves (safe-save = atomic write-to-temp + rename, network/virtual
  filesystems, event coalescing), so after editing+saving a tracked file the SCM
  "Changes" group stayed empty until something else triggered a refresh.

**Fix (`extension.ts` `setupWatcher`):** also refresh on the editor's own
document events — `onDidSaveTextDocument`, `onDidCreateFiles`, `onDidDeleteFiles`,
`onDidRenameFiles` (scoped to this repo's working tree, ignoring `.lore/`). These
fire for the exact buffers the user touched, independent of the fs watcher.

Guards: `scm.test.ts` → "editing a tracked file in the editor + saving surfaces
it as a working-tree change" (drives open → edit → save → assert the engine
status the SCM groups are built from lists it). Engine contract guarded by
`crates/lore-vm/tests/stage_real_flow.rs` →
`status_scan_lists_editor_edited_and_untracked_files`.

---

## BUG #6 — `file.stage` permanently broken by a dangling staged anchor  (P0) — FIXED (SBAI-4080, vscode 0.2.3)

**FIXED in the engine (`crates/lore-vm/src/ops/file/stage.rs`).** The user's
report — staging a file fails with `Lore: CommandFailed: Failed to deserialize
staged state: Failed to read state data` — is a **persistent** corruption, not a
one-shot flush race.

Root cause: every `file.stage` first deserialises the *pre-existing* staged state
(upstream `State::deserialize_current_and_staged`). The cross-process flush is
ordered fragment-then-anchor but the upstream post-command flush swallows errors
and still writes the mutable **anchor** even if the immutable **state fragment**
flush failed (upstream `lore-revision/src/repository.rs:637-642`,
`let _ = immutable.flush(); let _ = mutable.flush();`). That leaves an anchor in
the mutable store pointing at a staged revision whose state fragment is missing
from the immutable store. From then on **every** stage (and `status`, branch
switch, sync) fails to read it — the SCM "stage" button is dead forever. The
literal `Failed to read state data` surfaces when the durable/remote read errors
with a non-not-found class; a local-only store surfaces the same dangling anchor
as a bare `Not found`.

The only thing that clears a dangling anchor is dropping it before any
deserialize touches it: `repository.status` with `reset = true` calls
`delete_staged_anchor` up front. So `file::stage` now **self-heals**: a stage that
fails with the dangling-anchor signature drops the bad anchor and retries once
against a clean staged state, re-staging the file the user edited. A repo that was
permanently stuck recovers transparently on the user's next stage. The retry is
gated to that specific signature, so genuine stage errors (bad path, conflict)
still surface immediately.

Guards: `knownBugs.test.ts` → "BUG#5: file.stage self-heals a dangling staged
anchor (real cross-process flow)" (separate `lorevm` processes, deletes the
on-disk fragment to reproduce the corruption, asserts recovery + commit). Engine
unit + cross-process coverage in `crates/lore-vm/tests/stage_real_flow.rs`
(`dangling_anchor_signatures_are_recognized`,
`dangling_anchor_self_heals_across_processes`).

> **Upstream follow-up (not fixed here):** the swallowed-error, anchor-after-
> failed-fragment ordering lives in the vendored `lore` engine and should be
> reported upstream (flush immutable; only persist the anchor if the fragment
> flush succeeded). Our self-heal recovers any repo that already hit it.

---

## BUG #4 — Locks view / lock decorations dead on local (offline) repos  (P2) — FIXED (SBAI-4080)

**FIXED.** When `lock.file_query` fails with "No remote configured", the
`LocksTreeProvider` now sets the `lore.locksNoRemote` context key, which gates a
`viewsWelcome` entry ("File locks require a connected lore server …") so the
empty Locks view is explained instead of looking broken. The lock-badge
decorations remain a non-fatal no-op on local repos (correct — there are no
locks). The key is cleared as soon as a lock query succeeds.

On a purely-local/offline repo (`lore.offline=true`, no remote), every lock op
fails with `{"error":{"kind":"CommandFailed","message":"No remote configured"}}`:
- `lock.file_query` (the Locks tree view source)
- `lock.file_status` (the file-decoration / lock-badge source)

The extension wraps both in try/catch and treats them as non-fatal (good — the
SCM view and refresh keep working). But the **Locks view is permanently empty and
lock badges never appear** for local repos, with no in-UI hint that locks need a
remote. Not a crash, but a silent dead feature.

**Suggested fix:** when the lock service reports "No remote configured", show a
one-line `viewsWelcome` in the Locks view ("Locks require a connected lore
server") instead of an empty tree, so the emptiness is explained rather than
looking broken.

Guard: `scm.test.ts` → "Locks view degrades gracefully…" asserts the documented
failure mode + that refresh survives it.

---

## Notes / non-bugs observed

- **`branch.create` auto-switches** the current branch (lore branches stack; the
  new branch becomes `is_current`). This is expected lore behavior, but the
  extension's `lore.branchCreate` doesn't tell the user they've switched — minor
  UX gap, not filed as a bug.
- **`revision.revert` / `file.reset` are not in the dispatch table**, so the
  extension's Discard = unstage only and Revert = sync-with-reset. The extension
  already documents this in-code; the harness does not assert a true per-file
  working-tree reset because the engine surface doesn't expose one.
