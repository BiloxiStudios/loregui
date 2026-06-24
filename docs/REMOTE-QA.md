# Remote / Multi-User QA

`lore`'s headline, **"Perforce-class"** selling point is its *remote, multi-user*
path: a real server that many clients **sync** from and **push** to, with
server-side **file locks** for coordination and **conflict** detection when two
users race. This document is the QA charter for that path — what's automated, how
to run it, and the manual checklist for what isn't (yet).

Until this harness landed, the remote/multi-user path had **zero** automated
coverage. The CI-gated `crates/lore-vm/tests/e2e_lifecycle.rs` suite is single
user, and its `multi_repo_shared_store_sync_observes_remote_commit` test is
*explicitly* a CI-headless stand-in — it shares an on-disk store instead of a live
server. Its own module docs flag the genuine networked path (`loreserver` +
`branch::push` + `repository::clone` + cross-user locks) as a **deferred gap**.
This harness closes it.

---

## What's automated

| Asset | What it does |
|-------|--------------|
| `crates/lore-vm/tests/remote_multiuser.rs` | Feature-gated (`remote-integration-tests`) integration test. Boots a **real `loreserver`** on loopback and drives **two independent clones** (alice + bob) through it over the wire. Self-skips (green) when no server binary is resolvable. |
| `scripts/remote-multiuser-qa.sh` | Resolves/builds the `loreserver` binary once, then runs the suite against it (passing `LOREVM_SERVER_BIN`). The local entry point. |
| `.github/workflows/remote-qa.yml` | `workflow_dispatch` + weekly-cron CI job. Builds + caches the server, runs the suite. Not a blocking PR gate — see [CI rationale](#why-this-isnt-a-blocking-pr-gate). |
| `crates/lore-vm/examples/live_server_client.rs` + `scripts/live-server-client.sh` | The pre-existing SBAI-4064 single-round-trip spike this harness extends. Still the simplest "is the wire alive?" smoke test. |

### Scenarios the test asserts (over the wire, two users)

Each client (alice, bob) has its **own** local store in a separate temp dir, so
any cross-client visibility is a genuine network round trip — never a shared-store
shortcut.

1. **clone** — bob clones alice's pushed repo; sees her file + revision.
2. **sync — file UPDATE propagation** — alice pushes V2 of a file; bob `sync`s
   and his on-disk copy becomes V2.
3. **sync — file DELETE propagation** — alice deletes a tracked file + pushes;
   bob `sync`s and the file disappears from his working tree.
4. **push CONFLICT** — alice and bob both commit on the same tip; alice's push
   wins, **bob's push is rejected** (stale remote tip → linear history enforced;
   one user can't clobber another).
5. **file CONFLICT state** — after a real divergence, the conflicted file surfaces
   through `file::info`'s `flag_conflict` (the GUI's SCM conflict-badge source).
6. **two-user LOCKS** — alice acquires a lock; `lock::file_query` shows her as
   owner; bob's acquire of the same path is refused / not granted (**contention**);
   alice releases; bob then acquires (**handoff**). Exercises `lock.acquire`,
   `lock.query`, `lock.release`.

---

## How to run

### Fastest: hand it a pre-built `loreserver`

```sh
# Build the upstream server once (from the pinned lore checkout) ...
REV=$(grep -oE 'rev = "[0-9a-f]{40}"' Cargo.toml | head -1 | grep -oE '[0-9a-f]{40}')
CHECKOUT=$(find "${CARGO_HOME:-$HOME/.cargo}/git/checkouts" -maxdepth 2 -type d -name "${REV:0:7}" | head -1)
( cd "$CHECKOUT" && cargo build --release -p lore-server --bin loreserver )

# ... then run the suite against it (the test boots its own server per run):
LOREVM_SERVER_BIN="$CHECKOUT/target/release/loreserver" \
  cargo test -p lore-vm --features remote-integration-tests \
    --test remote_multiuser -- --nocapture
```

### One-shot wrapper (builds the server for you)

```sh
scripts/remote-multiuser-qa.sh
```

Honors `LOREVM_SERVER_BIN` (skip the build), `SKIP_BUILD=1`, and
`CARGO_PROFILE=debug|release`.

### Binary resolution order

The test resolves a `loreserver` without ever triggering a heavy build itself:

1. `LOREVM_SERVER_BIN` env override (CI-friendly, fastest).
2. The pinned `lore` checkout's `target/{release,debug}/loreserver`, **if already
   built** (the test never kicks off a multi-minute build inline).
3. A sibling `loreserver` next to the test executable (the bundled sidecar
   layout).

If none resolve, the test prints `SKIP remote_multiuser: …` and **passes** — a
contributor without the heavy upstream build is never blocked.

---

## Why this isn't a blocking PR gate

The suite needs a built `loreserver` — the **heavy** upstream build (~1 GB) from
the pinned `lore` git checkout. That's the *same* artifact `release.yml` builds
once-per-rev-per-platform and bundles as the Tauri `externalBin` sidecar
(`src-tauri/tauri.conf.json` → `"externalBin": ["binaries/loreserver"]`). Building
it on every PR would add many minutes and a GB of cache to the critical path of
every op change.

So, exactly as `integration.yml` is kept separate from the fast `ci.yml`, the
remote QA job (`remote-qa.yml`) runs **on-demand + weekly**, caches the server
binary keyed on the pinned lore rev, and the test self-skips if the binary is
absent.

**Promotion path:** flip `remote-qa.yml`'s `on:` to include `pull_request` once
the org self-hosted runners cache the `loreserver` sidecar the release pipeline
already produces. At that point the build cost is amortized and the suite can be a
required gate.

---

## Manual QA checklist

Run this against a real two-machine (or two-user) setup before any release that
touches the remote path (`sync`, `push`, `clone`, `lock.*`, merge/conflict
resolution). Host the server via the GUI's **"Host a server"** flow (which spawns
the bundled `loreserver` sidecar — see `src-tauri/src/server_host.rs`) or
`scripts/live-server-client.sh`.

Legend: ☐ = to verify. Use two distinct identities (User A, User B), ideally on
two machines, both pointed at the same `lore://host:port/<repo>`.

### Connect / clone
- ☐ User A: "Host a server" → server reports a `lore://…` URL + green status.
- ☐ User B: clone that URL into a fresh working dir → succeeds, files present.
- ☐ User B's cloned tip revision == User A's last pushed revision.

### Sync — updates
- ☐ A edits a file, stages, commits, pushes. B `sync`s → B's file shows A's edit.
- ☐ A adds a NEW file + pushes. B `sync`s → the new file appears in B's tree.
- ☐ Revision author/message on B match A's commit (metadata crossed the wire,
  not just bytes).

### Sync — deletes
- ☐ A deletes a tracked file (remove from disk → stage → commit → push). B
  `sync`s → the file is **removed** from B's working tree.
- ☐ A renames/moves a file + pushes. B `sync`s → old path gone, new path present.

### Push contention / conflict
- ☐ A and B both commit on the same tip. A pushes first → succeeds. B pushes →
  **rejected** with a clear "remote moved / stale tip" message (not a silent
  clobber).
- ☐ B `sync`s to reconcile; if the changes collide, B sees a **conflict** state
  (SCM badge / `file::info` conflict flag) rather than a silent overwrite.
- ☐ B resolves (mine/theirs/merge) and can then push successfully.

### File locks (the Perforce-class core)
- ☐ A acquires a lock on `path/to/file`. The lock is visible to B via lock query
  / the locks panel, attributed to A.
- ☐ B attempts to acquire the **same** path → refused / shown as held by A
  (contention), not double-granted.
- ☐ B attempts to edit/stage a path A locked → B is warned it's locked by A.
- ☐ A releases the lock. The lock disappears from the query for both users.
- ☐ B can now acquire the freed path (lock **handoff**).
- ☐ Lock messaging: A sends a lock message; B receives it (`lock.message.send` —
  see `docs/lock-messaging-spike.md`).
- ☐ "Acquire as owner" / force paths behave (admin reassigns a stuck lock).

### Branch coordination
- ☐ A creates a branch + pushes it. B sees it in branch list after sync/refresh.
- ☐ Two users on different branches merge into a shared branch; conflicts surface
  and resolve.

### Resilience
- ☐ Kill the server mid-`sync` → client surfaces a clear network error, no
  corrupt local state; re-sync after restart recovers.
- ☐ Client offline → `push`/`sync`/`lock.*` fail clearly (`NoRemote`-class), and
  succeed again once reconnected.

---

## Known gaps / follow-ups

- **Auth.** The harness runs the server **auth-disabled** (no `[server.auth]`
  block), mirroring the local "Host a server" flow. Authed hosting (JWK/issuer +
  token login via `auth::login_with_token`) is not yet exercised end-to-end — add
  an authed variant when authed hosting lands (the `auth` hook already exists on
  `HostServerOptions`).
- **Merge-resolution depth.** The automated push-conflict scenario asserts the
  *rejection* + conflict surfacing; full mine/theirs/merge resolution over the
  wire is covered by the manual checklist and the single-user
  `branch_merge_resolve_*` tests, not yet by the remote suite. Extending it is the
  natural next increment.
- **S3 / composite stores, multi-node topology.** The harness uses local stores
  on one node (the default host config). Cloud/replicated topologies are out of
  scope here.
