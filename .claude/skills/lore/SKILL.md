---
name: lore
description: Drive and reason about Epic Games' lore VCS the way you would git or p4 — the mental model, the full command/op surface across 14 domains, git/p4→lore translation, common workflows, and how to execute ops (via the lore-mcp tools / lore-vm / the LoreGUI palette). Use whenever a task involves a lore repository, revisions, branches, staging, locks, fragments, or shared stores.
---

# lore — version control for agents

`lore` is Epic's content-addressed VCS (built for large game projects). LoreGUI
binds it **in-process** via the `lore-vm` crate (~136 ops in 14 domains). Drive it
through the **lore-mcp** tools (one tool per op) or the LoreGUI command palette;
never assume a `lore` CLI is on PATH.

## Mental model (lore ≠ git)

- **Revision** — a content-addressed snapshot on a **branch**. History is linear
  per branch; combining branches is an explicit **merge** state machine, not a
  fast-forward.
- **Staging** — files are **dirty** (modified) → **stage** → **commit**. Extra
  moves: `dirty_copy`/`dirty_move`, `stage_move`/`stage_merge`, `unstage`,
  `reset`/`reset_to_last_merged`, and `obliterate` (permanent removal — destructive).
- **Fragments & partitions** — content is stored as fragments addressed by hash
  inside a **partition** (32-hex namespace; the zero partition is invalid). The
  immutable store holds fragments; a separate **mutable KV store** holds branch
  pointers.
- **Shared store** — a store multiple repos share (created before repos in host
  setup). Backends: local packfiles, or S3/MinIO/Garage object storage.
- **Locks** — per-file advisory locks (acquire/release/query/status); show who holds.
- **Links / Layers / Dependencies** — compose content and track file relationships.

## Command surface (by domain)

| Domain | Key ops (verbs) |
|---|---|
| **repository** | clone, create, info, status, list, delete, release, flush, gc, verify_state, instance_list/prune, metadata_*, config_get, store_immutable_query |
| **revision** | commit, amend, info, history, diff, find, sync, restore, revert (+resolve/abort/restart), metadata_* (cherry_pick/bisect deferred upstream) |
| **branch** | create, list, info, switch, push, diff, reset, archive, protect/unprotect, merge_start → resolve_mine/theirs/unresolve → merge_into/abort/restart, metadata_* |
| **file** | stage(+move/merge), unstage, dirty(+copy/move), reset(+to_last_merged), obliterate, info, history, diff, write, hash, dump, dependency_*, metadata_* |
| **lock** | acquire, acquire_as_owner, query, status, release |
| **storage** | open, close, flush, put(+file), get(+file), get_metadata, copy, obliterate, upload |
| **shared_store** | create, info, set_use_automatically |
| **auth** | login_interactive, login_with_token, user_info, local_user_info, list, logout, clear |
| **link / layer / dependency** | add, remove, update/list (compose & dep graph) |
| **service / notification** | start/stop · subscribe/unsubscribe (streaming) |

(The authoritative, machine-readable catalog with arg schemas is the LoreGUI
palette manifest: `frontend/src/palette/manifest/<domain>/<op>.ts`. The lore-mcp
server generates its tools from it.)

## git / p4 → lore translation

| You want (git / p4) | lore |
|---|---|
| `git status` / `p4 opened` | repository **status** |
| `git add` / `p4 add/edit` | file **stage** |
| `git commit` / `p4 submit` | revision **commit** |
| `git log` / `p4 changes` | revision **history** (metrics: who/when/what) |
| `git diff` / `p4 diff` | revision **diff** / file **diff** |
| `git blame` / `p4 annotate` | file **history** (per-file change trail) |
| `git checkout -b` / `git switch` | branch **create** / **switch** |
| `git merge` | branch **merge_start → resolve_* → merge_into** |
| `git revert` | revision **revert** (+ resolve/abort) |
| `git clone` | repository **clone** |
| `git push` / `git pull` | branch **push** / revision **sync** |
| `p4 lock` / `p4 unlock` | lock **acquire** / **release** |
| `git gc` | repository **gc** |

## Common workflows

- **Connect to a server:** auth `login_interactive(url)` → repository `clone`/`list`.
- **Host setup:** shared_store `create` → repository `create` → service `start`.
- **Edit loop:** edit files → file `stage` → revision `commit` → branch `push`.
- **Branch & merge:** branch `create`+`switch` → … → `merge_start` → resolve
  conflicts (`resolve_mine`/`resolve_theirs`/`unresolve`) → `merge_into` (or `abort`).
- **Locking:** lock `acquire` before editing a binary asset; `release` after.

## How to execute

1. **lore-mcp tools** (preferred for agents) — one tool per op, schema-validated;
   read ops (status/history/diff/file-history) give repo **metrics/intelligence**.
2. **LoreGUI palette** — interactive, ⌘K; every op via a generated form.
3. **lore-vm ops** (Rust) — `LoreApi::new(dir)` + `ops::<domain>::<op>(api, args)`;
   see `crates/lore-vm/tests/integration_roundtrip.rs` for the canonical pattern.

## Safety
`obliterate`, `delete`, `gc`, `reset`, branch `unprotect` are **destructive** —
confirm intent and explain consequences before running. The zero partition is
invalid. Never log auth tokens. Spawn `loregui-vcs-domain-expert` /
`loregui-storage-expert` for deep domain behavior.
