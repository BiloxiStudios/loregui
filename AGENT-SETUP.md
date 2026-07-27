# AGENT-SETUP.md — Lore Agent Bootstrap Runbook

**Audience:** AI agents following this document end-to-end.
**Goal:** Install and verify the Lore software stack (CLI, MCP, GUI, and Server),
configure a repository, and decide on login autostart.

This document provides three parallel setup paths depending on your needs:
1. **Path A: Headless (CLI + MCP)** — Best for automated VCS operations.
2. **Path B: Desktop GUI** — Best for visual management and manual review.
3. **Path C: Standalone Server** — Best for hosting lore repositories.

Follow the steps in order. Each section gives exact commands and expected outcomes.
After the paths, Step 4 (repository configuration), Step 5 (autostart), Step 6
(MCP registration), and Step 7 (verification) apply regardless of which path(s)
you took.

---

## Step 0 — Identification & Artifact Selection

Identify your host operating system and architecture to select the correct artifacts.

| OS | Architecture | Installer Pattern | Raw Binary Pattern | Server Binary |
|---|---|---|---|---|
| **Linux** | x64 | `LoreGUI_*_amd64.deb` or `.AppImage` | `LoreGUI_Linux_x64` | `loreserver_Linux_x64` |
| **Windows**| x64 | `LoreGUI_*_x64-setup.exe` | `LoreGUI_Windows_x64.exe`| `loreserver_Windows_x64.exe`|
| **macOS**  | ARM64 | `LoreGUI_*_aarch64.dmg` | `LoreGUI_MacOS_arm64` | `loreserver_MacOS_arm64` |

Latest artifacts are available on the rolling `nightly` release:
<https://github.com/BiloxiStudios/loregui/releases/tag/nightly>

---

## Path A: Headless (CLI + MCP)

This path sets up the `lorevm` CLI and the `lore-mcp` server. Use this for server-side automation or when no display is available.

### A.1 — Get the `lorevm` binary

`lorevm` is a thin JSON CLI that calls the in-process `lore-vm` ops. The `lore-mcp` server shells out to it for every tool call.

#### Option 1: Build from source (Recommended for developers)
```sh
# From the root of the loregui checkout:
cargo build --release -p lorevm-cli
# Binary lands at: ./target/release/lorevm  (lorevm.exe on Windows)
```

#### Option 2: Download pre-built raw binary
Download the `LoreGUI_<OS>_<Arch>` raw binary from the nightly release. It contains the same engine functionality.

### A.2 — Set up the lore-mcp Python server

The MCP server lives in `lore-mcp/` inside the repo. It needs its own virtual
environment — the venv's interpreter path is **OS-dependent** (`bin/` on
Linux/macOS, `Scripts\` on Windows):

```sh
# From the loregui root:
python3 -m venv lore-mcp/venv

# Linux / macOS:
lore-mcp/venv/bin/pip install -r lore-mcp/requirements.txt
LOREGUI_DIR="$(pwd)" lore-mcp/venv/bin/python lore-mcp/generate_catalog.py

# Windows:
lore-mcp\venv\Scripts\pip.exe install -r lore-mcp\requirements.txt
$env:LOREGUI_DIR = (Get-Location).Path; lore-mcp\venv\Scripts\python.exe lore-mcp\generate_catalog.py
```

> **IMPORTANT:** always reference the venv interpreter directly —
> `lore-mcp/venv/bin/python` on Linux/macOS, `lore-mcp\venv\Scripts\python.exe`
> on Windows. Using bare `python3`/`python` will fail with
> `ModuleNotFoundError` because the venv packages are not in the system
> Python's site-packages.

### A.3 — Verify the MCP chain
```sh
# Linux / macOS:
LOREVM_BIN="$(pwd)/target/release/lorevm" \
  lore-mcp/venv/bin/python lore-mcp/server.py --list

# Windows:
$env:LOREVM_BIN = "$(Get-Location)\target\release\lorevm.exe"; lore-mcp\venv\Scripts\python.exe lore-mcp\server.py --list
```
Expected output: `lore-mcp exposes 22 tools` and a list of tool names. The
`lorevm binary: <path>` line should show your binary, not `NOT FOUND`.

---

## Path B: Desktop GUI

This path installs the full LoreGUI application. Use this for rich visual interaction and manual repo management.

### B.1 — Install LoreGUI (OS-specific signed installers)

Download the **signed installer** for your OS from the nightly release:
<https://github.com/BiloxiStudios/loregui/releases/tag/nightly>

**Windows (x64):**
```sh
# Download the NSIS installer (signed .exe)
curl -fsSLO https://github.com/BiloxiStudios/loregui/releases/download/nightly/LoreGUI_0.1.3_x64-setup.exe
# Run the installer (silent mode for agents):
./LoreGUI_0.1.3_x64-setup.exe /S
# Or: double-click for interactive install. Binary installs to %LOCALAPPDATA%\Programs\LoreGUI\
```

**Linux (Debian/Ubuntu):**
```sh
curl -fsSLO https://github.com/BiloxiStudios/loregui/releases/download/nightly/LoreGUI_0.1.3_amd64.deb
sudo dpkg -i LoreGUI_0.1.3_amd64.deb
# Binary: /usr/bin/LoreGUI (or loregui)
```

**Linux (AppImage — no install required):**
```sh
curl -fsSLO https://github.com/BiloxiStudios/loregui/releases/download/nightly/LoreGUI_0.1.3_amd64.AppImage
chmod +x LoreGUI_0.1.3_amd64.AppImage
./LoreGUI_0.1.3_amd64.AppImage &
```

**macOS (ARM64):**
```sh
curl -fsSLO https://github.com/BiloxiStudios/loregui/releases/download/nightly/LoreGUI_0.1.3_aarch64.dmg
# Mount the DMG, drag LoreGUI.app to /Applications
hdiutil attach LoreGUI_0.1.3_aarch64.dmg
cp -R /Volumes/LoreGUI/LoreGUI.app /Applications/
hdiutil detach /Volumes/LoreGUI
# Launch: open -a LoreGUI
```

> **Artifact identification:** The installer artifact is the OS-specific package
> (`.exe`, `.deb`, `.AppImage`, `.dmg`). The raw binary (`LoreGUI_<OS>_<Arch>`)
> is for headless / embedded use only — agents should prefer the signed installer
> so the app registers with the OS (start menu, file associations, autostart).

### B.2 — Verify Launch (real GUI check)

**Check 1: Confirm the process is running:**
- **Linux/macOS:** `pgrep -x loregui || pgrep -x LoreGUI`
- **Windows:** `Get-Process LoreGUI` or `tasklist | findstr LoreGUI`

> Note: After a signed install the binary name may differ from the raw download.
> Check both `LoreGUI` (camel, from installer) and `loregui` (lowercase, from raw binary).

**Check 2: Real Launch Check — CDP endpoint:**
If the app was launched with debugging enabled (`--remote-debugging-port=9222`),
verify the Chromium DevTools endpoint responds:
```sh
curl -s http://localhost:9222/json/version
```
Expected: A JSON object containing `"Browser"`, `"User-Agent"`, and webSocket debugger URL.

**Check 3: Verify the app window is visible (optional, for agents with screen access):**
- **macOS:** `osascript -e 'tell application "LoreGUI" to activate'`
- **Linux (X11):** `xdotool search --name LoreGUI`
- **Windows:** `powershell -Command "Get-Process LoreGUI | Select MainWindowTitle"`

If no window appears, stop and debug the install before continuing — a working
MCP tool catalog does not prove the app exists; don't silently fall back to
CLI-only.

---

## Path C: Standalone Server

This path installs and runs the `loreserver` sidecar. Use this to host lore repositories that can be reached by other clients.

> **Parallel outcome:** Paths B (GUI) and C (Server) are independent. An agent
> can run both — install LoreGUI for visual management AND run loreserver for
> hosting. They share the same lore repo but serve different purposes.

### C.1 — Download `loreserver`

Download the `loreserver_<OS>_<Arch>` binary from the nightly release:
<https://github.com/BiloxiStudios/loregui/releases/tag/nightly>

| OS | Binary name |
|---|---|
| Linux x64 | `loreserver_Linux_x64` |
| Linux arm64 | `loreserver_Linux_arm64` |
| macOS ARM64 | `loreserver_MacOS_arm64` |
| Windows x64 | `loreserver_Windows_x64.exe` |

Or build it from source: `cargo build --release -p lore-server --bin loreserver`.

### C.2 — Launch loreserver

The server requires a configuration directory. Create a basic configuration and launch:

```sh
mkdir -p lore-config
cat > lore-config/local.toml << 'TOML'
server_name = "agent-host-server"

# Force TCP h2c (required for bore tunnel compatibility)
[server.quic]
enabled = false

[server.grpc]
enabled = true
host = "127.0.0.1"
port = 41338

[server.http]
enabled = true
host = "127.0.0.1"
port = 41339
TOML

LORE_CONFIG_PATH="./lore-config" LORE_ENV=local ./loreserver_Linux_x64 &
```

> Replace `./loreserver_Linux_x64` with the correct binary for your OS.

### C.3 — Verify Health

Check the HTTP status endpoint (default HTTP port `41339`):
```sh
curl -s http://localhost:41339/status
```
Expected: `{"running":true, ...}`.

---

## Step 4 — Configure the repository (choose a mode)

Before wiring anything up, decide which of these three modes the user wants.
Don't default to local/offline silently — the mode determines both the
`LORE_OFFLINE` setting in Step 6 and whether a server needs to be stood up.
This is the one place this document defines `LORE_OFFLINE` per mode:

**(a) Fully local, no server.** Simplest — a repo that lives only on this
machine, no multi-user sharing. Use `LORE_OFFLINE=1`. Good for a solo user or
a quick trial.

**(b) Connect to an existing Lore server.** The user (or their team) already
has a server running somewhere. In the GUI: onboarding → "Connect to a
server" → `auth.login_with_token` / `login_interactive(url)` → pick or clone
a repo. For agents/headless use, point `LORE_REPO` at the resulting local
working copy. Leave `LORE_OFFLINE` unset/`0` since writes need the connection.

**(c) Host a new Lore server.** For multi-user/shared use. Two ways to do this,
in order of preference:

1. **From the GUI (preferred — no extra build needed).** The desktop app
   already bundles the real `loreserver` binary as a packaged sidecar. In the
   app: onboarding → "Host a server" → this drives `shared_store.create` →
   `repository.create` → `service.start` internally.
2. **Headless (no GUI on this host).** Install and launch `loreserver` per
   Path C, then drive the same op sequence via `lorevm`:
   `shared_store.create` → `repository.create` → `service.start`.

Either way you'll be asked to pick a **storage backend** — local packfiles on
disk, or a remote object store (S3/MinIO/Garage — anything `lore`'s transport
layer accepts as a `remote_url`). Ask the user which they want and how much
retention/space to allocate before confirming; this decision is persistent and
not easily undone later. Use `LORE_OFFLINE=0` for this mode.

Whichever mode you land on, set the environment variables in Step 6 to match:
`LORE_REPO` should point at the real, **persistent** repository path (not a
`mktemp`/`--in-memory` scratch dir — those are for the Step 7 smoke test
only), and `LORE_OFFLINE` should be `1` only for mode (a).

---

## Step 5 — Launch at login (autostart)

Ask whether the user wants LoreGUI to start automatically when they log in.

- **If yes (GUI users):** use the app's own **Settings → Account → "Start
  LoreGUI at login"** toggle. This is backed by `tauri-plugin-autostart` and
  is already wired up — don't hand-roll a Registry Run key / Startup-folder
  shortcut / systemd user unit for the GUI app; the in-app toggle is the
  supported path and it also handles "close to tray instead of quitting."
- **If a headless `loreserver` (Path C) needs to survive reboots** on a host
  with no GUI, that's a different problem from GUI autostart — the app exposes
  a `service start` op but persisting a headless server across reboots needs
  an OS-level service (systemd unit on Linux, Windows Service / Scheduled
  Task, launchd on macOS). This isn't fully documented yet; if the user needs
  it, treat it as a follow-up rather than guessing at a one-off script.

---

## Step 6 — Register the lore MCP server with your agent host

### 6a. Claude Code (recommended)
```sh
claude mcp add lore   --command "/path/to/loregui/lore-mcp/venv/bin/python"   --args "/path/to/loregui/lore-mcp/server.py"   --env LOREVM_BIN="/path/to/loregui/target/release/lorevm"   --env LORE_REPO="/path/to/your/lore/repo"   --env LORE_OFFLINE="1"
```

(Windows: use `lore-mcp\venv\Scripts\python.exe` for `--command` and
`target\release\lorevm.exe` for `LOREVM_BIN`.)

### 6b. OpenAI Codex CLI / generic `mcp_servers` TOML format
Add to `~/.codex/config.toml`:
```toml
[mcp_servers.lore]
command = "/path/to/loregui/lore-mcp/venv/bin/python"
args = ["/path/to/loregui/lore-mcp/server.py"]
env = { LOREVM_BIN = "/path/to/loregui/target/release/lorevm", LORE_REPO = "/path/to/your/lore/repo", LORE_OFFLINE = "1" }
```

### Environment variables

| Variable | Required? | Meaning |
|---|---|---|
| `LOREVM_BIN` | Recommended | Path to the `lorevm` binary. If unset, the server searches `PATH` then `<loregui>/target/{release,debug}/lorevm`. |
| `LORE_REPO` | Recommended | Default repository working directory (the **persistent** path chosen in Step 4, not a scratch dir). Each tool call can also pass a `repo` argument to override this. |
| `LORE_OFFLINE` | Optional | Set to `1` or `true` to pass `--offline` to every `lorevm` invocation. Only appropriate for Step 4 mode (a); leave unset/`0` for modes (b)/(c). |
| `LORE_IDENTITY` | Optional | Identity string passed to `lorevm` via `--identity`. |
| `LOREGUI_DIR` | Optional | Path to the loregui checkout (used for catalog generation and binary auto-discovery fallback). Defaults to the parent of `server.py`. |

---

## Step 7 — Verification Suite (No-Regression Check)

Run the included verification script to confirm your setup is complete and functional:

```sh
# From the loregui root:
./scripts/check-agent-setup.py
```

The script checks the `lorevm` binary, the lore-mcp tool catalog, the LoreGUI
process (if the GUI path was taken), and the loreserver health endpoint (if
the Server path was taken).

### 7a. Throwaway-repo smoke test (disposable)

This is a **smoke test only** — it validates the CLI/server chain using a
disposable temp repo, separate from the real persistent repo configured in
Step 4:

```sh
REPO="$(mktemp -d)"
LOREVM="$(pwd)/target/release/lorevm"

# Create a minimal in-memory repo (smoke test only — not persisted)
"$LOREVM" repository.create --dir "$REPO" --offline --in-memory \
  --identity "agent-smoke" \
  --args '{"repository_url":"lore://localhost/smoke"}'

# Query the catalog via the MCP server against the throwaway repo
LORE_REPO="$REPO" LORE_OFFLINE=1 LOREVM_BIN="$LOREVM" \
  lore-mcp/venv/bin/python lore-mcp/server.py --list
```

A clean `--list` run after pointing at a valid repo confirms the chain
`agent → server.py → lorevm → lore-vm ops` is working. Delete `$REPO`
afterwards — it's disposable.

### 7b. Real-repo check (persistent)

Once the MCP server is registered (Step 6), call a tool against the **real,
persistent** repo path from Step 4 — not the throwaway one from 7a. Ask your
agent:

```
Using the lore MCP tools, call lore_repo_summary with repo="/path/to/your/lore/repo"
and report what comes back.
```

A well-formed JSON response from `lore_repo_summary` — with keys `repo`,
`current_branch`, `branch_count`, and `recent_revisions` — means the full
chain is working.

### 7c. Manual fallback checks

If you don't have the script, you can run these manual checks:
1. `lorevm --list` returns a list of ops.
2. `server.py --list` lists 22 tools.
3. `pgrep LoreGUI` (if GUI path) returns a PID.
4. `curl localhost:41339/status` (if Server path) returns `{"running":true}`.

---

## Advanced: Build full LoreGUI from source (heavier developer option)

```sh
cd loregui
npm install
cargo tauri build
```

> ⚠️ Use `cargo tauri build`, **not** `cargo build -p loregui` — the latter
> skips the frontend build (`beforeBuildCommand`), so the resulting binary
> embeds the dev URL and the window shows "connection refused" instead of the
> app. On a headless build box (no display), run under `xvfb-run`.

Note: Requires Rust, Node.js 20+, and platform-specific dependencies (WebKit2GTK on Linux, Xcode on macOS, MSVC on Windows).
Use this only when you need to modify the LoreGUI source or when signed installers are unavailable.
For day-to-day usage, prefer the signed installers in Path B.
