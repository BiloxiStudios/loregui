# ADR-0002 Appendix — Storage model head-to-head evaluation (2026-06-25)

Evidence base for ADR-0002's "complementary layers" decision. Two parallel evidence-based audits of the actual code (`/opt/studiobrain-dev/{cloud,core,app}` and `/srv/studiobrain-dev/{lore,loregui,loregui-cloud}`).

## A. StudioBrain's current storage + sync model — real maturity

| Component | Status | Evidence |
|---|---|---|
| Write-through cloud PUT (YB+Garage+Qdrant) | **BUILT + unit-tested (mocks only)** | `write_through.rs`, 17 tests incl. rollback/idempotency; all use in-process mocks — no real-infra integration test |
| Indexer pipeline (events/debounce/worker) | BUILT + unit-tested; integration unverified | `indexer/`; embeddings stub when env unset; dead-letter log-only (lost on restart) |
| BYO-storage via OpenDAL (S3/GDrive/OneDrive/Dropbox) | BUILT; partially tested | `storage_state.rs`; only config round-trip tested, no real remote connection test |
| Binary asset proxy (stream + HTTP **range**) | **TESTED (39 tests)** — strongest surface | `routes/asset_proxy.rs`; 206/Range, tier-gated upload |
| Presigned URLs / direct-to-storage | **UNBUILT** | zero `presign` matches; all I/O proxies through the pod |
| Chunked / large-file delta | **UNBUILT** | `chunk_routes.rs` every endpoint returns 501 "not yet wired" |
| Desktop offline edits → cloud (SBAI-2374) | **FIXED in code — CLAUDE.md stale**; **E2E never proven** | `app/.../sync.rs` `cloud_reindex()` merged; only E2E is `test.skip(!reachable)`, self-flagged "WILL FAIL" |
| Reverse sync cloud→desktop (SBAI-2381) | **STUB** | `valkey.rs` `subscribe()` returns empty stream; `publish_entity_update` zero callers |
| Conflict resolution | **BUILT + 16 tests, ZERO callers (dead)** | `conflict_resolver/` declared but unwired |

**Verdict:** single-writer cloud storage + serving is real (mock-tested, not E2E-proven); the **distributed/multi-client/offline/conflict half is stubs + dead code + unvalidated wiring.** Owner's instinct confirmed, with one correction (the desktop→cloud reindex fix *is* in code).

## B. lore as a general content + asset backend — capability

| DAM need | Supports? | Evidence / friction |
|---|---|---|
| Store binary asset / **flat byte-copy** | **YES** | `StorageService.Put` = direct content-addressed blob put, FastCDC-chunked, binary-safe, no commit needed |
| Path addressing (`{tenant}/{project}/path`) | **partial / needs-build** | hash-addressed only; paths via tree/revision (stage→commit→push ceremony) |
| Tenant's-own-S3 as backend | **partial** | `immutable_store.mode="aws"` S3 plugin — **S3 only**, no GDrive/OneDrive/OpenDAL |
| Presigned asset download | **YES** | HMAC mint→redeem→stream, built |
| Large-file **range** read | **NO** | zero `Range`/`206` in `http/`; always full 200 |
| Native multi-client sync | **YES (core strength)** | branch push/pull + `Subscribe`; **no replay/cursor** → needs reconcile |
| Cloud write API | **needs-build** | `sb-lore-client` read-only; writes only via disk-based `lore-vm`; no headless service-token mint |
| Multi-tenant isolation (one shared server) | **partial** | per-repo JWT scoping enforced; **no per-repo crypto**, cross-repo dedup oracle → moot in our one-server-per-desktop-tenant topology |
| Maturity | **production-grade server** | zero `todo!`/`unimplemented!` in `lore-server/src`; `service::start` stub is an unrelated repo-daemon helper |

**Verdict:** lore stores bytes (incl. binary, owner's question = YES) and can S3-back onto the tenant's bucket, but is **VCS-shaped** where a DAM wants a flat path-keyed object store (hash-addressed, working-copy write ceremony, no range, read-only cloud client, S3-only). It is best as the **versioned sync/source-of-truth backbone**, not the flat asset store — exactly complementary to BYO-storage.
