---
name: loregui
description: The single entry point for getting an agent (or a person) productive with LoreGUI — the cross-platform desktop GUI + MCP toolkit for Epic's lore VCS. Use this to INSTALL or build LoreGUI, set up and register the lore-mcp server, configure a repo (connect to a server or host one), and then drive/manage lore. Point a new agent here and it can install, configure, and operate the whole stack. For the lore VCS mental model + per-op semantics, defer to the [[lore]] skill.
---

# LoreGUI — install, configure, and drive lore (agent runbook)

LoreGUI is a Tauri (Rust + React) desktop app that binds Epic's **lore** VCS
**in-process** (the `lore-vm` crate, ~136 ops), plus an **MCP server** so AI agents
drive the same ops. Three pieces, one repo (`github.com/BiloxiStudios/loregui`):

| Piece | What | Where |
|---|---|---|
| **Desktop app** | The GUI (palette + panels) | installers on GitHub Releases / `cargo tauri build` |
| **`lorevm` bin** | Thin JSON CLI over the in-process ops | `crates/lorevm-cli` → `target/{debug,release}/lorevm` |
| **`lore-mcp`** | Python MCP server: one tool per op, schemas from the palette manifests | `lore-mcp/` (server.py + venv) |

The companion **[[lore]] skill** explains the VCS itself (revisions/branches/staging/
locks/git↔lore translation). This skill is the *operational* runbook.

## 1. Install / run the desktop app

**Easiest — download a signed installer** from the rolling current build:
- Windows `.exe`/`.msi`, Linux `.deb`/`.AppImage`/`.rpm`: <https://github.com/BiloxiStudios/loregui/releases> (the `nightly` build is the latest `main`). macOS `.dmg`: when signing is enabled.

**From source:**
```sh
git clone https://github.com/BiloxiStudios/loregui && cd loregui
cargo tauri build            # produces installers under target/release/bundle/
# dev loop:  cargo tauri dev
```
> ⚠️ Use `cargo tauri build`, NOT `cargo build -p loregui` — the latter skips the
> frontend build (`beforeBuildCommand`), so the binary embeds the dev URL and the
> window shows "connection refused". `cargo tauri build [--no-bundle]` is correct.
> Headless test box: run under `xvfb-run` (DISPLAY :0 may be a locked session).

## 2. Set up the MCP server (so an agent can drive lore)

```sh
cd /path/to/loregui
cargo build -p lorevm-cli                          # → target/debug/lorevm
python3 -m venv lore-mcp/venv
lore-mcp/venv/bin/pip install -r lore-mcp/requirements.txt
lore-mcp/venv/bin/python lore-mcp/server.py --list # sanity: lists ~22 tools
```
> Windows: the venv layout is `lore-mcp\venv\Scripts\` not `bin\` — use
> `lore-mcp\venv\Scripts\pip.exe` / `...\Scripts\python.exe` throughout.

**Register it** in the agent's MCP config (Claude Code / Codex `mcp_servers`):
```json
"lore": {
  "command": "/path/to/loregui/lore-mcp/venv/bin/python",
  "args": ["/path/to/loregui/lore-mcp/server.py"],
  "env": { "LORE_REPO": "/path/to/a/lore/repo",
           "LOREVM_BIN": "/path/to/loregui/target/debug/lorevm",
           "LORE_OFFLINE": "1" }
}
```
- `LOREVM_BIN` auto-resolves from the loregui `target/` if unset; `LORE_REPO` is the
  default repo (a tool's `repo` arg overrides per-call). The server self-locates the
  loregui repo from its own path. The pipeline registers this in
  `/opt/BrainMon/config/codex-config.toml.template` (a reference, not a copy).
- Regenerate the tool catalog after lore-vm op changes:
  `lore-mcp/venv/bin/python lore-mcp/generate_catalog.py`.

## 3. Configure a repository

In the **app** (onboarding picks the mode):
- **Connect to a server (client):** auth `login_interactive(url)` → pick/clone a repo.
- **Host a server:** shared_store `create` → repository `create` → service `start`
  (storage backend: local packfiles or S3/MinIO/Garage).

For **agents/headless**, point `LORE_REPO` at an existing on-disk repo. To create one
programmatically, drive the lore-vm ops (see `crates/lore-vm/tests/integration_roundtrip.rs`):
`shared_store::create` → `repository::create` → `file::stage` → `revision::commit`.

## 4. Drive & manage lore

- **Agents:** call the `lore` MCP tools — `repository.status`, `revision.history`,
  `revision.diff`, `branch.list`, `file.stage`/`unstage`, `file.history`, `lock.*`,
  plus `lore_repo_summary`. Read/metrics tools are fully solid; see [[lore]] for the
  git/p4↔lore mapping and op semantics.
- **People:** the GUI — ⌘K command palette (every op via a generated form) + the
  Storage / Manage / Locks / Dependencies / History / Branches / Account panels.

## 5. Launch at login?

The app has a working **Settings → Account → "Start LoreGUI at login"**
toggle (`tauri-plugin-autostart`) plus "close to tray instead of quitting."
Ask the user if they want this enabled rather than assuming either way — it's
a one-click in-app setting, no OS-level Registry/systemd hack needed.

## 6. Known limits / gotchas

- **Offline staging isn't cross-process:** in `LORE_OFFLINE=1`, staging lives in
  process-local memory, so a `file.stage` in one `lorevm` call isn't visible to a
  `commit` in a *separate* call. Read/metrics + single-call ops are unaffected;
  multi-step write workflows (stage→commit) need a connected repo. (Write/merge/sync
  ops are extensible — one dispatch arm each in `crates/lorevm-cli`.)
- **`lock.*` / auth ops** require a connected server (return "needs a server" offline).
- Build/run gotchas: see §1 (`cargo tauri build`, xvfb).

## 7. Pointers
- VCS semantics & op surface → **[[lore]] skill**.
- User guide + screenshots → `website/` `/guide` + `website/public/screenshots/`.
- Architecture & coherence rules → repo `CLAUDE.md`, `docs/`.
- Safety: `obliterate`/`delete`/`gc`/`reset`/branch `unprotect` are destructive —
  confirm before running. Never log auth tokens.
