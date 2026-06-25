# ADR-0002 — lore is StudioBrain's content **sync + versioning backbone**; BYO-storage stays the **storage/serving** layer (complementary, not a swap)

- **Status:** Accepted — 2026-06-25 (owner-ratified after a head-to-head evaluation)
- **Deciders:** BizaNator (owner)
- **Relationship to ADR-0001:** *Refines* it. ADR-0001 said "lore is NOT a backend" and kept lore read-only/additive. ADR-0002 keeps ADR-0001's **no-host / no-store-bytes-ourselves / federate-via-relay** topology, its **LSG identity** (§3.2) and **canonical entitlement** (§2.5) models, and its **satellite sidecar + license-unlock** model (§2.3/§3.5) — but **elevates lore from "read-only federated index source" to the source-of-truth + multi-client sync fabric for desktop-hosted tenants**, while **keeping StudioBrain's BYO-storage as the storage/serving layer**. The two are complementary layers, established by evidence (below), not a replacement.

---

## 1. Context — the evaluation that settled "replace vs. keep vs. optional"

The owner's steer ("lore as primary backend, stop writing repo-sync ourselves") raised the question of retiring StudioBrain's storage model. A two-sided, evidence-based evaluation (2026-06-25) found the two systems solve **different layers**:

**StudioBrain's current model — storage/serving is real, distributed sync is not.**
- BUILT + (mock-)tested: `write_through` single-writer cloud PUT (YB+Garage+Qdrant, rollback, idempotency); BYO-storage over OpenDAL (S3/GDrive/OneDrive/Dropbox); **binary asset serving with HTTP range — the most-tested surface (39 tests)**.
- Stale-doc correction: the desktop→cloud reindex bug (SBAI-2374/2375) is **fixed in code** (CLAUDE.md prose is stale) — but the end-to-end loop is skip-gated and **never proven green**.
- NOT built: reverse-sync (SBAI-2381) is a **stub** (`subscribe()` returns empty, zero publish callers); the conflict-resolver is built+tested but has **zero callers (dead)**; chunked/large-file unbuilt. **The multi-client/offline/conflict half was never built or tested.**

**lore — distributed sync is real, but it is VCS-shaped, not a flat object store.**
- YES, lore stores arbitrary bytes incl. binary: `StorageService.Put` is a direct content-addressed, FastCDC-chunked blob put — **no commit ceremony for a raw put**. Its CAS can sit on the **tenant's own S3** (S3-only; no GDrive/OneDrive plugin).
- BUT content is **hash-addressed**: landing bytes at a stable `{tenant}/{project}/path` requires the stage→commit→push **working-copy ceremony**, not a flat `PutObject`. **No HTTP range reads.** The cloud client (`sb-lore-client`) is **read-only — there is no cloud write API yet.** lore-server itself is production-grade (zero `todo!`); the "`service::start` stub" is an unrelated repo-daemon helper.
- Multi-tenant on one shared server is **authorization-isolated (per-repo JWT scoping), not crypto-isolated** (no per-repo encryption; a cross-repo dedup existence-oracle). **Moot for us** — each tenant runs *their own* lore server (desktop); we never host one shared server for many tenants.

**Conclusion:** lore is *not* a storage replacement (it fights path-addressing, is S3-only, has no range read, no cloud-write). lore *is* exactly the layer StudioBrain stubbed: versioned multi-client **sync** (CAS + branching + dedup + native notifications). They stack.

## 2. Decision

### 2.1 Two complementary layers
1. **Storage + serving layer = StudioBrain's existing BYO-storage — KEPT.** OpenDAL S3/GDrive/OneDrive/Dropbox routing, `write_through` index, and the range-capable asset proxy remain the flat, path-addressed object + serving layer. This is the **only** viable layer for flat path addressing, multi-provider, range serving, and **non-desktop (web/mobile-only) tenants**.
2. **Sync + versioning + source-of-truth layer = lore — ADOPTED (desktop-hosted).** lore's CAS + branch/revision model + notifications are the multi-client sync fabric for desktop⟷cloud⟷mobile⟷UE — the thing we stubbed. studiobrain-cloud federates as a **per-tenant lore client** (`sb-lore-client` over sb-relay), indexes lore for search/DAM/AI, and **writes edits back through lore** (the one structural gap — §3).

### 2.2 Two tenant modes (one shared storage substrate)
- **Desktop-hosted (lore) tenant:** runs lore on desktop; lore = source of truth + sync fabric; bytes live in the tenant's store (lore's CAS, optionally **S3-backed onto the tenant's own bucket**). Cloud indexes + writes back via lore. Full multi-client experience.
- **BYO-only (no desktop) tenant:** no lore host; uses BYO-storage (S3/GDrive) via `write_through`; cloud is single-writer primary. The existing model, unchanged.
- **Shared substrate:** the tenant's S3 bucket can be *both* the BYO-storage target *and* lore's CAS backend — so the storage substrate is common; lore is an optional sync/versioning layer **on top** for desktop tenants.

### 2.3 Bolt-on / sidecar / license-unlock (reaffirms ADR-0001 §2.3/§3.5)
- **LoreGUI stays a standalone, open-source (MIT), fully-functional DAM-over-lore GUI.**
- **Optionally bundled with studiobrain-app as an `externalBin` sidecar** (pinned by tag) — the *same open binary*, customized at install/runtime via injected config + the StudioBrain-signed entitlement.
- **The StudioBrain accounts JWT (canonical `tier`/`features[]`) unlocks LoreGUI premium** — premium panels physically present (dark by default), lit by the injected entitlement (SBAI-1935 bridge). No separate license system (ADR-0001 §2.6: the Ed25519 path is dropped). Free standalone has no license; bundled = StudioBrain-licensed.
- This is the **feature-coordination seam**: StudioBrain and LoreGUI ship one ecosystem; the license is the single unlock.

## 3. The one structural gap — cloud write path to lore
`sb-lore-client` is read-only. For desktop-hosted tenants, cloud edits must reach lore (online only). Recommendations (pending a design spike, EW.0):
- **Write surface:** a thin gRPC write facade on `sb-lore-client` (CAS Put → Revision → BranchPush, flush-verified), no server-side working copy.
- **Tenant offline:** cloud stays read/search-only over the YB index + preview cache; cloud edits **queue and replay** on reconnect (lore's revision model gives conflict-merge for free).
- **Durability:** push-then-verify before acking; periodic reconcile (E1.6) backstops lore's no-replay notifications.
- **Path mapping:** a `{tenant}/{project}/...` ↔ lore tree mapping shim (lore is hash-addressed; the DAM needs stable paths).

## 4. Consequences

**Positive:** we get the multi-client sync we never built (retires the SBAI-2381 stub + the dead conflict-resolver) **for free** via lore's native model; BYO-storage keeps doing what it provably does (flat storage, multi-provider, range serving, non-desktop); the tenant's S3 can unify both layers; strong privacy (bytes + embeddings stay on studio hardware); reuses the entire E0/E1 federation investment; LoreGUI ecosystem coordination via one license.

**Costs / risks (managed):** the cloud **write facade is net-new** (read-only today) — EW.0 spike de-risks; **writes/full-res reads are coupled to the tenant's machine** for lore tenants — mitigated by index/preview read-mostly availability + queue-replay (deliberately *not* by us hosting storage); **pre-1.0 lore on the sync path** — mitigated by the index keeping search/browse alive independent of lore, and lore proven through the LoreGUI build; **lore is S3-only** — GDrive/OneDrive tenants stay BYO-storage (no lore layer) until/unless lore gains those plugins.

## 5. What's kept / superseded / re-scoped
| Item | Disposition |
|---|---|
| BYO-storage (OpenDAL S3/GDrive/OneDrive), `write_through`, asset proxy + range | **KEPT** — the storage/serving layer |
| Reverse-sync (SBAI-2381, stub) | **Superseded by lore sync** for desktop-hosted tenants; remains only the (lower-priority) BYO-only multi-device case |
| Conflict-resolver (dead shelf-ware) | **Superseded** by lore's revision/merge for lore tenants; retire or repurpose for the BYO path |
| Desktop sync / reindex (SBAI-2374, fixed-but-unproven) | **Keep + prove** for the BYO path; lore path uses lore sync |
| `sb-lore-client` (read-only) | **Extend** with the write facade (§3) |
| ADR-0001 topology / LSG / entitlement / sidecar | **Unchanged** |

## 6. Revised plan (delta over ADR-0001 §5–6)
- **Keep** E0/E1 (read/index/notify/reconcile), E2 identity, E3 DAM, E4 deploy, E5 hardening.
- **Add E-write (gated by EW.0 spike):** EW.0 ⚠ spike (write facade + path-mapping + durability + offline-queue); EW.1 `sb-lore-client` write facade; EW.2 LSG write scopes (`lore:repo:write`) minted by accounts + enforced server-side; EW.3 cloud entity write → lore (lore-tenant path), index from the resulting `BranchPushed`; EW.4 offline queue/replay + "tenant offline" UX + conflict surfacing; EW.5 path↔tree mapping shim.
- **Re-scope SBAI-2381 + the conflict-resolver** from "build" to "**superseded by lore sync (desktop); retain minimal BYO path**."

## 7. Open items for owner sign-off
1. §3 write path — ratify the four recommendations (thin facade / queue-replay / push-verify / path-shim) at the EW.0 spike, or steer.
2. Web-only tenants — BYO-only mode (no lore) is the accepted answer (no desktop = no lore host). Confirm.
3. Whether to ever fund lore GDrive/OneDrive store plugins (would let those tenants join the lore sync layer); deferred.
