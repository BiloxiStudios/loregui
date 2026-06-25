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
