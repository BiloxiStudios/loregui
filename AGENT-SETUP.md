# AGENT-SETUP.md — Lore Agent Bootstrap Runbook

**Audience:** AI agents following this document end-to-end.
**Goal:** Install and verify the Lore software stack (CLI, MCP, GUI, and Server).

This document provides three parallel setup paths depending on your needs:
1. **Path A: Headless (CLI + MCP)** — Best for automated VCS operations.
2. **Path B: Desktop GUI** — Best for visual management and manual review.
3. **Path C: Standalone Server** — Best for hosting lore repositories.

Follow the steps in order. Each section gives exact commands and expected outcomes.

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
# Binary lands at: ./target/release/lorevm
```

#### Option 2: Download pre-built raw binary
Download the `LoreGUI_<OS>_<Arch>` raw binary from the nightly release. It contains the same engine functionality.

### A.2 — Set up the lore-mcp Python server

The MCP server lives in `lore-mcp/` inside the repo.

```sh
# From the loregui root:
python3 -m venv lore-mcp/venv
lore-mcp/venv/bin/pip install -r lore-mcp/requirements.txt
LOREGUI_DIR="." lore-mcp/venv/bin/python lore-mcp/generate_catalog.py
```

### A.3 — Verify the MCP chain
```sh
LOREVM_BIN="./target/release/lorevm"   lore-mcp/venv/bin/python lore-mcp/server.py --list
```
Expected output: `lore-mcp exposes 22 tools` and a list of tool names.

---

## Path B: Desktop GUI

This path installs the full LoreGUI application. Use this for rich visual interaction and manual repo management.

### B.1 — Install LoreGUI
Download the appropriate installer for your OS.

**Linux (Debian/Ubuntu):**
```sh
wget https://github.com/BiloxiStudios/loregui/releases/download/nightly/LoreGUI_0.1.3_amd64.deb
sudo dpkg -i LoreGUI_0.1.3_amd64.deb
```

**Linux (AppImage):**
```sh
wget https://github.com/BiloxiStudios/loregui/releases/download/nightly/LoreGUI_0.1.3_amd64.AppImage
chmod +x LoreGUI_0.1.3_amd64.AppImage
./LoreGUI_0.1.3_amd64.AppImage &
```

### B.2 — Verify Launch
Confirm the process is running:
- **Linux/macOS:** `pgrep -x loregui` or `pgrep -x LoreGUI`
- **Windows:** `Get-Process LoreGUI`

**Real Launch Check (CDP):**
If the app was launched with debugging enabled (`\--remote-debugging-port=9222`), verify the Chromium DevTools endpoint:
```sh
curl -s http://localhost:9222/json/version
```
Expected: A JSON object containing the browser version and user agent.

---

## Path C: Standalone Server

This path installs and runs the `loreserver` sidecar. Use this to host lore repositories that can be reached by other clients.

### C.1 — Download `loreserver`
Download the `loreserver_<OS>_<Arch>` binary from the nightly release.

### C.2 — Launch loreserver
The server requires a configuration directory. Create a basic configuration and launch:
```sh
mkdir -p lore-config
echo 'server_name = "agent-host-server"' > lore-config/local.toml

LORE_CONFIG_PATH="./lore-config" LORE_ENV=local ./loreserver_Linux_x64 &
```

### C.3 — Verify Health
Check the HTTP status endpoint (default port `41339`):
```sh
curl -s http://localhost:41339/status
```
Expected: `{"running":true, ...}`.

---

## Step 4 — Register the lore MCP server with your agent host

### 4a. Claude Code (recommended)
```sh
claude mcp add lore   --command "/path/to/loregui/lore-mcp/venv/bin/python"   --args "/path/to/loregui/lore-mcp/server.py"   --env LOREVM_BIN="/path/to/loregui/target/release/lorevm"   --env LORE_REPO="/path/to/your/lore/repo"   --env LORE_OFFLINE="1"
```

### 4b. OpenAI Codex CLI / generic `mcp_servers` TOML format
Add to `~/.codex/config.toml`:
```toml
[mcp_servers.lore]
command = "/path/to/loregui/lore-mcp/venv/bin/python"
args = ["/path/to/loregui/lore-mcp/server.py"]
env = { LOREVM_BIN = "/path/to/loregui/target/release/lorevm", LORE_REPO = "/path/to/your/lore/repo", LORE_OFFLINE = "1" }
```

---

## Step 5 — Verification Suite (No-Regression Check)

Run the included verification script to confirm your setup is complete and functional:

```sh
# From the loregui root:
./scripts/check-agent-setup.py
```

If you don't have the script, you can run these manual checks:
1. `lorevm --list` returns a list of ops.
2. `server.py --list` lists 22 tools.
3. `pgrep LoreGUI` (if GUI path) returns a PID.
4. `curl localhost:41339/status` (if Server path) returns `running: true`.

---

## Advanced: Build full LoreGUI from source
```sh
cd loregui
npm install
cargo tauri build
```
Note: Requires Rust, Node.js 20+, and platform-specific dependencies (WebKit2GTK, etc.).
