# StudioBrain ↔ lore Write-Back Integration — Consolidated Design (SBAI-4088, epic "EW")

*Post ADR-0002 + EW.0 spike. Consolidates the EW.1 (write facade), EW.2 (attribution), EW.3 (cloud write-path), EW.4/EW.8 (offline), and EW.5 (pathmap/cache) stream designs plus the cross-cutting build/mode/migration/risk work into one deduplicated plan with a single stable EW.* numbering.*

---

## 1. Overview & decisions (locked)

- **lore is the content SYNC/VERSIONING backbone for DESKTOP-HOSTED tenants.** StudioBrain's BYO-storage (OpenDAL S3/GDrive) stays the STORAGE/serving layer and the only path for non-desktop tenants. These are **complementary layers, not a swap.**
- **studiobrain-cloud is a per-tenant lore CLIENT + cache.** It reads via `sb-lore-client` and writes back to the tenant's lore. Cloud is **not in the storage business and hosts NO loreservers** — the desktop user hosts lore.
- **EW.0 verdict (proven with a running prototype):** working-copy-less write is achievable, but **NOT via raw gRPC stubs** (lore's Revision/Tree blobs are compressed `repr(C)` structs, server-hash-verified). Two mechanisms:
  - **A (PRIMARY, in-process, path-less):** `RepositoryContext::new(None, …)` → connect → `State::deserialize(tip)` → `immutable::write(content)` (client computes `address.hash = blake3(payload)`) → `node_add(path)` → `update_tree_root_hash` → `serialize` → sign → `BranchPush`. No disk.
  - **B (PROVEN FALLBACK):** ephemeral tmp working copy via `lore-vm` (`stage → commit → push → rm`).
- **Oodle is OPT-IN** (Cargo feature OFF by default). lore-storage defaults to LZ4+Zstd (open, crates.io); it only links Oodle when `--features oodle` + `OODLE_LIB_DIR`. We control our lore builds → **no proprietary dependency. Non-issue**, but CI-gated.
- **Attribution flows from the authenticated token's `sub`** (`execution_context().user_id()` → `commit_impl`); there is **no author arg** on commit. Therefore web edits require **PER-USER write LSGs** (`sub` = web user), scoped tenant+repo, carrying `lore:repo:write` / `lore:asset:write`.
- **Caching model (confirmed):** index markdown text/frontmatter into YB + Qdrant for search; assets keep **metadata + derived preview only**, with lazy full-res fetched over the live link (`StorageService.Get`/presigned). On WRITE, asset bytes pass **THROUGH** cloud into lore CAS and are **NOT persisted in cloud** (preview retained). Orthogonal to mechanism A/B.

## 2. Two-layer model

| Layer | Role | Authority | Writable by |
|---|---|---|---|
| **lore CAS (desktop-hosted)** | Content sync/versioning backbone for lore-mode tenants | **Source of truth** for lore-authoritative tenants | desktop host + cloud (via per-user write LSG) |
| **Garage S3 (BYO/OpenDAL)** | Storage/serving for BYO tenants; preview cache for lore tenants | Source of truth for BYO tenants only | cloud `WriteThroughCache` |
| **YB `studiobrain_content`** | Search/read index | Cache (rebuildable) over markdown | indexer only |
| **Qdrant** | Vector search index | Cache | indexer only |

**Invariant:** YB/Qdrant/Garage-preview are *indexes over* the authoritative markdown (lore CAS for lore tenants, Garage for BYO). If they diverge, rebuild from the source of truth is always safe.

## 3. Per-tenant content-mode

`TenantContentMode { Byo, FederatedRead, LoreAuthoritative }` resolved per request from `tenant_lore_configs`:
- **Byo** (no active row): Garage = source of truth, native YB index. Today's behavior, unchanged.
- **FederatedRead** (`mode='read'`): lore is read-indexed alongside Garage; writes still go to Garage.
- **LoreAuthoritative** (`mode='authoritative'` AND a write grant present AND session not Offline): lore is the source of truth; web writes flow back to lore (one write, no dual-write); Garage demoted to preview.
- **LoreAuthoritativeDegraded** (authoritative but session Offline): routes to the EW.4 write outbox.

Mode is resolved from **config existence, not session health** — a lore tenant whose loreserver is offline stays in Lore mode (its write must fail/queue, never silently fall back to BYO and split-brain).

## 4. Per-tenant-mode write paths

**BYO tenant write (unchanged):** `cloud_entity` → `WriteThroughCache::write()` (DB+Garage+Qdrant) → async `IndexEvent`. `upload_asset` writes bytes to FileServer/OpenDAL.

**Lore tenant write (ONE write, NO dual-write):**
1. Resolve mode → `LoreAuthoritative`.
2. Set attribution: `execution_context().set_user_id(web_user_sub)` (scoped per-call).
3. Push markdown/asset to lore via `LoreWriteClient` (mechanism A; B fallback).
4. On push success, the **YB/Qdrant index is driven solely by the BranchPushed → reindex loop (E1.5)** — cloud does **not** call `wt.write` and does **not** `submit_index_event` for the lore write. A typed seam (`LoreWritePath` with no access to `IndexerHandle`/`WriteThroughCache`) makes accidental dual-write a compile error.
5. Reindex must fetch lore-sourced content from the **lore session** (`read_file`/`tree`), not Garage.
6. Lore push failure → HTTP error (503/403/409/404 per EW.3.5), never a BYO fallback.

**Asset write (lore tenant):** bytes streamed THROUGH cloud to lore CAS (`StorageService.Put`, FastCDC-chunked, client computes blake3), `node_add` at the asset tree path, push. Cloud retains **preview + metadata only**; full-res lazy via `read_file`.

## 5. EW.1 — Write facade (mechanisms A + B) + build/licensing

`sb-lore-writeclient` (new crate in loregui-cloud) exposes the `LoreWriter` trait (`write_entity`/`write_asset`/`delete`/`batch_write`/`put_blob`) with two interchangeable impls:
- **`LoreWriteClient` (mechanism A)** — in-process path-less flow from EW.0, productionized with per-user attribution, push-then-verify durability, batching (N ops → 1 revision), fast-forward-merge + retry against a moved tip, and session pooling.
- **`LoreVmWriter` (mechanism B)** — ephemeral tmp working copy via lore-vm behind the same trait; selected by config/strategy with auto-fallback on an A-error class.

**Build/licensing (EW.1.0):** add `lore-revision` + `lore-storage` to the cloud build behind a `lore-write`/`lore-writeback` Cargo feature, `default-features = false`, **oodle never enabled**, pinned to a single vendored rev shared with `lore-proto`. All `lore-revision` imports are confined to `sb-lore-writeclient` (CI grep guard); a `cargo-deny`/`cargo-tree` gate fails if `oodle` or any Oodle native lib appears in the cloud dep graph.

## 6. EW.2 — Per-user LSG WRITE scopes + attribution

Three load-bearing facts about canonical lore-server:
1. **Attribution** records the pushing user as the JWT `sub` only (`act.sub` is ignored) → per-tenant service `sub` misattributes every web edit. Fix: per-user write LSGs with `sub` = the editing user's stable lore id.
2. **Token shape** — lore decodes `AuthorizationToken`/`JWTUserInfo` requiring `env/name/preferred_username` (+`idp`) and authorizes purely off `resources:[{resource_id:"urc-<repo>", permission:[…]}]`. The current scope-based LSG cannot decode/authorize against stock lore — accounts must mint in lore's native shape.
3. **No write enforcement** today — `branch_push` never checks the permission Vec, so a read LSG that lists the repo can write. Fix: enforce the `"write"` permission string on every mutating RPC.

EW.2 delivers: lore-native per-user write LSG minting (sub=user, resources with `["read","write"]`), a stable non-PII web-user→lore-user-id mapping (`sub = user.user_id`), per-(tenant,user) Valkey write-token storage + a write-path token source, lore-server write-permission gating, lore-server JWKS/aud/iss config + repo→urc resolution, and an end-to-end attribution + scope-enforcement test.

## 7. EW.3 — Cloud entity write-path + asset writes (mode branch) + migration

`WriteModeResolver` (TTL-cached, invalidated on consent/disconnect) is the single branch point. `main.rs` layers `TenantLoreRegistry`, `WriteModeResolver`, and the per-user write-token provider as Extensions (gated by the lore feature; resolver returns Byo when the feature is off). Entity create/update/import/delete and asset upload each gain a mode branch: BYO unchanged; lore routes to `LoreWriter` + reindex-driven index. Read-after-write on the lore path returns the **in-hand payload projection** (not a YB read, which the reindex updates async). The "Connect StudioBrain" switch flow (consent rendered **only** in the accounts iframe — SBAI-1935 boundary) provisions the write LSG and flips `tenant_lore_configs.mode`. A resumable, idempotent migration seeds an existing BYO tenant's lore repo from the Garage/YB manifest, verifies hashes, then flips to authoritative; rollback re-pulls preview-only assets to Garage (blocked while offline).

## 8. EW.4 — Offline / web-only degraded write

For a `LoreAuthoritativeDegraded` tenant, a durable per-tenant YB `lore_write_outbox` accepts writes: optimistically index to YB + Garage preview (status `pending_lore`), append to the outbox keyed by `user_sub`, return 202. On reconnect the reconcile loop drains the outbox **in order**, replays each op under its stored `user_sub`, and flips to `committed`; divergent edits park as `conflict` for the resolver. Hard caps on outbox depth/payload; over cap → 503 (no data loss). **Asset writes are rejected in degraded mode** (no CAS to land bytes in), not outboxed. Reads/search keep working throughout.

## 9. EW.5 — PATH↔TREE mapper + read/index/cache confirmation

A pure per-(tenant,project,repo) `PathMapper` is the single bijection between the DAM key `{tenant}/{project}/entities/{type}/{id}.md` and lore's repo-relative tree path `entities/{type}/{id}.md` (the `{tenant}/{project}` prefix is **repo identity, not tree path**). Used by BOTH the read/enumerate path (RemoteEnumerator over `tree()` → `IndexEvent(source='lore')`) and the A/B write paths (`node_add(path)`), guaranteeing read/write round-trip symmetry. The legacy folder-singularization heuristic is demoted to a `Heuristic` LayoutMode fallback for imported repos; StudioBrain-provisioned repos default to `Canonical` (entity_type/id come directly from the path). The markdown-cache + lazy-asset model is confirmed sound against the actual pipeline and `sb-lore-client`. **Blocker resolved here:** `tenant_lore_configs` is one-row-per-tenant; EW.5.7 adds a `project_id` column (project = repo identity) so the mapper has an authoritative `{project}` segment. Path conventions (entities project-qualified, assets not) are normalized.

## 10. Sequencing (critical path)

```
EW.0 (done)
  └─ EW.1.0 (build/deps/no-oodle/CI guard)
       └─ EW.1.1 (spike A e2e)
            └─ EW.1.2 (LoreWriteClient facade A)        ← also unblocks EW.1.3 (mech B)
                 └─ EW.3.0 (content-mode model + resolver + Extension wiring)
                      └─ EW.3.1 (entity create/update branch)
                           └─ EW.3.3 (asset pass-through) / EW.3.4 (single-write integrity)
                                └─ EW.4.1 (offline outbox)
                                     └─ EW.3.7 (BYO→lore migration)
                                          └─ EW.6.1 (risk register / GA gate)
```
Parallel chains feeding the join at EW.3.1:
- **Attribution:** EW.2.2 → EW.2.1 → EW.2.3 → EW.2.4 (+ EW.2.5, EW.2.6 independent) → EW.2.7.
- **Pathmap:** EW.5.7 → EW.5.1 → EW.5.2/EW.5.3 → EW.5.5 (write side) / EW.5.4 → EW.5.6.

**Longest chain:** EW.0 → EW.1.0 → EW.1.1 → EW.1.2 → EW.3.0 → EW.3.1 → EW.3.3 → EW.4.1 → EW.3.7 → EW.6.1.

## 11. Risk register (summary — full register in EW.6.1)

R1 Licensing of in-process lore-revision/lore-storage linkage · R2 Scope-escalation (read LSG must never write) · R3 Attribution-spoof · R4 Lost-update/split-brain · R5 Data-loss in degraded mode · R6 Latency/scale of synchronous relay RTT · R7 lore-version (repr(C)) skew · R8 Trust-surface of a large pre-1.0 Epic crate in the multi-tenant pod · R9 Accounts/PII/consent boundary leak · R10 Orphaned CAS blobs · R11 Relay Put (write) channel may be read-only today · R12 `execution_context` user_id scope (per-task vs global).

## 12. Cross-stream open questions (must resolve early)

- **Relay write channel:** does sb-relay (bore TCP/h2c) forward `StorageService.Put` (writes) or only `Get` (reads)? If Put isn't relayed, both A and B are blocked — **EW.1.1 must answer.**
- **In-memory store exposure:** are the stores behind `new_null_context`/`new_server_context` public enough to build a path-less write context from cloud, or is an upstream (BiloxiStudios fork) patch needed?
- **`execution_context` scope:** per-async-task or process-global? Concurrent multi-tenant writes need per-call isolation of `user_id` or attribution leaks across requests.
- **License terms:** do Epic's lore-revision/lore-storage terms permit linking into the proprietary sb-cloud binary? If NO, mechanism A is blocked and B (subprocess, no linking) becomes primary — a hard gate before EW.1.2.
- **Project↔repo identity:** per-`{tenant,project}` repo vs one repo per tenant with project-prefixed subtrees — decided in EW.5.7 (recommend per-project), must land before EW.5.1.
- **Conflict policy** for offline/concurrent divergent edits: last-writer-wins vs manual merge vs block.
- **aud decision:** keep LSG `aud='lore-service'` (configure lore to accept) vs switch to lore's native audience (re-validate accounts aud-segregation).
- **Desktop-hosted lore config distribution:** how do tenant-hosted loreservers receive the accounts JWKS URL + aud/iss config and trust accounts as issuer.
