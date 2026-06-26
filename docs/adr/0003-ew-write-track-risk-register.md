# EW.6.1 — StudioBrain ↔ lore Write-Track RISK REGISTER (SBAI-4236)

*The consolidated risk register and GA gate for the StudioBrain ↔ lore write-back
integration (epic "EW", SBAI-4088). This is the capstone deliverable referenced
by `0002-ew-implementation-plan.md` §11 ("full register in EW.6.1") and closes
out the EW.0–EW.5 write track (code-complete + locally validated).*

- **Status:** Active — 2026-06-25
- **Owner:** BizaNator (owner) — owner-decision items flagged ⚠ below
- **Scope:** the WRITE path only (cloud writes a web user's edit *back* to a
  desktop-hosted tenant's lore). The read/index/federation plane (ADR-0001 E1)
  and the BYO-storage path (`WriteThroughCache` → Garage) are unchanged and
  out of scope here.
- **Inputs consolidated:** ADR-0001 (federate-don't-replace), ADR-0002 §11
  (R1–R12 summary), the EW.0 spike verdict (`0002-ew0-cloud-write-facade-spike.md`),
  and the EW.1–EW.5 implementation findings.

---

## 0. How to read this register

Each risk has: **what could go wrong**, the **mitigation** that exists today (or
is staged), the **owner-decision** it depends on (if any), and a **status**:

| Status | Meaning |
|---|---|
| **MITIGATED** | Control exists in code + is tested; no further work to ship. |
| **STAGED** | Control is implemented on a feature branch but not yet rolled out / enforced in the deployed path. Tracked by a ticket. |
| **ACCEPTED** | Residual risk knowingly accepted for the pre-1.0 window; revisit at GA. |
| **OPEN** | Needs an owner decision or unblock before GA. |

**Pre-1.0 framing:** lore is a pre-1.0, Epic-controlled VCS on the *content* path
for lore-mode tenants. The whole design is "federate, don't replace" precisely so
lore churn can never corrupt StudioBrain's own source of truth — but several
risks below exist *because* lore is pre-1.0. None block the current pre-alpha
window; the GA gate is the union of the STAGED/OPEN items reaching MITIGATED.

---

## 1. Risk register

### R1 — In-process linkage of lore-revision/lore-storage into the cloud binary (licensing + Oodle)

- **What could go wrong:** EW.0 mechanism A links Epic's `lore-revision` +
  `lore-storage` + `lore-transport` + `lore-base` crates **in-process** into the
  proprietary cloud binary (the only way to synthesize a server-hash-verified
  `repr(C)` revision — raw gRPC stubs cannot). Two hazards: (a) a proprietary
  **Oodle** native dependency could be pulled into the cloud dep graph; (b) the
  license terms of linking Epic's pre-1.0 crates into a proprietary SaaS binary.
- **Mitigation:** Oodle is an **opt-in Cargo feature, OFF by default**
  (`lore-storage` `default = []`, `oodle = []`). The write crate
  (`sb-lore-writeclient`) builds with `default-features = false`, **never**
  enables `oodle`, and never sets `OODLE_LIB_DIR`; it links only the open
  `lz4-sys` + `zstd-sys` (crates.io). Proven in EW.1.1: a default-codec
  desktop-lore repo round-trips against a no-oodle cloud build. **All
  `lore-revision`/`lore-storage` imports are confined to `sb-lore-writeclient`**
  (one crate, grep-guardable). A `cargo-deny` / `cargo-tree` CI gate fails the
  build if `oodle` or any Oodle native lib ever enters the cloud dep graph.
- **Owner-decision:** ⚠ confirm Epic's `lore-revision`/`lore-storage` license
  permits linking into the proprietary `sb-cloud` binary. If NO → mechanism A is
  blocked and mechanism B (subprocess `lore-vm`, **no** linking) becomes primary.
- **Status:** **MITIGATED** (Oodle: confirmed non-issue, CI-gated) · **OPEN**
  (license sign-off — the one hard gate before mechanism A ships in prod).

### R2 — Scope escalation: a read-only LSG can write (the read/write enforcement gap)

- **What could go wrong:** canonical lore-server historically authorized purely
  off *read-presence* — `branch_push` and the other mutating RPCs never checked
  the per-resource `permission` Vec. A read LSG that merely **lists** the repo
  (`permission: ["read"]`) could therefore **write** it. This is the single most
  serious write-track integrity gap.
- **Mitigation (STAGED):** `lore-server` `verify_permission(token, repo, "write")`
  (SBAI-4213) requires the matching `urc-<repo>` resource to carry the `"write"`
  permission string; every mutating RPC (branch push/create/delete, repo
  create/delete, metadata set, protect/unprotect) calls it (SBAI-4241). A
  wildcard `urc-*` resource confers write **only if it itself carries** `"write"`
  — a wildcard read grant does not silently escalate. Unit-tested in
  `lore-server/src/auth/jwt.rs` (`write_permission_rejects_read_only_token`,
  `write_permission_denies_empty_permission_and_other_repo`,
  `write_permission_wildcard_requires_the_permission`) + an accounts-native-LSG
  end-to-end test. On the cloud side, accounts only ever mints `"write"` into a
  resource for a grant that already holds the `lore:repo:write` scope (SBAI-4214
  allow-set), and never for a wildcard repo.
- **Status:** **STAGED** — enforcement code + tests exist on the
  `SBAI-4213-write-enforcement` lore branch; the **rollout to the deployed /
  pinned loreserver is SBAI-4241**. Until that lands and the desktop-hosted
  loreservers run the enforcing build, scope-enforcement is *minted-correctly
  but not server-enforced*. **The live end-to-end scope-rejection threat test is
  PENDING this rollout** (see §2, sub-ticket).

### R3 — Attribution spoof / mis-attribution (web edits land under the wrong author)

- **What could go wrong:** lore attributes a commit to the **authenticated
  token's `sub`** (`execution_context().set_user_id()` → `commit_impl`); there is
  **no author argument** and lore **ignores `act.sub`**. A single per-tenant
  service token would misattribute **every** web edit to the service principal.
  Worse, a forged token could try to claim another user's `sub`.
- **Mitigation:** **per-user write LSGs** (EW.2 / SBAI-4211). accounts mints a
  token whose `sub` = the editing user's **stable, non-PII lore id**
  (`user.user_id`), in lore's native claim shape (`env`/`name`/
  `preferred_username`/`idp` + `resources:[{resource_id, permission}]`), scoped
  to (tenant, repo). Forgery is prevented by RS256 signature verification at the
  lore server (JWKS from accounts); a client cannot mint or alter `sub`. The
  cloud-side `author_from_identity` string is **attribution only** (read without
  verifying) and is irrelevant to authz — the server re-derives the author from
  the verified `sub`. No PII (`name`/`preferred_username` carry the lore id, never
  email/display name — tested `test_write_token_carries_no_pii`).
- **Status:** **MITIGATED** (minting + shape + non-PII tested in accounts
  SBAI-4211; signature-verified at the server). Depends on R2's server-side JWKS
  verification being live for end-to-end enforcement.

### R3b — Audience confusion (an LSG used as a user session, or vice-versa)

- **What could go wrong:** the LSG (`aud="lore-service"`) and the user session
  token (`aud="citybrains-app"`) are both accounts-signed RS256 JWTs. If either
  verifier accepted the other audience, a service grant could impersonate a user
  session (privilege escalation into the product) or a stolen user token could be
  replayed as a lore write grant.
- **Mitigation:** strict audience segregation at *both* verify paths —
  `AuthUtils.verify_token()` (user) pins `aud="citybrains-app"`;
  `verify_lore_service_token()` pins `aud="lore-service"`. Cross-audience tokens
  are rejected in **both** directions (tested for the read LSG in
  `test_lore_grant_routes.py::test_lsg_rejected_by_user_verify` /
  `test_user_token_rejected_by_lsg_verify`, and extended to the **native-shape
  write LSG** in the EW.6 complement — see §2). The lore server independently
  pins its expected audience.
- **Status:** **MITIGATED** (both-direction segregation tested, read + write LSG).

### R4 — Lost update / split-brain / concurrent-edit conflict

- **What could go wrong:** two writers (web + desktop, or two web users) advance
  the same branch concurrently; a naive write against a stale tip silently
  clobbers the other edit, or a cloud write races a desktop edit and the index
  diverges from lore.
- **Mitigation:** (a) **head-moved read-modify-write retry** in
  `sb-lore-writeclient`: each write queries the branch tip, builds the revision
  against it, and on a moved tip re-reads + rebuilds (up to
  `MAX_HEAD_MOVED_RETRIES`), so concurrent non-conflicting writers all succeed
  and the branch advances by exactly N (tested
  `concurrent_writes_all_succeed_via_head_moved_retry`). (b) **lore's revision /
  fast-forward-merge model owns true conflict** — same-field divergent edits
  surface via `RevisionDiff`; the retired `conflict_resolver` stays retired. (c)
  **Offline divergent edits park as `conflict`** in the EW.4 outbox for the
  resolver rather than auto-clobbering. (d) **Single-write integrity** (see R4b)
  ensures the index can never disagree with lore for a lore-mode write.
- **Owner-decision:** ⚠ conflict policy for offline/concurrent **same-field**
  divergence: last-writer-wins vs manual-merge vs block. Current default: lore
  fast-forward-merges non-conflicting; same-field parks as `conflict`.
- **Status:** **MITIGATED** (head-moved retry tested) · **OPEN** (same-field
  conflict UX/policy is an owner decision; parking is the safe default today).

### R4b — Dual-write / single-write integrity (index disagreeing with lore)

- **What could go wrong:** for a lore-mode write, if cloud wrote BOTH lore **and**
  the YB/Qdrant index directly (a dual-write), a partial failure would leave the
  index disagreeing with the authoritative lore CAS.
- **Mitigation:** **one write, no dual-write.** The lore write path pushes only
  to lore; the YB/Qdrant index is driven **solely** by the `BranchPushed →
  reindex` loop (E1.5). A **typed seam** — `LoreWritePath` has no access to
  `IndexerHandle` / `WriteThroughCache` — makes an accidental dual-write a
  **compile error**. The index is a rebuildable cache over lore; if it ever
  diverges, source-partitioned rebuild (`source='lore'`) from lore is always safe.
- **Status:** **MITIGATED** (typed seam + reindex-driven index; design-enforced).

### R5 — Data loss in the offline / web-only degraded write path

- **What could go wrong:** a `LoreAuthoritative` tenant whose desktop loreserver
  is offline still receives web edits. If those edits had nowhere durable to land,
  they'd be lost; or a naive "fall back to BYO/Garage" would split-brain the
  source of truth.
- **Mitigation (EW.4):** mode is resolved from **config existence, not session
  health** — a lore tenant never silently falls back to BYO. Offline writes go to
  a durable per-tenant YB `lore_write_outbox` (status `pending_lore`, keyed by
  `user_sub`), optimistically indexed to YB + Garage **preview**, returning 202.
  On reconnect the reconcile loop drains the outbox **in order**, replays each op
  under its stored `user_sub`, and flips to `committed`; divergent edits park as
  `conflict`. Hard caps on outbox depth/payload → over-cap returns **503, never
  silent loss**. **Asset writes are rejected in degraded mode** (no CAS to land
  bytes), not outboxed. Reads/search keep working from the index + preview cache
  throughout.
- **Status:** **MITIGATED** (outbox + ordered replay + caps; reads degrade
  gracefully). Depends on the EW.4 migration (outbox table) being applied — see R7b.

### R6 — Latency / scale of synchronous relay round-trips

- **What could go wrong:** every web write is a synchronous chain over the
  sb-relay (bore) to the tenant's desktop loreserver: resolve tip → build →
  `BranchPush` → push-then-verify. High RTT or many concurrent per-tenant
  connections through one relay degrade write latency or exhaust the pool.
- **Mitigation:** per-repo **connection pooling** + a per-tenant LRU session pool
  (`TenantLoreRegistry`, modeled on `TenantDbPool`); `max_connections` set to a
  sane client value (EW.1.1 trap #2 — the default of 0 starves the pool).
  Push-then-verify is the durability cost; the periodic reconcile (E1.6) is the
  backstop. Read-after-write returns the **in-hand payload projection**, not a YB
  read, so the user isn't blocked on async reindex.
- **Owner-decision:** ⚠ per-tenant connection-scale sizing through one relay
  needs a load target before GA.
- **Status:** **ACCEPTED** for pre-1.0 (pooling in place); **OPEN** (GA scale
  sizing).

### R7 — lore-version (`repr(C)`) skew between cloud and the desktop loreserver

- **What could go wrong:** lore's Revision/Tree/State blobs are
  compiler-/version-sensitive `repr(C)` structs the server hash-verifies. If the
  cloud's pinned `lore-revision` rev drifts from the desktop loreserver's lore
  version, serialized revisions fail verification and writes break.
- **Mitigation:** the cloud pins `lore-revision`/`lore-storage` to a **single
  vendored rev shared with `lore-proto`** (and with the sidecar LoreGUI build via
  `satellites.lock.json`). The same pin governs the desktop client shipped in the
  installer. A version-skew failure is **fail-closed** (hash-verify rejects), not
  a silent corruption.
- **Status:** **ACCEPTED / MITIGATED** (single pin; fail-closed). Revisit the pin
  bump procedure as part of the satellites release train.

### R7b — Migration / xCluster DDL discipline (write-track schema on both YB sites)

- **What could go wrong:** the write track adds/changes content-DB schema —
  `tenant_lore_configs` + its `project_id` column (**V4218**, EW.5.7) and the
  EW.4 `lore_write_outbox` (**V4234**). **xCluster replicates DML, NOT DDL.** A
  migration applied to only one YB site leaves the other broken on failover
  (e.g. the resolver can't read the project segment, or the outbox table is
  missing and offline writes 500 instead of 202).
- **Mitigation:** **V4218 and V4234 are human-applied to BOTH Biloxi AND
  Cloudcroft independently**, verified with `\d tenant_lore_configs` /
  `\d lore_write_outbox` on both sites before the dependent cloud image is
  deployed. SQLite mirror (`docs/schema_sqlite_mirror/`) kept in lockstep via
  `db_compat`. This is the standing schema-discipline rule (SBAI-2357), called
  out explicitly here because the write path **fails closed and visibly** if a
  site is missing the DDL.
- **Owner-decision:** ⚠ confirm V4218 (present in `cloud/crates/sb-cloud/migrations`)
  and V4234 (EW.4 outbox) are applied on both sites at rollout.
- **Status:** **OPEN** (operational gate at rollout — DDL is human-applied, not
  CI-automatable).

### R8 — Trust surface of a large pre-1.0 Epic crate inside the multi-tenant pod

- **What could go wrong:** linking `lore-revision`/`lore-storage` in-process pulls
  a large, pre-1.0, externally-controlled codebase into the **multi-tenant** cloud
  pod. A bug (panic, unbounded alloc, path traversal in tree handling) in that
  crate executes inside the shared cloud process and could affect other tenants.
- **Mitigation:** the write context is **path-less** (`RepositoryContext::new(None, …)`
  — `require_path()` fail-fasts, no working-tree/disk code path is reachable from
  cloud), over **fresh in-memory stores per write** (no shared mutable disk
  state). All writes run on lore's own runtime inside a per-call
  `LORE_CONTEXT.scope(ExecutionContext, …)` so attribution/`user_id` cannot leak
  across concurrent tenant requests (EW.1.1 confirmed per-task isolation). The
  blast radius is further bounded by mechanism B (subprocess) being available as
  a fallback for repo formats/transports mechanism A can't handle.
- **Owner-decision:** ⚠ resource/panic isolation posture for the in-process crate
  at GA (e.g. catch_unwind boundary, memory caps) — accept in-process for pre-1.0
  vs move to the mechanism-B subprocess for stronger isolation.
- **Status:** **ACCEPTED** (pre-1.0, path-less + per-write stores + per-call
  scope) · **OPEN** (GA isolation posture).

### R9 — Accounts / PII / proprietary boundary leak into the open repos

- **What could go wrong:** the open satellites (loregui, model-manager) and the
  open core could accidentally import accounts UI
  (`@biloxistudios/studiobrain-accounts-frontend`), copy accounts/PII source, or
  bundle proprietary `loregui-cloud` overlay code into an open/desktop build —
  widening the trust/compliance zone the accounts security boundary exists to
  protect (CLAUDE.md "Security Boundary — Accounts Isolation"; ADR-0001 §3.2).
- **Mitigation:** the **boundary CI guard** (EW.6 / SBAI-4236) — a deny-list grep
  wired into CI in **loregui** (open) and **loregui-cloud** — fails the build if
  accounts-frontend imports, copied accounts/PII source, hard-coded secret
  material, or proprietary-overlay *imports* (vs the legitimate documented seam
  stubs) appear in the scanned source. The open core only ever has **stub seams**
  (`commercial/overlay-entry.ts`, `premium-registry.ts`, `relay-registry.ts`)
  that a `loregui-cloud` build *replaces*; it never imports the overlay. accounts
  UI is reached only via the same-origin iframe (SBAI-1935), never bundled.
- **Status:** **MITIGATED** (boundary guard added + wired in CI this ticket; see §3).

### R10 — Orphaned CAS blobs (write debris in the tenant's lore storage)

- **What could go wrong:** a write that Puts content blobs to lore CAS but fails
  before `BranchPush` (or whose revision is never referenced) leaves **orphaned,
  unreferenced blobs** in the tenant's storage, slowly bloating it.
- **Mitigation:** push-then-verify means we only ack after the revision resolves
  and its blobs are confirmed stored; an aborted write simply leaves unreferenced
  blobs that lore's own GC/compaction reclaims (CAS is content-addressed, so a
  retry re-Puts the *same* address — no duplication). Cloud stores **no** asset
  bytes itself, so there is no cloud-side orphan surface; only the tenant's lore
  CAS is affected and it is the tenant's storage to GC.
- **Status:** **ACCEPTED** (bounded, self-healing via CAS + lore GC; not a data-
  integrity risk). File the lore-GC observability ask as an upstream gap if it
  proves material.

### R11 — Relay write (`StorageService.Put`) channel may be read-only today

- **What could go wrong:** the federation read plane only needs `StorageService.Get`
  over the relay. If sb-relay (bore TCP/h2c) forwards only reads and not `Put`
  (writes), **both** mechanism A and B are blocked — there is no path to land
  bytes in the desktop loreserver's CAS.
- **Mitigation:** EW.1.1 was tasked to answer this against a live relay; the write
  facade's integration tests exercise `write_file`/`write_many`/`delete` through
  the connection path. Where a relay deployment is Get-only, the write path
  **fails closed** (the tenant's writes 503/queue to the EW.4 outbox) rather than
  losing data.
- **Status:** **MITIGATED** if EW.1.1 confirmed Put is relayed end-to-end; **OPEN**
  for any relay topology not yet validated for `Put` — verify per relay
  deployment before enabling authoritative writes for that tenant.

### R12 — `execution_context` user_id scope (per-task vs process-global)

- **What could go wrong:** if lore's `execution_context().user_id` were
  process-global rather than per-async-task, concurrent multi-tenant writes would
  **leak attribution across requests** (user A's edit attributed to user B).
- **Mitigation:** each write runs inside its own
  `LORE_CONTEXT.scope(ExecutionContext, fut)` on lore's runtime (the
  `ExecutionContext` is created per-client and the `user_id` is set per-call), so
  attribution is isolated per request. EW.1.1 confirmed per-task isolation; the
  concurrent-writers integration test exercises the path under contention.
- **Status:** **MITIGATED** (per-call scope; concurrency-tested).

---

## 2. Threat-model test coverage (what's tested, where, and what's PENDING)

The threat model is enforced across three layers. EW.2 already added the
attribution + scope tests at the minting and server layers; EW.6 **complements**
them (audience-confusion for the native-shape write LSG) and **catalogs** the
gaps that depend on the SBAI-4241 enforcement rollout.

| Threat | Test | Layer / location | Status |
|---|---|---|---|
| **Audience confusion** — LSG ≠ user session, both directions (read LSG) | `test_lsg_rejected_by_user_verify`, `test_user_token_rejected_by_lsg_verify`, `test_audiences_are_distinct` | accounts `tests/test_lore_grant_routes.py` | **PASS** |
| **Audience confusion** — native-shape **write** LSG rejected as a user session + vice-versa | EW.6 complement `test_write_lsg_*_audience_*` | accounts `tests/test_lore_grant_routes.py` (EW.6) | **PASS** (added this ticket) |
| **Attribution** — write LSG `sub` = editing user, not the service | `test_write_token_native_shape_read_write`, `test_*_write_token_*attributes_to_user`, `test_internal_write_token_sub_from_body_user` | accounts (SBAI-4211) | **PASS** |
| **No PII in the write LSG** | `test_write_token_carries_no_pii` | accounts (SBAI-4211) | **PASS** |
| **Cross-tenant mint rejection** — can't mint a write token off another tenant's grant | `test_user_write_token_403_cross_tenant` | accounts (SBAI-4211) | **PASS** |
| **Read-only grant can't mint write** | `test_user_write_token_403_on_read_only_grant` | accounts (SBAI-4211, SBAI-4214 allow-set) | **PASS** |
| **Wildcard repo rejected on write-token mint** | `test_user_write_token_rejects_wildcard` | accounts (SBAI-4211) | **PASS** |
| **Scope enforcement** — read-only LSG **rejected at the server on write** | `write_permission_rejects_read_only_token` | lore `lore-server/src/auth/jwt.rs` (SBAI-4213) | **PASS on branch — PENDING rollout (SBAI-4241)** |
| **Cross-repo write rejection** — write grant on repo A doesn't authorize repo B | `write_permission_denies_empty_permission_and_other_repo` | lore (SBAI-4213) | **PASS on branch — PENDING rollout (SBAI-4241)** |
| **Wildcard write requires the permission** | `write_permission_wildcard_requires_the_permission` | lore (SBAI-4213) | **PASS on branch — PENDING rollout (SBAI-4241)** |
| **Live end-to-end** — a read-only write-LSG is rejected writing through `sb-lore-writeclient` against an **enforcing** loreserver | *(none yet)* | loregui-cloud `sb-lore-writeclient/tests` | **PENDING** → sub-ticket (needs SBAI-4241 + a live enforcing server fixture) |
| **Live end-to-end** — cross-tenant: a write-LSG for tenant A's repo is rejected writing tenant B's repo through the relay | *(none yet)* | loregui-cloud / two-tenant fixture | **PENDING** → sub-ticket |

**Why some are PENDING:** the lore scope/cross-repo enforcement (R2) is
code-complete + unit-tested on the `SBAI-4213-write-enforcement` branch, but the
*deployed / pinned* loreserver (and the desktop-hosted loreservers) do not run
the enforcing build until **SBAI-4241** rolls out. A live end-to-end rejection
test would pass against an enforcing server and **fail-open against today's
non-enforcing server**, so it must not be wired green until the rollout. The
unit tests guarantee the logic; the live tests are filed as sub-tickets gated on
SBAI-4241.

---

## 3. Boundary CI guard (R9 control)

A deny-list grep guard runs in CI on the **open** repos:

- **loregui** — `scripts/check-open-boundary.sh` + `.github/workflows/boundary-guard.yml`.
  Fails if accounts-frontend (`@biloxistudios/studiobrain-accounts-frontend`)
  imports, copied accounts/PII source, hard-coded secret material
  (`JWT_PRIVATE_KEY`, `STRIPE_SECRET_KEY`, …), or **imports** of the proprietary
  `loregui-cloud` overlay (vs the documented stub seam) appear in scanned source.
- **loregui-cloud** — `scripts/check-cloud-boundary.sh` + workflow. loregui-cloud
  is proprietary, so it *may* contain premium overlay code, but it **must never**
  bundle accounts UI / import accounts-frontend / copy accounts PII source — the
  guard enforces exactly that subset.

The guard distinguishes the legitimate documented seam (open core *mentions*
`loregui-cloud` in comments/strings to describe the overlay it stubs) from an
actual **leak** (an `import`/`require`/`from` of accounts-frontend or overlay
source). Verified: passes on the clean tree, fails on a planted leak.

---

## 4. GA gate (the union that must close)

Ship-blocking before GA (all others are MITIGATED/ACCEPTED for pre-1.0):

1. **R1** — Epic license sign-off for in-process linkage (else fall back to
   mechanism B).
2. **R2 / SBAI-4241** — write-permission enforcement rolled out to the deployed +
   desktop-hosted loreservers; the two **live** end-to-end threat tests (§2) wired
   green.
3. **R7b** — V4218 + V4234 confirmed applied on **both** YB sites.
4. **R6 / R8 / R11** — GA decisions: relay-Put validated per topology, connection
   scale sizing, and the in-process isolation posture.

When 1–4 are closed and the pending §2 live tests pass, the EW write track is GA.

---

## 5. Sources

- `docs/adr/0001-studiobrain-lore-federation.md` — federate-don't-replace; LSG; entitlement model.
- `docs/adr/0002-ew-implementation-plan.md` §11 — R1–R12 summary (this register is the full form).
- `docs/adr/0002-ew0-cloud-write-facade-spike.md` — mechanism A/B, `repr(C)`/hash-verify, Oodle-opt-in, per-user-LSG attribution, markdown-cache/lazy-asset findings.
- accounts `SBAI-4211-ew2-write-lsg` — write-LSG minting + attribution/scope tests.
- lore `SBAI-4213-write-enforcement` (rollout SBAI-4241) — `verify_permission` + enforcement tests.
- `loregui-cloud/crates/sb-lore-writeclient` — mechanism A facade + head-moved retry + concurrency tests.
