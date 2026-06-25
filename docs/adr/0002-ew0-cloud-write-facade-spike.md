# EW.0 Spike — cloud write facade for lore (the lore-tenant write path)

- **Status:** Spike / Design — 2026-06-25. Implements ADR-0002 §3 (the one structural gap). Gates EW.1–EW.5.
- **Scope:** how studiobrain-cloud writes a user's edit *back* to a desktop-hosted tenant's lore (the source of truth), given `sb-lore-client` is read-only today.

---

## 1. The gap
ADR-0002 makes lore the source-of-truth + sync fabric for **desktop-hosted** tenants; cloud is a per-tenant client. Today `sb-lore-client` is **read-only** (`list_branches/head/tree/read_file/read_meta/subscribe`; LSG scope `lore:repo:read`). When a user edits an entity in the cloud web UI for a lore-hosted tenant, the write has **no path to lore**. The BYO-only tenant path (`write_through` → S3) is unaffected and unchanged.

## 2. Write path overview
```
PUT /api/entity/:type/:id
  ├─ tenant mode == BYO-only  → write_through (YB + S3 + Qdrant)            [UNCHANGED]
  └─ tenant mode == lore      → LoreWriteClient.write_entity(path, bytes, base_rev)
                                  → CAS Put(bytes) → build Revision(path in tree) → BranchPush
                                  → push-then-verify → ack user
                                  → YB/Qdrant index follows from the BranchPushed notification (E1.5)
                                    (ONE write to lore; the index is derived, not a dual-write)
```

## 3. The write facade (`sb-lore-client`, read → read+write)
New `LoreWriteClient` companion to the read `LoreClient`; LSG scopes `lore:repo:write` + `lore:asset:write`:
- `write_entity(tree_path, markdown_bytes, base_rev) -> WriteResult{ new_rev, address }`
- `write_asset(tree_path, bytes, mime) -> WriteResult` (FastCDC-chunked via CAS)
- `delete(tree_path, base_rev) -> WriteResult` (tombstone in a new revision)
- `batch_write(items, base_rev) -> WriteResult` (multiple paths in ONE revision — atomic multi-file commit)

Underlying lore RPC sequence (per the evaluation): `StorageService.Put`(blob → `Address`) → assemble a `Revision` referencing the staged blob(s) at their tree path(s) → `RevisionService.BranchPush(branch_id, revision_sig, force=false, fast_forward_merge=true)`.

### 3.1 THE LOAD-BEARING UNKNOWN — working-copy-less revision construction
lore's write surface is **working-copy oriented**: `lore-vm` `stage → commit → push` operate on a checked-out directory on disk. The cloud has **no working copy** (symmetric with the read facade, which is working-copy-less by design). So the facade must **construct the revision/tree graph directly from the gRPC stubs** — Put the blob, compute the new tree (prior tree + the changed path), build the `Revision` blob, push the branch — **without** disk state or `lore-vm`.

**This is the net-new engineering and the first thing to prototype.** Options, in preference order:
1. **Working-copy-less revision builder in `sb-lore-client`** (RECOMMENDED): replicate the minimal stage/commit logic against `StorageService` + `RevisionService` stubs. Read the base revision's tree (already have `tree()`), splice the changed path, write the new tree + revision blobs via `StorageService.Put`, `BranchPush`. No disk, no `lore-vm`.
2. **Ephemeral server-side working copy** (fallback): a tmpdir per write, `lore-vm` stage/commit/push, discard. Simpler to build (reuses `lore-vm`) but adds disk I/O + a working-copy lifecycle in cloud — rejected unless (1) proves infeasible.
3. **Upstream ask** for a headless "write file + commit" RPC (file as a lore gap; don't block on it).

**Spike deliverable: prototype option (1) end-to-end (Put → tree splice → Revision → BranchPush → read back) against a local loreserver before committing EW.1–EW.5.**

## 4. Path ↔ tree mapping
DAM paths are `{tenant}/{project}/entities/{type}/{id}.md`. lore is hash-addressed; paths live only in a revision tree. The tenant's lore **repo == the project**, so the DAM path maps to the lore **tree path** `entities/{type}/{id}.md` (the `{tenant}/{project}` prefix is the repo identity, not part of the tree). A small per-repo `PathMapper` (DAM path ↔ lore tree path) lives in the cloud lore module; the existing `RemoteEnumerator` already walks these same tree paths on read, so read and write share one mapping.

## 5. Durability — push-then-verify
lore's dangling-anchor / deferred-flush model means a `BranchPush` can return before the post-command flush persists the blobs. A cloud write is **durable** only after: (1) `BranchPush` succeeds, **and** (2) `RevisionService.RevisionInfo(new_rev)` resolves **and** `StorageService.Query([addresses])` confirms the blobs are stored (not dangling). **Only then ack the user.** The periodic reconcile (E1.6) is the backstop for any slip.

## 6. Offline queue + replay (replaces the SBAI-2381 stub)
When the tenant's lore is unreachable (their desktop is off):
- **Reads:** YB index + preview cache → read/search-only (works today).
- **Writes:** enqueue a durable pending write `{tree_path, bytes, base_rev, user, ts}` per tenant (Valkey list or a YB `pending_lore_writes` table). UX: *"<tenant> is offline — your change will sync when their app reconnects."*
- **Reconnect:** the registry health loop detects the tenant's lore is back → drain the queue → replay each write. lore's `BranchPush(fast_forward_merge)` handles non-conflicting concurrent desktop edits; true same-field conflicts surface via `RevisionDiff` to the user. **lore's revision/merge owns conflict — the dead `conflict_resolver` stays retired.**

## 7. Auth — LSG write scopes (EW.2)
accounts mints the LSG with `lore:repo:write` + `lore:asset:write` (read-only today). The lore server already JWKS-verifies + per-repo scope-checks on every RPC; extend enforcement to the write ops. The grant stays tenant-pinned, short-lived, Valkey-stored, revocable.

## 8. Integration point (EW.3)
`cloud_entity.rs` `PUT/POST/DELETE /api/entity/*`: branch on **tenant mode** (a `tenant_lore_configs` lookup — already exists). lore tenants → `LoreWriteClient`; BYO tenants → `write_through` (unchanged). For lore tenants the YB/Qdrant index update is driven by the resulting `BranchPushed` (the existing E1.5 notification loop) — **one source of truth, no dual-write**, so cloud and desktop edits index identically.

## 9. Risks / open questions
- **(load-bearing)** working-copy-less revision construction — prototype first (§3.1).
- **base_rev staleness / lost-update:** the facade must `head()` before push and pass `base_rev`; a stale base → merge or a 409 to retry.
- **lore no-range-read / metadata-thin tree:** affects large-asset *read*, not write — out of EW.0 scope.
- **Large-asset writes:** CAS Put is FastCDC-chunked, so large blobs are fine on write; the read-side range gap is tracked separately.

## 10. Implementation breakdown (post-spike)
- **EW.1** `LoreWriteClient` — the working-copy-less revision builder + `write_entity/write_asset/delete/batch_write` + push-then-verify. *(prototype §3.1 first — gates the rest)*
- **EW.2** LSG write scopes in accounts + server-side enforcement.
- **EW.3** cloud entity route: tenant-mode branch → facade; index from `BranchPushed`.
- **EW.4** offline queue + replay + tenant-offline UX + conflict surfacing.
- **EW.5** `PathMapper` (DAM path ↔ lore tree path) shim.

**Critical path:** EW.1 prototype (§3.1) → EW.1 → EW.3 (with EW.5) → EW.2 → EW.4.


---

## UPDATE 2026-06-25 — EW.0 verdict + corrections (running prototype + scoping)

## EW.0 — Write-back spike verdict (CORRECTED)

**Status: DONE / proven with a running prototype.** This section is the single source the rest of the epic cites for the write-back mechanism contract.

### Verdict (corrected)

Working-copy-less write to a tenant's lore **is achievable**, but **NOT via raw gRPC stubs**. lore's Revision/Tree blobs are compressed `repr(C)` structs that the server hash-verifies on receipt, so a hand-rolled protobuf client cannot synthesize a valid revision. The write path must **link `lore-revision` + `lore-storage` in-process** and drive their real APIs. Two mechanisms were validated:

- **Mechanism A — in-process, PATH-LESS lore-revision write (PRIMARY).** No disk, no working copy:
  1. `RepositoryContext::new(None, immutable_store, mutable_store, repo_id, instance_id, Ok(connection), filter, format)` — `path = None`, so `require_path()` fail-fasts and no working-tree code path is hit.
  2. Connect to the tenant's loreserver over the relay; resolve the branch tip.
  3. `State::deserialize(repo, tip)` — fetches and decodes the tip `StateData` blob over the wire.
  4. `immutable::write(repo, ctx, content_bytes, WriteOptions::default().with_remote_write())` → `(Address, Fragment)`. **The client computes `address.hash = blake3(payload)`** (`lore-storage::hash_slice`); the server then hash-verifies the Put.
  5. `node_add(path)` into the tree, `update_tree_root_hash`, `serialize`, sign, `branch::push::push`.
  Raw stubs cannot do steps 3–5 because the serialized blobs are compiler-/version-sensitive `repr(C)` structs.

- **Mechanism B — ephemeral tmp working copy via `lore-vm` (PROVEN FALLBACK).** `lore clone/checkout` shallow at tip into a tenant+request-scoped tmpdir → `stage` → `commit` → `push` → `rm -rf` (Drop guard, always). Slower and not cross-pod concurrency-safe; reserved for repo formats/transports mechanism A can't yet handle, and for entity markdown (not large assets).

`sb-lore-client` remains strictly **read-only** (`tree`/`read_file`/`read_meta`/`subscribe` — no write/push). The write surface is a new crate, `sb-lore-writeclient`.

### Oodle finding (corrected — NON-ISSUE, but CI-gated)

Oodle is an **OPT-IN Cargo feature, OFF by default**. `lore-storage` declares `default = []` and `oodle = []`; `lore-revision` maps `oodle = ["lore-storage/oodle"]` under a non-default feature. With `default-features = false` and the `oodle` feature **never enabled** (and `OODLE_LIB_DIR` never set), the cloud build links only the open `lz4-sys` + `zstd-sys` (crates.io). We control our lore builds, so there is **no proprietary dependency**. A default (LZ4/Zstd) desktop-lore repo round-trips against a no-oodle cloud build (proven in EW.1.1). A CI `cargo-deny`/`cargo-tree` gate fails the build if `oodle` or any Oodle native lib ever enters the cloud dependency graph.

### Attribution finding (corrected — per-user LSGs are MANDATORY)

lore attributes a commit/push to the **authenticated token's `sub`** (`execution_context().set_user_id(...)` → `commit_impl`, consumed as the revision author). There is **no author argument** on commit — only a message — and lore **ignores `act.sub`**. Therefore a single per-tenant service token would misattribute **every** web edit to the service principal. Honest attribution requires **per-user write LSGs**: `sub` = the editing web user's stable lore id (`user.user_id`, non-PII, immutable), scoped to (tenant, repo), carrying `lore:repo:write` / `lore:asset:write`. Two corollaries the spike surfaced: (a) canonical lore-server decodes tokens in a **native claim shape** (`env`/`name`/`preferred_username`/`idp` + `resources:[{resource_id:"urc-<repo>", permission:[…]}]`), not the current scope-based LSG shape; and (b) the normal write path **does not yet enforce** the `"write"` permission, so a read LSG that merely lists the repo can write today — both must be fixed (EW.2).

### Markdown-cache / lazy-asset finding (corrected — model confirmed)

The markdown-cache + lazy-asset model holds end to end with lore as the content backbone:
- **Markdown:** entity text/frontmatter is indexed into YB (`entities`) + Qdrant (`sb_entities_{type}`). The pipeline reads bytes via `storage.read_with_cache(tenant, storage_path)`, which for lore tenants is a cloud **cache populated from lore CAS** by the enumerator — the cache is an **index over lore's markdown, never authoritative**.
- **Assets:** on WRITE, bytes pass **THROUGH** cloud into lore CAS (`StorageService.Put`, FastCDC-chunked, client blake3) and are **NOT persisted in cloud** — only **metadata + a derived preview** are kept. Full-res is fetched **lazily over the live link** (`read_file`, online-only), size-gated by `read_meta`.
- **Provenance:** every lore-sourced row carries `source='lore'`, enabling rebuild partitioning. Notification has no replay → the reconcile-diff loop is the catch-up backstop.

### Acceptance (record-only)

- ADR-0002 §write-back captures mechanism A vs B, the `repr(C)`/hash-verify finding, the Oodle-opt-in finding, the per-user-LSG attribution finding, and the markdown-cache/lazy-asset confirmation; notes `sb-lore-client` is read-only.
- The prototype branch demonstrating a path-less `BranchPush` against a throwaway loreserver is archived/linked.
- Downstream stories (EW.1–EW.6) reference EW.0 for the mechanism contract.

### Open blockers handed to EW.1.1

1. Are the in-memory `ImmutableStore`/`MutableStore` impls public enough to build a path-less write context from cloud, or is an upstream (BiloxiStudios fork) patch needed?
2. Does the sb-relay (bore TCP/h2c) data plane forward `StorageService.Put` (writes) or only `Get` (reads)? If Put isn't relayed, both A and B need a different reach to the desktop loreserver.
3. Is `execution_context()` `user_id` per-async-task or process-global? Concurrent multi-tenant writes need per-call isolation or attribution leaks across requests.