# Live lore server ↔ client round trip (SBAI-4064 spike)

**Verdict: YES — LoreGUI can host a real lore server and connect a real client to
it today, locally, with no external infrastructure.** The full
`connect → create → commit → push → clone → verify` loop runs end-to-end over a
genuine QUIC/gRPC network round trip.

This documents the spike that proved it, the exact server-boot + cert + auth
setup that worked, and how to re-run it.

## TL;DR — run it

```sh
scripts/live-server-client.sh
```

That script: builds the upstream `loreserver` binary (from the pinned `lore` git
checkout) and the `lore-vm` client example, writes a localhost-only server
config, boots the server on `127.0.0.1`, runs the client loop, prints a verified
round trip, then stops the server and removes its temp dirs. Exit 0 = verified.

Env knobs: `LORE_PORT` (default `41337`), `KEEP_TMP=1` to keep the temp dir +
server log for inspection.

Sample output:

```
==> starting loreserver on 127.0.0.1:43337 (gRPC+QUIC), http 43339
==> waiting for server to listen. — up (tcp+udp 43337)
[A] repository::create lore://127.0.0.1:43337/spikerepo-3217086
[A] file::stage hello.txt
[A] revision::commit
[A] committed revision 8db01cc7…
[A] branch::push  → lore://127.0.0.1:43337/spikerepo-3217086
[A] pushed branch=main local_rev=8db01cc7… already_pushed=false
[B] repository::clone lore://127.0.0.1:43337/spikerepo-3217086  (separate local store)
[B] cloned repo=… branch=main revision=8db01cc7…
ROUND TRIP VERIFIED:
  revision 8db01cc7… authored by alice
  file hello.txt content matches, over the network
RESULT: SUCCESS — live networked round trip verified.
```

## What this is the proof of

The CI-gated test
`crates/lore-vm/tests/e2e_lifecycle.rs::multi_repo_shared_store_sync_observes_remote_commit`
deliberately uses a **shared on-disk store** (two repos, one store, `offline=1`,
no server) as a stand-in for the networked path, because CI has no live server.
Its own doc comment names the gap:

> The genuine networked path (a QUIC `lore` server + `repository::clone` of a
> `lore://host/repo` URL + token auth + `branch::push`) requires TLS material and
> a live server … `branch::push` itself fails offline with `NoRemote`.

This spike closes that gap. Client B here has its **own separate local store**
and clones **from the server**, so B seeing A's revision/file/author proves a
real network round trip — not a shared-local-store shortcut.

## How the server boots (the setup that worked)

The server is the upstream **`loreserver`** binary (crate `lore-server`, bin
`loreserver`), which is just `server_main(ServerConfig::default())`. It is fully
config-driven via layered TOML (`lore-server/src/settings.rs`):

- built-in `config/default.toml` (baked in) → optional on-disk
  `default.toml`/`<env>.toml`/`local.toml` from `LORE_CONFIG_PATH` → `LORE__*`
  env vars.

The spike writes a `local.toml` (loaded via `LORE_ENV=local`) that is the minimal
single-node, no-infra, no-auth configuration:

```toml
[server.quic]            # QUIC storage service
host = "127.0.0.1"
port = 41337
[server.quic.certificate]
cert_file = "<lore checkout>/lore-server/src/protocol/test_data/test_cert.pem"
pkey_file = "<lore checkout>/lore-server/src/protocol/test_data/test_key.pem"

[server.grpc]            # gRPC revision/repository/lock/branch services
host = "127.0.0.1"
port = 41337             # same port number; gRPC is TCP, QUIC is UDP

[server.http]
host = "127.0.0.1"
port = 41339

[immutable_store.local]  # local filesystem stores — NO S3/MinIO/DynamoDB needed
path = "<tmp>/store"
[mutable_store.local]
path = "<tmp>/store"

[topology]
provider = "none"        # single-node, no Consul
```

Boot:

```sh
LORE_CONFIG_PATH=<tmp>/config LORE_ENV=local loreserver
```

It comes up listening on **TCP 127.0.0.1:41337** (gRPC), **UDP 127.0.0.1:41337**
(QUIC), **TCP 127.0.0.1:41339** (HTTP). No external services required.

### Certs

The shipped test fixtures live at
`lore-server/src/protocol/test_data/{test_cert,test_key,test_ca}.pem` (plus
client/untrusted variants). The QUIC server just needs `test_cert.pem` +
`test_key.pem`.

### Auth — the key enabler

There is **no `[server.auth]` block**, so the server's JWT verifier is `None`
(`lore-server/src/server.rs` ~line 1713) and the gRPC server logs
`Auth: disabled`. No token, JWK, or login is required. This is the documented
dev/no-auth path — the server only enforces JWT when an `auth.jwk` is configured.

### Client trust (why no CA wiring is needed)

The client connects with a **`lore://`** URL (no trailing `s`). In
`lore-transport/src/quic/client.rs`, a scheme **not** ending in `s` sets
`validate_server_certificate = false`, which installs
`SkipServerVerification` — the client does **not** validate the server cert.
So the shipped self-signed test cert works with zero client-side trust setup.
(`lores://` would require the test CA in the client root store.)

## How the client loop runs

The client is a small example, `crates/lore-vm/examples/live_server_client.rs`,
that drives the existing `lore-vm` op bindings **in one process** (important —
staged state lives in the engine session and does not persist across separate
processes). It runs ONLINE (`offline=false`) so push/clone hit the server:

1. `repository::create` `lore://host/<repo>` in client-A's dir (local store).
2. write `hello.txt` → `file::stage` (absolute path + `scan=true`) →
   `revision::commit`.
3. `branch::push` — uploads the revision to the server over the wire.
4. `repository::clone` the same URL into client-B's separate dir.
5. assert B's cloned revision hash == A's pushed hash, file bytes match, and the
   revision author/message (`alice` / "initial commit from alice") match.

`identity` ("alice"/"bob") flows through as the connection identity and revision
author; against the no-auth server any non-empty value is accepted.

## Driving the same loop from the CLI

The spike also wired the three networked ops into the `lorevm` JSON CLI
(`crates/lorevm-cli`): `repository.clone`, `branch.push`, `revision.sync`, plus
`auth.login_with_token`. These were previously implemented in `lore-vm/src/ops/`
but not dispatchable from the CLI.

⚠️ The CLI runs **one op per process**, and lore's staged state is per-engine-session
and is *not* flushed to disk for the next process — so a `file.stage` in one
`lorevm` invocation is invisible to a `revision.commit` in the next (observed:
`status` shows `revision_staged: ""` and commit fails with "Nothing staged").
The multi-step author side (stage→commit) therefore must run in a single process
(hence the Rust example). The CLI ops are still useful for single-shot networked
calls (e.g. `repository.clone`, `branch.push` after a commit made in-process).

## Notes / limits

- **Local-only.** Everything binds `127.0.0.1`. Not wired into CI: the
  `loreserver` binary is built from the pinned upstream `lore` git checkout under
  `~/.cargo/git/checkouts/lore-*/`, which CI runners don't have unpacked.
- First run builds `loreserver` (~1 GB debug binary, several minutes). Subsequent
  runs are incremental.
- Server boot method that worked: the **stock `loreserver` binary + a TOML
  config**, *not* a custom Rust harness linking `QuinnServer` directly (that
  would also require implementing a `StreamHandlerFactory` and wiring stores by
  hand). Note `lore::service::start` (the lore-vm `service.start` op) is an
  upstream **stub** that returns 1 — it does not launch a server, so it can't be
  used to host.
```
