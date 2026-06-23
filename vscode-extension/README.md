# Lore Source Control for VS Code

**`loregui-lore`** — full, native source control for Epic Games'
[`lore`](https://github.com/EpicGames/lore) VCS, driven by the **loregui
`lorevm` engine** (not Epic's public `lore` CLI). It plugs lore into VS Code the
way the bundled Git extension plugs in Git — and then goes further, with a
dedicated **Lore** activity-bar with Branches, History, and Locks tree views.

> Docs & marketplace listing: **https://loregui.com/docs/vscode**
> Part of [loregui](https://github.com/BiloxiStudios/loregui). Tracking ticket: **SBAI-4080**.

---

## What you get

A Perforce-class experience for lore, entirely inside VS Code:

- **Native Source Control view** — `Staged Changes` / `Changes` groups with
  A / M / D / move / copy / conflict decorations, a commit (check-in) input box,
  and inline + context-menu actions on every file.
- **Stage / unstage / discard** per file or for the whole group.
- **Check in** (commit), **Sync (pull)**, **Push**, and **Revert to a revision**
  from the Source Control title bar or the command palette.
- **Side-by-side diff** (working ↔ committed baseline, or working ↔ any chosen
  revision) plus the **quick-diff gutter** that marks edited lines live.
- **Lore activity bar** with three tree views:
  - **Branches** — list, switch (inline), and create branches.
  - **History** — the branch revision log with revision number, message, and
    author; click a revision to open its overview (metadata + changed files).
  - **Locks** — who holds which file lock on the current branch; release your
    own locks inline, or request one held by someone else.
- **Status bar** — current branch + revision + a dirty indicator; click to Sync.
- **File decorations in the Explorer** — an `L` badge on locked files, themed one
  way for *locked by you* and another for *locked by someone else*.
- **First-run walkthrough** and an in-editor **Open Documentation** command.

---

## Quick start

1. **Install** the extension (or build the `.vsix` — see below).
2. **Connect the engine.** The extension drives the `lorevm` binary. It is
   resolved in this order: the `lore.lorevmPath` setting → the `LOREVM_BIN`
   environment variable → `lorevm` on your `PATH` → a loregui
   `target/{release,debug}/lorevm` build under the workspace or an ancestor →
   the binary bundled inside the extension's `bin/` folder. If none is found the
   Source Control view still appears, with a clear *"lorevm not found — set
   lore.lorevmPath"* message in the input box and status bar.
   To build it: `cargo build -p lorevm-cli` in the loregui repo.
3. **Open a lore repo folder** (one containing a `.lore` directory). The
   **Source Control** view shows a **Lore** provider; the **Lore** icon appears
   in the Activity Bar.
4. Run **`Lore: Getting Started Walkthrough`** from the command palette for a
   guided tour.

---

## Everyday workflow

| You want to… | Do this |
|---|---|
| See what changed | Open the **Source Control** view (the Lore provider). |
| Stage a file | Hover the file in **Changes** → click **＋**, or right-click → **Stage Changes**. |
| Stage everything | Hover the **Changes** group header → click **＋**. |
| Discard local edits | Right-click a file in **Changes** → **Discard Changes** (confirmed). |
| Check in (commit) | Type a message in the input box → **Ctrl/Cmd+Enter** (or the ✓ in the title bar). |
| Pull updates | Title-bar **Sync (Pull)** / `Lore: Sync (Pull)`. |
| Publish revisions | Title-bar **Push** / `Lore: Push`. |
| Revert the tree | `Lore: Revert to Revision…` → pick a revision. |
| Diff a file | Click the file in the SCM view, or right-click → **Open Changes**. |
| Diff against an old revision | Right-click → **Compare with Revision…**. |
| Browse a file's history | Right-click → **View File History** → pick a revision to diff. |
| Switch / create a branch | **Lore** activity bar → **Branches** (inline switch; ＋ to create). |
| Browse the log | **Lore** activity bar → **History**; click a revision for its overview. |
| See / manage locks | **Lore** activity bar → **Locks**; release your own inline. |
| Lock a file | Right-click a file in the SCM view → **Acquire Lock**. |

---

## How it drives lore

The extension never reimplements lore logic. It spawns the **`lorevm` JSON CLI**
(`crates/lorevm-cli` in the loregui repo) — the exact same contract the
[`lore-mcp`](../lore-mcp) server uses:

```
lorevm <domain>.<op> --dir <workspace> [--offline] [--identity <id>] --args '<json>'
```

`lorevm` binds the upstream `lore` engine **in-process**. The extension's
`LorevmClient` (`src/lorevmClient.ts`) is a thin wrapper: spawn → parse JSON →
raise a structured `LorevmError` on `{"error": {...}}`.

### Op mapping (≥ Perforce coverage)

| Feature | Command | lorevm op |
|---|---|---|
| Status / refresh | `lore.refresh` | `repository.status` (`scan:true`) |
| Stage / stage all | `lore.stage` / `lore.stageAll` | `file.stage` |
| Unstage / unstage all | `lore.unstage` / `lore.unstageAll` | `file.unstage` |
| Discard | `lore.discard` | `file.reset` |
| Check in (commit) | `lore.commit` | `revision.commit` |
| Diff / diff vs revision | `lore.openDiff` / `lore.diffWithRevision` | `file.diff` |
| File history | `lore.fileHistory` | `file.history` |
| Sync (pull) | `lore.sync` | `revision.sync` |
| Push | `lore.push` | `branch.push` |
| Revert | `lore.revert` | `revision.revert` |
| Branch list | `lore.branchList` / Branches view | `branch.list` |
| Branch create | `lore.branchCreate` | `branch.create` |
| Branch switch | `lore.branchSwitch` | `branch.switch` |
| Revision log + overview | History view / `lore.openRevisionDiff` | `revision.history` + `revision.info` |
| Lock badges / list | Locks view, decorations | `lock.file_status`, `lock.file_query` |
| Acquire / release lock | `lore.lockAcquire` / `lore.lockRelease` | `lock.file_acquire`, `lock.file_release` |

### Lock requests (stubbed — SBAI-4044)

`Lore: Request Lock from Owner` is a **clearly-marked stub**. Acquiring a lock
another user holds needs a cross-network "request → tray message → reply" round
trip, which depends on **SBAI-4044**. Until then the command explains the
limitation; acquiring an *unheld* lock via **Acquire Lock** works today.

---

## Configuration

| Setting | Default | Meaning |
|---|---|---|
| `lore.lorevmPath` | `""` | Explicit path to `lorevm`. |
| `lore.offline` | `true` | Pass `--offline` (local repos with no remote). |
| `lore.identity` | `""` | `--identity` value; also used for locked-by-me detection and releasing your own locks. |
| `lore.autoRefresh` | `true` | Refresh the views on workspace file changes. |
| `lore.docs` | `https://loregui.com/docs/vscode` | Target of **Lore: Open Documentation**. |

The settings page links to the docs, and **`Lore: Open Documentation`** opens
them from anywhere.

---

## Why the Source Control view always appears now

Earlier builds only registered the SCM provider *after* a successful status
shell-out, so a missing engine or any status error left the view invisible and
the failure silent. This build detects a lore repo by the presence of a `.lore`
directory (a cheap filesystem check, no binary required), **registers the
provider unconditionally**, and surfaces a clear *"lorevm not found"* state if
the engine can't be resolved — then layers status in best-effort on top.

---

## Develop / debug (F5)

1. `cd vscode-extension && npm install`
2. `npm run compile` (or `npm run watch` for incremental rebuilds)
3. Build the engine once, from the loregui repo root: `cargo build -p lorevm-cli`
   (produces `target/debug/lorevm`).
4. Open the `vscode-extension/` folder in VS Code and press **F5** (the
   "Run Extension" launch config). This opens an **Extension Development Host**.
5. In that window, open a folder that is a lore repo (a `.lore` directory). The
   **Source Control** view shows the **Lore** provider; the **Lore** activity
   bar appears. With `lore.offline` on (default), a purely local repo works with
   no remote.

If `lorevm` isn't on `PATH`, set `LOREVM_BIN` in the launch env or the
`lore.lorevmPath` setting — the dev host inherits the parent VS Code's
environment.

## Package a `.vsix`

```sh
cd vscode-extension
npm install
npm run compile
# Bundle a per-platform lorevm (CI) — or ship runtime-resolved:
LOREVM_BUNDLE_OPTIONAL=1 npx @vscode/vsce package   # → loregui-lore-0.2.0.vsix
```

Install it with `code --install-extension loregui-lore-0.2.0.vsix`.

---

## Open-core seam

This is the lore-SCM layer. The StudioBrain entity-aware premium layer
(template-driven validation, cross-reference decorations, asset previews) is a
later gated addon that reuses this extension's `LorevmClient` — see the
`PREMIUM SEAM` markers in `src/extension.ts`.

## License

**Proprietary.** Copyright © 2026 Biloxi Studios Inc. All Rights Reserved.
See [LICENSE.txt](./LICENSE.txt). This extension is published inside the
otherwise MIT-licensed `loregui` repository for development convenience; that
placement does not make the extension MIT — `LICENSE.txt` governs the
`vscode-extension/` directory.
