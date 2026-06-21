# lore-mcp

An MCP server that lets an AI agent drive Epic Games' [`lore`](https://github.com/EpicGames/lore)
VCS in-process, the way a git/p4 MCP server exposes git/p4. It is the **MCP half**
of the LoreGUI lore tooling; it pairs with the `.claude/skills/lore` skill in the
[loregui](https://github.com/BiloxiStudios/loregui) repo (the skill teaches the
mental model and op surface; this server is how an agent actually executes ops).

## Architecture

```
agent ──MCP──▶ server.py ──subprocess──▶ lorevm (Rust JSON CLI) ──in-process──▶ lore engine
                  ▲
                  │ tool catalog + arg schemas
            lore-tools.json  ◀── generate_catalog.py ◀── LoreGUI palette manifests
```

Two pieces, one in each repo:

1. **`lorevm`** — a thin JSON CLI (`crates/lorevm-cli` in loregui) that links the
   `lore-vm` crate and exposes its ops as
   `lorevm <domain>.<op> --dir <repo> --args '<json>'`, printing the op's typed
   result as JSON (and errors as `{"error": {...}}`). It binds the upstream
   `lore` engine in-process — it does **not** shell out to the legacy `lore` CLI.

2. **`server.py`** — this MCP server. It registers **one MCP tool per supported
   op** plus a `lore_repo_summary` aggregate. Each tool's name, description, and
   JSON input schema are derived from the **LoreGUI command-palette manifest**
   (`frontend/src/palette/manifest/<domain>/<op>.ts`) — the single source of
   truth for what each op is called and what it takes — pre-baked into
   `lore-tools.json` by `generate_catalog.py`. Tool calls shell out to `lorevm`
   and return its JSON.

### Why a generated catalog?

The palette FieldSpecs give us labels, descriptions, types, and required-ness for
free, and stay in lock-step with the GUI. `generate_catalog.py` converts each
FieldSpec to a JSON-Schema property, converting the manifest's camelCase arg
names to the snake_case keys the `lorevm` CLI deserialises (Tauri v2 does the
same camel→snake mapping). A small override table handles the few palette entries
whose arg shape differs from the raw op, and `file.history` (no manifest yet) is
defined directly from its Rust `Args`.

## Tools

21 op tools + `lore_repo_summary` (22 total). Read ops are the repo "metrics"
surface:

- **Read / metrics:** `lore_repository_status`, `lore_repository_info`,
  `lore_repository_list`, `lore_revision_history`, `lore_revision_diff`,
  `lore_revision_info`, `lore_revision_find`, `lore_branch_list`,
  `lore_branch_info`, `lore_file_info`, `lore_file_history`, `lore_file_diff`,
  `lore_lock_file_query`, `lore_lock_file_status`
- **Mutations:** `lore_revision_commit`, `lore_branch_create`,
  `lore_branch_switch`, `lore_file_stage`, `lore_file_unstage`,
  `lore_lock_file_acquire`, `lore_lock_file_release`
- **Aggregate:** `lore_repo_summary` — current branch/revision, branch count, and
  recent revisions in one call.

Every tool takes an optional `repo` arg that overrides the `LORE_REPO` env for
that call.

Run `python server.py --list` to print the live catalog (also a smoke test).

## Build the `lorevm` binary

In the loregui checkout:

```sh
cargo build -p lorevm-cli       # → target/debug/lorevm
# or: cargo build -p lorevm-cli --release  → target/release/lorevm
```

`server.py` finds the binary via `LOREVM_BIN`, then `PATH`, then
`$LOREGUI_DIR/target/{release,debug}/lorevm`.

## Setup

```sh
cd /srv/studiobrain-dev/loregui/lore-mcp
python3 -m venv venv
./venv/bin/pip install -r requirements.txt

# (re)generate the tool catalog from the LoreGUI manifests
LOREGUI_DIR=/srv/studiobrain-dev/loregui ./venv/bin/python generate_catalog.py
```

## Configuration (env)

| Var | Meaning | Default |
|-----|---------|---------|
| `LORE_REPO` | Default repository working directory | (none — must set or pass `repo`) |
| `LOREVM_BIN` | Path to the `lorevm` binary | PATH, then loregui target dirs |
| `LOREGUI_DIR` | LoreGUI checkout (catalog gen + binary fallback) | `/srv/studiobrain-dev/loregui` |
| `LORE_OFFLINE` | `1`/`true` → pass `--offline` (local repos, no remote) | unset |
| `LORE_IDENTITY` | Identity passed to `lorevm` (`--identity`) | unset |

## Register with an agent (`mcpServers`)

Add to your agent's MCP config (e.g. `~/.claude.json` / `.mcp.json`):

```json
{
  "mcpServers": {
    "lore": {
      "command": "/srv/studiobrain-dev/loregui/lore-mcp/venv/bin/python",
      "args": ["/srv/studiobrain-dev/loregui/lore-mcp/server.py"],
      "env": {
        "LORE_REPO": "/path/to/your/lore/repo",
        "LOREVM_BIN": "/srv/studiobrain-dev/loregui/target/debug/lorevm",
        "LORE_OFFLINE": "1"
      }
    }
  }
}
```

The agent then sees `lore_*` tools. Pair it with the loregui `.claude/skills/lore`
skill for the git/p4→lore mental model and workflows.

## Smoke test

```sh
# list tools (no repo needed)
./venv/bin/python server.py --list

# end-to-end against a throwaway repo
REPO=$(mktemp -d)
LOREVM_BIN=.../lorevm "$LOREVM_BIN" repository.create --dir "$REPO" --offline \
  --identity me --args '{"repository_url":"lore://localhost/smoke"}'
LORE_REPO="$REPO" LORE_OFFLINE=1 LOREVM_BIN=.../lorevm ./venv/bin/python server.py --list
```

## Deferred / not yet exposed

`lorevm`'s dispatch covers the git/p4-equivalent surface (status, history, diff,
info, find, commit, branch list/info/create/switch, stage/unstage, file
info/history/diff, locks). Not yet dispatched (the `lore-vm` ops exist; adding
each is one arm in the Rust `dispatch` match + one line in `SUPPORTED_OPS` here):
merges (`branch.merge_*`), push/pull/sync, shared stores, dependencies, links,
layers, auth, notifications, services, and repository admin (clone, gc, delete,
metadata). See `lore-vm/src/ops/<domain>/` for the full set.
