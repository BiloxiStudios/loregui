# LoreGUI × StudioBrain cloud sync — scoping design

**Status:** scoping only — do not implement until the owner decisions in §8 land.
**Ticket:** SBAI-7200
**Date:** 2026-08-16
**Author:** sb-lore-sync (dispatched by brain-chat)
**Branch / worktree:** `feature/cloud-sync-scoping` @ `/srv/AI_Stuff/sbcrew/sb-lore-sync-wt`
**Pinned lore:** Epic `lore` rev `9664606f5` (workspace `Cargo.toml`)
**Related ADRs (already accepted):** [0001](adr/0001-studiobrain-lore-federation.md), [0002](adr/0002-lore-as-primary-content-backend.md), [0004](adr/0004-lore-control-plane-design.md)

Owner ask (bizanator, via brain-chat): a checkbox in the desktop install / host flow that auto-configures a lore server to sync with StudioBrain cloud — *"unlimited local storage with version control mounted to the StudioBrain cloud like magic."*

This document answers the four questions from the actual code, not the marketing copy.

---

## 0. Verdict in one page

| Question | Answer |
|---|---|
| 1. Does topology & mTLS replication already do server-to-server sync? | **Yes, but it is a lore-cluster feature, not "mount to StudioBrain."** Peers must be mutually reachable over QUIC (UDP) and/or internal gRPC (TCP) with mTLS. It will not punch NAT. It is the wrong primitive for this product checkbox. |
| 2. Can we tunnel lore replication through a Cloudflare Worker/DO WebSocket? | **Not the native replication protocol.** QUIC-over-WebSocket is a non-starter. gRPC/h2c byte-stream over an outbound WS rendezvous *can* work, and that is the same shape as the **already-shipped bore relay** (SBAI-4072). PR #767 (event-tap WS) is infrastructure precedent, not a lore-protocol tunnel. |
| 3. Can loreserver run behind the DO? | **No, not in the Worker/DO.** loreserver is a persistent stateful Rust binary (gRPC+QUIC+HTTP, local/S3 store). The desktop sidecar already runs it. A cloud replica, if ever wanted, is a **container on k3s** (SBAI-5461 already did headless Linux). The DO is only a rendezvous. **ADR-0001 forbids hosting per-tenant lore servers in StudioBrain cloud.** |
| 4. Where does the checkbox live, and what does settings store? | **Host-flow Basic mode**, next to the existing relay hook in `ServiceSetup.tsx` — not Expert topology. Settings go on `HostedServerProfile` / `ContextSettings` (`src-tauri/src/context.rs`), never raw tokens. `ServerSource::StudioBrain` already exists. |

**Recommended product shape (pending owner call in §8):** the checkbox does **not** stand up a cloud loreserver and does **not** turn on lore's cluster-replication endpoints. It is the missing *one-click glue* over work that already exists:

1. Host local `loreserver` (unlimited local disk) — shipped.
2. Open an outbound tunnel so the cloud can reach it through home NAT — **bore is shipped**; a CF WS rendezvous is an optional replacement, not a prerequisite.
3. Advertise `lore://relay-host:port/repo` to `/api/tenant/lore/relay-advertise` — shipped on the cloud side.
4. Mint the LSG via the accounts consent iframe — designed, largely built.
5. Cloud stays a **per-tenant lore client** (index + write-back). Bytes stay on the tenant machine (and optionally on the tenant's own S3/R2).

That *is* "unlimited local storage with version control, mounted to StudioBrain." It is also what ADR-0001/0002 already decided. Building a second sync stack would fight those ADRs and lore's own protocol.

---

## 1. What is actually in the code

### 1.1 LoreGUI host flow (the right surface)

Onboarding is two cards (`frontend/src/onboarding/ModeSelect.tsx`): **Connect to a Lore Server** or **Set Up / Host a Server**. Host is:

`BackendPicker` → `ValidateConnectivity` → `InitStore` → `ServiceSetup`

`ServiceSetup` already has:

- **Basic** — one store path (from the previous step), start/stop, advertised `lore://` URL.
- **Expert** — all 54 lore-server options, including **Topology & replication**.
- **"View generated config"** — `host_server_render_config` dry-runs the TOML.
- A **cross-network relay slot** (`getRelayControl()`). Open core registers nothing; a `loregui-cloud` overlay mounts the bore toggle when the studio is entitled to `lore_relay`.

The host backend (`src-tauri/src/server_host.rs`) generates TOML, resolves the bundled `loreserver` sidecar, binds **`127.0.0.1` by default**, and exposes `advertised_url` so a relay overlay can replace the loopback URL. Auth is off in the local/no-auth first-run path.

`lore::service::start` is still an **upstream stub**. The real server is always the standalone `loreserver` binary.

### 1.2 Settings (`settings.rs` + `context.rs`)

`AppSettings` today:

| Field | What it is |
|---|---|
| `autostart_enabled` | login autostart |
| `close_to_tray` | hide vs quit |
| `active_repository` | last validated **local working-tree path** (never a remote URL) |
| `context` | `ContextSettings` v1 |

`ContextSettings` already has the hooks this feature needs:

- `ServerSource::StudioBrain` (enum variant exists, unused by the host checkbox).
- `ServerProfile.credential_ref` — opaque OS-store handle. Raw tokens are rejected (`validate_no_raw_secrets`, `deny_unknown_fields`).
- `HostedServerProfile` — `{id, display_name, store_path, advertised_url, last_configured_at}`.

There is **no** `cloud_sync_enabled` flag yet. Adding fields requires a schema bump or a new nested struct under `HostedServerProfile` (also `deny_unknown_fields`).

### 1.3 StudioBrain federation that already exists

This is not a green field. The control-plane design (ADR-0004 / SBAI-4088) is:

```
desktop LoreGUI ──hosts loreserver──► bore (TCP/h2c) ──► cloud sb-lore-client
   (tenant bytes)     lore://relay.studiobrain.ai:<port>/repo
                      POST /api/tenant/lore/relay-advertise  (heartbeat TTL 90s)
```

Shipped pieces:

| Piece | Where | Ticket |
|---|---|---|
| Host `loreserver` sidecar | loregui `server_host.rs` | SBAI-4065 / 4069 |
| bore TCP relay crate | `loregui-cloud/crates/loregui-relay` | SBAI-4072 |
| advertised-URL seam | `HostStatus.advertised_url` | SBAI-4072 |
| `tenant_lore_configs` + advertise API | studiobrain-cloud | CP.1 / CP.6.4 |
| Lore host controller / health | cloud | SBAI-5469 |
| Desktop always-on + bore on boot | studiobrain-app | SBAI-4603 |
| Hibernatable CF WebSocket (event-tap) | cloud PR #767 | SBAI-6984 |
| Desktop-as-fileserver via cloudflared | studiobrain-app | SBAI-6816 (PendingAgent) — **files, not lore VCS** |
| S3/R2 design constraints | loregui docs | SBAI-6756 (Done, capture-only) |
| Headless Linux loreserver + remote mgmt | | SBAI-5461 (Done) |

ADR-0001 (owner-ratified): *StudioBrain does NOT host per-tenant lore servers. Cloud is a per-tenant READ (now write-facade) CLIENT. Tenant storage stays the master.*

ADR-0002 (owner-ratified): *lore is the sync + versioning backbone for desktop-hosted tenants; BYO-storage stays the serving layer. Complementary, not a swap.*

---

## 2. Q1 — What lore "topology & mTLS replication" actually is

Read from pinned lore `9664606` (`lore-server/src/{topology,store,grpc,quic,settings,server}.rs`) and LoreGUI's renderer (`server_host.rs` `render_topology` / `render_internal_endpoint`).

### 2.1 Three different things people call "sync"

Do not collapse these. They are different layers.

| Layer | What it does | Protocol | NAT-friendly? |
|---|---|---|---|
| **Client `revision.sync` / `branch.push`** | Working-copy ↔ *one* server. This is git-pull/push. | Public QUIC + public gRPC (JWT) | Only if the client can dial the server |
| **Topology** (`none` / `fixed` / `rotating_id_fixed` / `consul`) | Peer *discovery* for a multi-node lore cluster. A static list of `{address, port, locality}`. | Used by the cluster to find replicas | **No** — peers must be routable |
| **Internal replication endpoints** | Server-to-server CAS / fragment copy. `quic_internal` (UDP, `ReplicationStoreService`, **no JWT**, blanket partition access) and `grpc_internal` (TCP, `ReplicationService` + forwarded revision). Default **off**. mTLS required unless explicitly disabled (warned as unsafe). | QUIC/UDP 41340 and/or gRPC/TCP 41340 | **No** |
| **`ReplicatedStore`** | An `ImmutableStore` that *forwards every op* to a remote lore server over the internal QUIC replication client. For edge-region servers that should not talk to S3 themselves. | Internal QUIC + mTLS | **No** — the edge dials the core |
| **`composite` store** (wizard does **not** expose this) | Local cache tier + durable `aws`/S3 tier with `ReplicationMode` `{Read, Write, ReadWrite}`. | S3 + DynamoDB SDK, not lore-to-lore | Yes, if both sides share the bucket — but see §2.4 |

Topology is **not** "mirror my laptop to the cloud." It is "here are my cluster peers, all of which I can already reach."

### 2.2 How the internal endpoints work

From `lore-server/src/server.rs` and `grpc/replication_service.rs`:

- `quic_internal` hosts `ReplicationStoreService` with **blanket storage access and no JWT**. Startup refuses to boot it without mTLS unless `verify_client_certs=false`, and then it logs a "only safe on isolated networks" warning.
- `grpc_internal` hosts `ReplicationService` (`Put` streaming fragments into the **local** immutable store — constructor errors if the store is not local) plus a forwarded revision service.
- Default ports: public QUIC/gRPC `41337`, HTTP `41339`, internal `41340`.
- Public QUIC uses JWT (`verify_client_certs = false` by default). Internal endpoints are the opposite: identity is the client cert.

A cloud replica using this path would need:

1. A second `loreserver` with a reachable address.
2. Provisioned mTLS certs on both sides.
3. Topology peer entries pointing at that address.
4. Either the cloud dialing the home server (blocked by NAT) or the home server dialing the cloud (possible) as a `ReplicatedStore` / replica factory client.

That last option is the only NAT-compatible use of *native* replication: **the local server is the client, the cloud replica is the server.** It still requires a cloud-hosted `loreserver` per tenant (or a shared one), which ADR-0001 rejected, plus mTLS provisioning the host wizard does not do.

### 2.3 Pre-existing wizard bug (ticket separately)

LoreGUI Expert mode renders:

```toml
[server.replication]
enabled = true
```

Upstream `ServerSettings` has **`grpc_internal`**, not `replication`:

```toml
[server.grpc_internal]
enabled = false
```

`#[serde(deny_unknown_fields)]` is commented out on `ServerSettings`, so `[server.replication]` is **silently ignored**. The Expert "gRPC replication" toggle does not enable the internal endpoint. `quic_internal` *is* named correctly.

This is not a blocker for the product checkbox (we should not be flipping these for cloud sync anyway). It is a real Expert-mode footgun. Filed as **SBAI-7201**.

### 2.4 S3/R2 is not a free "just share the bucket" replica

`docs/domains/storage.md` and `S3StoreOptions` are explicit: lore's `aws` immutable store is **S3 + DynamoDB**. Fragment payloads go to S3; fragment associations + metadata go to DynamoDB. There is **no S3-only immutable store**. The mutable (branch-pointer) store in the host wizard stays local because the `aws` mutable store is a dedicated DynamoDB table the wizard does not provision.

So "auto-back CAS to tenant R2" is not a checkbox. It needs a DynamoDB-compatible metadata service (real DynamoDB, DynamoDB Local, Scylla Alternator, …) plus the SBAI-6756 rules (content-addressed/UUID keys, never sequence-derived; `tenant_lore_configs` is a V4249 table — no boot-time DDL).

Composite/replicated store modes are documented as deferred in `storage.md` for exactly this reason.

---

## 3. Q2 — WebSocket rendezvous vs lore's protocols

### 3.1 What PR #767 actually landed

Cloud PR #767 (`SBAI-6984`, merged 2026-08-16) is `GET /ws/event-tap`: a hibernatable Durable Object WebSocket for **entity-event taps** (`ping`/`pong`, topic subscribe, owner/admin + tenant/project auth). It is small JSON frames on a hibernating DO. It is **not** a byte-stream mux, not gRPC, not QUIC, and not sized for fragment replication.

Useful as: auth pattern, hibernation lifecycle, DO-per-project routing. Not reusable as the lore transport.

### 3.2 Protocol-by-protocol

**QUIC (UDP, public or `quic_internal`).** Cannot be tunneled through a WebSocket without rewriting lore-transport. QUIC owns its own TLS, congestion control, connection IDs, and UDP path. Wrapping it in WS/TCP defeats it and lore has no "QUIC-over-WS" mode. bore already documented this: *"bore is TCP-only; it cannot carry lore's QUIC transport."* A Cloudflare Worker cannot speak UDP to a home client in any case.

**Public gRPC (TCP, HTTP/2, JWT).** This is the path every existing StudioBrain integration uses. Clients connect with `lore://` (no trailing `s`) which **disables server-cert validation** (`SkipServerVerification`). bore tunnels this today as raw TCP. A WS rendezvous can do the same *if* it is a **raw byte pipe** (or HTTP/2 CONNECT) from the cloud dialer down to the open socket — not "put gRPC messages in JSON WS frames."

Constraints if we replace bore with a CF Worker/DO:

| Constraint | Why it matters |
|---|---|
| Workers/DOs have tight CPU / 128 MB / I/O limits | A multi-GB asset push will evict or time out a hibernating DO |
| Hibernation drops in-memory stream state | gRPC streams and HTTP/2 connections cannot hibernate mid-transfer |
| No client-cert (mTLS) passthrough | Internal replication endpoints die at the proxy; public JWT gRPC is fine |
| Worker cannot be the lore peer | It can only forward bytes to something that *is* a lore peer |
| Cloudflare has ~100s request limits on some paths | Long-lived streams need the hibernatable WS API, which PR #767 uses — but hibernation ≠ large binary |

**Internal replication gRPC / QUIC.** Same problems, plus mTLS. Do not tunnel these through a Worker.

### 3.3 Recommendation on reachability

There are two viable NAT-safe designs. They are alternatives, not a stack.

**A. Keep bore (default, already shipped).** Desktop opens outbound TCP to `relay.studiobrain.ai:7835`, bore assigns `10000–19999`, cloud dials `http://relay.studiobrain.ai:<port>` (h2c). Matches ADR-0004 §C3. Cloud needs no secret. Host needs `BORE_SECRET`. QUIC is LAN-only.

**B. CF Worker/DO outbound-WS rendezvous (owner-approved tonight).** Desktop (or a tiny sidecar next to loreserver) opens `wss://…/ws/lore-tunnel`, keeps it alive. Cloud's `sb-lore-client` dials the DO; the DO copies bytes onto the parked socket. Same *shape* as fileserver/cloudflared (SBAI-6816) and event-tap (PR #767), but it must be a **binary pipe**, not event-tap JSON, and it must **not hibernate while a gRPC stream is open**.

B is justified if we want to retire bore, drop the HMAC shared secret, or run on the same CF edge as the rest of auth. It is **not** required to ship the checkbox. A already exists.

**Do not** try to carry `quic_internal` / topology-peer replication through either tunnel.

---

## 4. Q3 — Where loreserver runs

| Place | Fit | Notes |
|---|---|---|
| **Desktop LoreGUI sidecar** | **Yes — this is the product.** | Already bundled (`externalBin`). Always-on OS service exists in studiobrain-app (SBAI-4603). This is the "unlimited local storage." |
| **Cloudflare Worker / Durable Object** | **No.** | Cannot run the binary, cannot hold the store, cannot speak QUIC, cannot stream multi-GB CAS. |
| **Container behind the DO (k3s / Fly / etc.)** | Technically yes, product-no by default. | Upstream has `lore-server/DOCKER.md` (TCP+UDP 41337, `/data` volume). SBAI-5461 already stood up a headless Linux loreserver + remote management. A per-tenant cloud replica would live here, **not** in the DO. ADR-0001 rejected this as the architecture. Revisit only if the owner explicitly wants a cloud replica (decision D2). |
| **The DO itself** | Rendezvous only. | Holds the open socket + authz. Never the store. |

The desktop app is the outbound-connecting *client of the rendezvous*. The `loreserver` child is a separate process the app already supervises. Do not merge them. Do not put the store inside the Tauri renderer.

If we ever run a cloud replica, it is one container per tenant (or per project) on our k3s, with its own volume or the tenant's S3+DynamoDB, and the **local** server dials **it** (outbound). The DO is still only the way *StudioBrain's client* reaches the *desktop* server, not the replica's transport.

---

## 5. Q4 — UI hook and settings

### 5.1 Where the checkbox goes

**Host flow, Basic mode, `ServiceSetup.tsx` — before Start Hosting.**

Not Expert → Topology. Topology is a cluster-ops surface. This is a product offer.

Suggested copy (docs-writer / ux-designer to finish):

- Label: **Sync with StudioBrain Cloud**
- Help: *Keep this repository on this computer (unlimited local disk) and make it available in StudioBrain on the web and other devices. Your machine opens an outbound connection — no port forwarding.*
- Off by default. Local/LAN host stays the zero-config path (IA: "StudioBrain sign-in is never required for local or LAN Lore").
- When on and the user is not signed in: the existing Premium / accounts iframe, then continue.
- When on and entitled: starting the server also opens the tunnel, advertises the URL, and writes the hosted-server profile with `source = StudioBrain`.
- After start: show the advertised `lore://` URL **and** a status line (`Connected to StudioBrain` / `Waiting for sign-in` / `Tunnel down — local only`). Reuse the existing relay-control slot rather than a second toggle.

The current relay UI is a **premium overlay** gated on `lore_relay`. The owner's "unlimited local storage" wording (also used on SBAI-6816) sounds **free-plan**. That is decision D3. If it is free, the checkbox lives in open-core Basic mode and calls a core-safe "advertise + tunnel" API; the overlay becomes an implementation, not a paywall. If it stays premium, keep the existing entitlement gate and just make the checkbox the discoverable on-ramp.

Do **not** add a third onboarding card. IA is two-card (connect / host). Cloud sync is a modifier on Host.

Connect-mode is out of scope for the checkbox. A user who only wants to consume an already-synced server uses Connect with the advertised URL, or opens the project from StudioBrain.

### 5.2 What `settings.rs` / `context.rs` should store

Extend `HostedServerProfile` (schema v1 → v2, or a nested optional bag so old files still parse — today's `deny_unknown_fields` will otherwise fail-closed and `.bak` the file).

Proposed persistable fields (all non-secret):

```text
HostedServerProfile {
  …existing…
  cloud_sync: Option<CloudSyncSettings>
}

CloudSyncSettings {
  enabled: bool
  tenant_id: Option<String>          // from JWT, not typed in
  project_id: Option<String>         // StudioBrain project
  repo_name: String
  branch: String                     // default "main"
  advertised_url: String             // lore://… (also on the parent)
  tunnel_kind: "bore" | "cf_ws" | "none"
  last_heartbeat_at: Option<String>
  credential_ref: Option<String>     // OS-store handle for LSG / session — NEVER the token
}
```

`AppSettings` itself does not grow random flags. Autostart of the *server* is already a desktop concern (SBAI-4603); if the checkbox should survive logout, hook it to that existing always-on path rather than a new settings key.

`active_repository` stays a **local working-tree path**. The hosted store path is `HostedServerProfile.store_path`. Do not conflate them (IA: "Server store is not a client local path").

### 5.3 What happens when the box is checked (happy path)

```
[x] Sync with StudioBrain Cloud
        │
        ├─ if no session → accounts iframe (existing SBAI-1935 bridge)
        ├─ host_server_start(store_dir, repo)          // existing
        ├─ open tunnel (bore today; cf_ws later)       // existing overlay / new
        ├─ host_server_set_advertised_url(public)      // existing seam
        ├─ POST /api/tenant/lore/relay-advertise       // existing cloud
        ├─ consent iframe if no LSG grant_ref          // existing
        └─ persist HostedServerProfile.cloud_sync
```

Heartbeat: re-POST advertise every ~45s while the server is up (TTL 90s). On stop: DELETE advertise, drop tunnel, leave the durable `tenant_lore_configs` row unless the user turns the checkbox off.

No Expert topology fields are written. No `[server.quic_internal]`. No `[server.grpc_internal]`.

---

## 6. What "mounted like magic" should mean (product)

Reconcile the owner's sentence with the ADRs:

> unlimited local storage with version control mounted to the StudioBrain cloud like magic

| Phrase | Mechanism |
|---|---|
| unlimited local storage | `loreserver` on the desktop, local filesystem store. Cloud does not pay for bytes. |
| with version control | lore revisions / branches. Already the host flow. |
| mounted to StudioBrain cloud | Cloud federates as a lore **client** over the tunnel. Web/phone/DAM see the repo. Writes go back through lore (EW write facade) when the desktop is online. |
| like magic | One checkbox. No TOML, no port-forward, no peer list, no cert files. |

When the desktop is **off**:

- Search / previews stay up from the YB index + Garage preview cache (ADR-0001).
- Full-res bytes and new writes wait for reconnect (ADR-0002).
- That is the accepted trade-off. SBAI-6816 (cloudflared fileserver) and SBAI-6884 (R2 visibility) are the parallel "keep serving files when we want to" tracks — they are **not** lore replication.

If the owner later wants bytes available with the desktop off, the next increment is **tenant R2/Wasabi as the lore CAS backend** (with a DynamoDB-compatible metadata store — §2.4), not a cloud `loreserver` and not topology-peer replication.

---

## 7. Scope estimate

Do not start a "build lore sync" epic. Split after the owner calls.

### Phase 0 — this doc (done)

Scoping. No code.

### Phase 1 — one-click glue (the actual checkbox) — **S / ~3–7 days**

LoreGUI + small cloud/accounts wiring. No new protocol.

- Basic-mode checkbox + copy + empty/error/offline states (ux-designer + frontend-engineer).
- Settings schema bump on `HostedServerProfile`.
- Call existing relay control when checked; fail clearly if overlay/secret missing.
- POST/DELETE `/api/tenant/lore/relay-advertise` + 45s heartbeat from the desktop.
- Consent iframe if needed.
- Help blurb / tutorial (`write-tutorial`).
- Tests: settings round-trip, start-with-flag-opens-tunnel (mock), advertise heartbeat, fail-closed on secret-in-settings.

Depends on: bore overlay present in the StudioBrain-composed build; accounts consent page (CP.A3) live.

### Phase 2 — optional CF WS rendezvous — **M / ~2–3 weeks**

Only if we want to retire bore or the owner insists on the PR #767 pattern for lore.

- New `GET /ws/lore-tunnel` (binary, no hibernation while a stream is active).
- Desktop outbound client (Tauri/Rust, next to `server_host`).
- Cloud `sb-lore-client` dials the DO instead of `http://relay.studiobrain.ai:port`.
- Allowlist / auth same as event-tap (owner/admin + tenant/project).
- Load test: a real `branch.push` of a >100 MB revision, reconnect mid-stream.

This is a **transport swap**. The checkbox and settings do not change.

### Phase 3 — do **not** schedule unless D2 = yes

Cloud-hosted loreserver replica + native topology/mTLS. Would include: per-tenant k3s container, cert provisioning, `[topology] provider=fixed` auto-fill, local server as `ReplicatedStore` client, DynamoDB-compatible metadata if S3-backed. Conflicts with ADR-0001. Estimate **L / 6+ weeks** and a new ADR that *supersedes* 0001 §2.1.

### Pre-existing bug (unrelated, file now)

Expert TOML emits `[server.replication]` instead of `[server.grpc_internal]`. Small fix, own ticket.

---

## 8. Owner decisions (block implementation)

Reply on `group:studiobrain` is enough. Until these land, no implementation ticket should leave PendingPlan.

**D1 — Confirm the product is federation-over-tunnel, not a cloud lore replica.**
Recommended: **yes, federation.** Matches ADR-0001/0002 and the "unlimited *local* storage" wording. A cloud replica is a different product (and a different bill).

**D2 — If D1 is no: are we willing to host per-tenant `loreserver` containers?**
This reverses ADR-0001 §2.1. Needs an explicit ADR amendment, a storage bill, and a cert story. Default: **no**.

**D3 — Free plan or `lore_relay` premium?**
SBAI-6816's sibling sentence is free-plan ("as long as their desktop is on they have unlimited storage from phone or web"). The existing LoreGUI relay toggle is premium. These cannot both stay true without a call.

**D4 — Reachability: keep bore, or build the CF WS rendezvous first?**
Recommended: **ship Phase 1 on bore**, schedule Phase 2 only if we want to drop bore. The checkbox should not wait on a new tunnel.

**D5 — Offline bytes: index/preview-only (current ADR), or fund S3+DynamoDB CAS so web/phone can read originals with the desktop off?**
Recommended: **not in v1.** Point at SBAI-6884 / SBAI-6756 if we want it later.

**D6 — Surface: LoreGUI only, or also the studiobrain-app FirstRun "Lore" card (CP.4.3)?**
Recommended: **same checkbox copy in both**, one settings schema. App already hosts lore + bore (SBAI-4271/4272/4603). Doing LoreGUI-only leaves the composed installer with two different host UIs.

---

## 9. What this is not

- Not a new VCS. `revision.sync` / `branch.push` already move revisions.
- Not SBAI-6816 (cloudflared **file** serving). Complementary: 6816 serves a project file tree; this serves a lore repo.
- Not SBAI-6884 (R2 as a universal visibility layer). Complementary storage-cache work.
- Not "auto-fill topology peers and flip mTLS on." That is how you build an Epic-style multi-DC lore cluster on a LAN or with real public IPs, not a home desktop.
- Not running loreserver on a Worker.

---

## 10. Six-gates record (2026-08-16)

1. **Jira.** No existing ticket for this checkbox. Closest: SBAI-4088 (federation epic), SBAI-4072 (bore), SBAI-5469 (host controller), SBAI-5461 (headless server), SBAI-6756 (S3 constraints), SBAI-6816 (fileserver tunnel), SBAI-6884 (STORAGE_CACHE), SBAI-6984 (event-tap WS). Filed this scoping + the TOML-key bug as follow-ups.
2. **Docs.** ADRs 0001/0002/0004, `docs/domains/storage.md`, `docs/live-server-client-spike.md`, `docs/host-server-sidecar.md`, `cloud/docs/lore-relay-cp6.md`.
3. **Memory.** Knowledge search hits were docs-sync / relay chatter, not a prior design for this checkbox. MemPalace HTTP endpoint 404 on this host.
4. **GitHub.** loregui PRs for relay/host/S3 docs (300, 306, 354, 464). cloud #767 (event-tap WS, merged). No open PR for this feature.
5. **GitNexus.** MCP not attached in this session. Impact read was done via the pinned lore checkout + loregui `server_host.rs` / `context.rs` / onboarding.
6. **Worktree.** Fresh from `origin/main` `866ead9` (v0.1.6) at `/srv/AI_Stuff/sbcrew/sb-lore-sync-wt`, branch `feature/cloud-sync-scoping`. Shared checkout `/srv/studiobrain-dev/loregui` was on `worker/sbai-6636` and was not used.

---

## 11. Implementation notes for whoever picks this up (after D1–D6)

- Spawn `loregui-ux-designer` for the checkbox copy/states and `loregui-frontend-engineer` for the ServiceSetup hook. Do not bury it in `AdvancedServerConfig.tsx`.
- Spawn `loregui-storage-expert` only if D5 opens the S3/composite path.
- Coherence: DESIGN-SYSTEM tokens, IA two-card host flow, help description on any new palette entry, design-review before PR.
- Keep secrets out of `settings.json`. Follow the existing `credential_ref` + `validate_no_raw_secrets` path.
- If Phase 2 happens, put the WS client in loregui-cloud (proprietary) or studiobrain-app — not in MIT loregui core — unless the owner wants the rendezvous as an open protocol.
- Do not "fix" topology as part of the checkbox PR.
