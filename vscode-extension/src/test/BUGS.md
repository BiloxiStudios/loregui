# Lore VS Code extension — bug catalog (surfaced by the E2E harness)

This is the running catalog the `src/test/suite/*.test.js` E2E harness either
guards against or surfaced while being built. Reproduce any of these with the
checked-in `lorevm` CLI; the harness exercises the real extension + a real
`.lore` repo headless (`xvfb-run -a npm test`).

The bug tests live in `suite/knownBugs.test.ts`. BUG#1/#2 are `test.skip`
(pending) so CI stays green on current behavior while keeping the bug visible;
flip `.skip` → `test` to make them live regression guards once fixed.

---

## BUG #1 — `file.stage` silently no-ops on repo-relative paths  (P0)

**The dominant manual bug.** The extension stages files using **repo-relative**
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

## BUG #4 — Locks view / lock decorations dead on local (offline) repos  (P2)

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
