# Lore Source Control for VS Code

`loregui-lore` — native VS Code source control for Epic Games'
[`lore`](https://github.com/EpicGames/lore) VCS, driven by the **loregui
`lorevm` engine** (not Epic's public `lore` CLI). It plugs lore into VS Code's
built-in SCM view the same way the bundled Git extension does: a "Changes" /
"Staged Changes" tree, inline stage/unstage, a commit box, per-file diff and
history, branch sync, and lock-awareness badges.

This is the **free, open-core (MIT)** lore-SCM layer. The StudioBrain
entity-aware premium layer (template-driven validation, cross-reference
decorations, asset previews) is a later gated addon that reuses this
extension's `LorevmClient` — see the `PREMIUM SEAM` markers in
`src/extension.ts`.

> Part of [loregui](https://github.com/BiloxiStudios/loregui). Tracking ticket:
> **SBAI-4080**.

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

### Binary resolution (mirrors lore-mcp)

`lorevm` is located in this order:

1. The `lore.lorevmPath` setting, if set.
2. The `LOREVM_BIN` environment variable.
3. `lorevm` on `PATH`.
4. `target/{release,debug}/lorevm` under the workspace folder or any ancestor
   that is a loregui checkout (and `$LOREGUI_DIR` if set).

If it can't be found, the extension shows a one-time warning telling you to
build it (`cargo build -p lorevm-cli`) or set the path. The view stays inert
rather than erroring repeatedly.

## SCM features

| Feature | Command | lorevm op |
|---|---|---|
| Refresh status | `lore.refresh` | `repository.status` (`scan:true`) |
| Stage file(s) | `lore.stage` | `file.stage` |
| Unstage file(s) | `lore.unstage` | `file.unstage` |
| Commit | `lore.commit` | `revision.commit` |
| Diff a file | `lore.openDiff` | `file.diff` |
| File history | `lore.fileHistory` | `file.history` |
| Sync | `lore.sync` | `revision.sync` |
| Lock badges | (decoration) | `lock.file_status` |

- **Changes / Staged Changes groups** are populated from `repository.status`.
  Each file is decorated A/M/D (add/modified/deleted, with rename/copy and
  conflict variants) from the op's `action` + `conflict` fields.
- **Refresh** runs on a debounced filesystem watcher (toggle with
  `lore.autoRefresh`) and via the SCM title refresh button / command palette.
- **Commit** reads the SCM input box (falling back to an input prompt) and runs
  `revision.commit`, then clears the box and refreshes.
- **Diff** renders the engine's native unified patch (`file.diff`) in a
  read-only `lore-doc:` virtual document with `diff` syntax highlighting. A
  quick-diff provider also marks the gutter against the current revision.
- **File history** lists revisions in a quick-pick (`file.history`).
- **Sync** runs `revision.sync` and reports files updated/deleted.
- **Lock awareness** decorates files with an `L` badge — themed one way for
  *locked by you* and another for *locked by `<owner>`* — using
  `lock.file_status` for the changed paths on the current branch. (Best-effort;
  silently skipped when no lock service / remote is reachable.)

### Lock requests (stubbed — SBAI-4044)

The `lore.requestLock` command is a **clearly-marked stub**. Acquiring a lock
another user holds needs a cross-network "request from the owner → tray
message → reply" round trip, which depends on **SBAI-4044**. Until then the
command just explains the limitation; acquiring an *unheld* lock via
`lock.file_acquire` still works at the engine level.

## Configuration

| Setting | Default | Meaning |
|---|---|---|
| `lore.lorevmPath` | `""` | Explicit path to `lorevm`. |
| `lore.offline` | `true` | Pass `--offline` (local repos with no remote). |
| `lore.identity` | `""` | `--identity` value; also used for locked-by-me detection. |
| `lore.autoRefresh` | `true` | Refresh on workspace file changes. |

## Develop / debug (F5)

1. `cd vscode-extension && npm install`
2. `npm run compile` (or `npm run watch` for incremental rebuilds)
3. Build the engine once: from the loregui repo root,
   `cargo build -p lorevm-cli` (produces `target/debug/lorevm`).
4. Open the `vscode-extension/` folder in VS Code and press **F5** (the
   "Run Extension" launch config). This opens an **Extension Development Host**
   window.
5. In that window, open a folder that is a lore repo (or create one:
   `lorevm repository.create --dir <dir> --offline --identity me --args '{"repository_url":"lore://localhost/x"}'`).
   The **Source Control** view shows a "Lore" provider. With
   `lore.offline` on (default), a purely local repo works with no remote.

If `lorevm` isn't on `PATH`, set `LOREVM_BIN` in the launch env or the
`lore.lorevmPath` setting — the dev host inherits the parent VS Code's
environment.

## Package a `.vsix`

```sh
cd vscode-extension
npm install
npm run compile
npx @vscode/vsce package        # → loregui-lore-0.1.0.vsix
```

Install it with `code --install-extension loregui-lore-0.1.0.vsix`.

## License

MIT © Biloxi Studios Inc. See [LICENSE](./LICENSE).
