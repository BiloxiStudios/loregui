# AGENT-SETUP.md — LoreGUI + lore-mcp bootstrap runbook

**Audience:** AI agents following this document end-to-end.
**Goal:** clone (or download) LoreGUI, build the `lorevm` binary, set up the
`lore-mcp` Python server, register it with your agent host, and verify the full
chain works.

Follow the steps in order. Each step gives exact commands and expected outcomes.
If a command fails, check the "What can go wrong" note under that step before
retrying.

---

## Step 1 — Get the LoreGUI repo

### Option A: clone from GitHub (source build)

```sh
git clone https://github.com/BiloxiStudios/loregui
cd loregui
```

### Option B: download a signed binary installer (GUI only, skips the build)

Pre-built installers for the **desktop app** are on the rolling `nightly` release:
<https://github.com/BiloxiStudios/loregui/releases>

Formats: Windows `.exe`/`.msi`, Linux `.deb`/`.AppImage`/`.rpm`, macOS `.dmg`
(when signing is enabled).

> If you only want to drive lore via the MCP server (no GUI), you still need
> the `lorevm` binary from step 2 — it is not included in the GUI installers.
> Clone the source (Option A) and build just `lorevm-cli`.

---

## Step 2 — Build the `lorevm` binary

`lorevm` is a thin JSON CLI that calls the in-process `lore-vm` ops. The
`lore-mcp` server shells out to it for every tool call.

```sh
# From the root of the loregui checkout:
cargo build --release -p lorevm-cli
```

The binary lands at:

```
<loregui-root>/target/release/lorevm
```

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

## Step 3 — Set up the lore-mcp Python server

The MCP server lives in `lore-mcp/` inside the repo. It needs its own virtual
environment.

```sh
# From the loregui root:
python3 -m venv lore-mcp/venv
lore-mcp/venv/bin/pip install -r lore-mcp/requirements.txt
```

Dependencies (`lore-mcp/requirements.txt`):
- `mcp>=1.0.0` — the MCP SDK (required for stdio mode)
- `starlette>=0.40.0` and `uvicorn>=0.30.0` — only needed for `--sse` mode

> **IMPORTANT:** always reference the venv interpreter directly
> (`lore-mcp/venv/bin/python`). Using bare `python3` will fail with
> `ModuleNotFoundError` because the venv packages are not in the system
> Python's site-packages.

Regenerate the tool catalog from the LoreGUI palette manifests (run once after
install, and again after any change to `lore-vm` ops):

```sh
LOREGUI_DIR="$(pwd)" lore-mcp/venv/bin/python lore-mcp/generate_catalog.py
```

Smoke-test the server (no repo needed — just lists the tool catalog):

```sh
LOREVM_BIN="$(pwd)/target/release/lorevm" \
  lore-mcp/venv/bin/python lore-mcp/server.py --list
```

Expected output: `lore-mcp exposes 22 tools` followed by each tool name and
its description. The line `lorevm binary: <path>` should show your binary, not
`NOT FOUND`.

---

## Step 4 — Register the lore MCP server with your agent host

### 4a. Claude Code (recommended)

Add via the CLI (replace paths with your actual `loregui` checkout path and
your lore repository path):

```sh
claude mcp add lore \
  --command "/path/to/loregui/lore-mcp/venv/bin/python" \
  --args "/path/to/loregui/lore-mcp/server.py" \
  --env LOREVM_BIN="/path/to/loregui/target/release/lorevm" \
  --env LORE_REPO="/path/to/your/lore/repo" \
  --env LORE_OFFLINE="1"
```

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

### 4b. OpenAI Codex CLI / generic `mcp_servers` TOML format

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

### 4c. Generic `mcpServers` JSON (Cursor, Windsurf, any MCP-compatible host)

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
| `LORE_REPO` | Recommended | Default repository working directory. Each tool call can also pass a `repo` argument to override this. |
| `LORE_OFFLINE` | Optional | Set to `1` or `true` to pass `--offline` to every `lorevm` invocation (useful for local repos with no remote). |
| `LORE_IDENTITY` | Optional | Identity string passed to `lorevm` via `--identity`. |
| `LOREGUI_DIR` | Optional | Path to the loregui checkout (used for catalog generation and binary auto-discovery fallback). Defaults to the parent of `server.py`. |

---

## Step 5 — Verify end-to-end

### 5a. Verify the tool catalog is loaded (no repo required)

```sh
LOREVM_BIN="$(pwd)/target/release/lorevm" \
  lore-mcp/venv/bin/python lore-mcp/server.py --list
```

You should see 22 tools including `lore_repository_status`, `lore_revision_history`,
`lore_branch_list`, `lore_lock_file_query`, and `lore_repo_summary`.

### 5b. Run a real op against a throwaway repo

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
`agent → server.py → lorevm → lore-vm ops` is working.

### 5c. Call a tool from your agent (once registered)

Ask your agent:

```
Using the lore MCP tools, call lore_repo_summary with repo="/path/to/your/lore/repo"
and report what comes back.
```

A well-formed JSON response from `lore_repo_summary` — with keys `repo`,
`current_branch`, `branch_count`, and `recent_revisions` — means the full
chain is working.

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
- **GUI build:** to build the full desktop app, use `cargo tauri build` (not
  `cargo build -p loregui`). On a headless box, run under `xvfb-run`.

---

## Pair with the agent skills

For the VCS mental model (revisions, branches, staging, locks, git/p4 → lore
translation, and per-op semantics), load the `lore` skill from `.claude/skills/lore/`.

For the full operational runbook (install, MCP setup, repo configuration, and
driving lore), the `loregui` skill at `.claude/skills/loregui/` mirrors this
document in the agent-skill format.
