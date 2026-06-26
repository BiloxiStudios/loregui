# StudioBrain ↔ lore Control Plane — Master Design (SBAI-4088, Phase 2)

## 0. Problem statement

The **data plane** is built, merged, and unit-tested but dormant behind `--features lore-write` (sb-lore-writeclient / writegrant / pathmap, `cloud_entity.rs` WriteModeResolver, `pending_lore_writes` outbox, `tenant_lore_configs` table with schema applied on both YB sites, and accounts' EW.2 native write-LSG mint). The scout confirmed a **real tenant cannot set up or connect a lore server through the product**. This epic builds the **control plane** that closes that gap end-to-end: a tenant configures + connects their lore server via wizards/settings, grants write consent (accounts-only iframe), the per-tenant sb-relay endpoint is delivered, the prod build is flipped on, and the whole flow is validated, screenshotted, and documented.

## 1. The six streams, woven

| Stream | Repo(s) | Owns | Canonical IDs here |
|---|---|---|---|
| **A. Cloud backend foundation** (gating contract) | cloud (sb-cloud) | CRUD routes over `TenantLoreConfigStore`, registry construct/inject, status/test-connection, internal grant deposit, build enablement, contract freeze | CP.0, CP.1, CP.1.1, CP.2, CP.3, CP.TC, CP.7, CP.C |
| **B. Cloud web setup UX** | cloud (frontend-cloud) | Lore provider card, LoreConfigForm, wizard step, tenant-settings Lore section, consent-iframe host + bridge | CP.5.1–CP.5.7 |
| **C. Accounts grant/consent + LSG lifecycle** | accounts | Consent endpoint (grant_ref, never token), token resolution, user revoke, repo-bound write tokens, consent page (E2.5), settings tab, entitlement | CP.A0–CP.A9 |
| **D. Per-tenant sb-relay config** | cloud + loregui/app | relay URL grammar/validate, advertise endpoint + Valkey endpoint registry, host-side push + heartbeat, secrets/metrics | CP.URL, CP.6.4–CP.6.6 |
| **E. studiobrain-app desktop/mobile** | app | lore_host sidecar, relay tunnel, tauri client fns + cloud push, wizard/settings UI, consent iframe launch, mobile read-only, build staging | CP.4.0–CP.4.8 |
| **F. Validation, screenshots & docs** | all + docs/landing/loregui.com | feature-on test env, FE/desktop smoke, write round-trip, screenshot suite, three doc sites | CP.8.1–CP.8.9 |

## 2. The reconciled flow (one path, all streams)

```
Tenant (web wizard OR desktop FirstRun)
  └─ picks "StudioBrain Lore" provider
  └─ enters/auto-discovers relay_url (lore://host:port/repo), repo, branch, project_id
        ├─ DESKTOP-HOSTED: app spawns loreserver sidecar (TCP/h2c gRPC) + bore tunnel → advertised lore:// URL
        └─ RELAY-HOST push (loregui/app): POST /api/tenant/lore/relay-advertise (heartbeat) → Valkey live endpoint + tenant_lore_configs
  └─ "Test connection"  → POST /api/tenant/lore-config/test-connection (parse→tcp→authenticated)
  └─ Save → PUT /api/tenant/lore-config (lsg_grant_ref = null) → TenantLoreConfigStore::upsert
  └─ "Connect StudioBrain" → mounts SAME-ORIGIN accounts iframe /account/settings/lore/connect#token=<jwt>?repo&relay&project&write
        accounts: POST /api/lore-grants/consent → mints read+write LSG (native shape), writes envelope to shared Valkey under grant_ref,
                  returns grant_ref (NEVER a token), postMessage sb:lore-connected{grantRef,...}
        accounts → cloud: POST /api/internal/lore/grant {tenant_id, grant_ref}  (X-Service-Secret, grant_ref only)
  └─ cloud: TenantLoreConfigStore::set_grant_ref + registry.upsert_config + get_or_connect → session Connected
  └─ status flips token_expired → connected (GET /api/lore/status)

Write round-trip (proof):
  product edit → PUT /api/entity/* → WriteModeResolver=LoreAuthoritative
    → sb-lore-writeclient (working-copy-less write, pathmap DAM↔tree) → NEW lore revision (source of truth)
    → studiobrain_content YB row + Qdrant point updated; pending_lore_writes created → drained (replay on relay outage)
```

## 3. Security boundary (non-negotiable, per CLAUDE.md + SBAI-1935)

- **accounts is the sole LSG minter and the sole owner of consent/identity/PII UI.** Every other repo embeds the consent page via the **same-origin iframe** (`/account/...`) and NEVER bundles accounts JS.
- **The browser/iframe never sees a raw LSG token** — only the opaque `grant_ref`. The cloud materializes the live token server-to-server (shared Valkey, or `/internal/lore-grants/{id}/token`).
- **Cloud holds the short-lived LSG in Valkey**; accounts keeps at most a short mint-memo. tenant_id is always pinned from the JWT (anti-spoof); `lsg_grant_ref` is never settable from a UI body.
- Cloud frontend/desktop may render **read-only federation analytics** (SessionHealth, last revision) — no privileged actions.

## 4. Build phases (summary; full detail in `sequencing`)

- **Phase 0 — Foundation contract (serial, blocking):** CP.0 build flip → CP.2 registry inject; CP.URL; CP.1 CRUD; CP.1.1 reconcile; ACC.0 repos column; CP.C contract freeze. This freezes the wire so the four UI/consent/relay/app streams can fan out without drift.
- **Phase 1 — Parallel control plane:** backend status/test-connection/internal-grant (CP.3, CP.TC, CP.7); accounts consent lifecycle (CP.A1–A9); cloud frontend (CP.5.1–5.7); desktop/mobile app (CP.4.0–4.8); relay advertise/push/metrics (CP.6.4–6.6).
- **Phase 2 — Prove + publish:** test env (CP.8.1) → FE/desktop smoke (CP.8.2/8.3) → write round-trip (CP.8.4) → screenshots (CP.8.5) → docs across studiobrain-docs, studiobrain-landing, loregui.com (CP.8.6–8.9).

## 5. What this deliberately does NOT do

- Does not resurrect any Python backend. Does not add tenant concepts or feature gates to core. Does not build native consent/billing UI outside accounts. Does not re-derive a feature→tier map (gate on minted `features[]`). Does not treat YB as authoritative — lore markdown remains the source of truth.


---

# Reconciled cross-repo contracts

## Reconciled cross-repo contracts (ALL streams MUST agree)

### C1 — tenant-lore-config schema + REST (cloud OWNS; consumed by frontend-cloud, app, accounts read-back, docs)

**Canonical route prefix:** `/api/tenant/lore-config` (NOT `/api/tenant/lore` — confirmed canonical to avoid the docs/FE/app drift across streams).

- `GET /api/tenant/lore-config[?project_id=]` → 200 `TenantLoreConfig` | 404 `{code:"ERR_LORE_NOT_CONFIGURED"}`
- `GET /api/tenant/lore-config/list` → `[TenantLoreConfig]` (multi-project tenants)
- `POST|PUT /api/tenant/lore-config` body `UpsertTenantLoreConfig` → 200 persisted `TenantLoreConfig`
- `DELETE /api/tenant/lore-config[?project_id=]` → soft-delete (is_active=false) + `registry.evict`
- `POST /api/tenant/lore-config/test-connection` body `{project_id?, relay_url, repo_name, branch?}` → `{reachable, stage:"parsed"|"tcp"|"authenticated"|"repo_resolved", health, error?, error_code?}`
- `GET /api/lore/status[?project_id=]` → status object (below); `GET /api/lore/status/all` (staff, tier≥90)
- `POST /api/internal/lore/grant` (X-Service-Secret, internal ingress only) — see C2
- `DELETE /api/internal/lore/grant` (revoke)
- `POST|DELETE /api/tenant/lore/relay-advertise` — see C3

**`TenantLoreConfig` (serde-verbatim from `lore_config.rs:41-90`):**
```json
{ "id": "...", "tenant_id": "...", "project_id": "default", "relay_url": "lore://host:port/repo",
  "repo_name": "...", "branch": "main", "lsg_grant_ref": null, "is_active": true,
  "created_at": "...", "updated_at": "..." }
```
**`UpsertTenantLoreConfig` (`lore_config.rs:130-149`):** `{ project_id?="default", relay_url(req), repo_name(req), branch?="main" }` — **`lsg_grant_ref` is stripped server-side (never UI-settable); `tenant_id` always from JWT.**

**`GET /api/lore/status` response:**
```json
{ "project_id":"default", "configured":true,
  "health":"connected|reconnecting|offline|token_expired|unconfigured",
  "last_ok_at_secs_ago":12, "last_error":null, "consecutive_failures":0,
  "repo_name":"...", "branch":"main", "relay_host":"host:port" }
```
`health` enum = `SessionHealth::as_str` (`health.rs:42`) — the STABLE wire enum. Response NEVER contains `lsg_grant_ref`, `jwt`, or full relay path — only `relay_host`. `token_expired` ⇒ UI shows the consent CTA.

**Error-code union (frozen):** `ERR_LORE_NOT_CONFIGURED`, `ERR_LORE_INVALID_RELAY_URL`, `ERR_LORE_RELAY_UNREACHABLE`, `ERR_LORE_LSG_REJECTED`, `ERR_FEATURE_NOT_ENTITLED`.

**Entitlement:** gate on `auth.features.contains("lore_federation")` (per SBAI-4165 — `features[]` only, never `plan`/tier map). accounts adds `lore_federation` to `FEATURE_MIN_TIER` (recommended min_tier = **Team**; `lore_relay` stays Enterprise). Single source: `accounts/config/entitlement.py`; accounts mints the resolved set; consumers keep NO local map.

**Persistence rule:** all writes go through `TenantLoreConfigStore::upsert`; ON CONFLICT `(tenant_id, project_id)`. Startup reconcile is idempotent and never CREATEs the base table (human-applied xCluster DDL on both YB sites).

### C2 — LSG / consent flow + native token shape (accounts OWNS; consumed by cloud, frontend-cloud, app)

**`grant_ref := LoreGrant.grant_id` (uuid).** Opaque; safe to transit browser/postMessage; cloud stores it as `tenant_lore_configs.lsg_grant_ref`. **The browser NEVER sees a token.**

**Consent endpoint** `POST /api/lore-grants/consent` (user JWT, entitlement-gated, tenant pinned from JWT):
- req `{repos:[urc-..|bare](≥1), allow_write:bool, lore_remotes:[], relay_url?, repo_name?, branch?="main", project_id?}`
- resp `{grant_ref, tenant_id, scopes:[], repos:[urc-..], allow_write, status}` — **NO token, NO jti.**

**Consent iframe (E2.5, SBAI-1935):** canonical URL `/account/settings/lore/connect` (basePath `/account`, same-origin). JWT via fragment `#token=<jwt>` (SBAI-1957, never logged); connect context via query `?repo=<repo_name>&relay=<relay_url>&project=<project_id>&write=<bool>`. CSP `frame-ancestors` must allow `app.studiobrain.ai`, `tauri://localhost`, `capacitor://localhost`.

**postMessage bridge (source:`accounts`) — single canonical naming:**
- `sb:lore-connected` `{grantRef, tenantId, repos, relayUrl?, repoName?, branch?, projectId?}`
- `sb:lore-connect-cancelled` `{}`
- `sb:lore-disconnected` `{grantRef}`

**Native write-LSG shape (EW.2):** `AuthorizationToken { sub: lore_user_id(user_id), resources:[{resource_id:"urc-<repo>", permission}], is_service_account:false, aud:"lore-service" }`. Read LSG carries `scope[]`. Write tokens require requested repos ⊆ consented `grant.repos`.

**Token materialization (reconciled):** accounts writes the LSG envelope `{"jwt":"...","ttl_secs":N}` into **shared Valkey** under the key named by `grant_ref`, in the EXACT shape `ValkeyLsgProvider::parse_stored` (`token.rs:185-202`) expects, on consent + on refresh. accounts also notifies cloud via `POST /api/internal/lore/grant {tenant_id, project_id?, grant_ref}` (grant_ref ONLY) so cloud attaches the ref + triggers immediate `get_or_connect`. `POST /internal/lore-grants/{grant_id}/token` (X-Service-Secret) is the cold-miss re-mint cloud calls if the Valkey key is absent. Optional `token_envelope` on `/api/internal/lore/grant` is a secondary path; primary is accounts-writes-Valkey.

**Revoke:** user-facing `POST /api/lore-grants/{id}/revoke` (owner/admin or creator; cross-tenant→404) + internal revoke → blacklist grant_id+jti, DEL Valkey key + mint-memo, publish `tenant:{id}:sync {"type":"lore_grant_revoked","grant_id":..}`; cloud subscribes and evicts the session.

### C3 — relay config + protocol (relay stream + loregui-relay OWNS grammar; cloud parses)

**Relay URL grammar:** `lore://<relay-host>:<port>[/<repo>]`, TLS variant `lores://`. Emitted by `TunnelHandle::public_url()` (loregui-relay); parsed by `endpoint_from_relay_url()` → `http(s)://host:port` (repo segment dropped). Explicit non-zero port required (bore always assigns one). `validate_relay_url()` enforces scheme + host **allowlist** (`LORE_RELAY_HOST_ALLOWLIST`, default `relay.studiobrain.ai`) to block SSRF.

**Transport constraint (load-bearing):** bore is **TCP-only h2c**; cloud dials `http://` (no TLS). A desktop-hosted loreserver MUST expose a **plaintext TCP gRPC (h2c)** listener on the tunneled port — NOT loreserver's default QUIC/UDP 41337.

**Advertise endpoint** `POST /api/tenant/lore/relay-advertise` (JWT, tenant-scoped): req `{project_id?, relay_url, repo_name, branch?}` → `{tenant_id, project_id, relay_url, endpoint, health, expires_at}`. Idempotent upsert + heartbeat (re-POST at TTL/2). `DELETE` clears + evicts.

**Valkey live-endpoint key** `lore:endpoint:{tenant_id}:{project_id}` → `{relay_url, repo_name, branch, advertised_at}`, TTL `LORE_ENDPOINT_TTL_SECS` (default 90s), plus `lore:endpoint:active` SET index. Mirrors `fileserver_registry.rs`. Durable seed stays in `tenant_lore_configs`; Valkey is the live/heartbeat truth.

**Env/feature contract:** prod build = `cargo build --release --features lore-transport,lore-write`. `lore-transport` ⇒ `LoreClientFactory` compiles ⇒ `TenantLoreRegistry<LoreClientFactory>` is the concrete Extension type. Env: `LORE_REGISTRY_MAX_SESSIONS`, `LORE_RELAY_HOST_ALLOWLIST`, `LORE_ENDPOINT_TTL_SECS`, `BORE_RELAY_HOST`. Cloud needs NO bore secret (dials the assigned public port); the HOST resolves `BORE_SECRET` from Azure KV `bore-relay-secret` via ESO.

### C4 — screenshot asset manifest (validation stream OWNS)

8 canonical PNGs, web shots 1440×900, desktop native: `lore-firstrun-option`, `lore-cloud-onboarding-option`, `lore-tenant-settings-config`, `lore-consent-iframe`, `lore-connected-state`, `lore-write-roundtrip`, `lore-desktop-connect`, `lore-config-error`. Distributed to `docs-repo/public/screenshots/lore/`, `landing/.../public/lore/`, `loregui/website/public/screenshots/` (studiobrain- prefix). Demo seed only (no PII/tokens).


---

# Build sequencing

## Build phases / critical path

### Phase 0 — Foundation contract (SERIAL, blocking everything; one squad)
The whole fan-out depends on a frozen wire. Order:
1. **CP.0** (enable lore build) — unblocks the registry type. **CP.URL** (relay validate) in parallel.
2. **CP.2** (construct/inject registry) — needs CP.0. **CP.1** (CRUD routes) — needs CP.URL; can land feature-gated alongside CP.2.
3. **CP.1.1** (reconcile), **CP.A0** (accounts repos column) — in parallel.
4. **CP.C** (contract freeze: docs + TS types + serde conformance) — once CP.1/CP.3/CP.TC/CP.7 shapes are settled. This is the seam that lets the four downstream streams build without drift; ship it early even if the backend stories are still in review, then keep it as the drift-gate.

**Critical path runs through CP.0 → CP.2 → CP.1 → CP.C.** Nothing in the UI/app/consent/relay streams should start its wire-coupled work until CP.C is published.

### Phase 1 — Parallel control plane (4 squads, fan out after CP.C)
- **Squad Backend:** CP.3 (status) → CP.TC (test-connection) → CP.7 (internal grant). All depend on CP.1+CP.2.
- **Squad Accounts (security-isolated):** CP.A1 → CP.A7 (entitlement) → CP.A2 (token resolution) → CP.A5 (revoke) + CP.A6 (repo-bound) → CP.A3 (consent page, the long pole) → CP.A4 (settings tab) → CP.A8 (tests) → CP.A9 (routers/ADR). CP.A3 is the gating dependency for both UI streams' consent entry.
- **Squad Relay:** CP.6.4 (advertise + Valkey endpoint registry, needs CP.URL+CP.TC) → CP.6.5 (host push) → CP.6.6 (secrets/metrics).
- **Squad Cloud Web:** CP.5.1 → CP.5.2 → CP.5.6 (needs CP.A3) → CP.5.3 → CP.5.4 + CP.5.5 → CP.5.7.
- **Squad App:** CP.4.0 → CP.4.1 → CP.4.2 (needs CP.1); CP.4.5 (needs CP.A3); CP.4.3/CP.4.4/CP.4.6 (need CP.4.2+CP.4.5); CP.4.8 (build, needs CP.4.0); CP.4.7 (e2e/docs, needs all 4.x).

**Phase-1 critical path = CP.A3 (consent page).** Both UI streams' consent entry (CP.5.6, CP.4.5) and the screenshot suite block on it — prioritize the accounts consent page + the postMessage bridge contract first within the accounts squad.

### Phase 2 — Prove + publish (after Phase 1 lands enough to run)
1. **CP.8.1** (feature-on test env) — needs CP.0, CP.1, CP.2, CP.6.4, CP.A1.
2. **CP.8.2** (cloud FE smoke) + **CP.8.3** (desktop validate, needs CP.A3) in parallel.
3. **CP.8.4** (write round-trip) — the headline acceptance gate; needs 8.1/8.2/8.3.
4. **CP.8.5** (screenshots, needs CP.A3 for the consent shot) → **CP.8.9** (manifest/distribute).
5. **CP.8.6** (Nextra, needs CP.C), **CP.8.7** (landing), **CP.8.8** (loregui.com) in parallel off the screenshot manifest.

### One-line critical path
`CP.0 → CP.2 → CP.1 → CP.C → CP.A3 → CP.5.6/CP.4.5 → CP.8.1 → CP.8.4 → CP.8.5 → CP.8.6/8.7/8.8`
