# AGENT-SETUP.md — LoreGUI + lore-mcp bootstrap runbook

**Audience:** AI agents following this document end-to-end.
**Goal:** get the user a fully working setup — the **desktop GUI app installed
and launching**, the `lorevm` binary built, the `lore-mcp` Python server set
up and registered with your agent host, a **repository configured** (local,
connected to an existing server, or a newly hosted server with storage
chosen), **login autostart** decided, and the whole chain verified.

Follow the steps in order. Each step gives exact commands and expected outcomes.
If a command fails, check the "What can go wrong" note under that step before
retrying. **Do not skip Step 2** — it is the step most likely to be skipped,
and skipping it is what leaves the user without an app to open.

---

## Step 1 — Get the LoreGUI repo

```sh
git clone https://github.com/BiloxiStudios/loregui
cd loregui
```

You need this checkout regardless of which path you take below — it's where
the `lorevm-cli` build and the `lore-mcp` server live even if you install the
GUI from a pre-built installer.

---

## Step 2 — Install the Desktop GUI app

**This step is not optional and not "GUI only" — do it every time**, even if
the user primarily wants agent/MCP access. A user who asked for LoreGUI
expects an app they can open; the MCP server alone does not deliver that.

### Default path: download the signed installer

Pre-built installers are on the GitHub Releases page (use the latest tagged
release, e.g. `v0.1.3`; the rolling `nightly` release tracks `main`):
<https://github.com/BiloxiStudios/loregui/releases>

| Platform | Asset |
|---|---|
| Windows | `LoreGUI_<version>_x64-setup.exe` (NSIS) or `LoreGUI_<version>_x64_en-US.msi` |
| Linux | `LoreGUI_<version>_amd64.AppImage`, `.deb`, or the `.rpm` |
| macOS | `LoreGUI_<version>_aarch64.dmg` (when signing is enabled) |

Download the asset matching the user's OS and **run it** (on Windows this
means launching the `.exe`/`.msi`, not just downloading it — confirm with the
user before running an installer with elevated/system-wide effects, same as
any other software install). This creates a Start Menu / Applications entry.

### Alternative: build from source (only if the user explicitly wants a dev/from-source build)

This is heavier — it needs the Tauri CLI plus the frontend (Node) toolchain in
addition to Rust, and takes longer than grabbing the installer.

```sh
cargo tauri build            # produces installers under target/release/bundle/
# dev loop instead of a full build:  cargo tauri dev
```

> ⚠️ Use `cargo tauri build`, **not** `cargo build -p loregui` — the latter
> skips the frontend build (`beforeBuildCommand`), so the resulting binary
> embeds the dev URL and the window shows "connection refused" instead of the
> app. On a headless build box (no display), run under `xvfb-run`.

### Verify the GUI actually launches

Open the installed app (Start Menu entry on Windows, Applications on
macOS/Linux) and confirm a window appears. **This is the check that was
missing before** — a working MCP tool catalog later in this doc does not
prove the app exists. If no window appears, stop and debug the install before
continuing; don't silently fall back to CLI-only.

---

## Step 3 — Build the `lorevm` binary

`lorevm` is a thin JSON CLI that calls the in-process `lore-vm` ops. The
`lore-mcp` server shells out to it for every tool call. This is needed even
if you already installed the GUI in Step 2 — the GUI does not expose this CLI.

```sh
# From the root of the loregui checkout:
cargo build --release -p lorevm-cli
```

The binary lands at:

```
<loregui-root>/target/release/lorevm
```

(On Windows this is `lorevm.exe`.)

For a faster (unoptimised) debug build during development:

```sh
cargo build -p lorevm-cli
# binary: <loregui-root>/target/debug/lorevm
```

Smoke-test the binary:

```sh
# Print every dispatchable op id (should list ~20+ ops):
./target/release/lorevm --list

# Or: check usage
./target/release/lorevm --help
```

Expected output: a list of `<domain>.<op>` ids such as `repository.status`,
`revision.history`, `branch.list`, etc.

> **What can go wrong:** `cargo` not found — install Rust via
> <https://rustup.rs>. On a headless build box, the `lore-vm` crate links the
> upstream lore engine in-process; it does not require `lore` to be installed
> separately.

---

## Step 4 — Set up the lore-mcp Python server

The MCP server lives in `lore-mcp/` inside the repo. It needs its own virtual
environment.

```sh
# From the loregui root:
python3 -m venv lore-mcp/venv
```

Install dependencies — the venv's interpreter path is **OS-dependent**:

```sh
# Linux / macOS:
lore-mcp/venv/bin/pip install -r lore-mcp/requirements.txt

# Windows:
lore-mcp\venv\Scripts\pip.exe install -r lore-mcp\requirements.txt
```

Dependencies (`lore-mcp/requirements.txt`):
- `mcp>=1.0.0` — the MCP SDK (required for stdio mode)
- `starlette>=0.40.0` and `uvicorn>=0.30.0` — only needed for `--sse` mode

> **IMPORTANT:** always reference the venv interpreter directly —
> `lore-mcp/venv/bin/python` on Linux/macOS, `lore-mcp\venv\Scripts\python.exe`
> on Windows. Using bare `python3`/`python` will fail with
> `ModuleNotFoundError` because the venv packages are not in the system
> Python's site-packages.

Regenerate the tool catalog from the LoreGUI palette manifests (run once after
install, and again after any change to `lore-vm` ops):

```sh
# Linux/macOS:
LOREGUI_DIR="$(pwd)" lore-mcp/venv/bin/python lore-mcp/generate_catalog.py

# Windows:
$env:LOREGUI_DIR = (Get-Location).Path; lore-mcp\venv\Scripts\python.exe lore-mcp\generate_catalog.py
```

Smoke-test the server (no repo needed — just lists the tool catalog):

```sh
# Linux/macOS:
LOREVM_BIN="$(pwd)/target/release/lorevm" \
  lore-mcp/venv/bin/python lore-mcp/server.py --list

# Windows:
$env:LOREVM_BIN = "$(Get-Location)\target\release\lorevm.exe"; lore-mcp\venv\Scripts\python.exe lore-mcp\server.py --list
```

Expected output: `lore-mcp exposes 22 tools` followed by each tool name and
its description. The line `lorevm binary: <path>` should show your binary, not
`NOT FOUND`.

---

## Step 5 — Configure the repository (ask the user)

Before wiring anything up, ask the user which of these three modes they want.
Don't default to local/offline silently — the mode determines both the
`LORE_OFFLINE` setting below and whether a server needs to be stood up.

**(a) Fully local, no server.** Simplest — a repo that lives only on this
machine, no multi-user sharing. Use `LORE_OFFLINE=1`. Good for a solo user or
a quick trial.

**(b) Connect to an existing Lore server.** The user (or their team) already
has a server running somewhere. In the GUI: onboarding → "Connect to a
server" → `auth.login_with_token` / `login_interactive(url)` → pick or clone
a repo. For agents/headless use, point `LORE_REPO` at the resulting local
working copy. Use `LORE_OFFLINE=0` (unset) since writes need the connection.

**(c) Host a new Lore server.** For multi-user/shared use. Two ways to do this,
in order of preference:

1. **From the GUI (preferred — no extra build needed).** The desktop app
   already bundles the real `loreserver` binary as a packaged sidecar. In the
   app: onboarding → "Host a server" → this drives `shared_store.create` →
   `repository.create` → `service.start` internally. You'll be asked to pick
   a **storage backend** — local packfiles on disk, or a remote object store
   (S3/MinIO/Garage — anything `lore`'s transport layer accepts as a
   `remote_url`). Ask the user which they want and how much retention/space
   to allocate before confirming; this decision is persistent and not easily
   undone later.
2. **Headless (no GUI on this host).** Build or fetch `loreserver` yourself:
   - Download the platform binary from the same GitHub Release as the GUI
     installers: `loreserver_Windows_x64.exe`, `loreserver_Linux_x64`, or
     `loreserver_MacOS_arm64`.
   - Or build it from source: `cargo build --release -p lore-server --bin loreserver`.
   - Then drive the same op sequence via `lorevm`: `shared_store.create`
     (pick storage backend/path here too) → `repository.create` →
     `service.start`.

Whichever mode you land on, set the environment variables in Step 6 to match:
`LORE_REPO` should point at the real, **persistent** repository path (not a
`mktemp`/`--in-memory` scratch dir — those are for the Step 8 smoke test
only), and `LORE_OFFLINE` should be `1` only for mode (a).

---

## Step 6 — Launch at login? (ask the user)

Ask whether the user wants LoreGUI to start automatically when they log in.

- **If yes (GUI users):** use the app's own **Settings → Account → "Start
  LoreGUI at login"** toggle. This is backed by `tauri-plugin-autostart` and
  is already wired up — don't hand-roll a Registry Run key / Startup-folder
  shortcut / systemd user unit for the GUI app; the in-app toggle is the
  supported path and it also handles "close to tray instead of quitting."
- **If a headless `loreserver` (Step 5c-2) needs to survive reboots** on a
  host with no GUI, that's a different problem from GUI autostart — the app
  exposes a `service start` op but persisting a headless server across
  reboots needs an OS-level service (systemd unit on Linux, Windows Service /
  Scheduled Task, launchd on macOS). This isn't fully documented yet; if the
  user needs it, treat it as a follow-up rather than guessing at a one-off
  script.

---

## Step 7 — Register the lore MCP server with your agent host

### 7a. Claude Code (recommended)

Add via the CLI (replace paths with your actual `loregui` checkout path and
your lore repository path from Step 5):

```sh
claude mcp add lore \
  --command "/path/to/loregui/lore-mcp/venv/bin/python" \
  --args "/path/to/loregui/lore-mcp/server.py" \
  --env LOREVM_BIN="/path/to/loregui/target/release/lorevm" \
  --env LORE_REPO="/path/to/your/lore/repo" \
  --env LORE_OFFLINE="1"
```

(Windows: use `lore-mcp\venv\Scripts\python.exe` for `--command` and
`target\release\lorevm.exe` for `LOREVM_BIN`.)

Or add the block manually to `.claude/mcp.json` (project-level) or
`~/.claude.json` (global):

```json
{
  "mcpServers": {
    "lore": {
      "command": "/path/to/loregui/lore-mcp/venv/bin/python",
      "args": ["/path/to/loregui/lore-mcp/server.py"],
      "env": {
        "LOREVM_BIN": "/path/to/loregui/target/release/lorevm",
        "LORE_REPO": "/path/to/your/lore/repo",
        "LORE_OFFLINE": "1"
      }
    }
  }
}
```

After adding, verify the server is visible:

```sh
claude mcp list
```

### 7b. OpenAI Codex CLI / generic `mcp_servers` TOML format

Add to `~/.codex/config.toml` (or your project's codex config):

```toml
# lore — drive Epic's lore VCS in-process.
# Server is in the loregui repo (github.com/BiloxiStudios/loregui → lore-mcp/).
# One-time setup: cargo build --release -p lorevm-cli && python3 -m venv lore-mcp/venv
#   && lore-mcp/venv/bin/pip install -r lore-mcp/requirements.txt
[mcp_servers.lore]
command = "/path/to/loregui/lore-mcp/venv/bin/python"
args = ["/path/to/loregui/lore-mcp/server.py"]
env = { LOREVM_BIN = "/path/to/loregui/target/release/lorevm",
        LORE_REPO = "/path/to/your/lore/repo",
        LORE_OFFLINE = "1" }
```

### 7c. Generic `mcpServers` JSON (Cursor, Windsurf, any MCP-compatible host)

```json
{
  "mcpServers": {
    "lore": {
      "command": "/path/to/loregui/lore-mcp/venv/bin/python",
      "args": ["/path/to/loregui/lore-mcp/server.py"],
      "env": {
        "LOREVM_BIN": "/path/to/loregui/target/release/lorevm",
        "LORE_REPO": "/path/to/your/lore/repo",
        "LORE_OFFLINE": "1"
      }
    }
  }
}
```

### Environment variables

| Variable | Required? | Meaning |
|---|---|---|
| `LOREVM_BIN` | Recommended | Path to the `lorevm` binary. If unset, the server searches `PATH` then `<loregui>/target/{release,debug}/lorevm`. |
| `LORE_REPO` | Recommended | Default repository working directory (the **persistent** path chosen in Step 5, not a scratch dir). Each tool call can also pass a `repo` argument to override this. |
| `LORE_OFFLINE` | Optional | Set to `1` or `true` to pass `--offline` to every `lorevm` invocation. Only appropriate for Step 5 mode (a); leave unset/`0` for modes (b)/(c). |
| `LORE_IDENTITY` | Optional | Identity string passed to `lorevm` via `--identity`. |
| `LOREGUI_DIR` | Optional | Path to the loregui checkout (used for catalog generation and binary auto-discovery fallback). Defaults to the parent of `server.py`. |

---

## Step 8 — Verify end-to-end

Verifying "it works" now means **all** of: the GUI opens, the MCP chain
responds, and (if applicable) the server is reachable — not just the MCP
catalog listing.

### 8a. Confirm the GUI app opens

Already done in Step 2 — re-confirm here if time has passed or the install
happened separately from this session.

### 8b. Verify the tool catalog is loaded (no repo required)

```sh
LOREVM_BIN="$(pwd)/target/release/lorevm" \
  lore-mcp/venv/bin/python lore-mcp/server.py --list
```

You should see 22 tools including `lore_repository_status`, `lore_revision_history`,
`lore_branch_list`, `lore_lock_file_query`, and `lore_repo_summary`.

### 8c. Run a real op against a throwaway repo

This is a **smoke test only** — it validates the CLI/server chain using a
disposable temp repo, separate from the real persistent repo configured in
Step 5.

```sh
REPO="$(mktemp -d)"
LOREVM="$(pwd)/target/release/lorevm"

# Create a minimal in-memory repo (smoke test only — not persisted)
"$LOREVM" repository.create --dir "$REPO" --offline --in-memory \
  --identity "agent-smoke" \
  --args '{"repository_url":"lore://localhost/smoke"}'

# Query status via the MCP server
LORE_REPO="$REPO" LORE_OFFLINE=1 LOREVM_BIN="$LOREVM" \
  lore-mcp/venv/bin/python lore-mcp/server.py --list
```

A clean `--list` run after pointing at a valid repo confirms the chain
`agent → server.py → lorevm → lore-vm ops` is working. Delete `$REPO`
afterwards — it's disposable.

### 8d. Call a tool from your agent (once registered)

Ask your agent:

```
Using the lore MCP tools, call lore_repo_summary with repo="/path/to/your/lore/repo"
and report what comes back.
```

Use the **real, persistent** repo path from Step 5 here, not the throwaway
one from 8c. A well-formed JSON response from `lore_repo_summary` — with keys
`repo`, `current_branch`, `branch_count`, and `recent_revisions` — means the
full chain is working.

### 8e. If you hosted a server (Step 5c), confirm it's reachable

From the GUI: the app's status/connection indicator should show connected,
not "offline". Headless: re-run `lore_repository_status` against the
connected repo and confirm it doesn't return an offline/needs-a-server error.

---

## Quick-reference: binary location

| Build type | Binary path |
|---|---|
| Release (production) | `<loregui-root>/target/release/lorevm` |
| Debug (development) | `<loregui-root>/target/debug/lorevm` |

The Cargo.toml `[[bin]]` section for `lorevm-cli` sets `name = "lorevm"`, so the
binary is always named `lorevm` (not `lorevm-cli`).

---

## Known limits

- **Offline staging is not cross-process:** with `LORE_OFFLINE=1`, staging lives
  in process-local memory, so a `file.stage` in one `lorevm` invocation is not
  visible to a `revision.commit` in a separate call. Read/metrics ops (`status`,
  `history`, `diff`, `branch.list`, `file.history`, `lock.*`) and single-call
  mutations are unaffected. Multi-step write workflows (stage → commit) need a
  connected repo (omit `LORE_OFFLINE` or set it to `0`).
- **Lock and auth ops** require a connected server and return a "needs a server"
  error when offline.
- **Headless server autostart-on-boot** (Step 6) isn't documented yet beyond
  pointing at the `service start` op — treat as a follow-up if the user needs it.

---

## Pair with the agent skills

For the VCS mental model (revisions, branches, staging, locks, git/p4 → lore
translation, and per-op semantics), load the `lore` skill from `.claude/skills/lore/`.

For the condensed operational reference (install, MCP setup, repo
configuration, and driving lore), the `loregui` skill at
`.claude/skills/loregui/` covers the same ground as this document in the
agent-skill format. Keep the two in sync when either changes — this document
is the step-by-step runbook; the skill is the terser lookup version.
