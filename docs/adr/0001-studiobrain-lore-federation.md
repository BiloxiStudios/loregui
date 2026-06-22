# ADR-0001 — StudioBrain federates over tenant lore (per-tenant client + sb-relay); lore is NOT a backend

- **Status:** Proposed — 2026-06-22
- **Deciders:** BizaNator (owner)
- **Source research:** five parallel deep-planning streams (federation data plane, identity/grant/license, DAM content engine & data model, model-manager inference, auto-deploy & installers), each grounded in the actual code across `loregui`, `loregui-cloud`, `model-manager`, `studiobrain` (core/cloud/accounts/app), and the pinned `lore` checkout.

---

## 1. Context

The product vision: **StudioBrain is the monetization + content engine. LoreGUI and model-manager are open-source satellites built as part of StudioBrain** — they make the DAM (digital asset management) better, ship inside the StudioBrain installers, are customized + license-unlocked when hooked into StudioBrain, and remain fully functional standalone.

The triggering question was: *should StudioBrain use lore as a storage backend?* Investigation of the lore source settled it:

- **lore has no tenant concept.** Its unit of isolation is the *repository* (JWT-gated); `namespace()` in the server is just gRPC service naming, not tenancy.
- **One storage config per server, global to all repos** (`immutable_store` / `mutable_store` / `lock_store`). No per-repo or per-tenant storage routing. "Multi-tenant with each tenant's own storage" would require *one loreserver per tenant* (rejected) or forking lore's storage layer.
- **lore is pre-1.0, Epic-controlled, and `lore::service::start` is still a stub upstream.** Betting StudioBrain's core storage on it is unacceptable risk.

## 2. Decision

1. **StudioBrain does NOT adopt lore as a storage backend, and does NOT host per-tenant lore servers.** Garage-S3-as-source-of-truth + `studiobrain_content` (YB) as the rebuildable index stays StudioBrain's native model.
2. **studiobrain-cloud connects to each tenant's lore as a per-tenant READ CLIENT** — gRPC/TCP over the **sb-relay** (bore) — and indexes their entity markdown + asset *metadata* into `studiobrain_content` tagged `source='lore'`. **It never stores asset bytes**; it caches a derived preview and fetches full-res lazily over the live link. The tenant's own lore storage (their S3/local/GDrive) stays the master.
3. **LoreGUI + model-manager ship inside the StudioBrain installers as `externalBin` sidecars**, pinned by release tag, **customized at install/runtime via injected config + a StudioBrain-signed license** — the *same open binary* runs free/standalone with no license.
4. **Identity is accounts-minted.** Two distinct accounts-issued JWTs carry the system: a new **Lore Service Grant (LSG)** (read-only, tenant-pinned, `aud: lore-service`) for "StudioBrain reads lore," and the user session token for "satellite unlocks premium." Accounts remains the sole minter; satellites only verify (JWKS) and read claims; **no storage creds or PII ever leave accounts.**
5. **Converge on ONE canonical entitlement model now (pre-beta) — eliminate the `plan`/`tier` impedance mismatch rather than bridge it.** A single ecosystem-wide model, defined by StudioBrain accounts and adopted verbatim by LoreGUI + model-manager:
   - **`tier`** — an integer ordinal, **spaced** for future insertion, **paired with a stable string id** for logs/readability: `0 free`, `10 indie`, `20 team`, `30 enterprise`, `40–89` reserved (future paid tiers), **`90 staff`** (read-mostly cross-tenant), **`99 superadmin`** (StudioBrain-internal total multi-tenant admin). A staff/superadmin tier is a strict superset, so ordinal comparison (`tier >= MIN`) is the gate for monotonic features.
   - **`features[]`** — authoritative set for **non-monotonic add-ons** (e.g. BYOK is Enterprise-only but isn't "more" than Team on every axis). Tier handles the common monotonic case; `features[]` covers the exceptions.
   - **`role`** (owner / admin / member) stays a **separate axis** — it's *within-tenant permission*, not subscription capability. Don't conflate it with `tier`; the 90/99 band is *our* cross-tenant staff, not a tenant's own org-owner.
   - **LoreGUI consumes the StudioBrain accounts JWT DIRECTLY** as its primary auth + entitlement source (via the existing SBAI-1935 bridge). model-manager reads the same claim per-request.
   - **Tier scheme LOCKED (2026-06-22):** `0 free · 10 indie · 20 team · 30 enterprise · 40–89 reserved · 90 staff · 99 superadmin`, each with a stable string id; `features[]` for add-ons; `role` separate.
6. **Unify on ONE token + verification path — drop LoreGUI's separate Ed25519 offline-license system.** Premium LoreGUI features are licensed *via StudioBrain* (a tier entitlement), so there is no need for a parallel account-less license. **Reuse exactly what studiobrain-app already ships:** the accounts RS256 JWT, JWKS-verified, **cached locally and honored offline within the `tenant_policy` grace window** (free 24h / monthly 7d / annual 30d, extendable per tenant). For the rare **air-gapped standalone** studio, issue a **long-lived "license JWT" from the same accounts signing key** (far-future `exp`, large offline grace) — *same format, same JWKS path* — rather than a second license format. Net: one token type, one verify path, everywhere. LoreGUI's `commercial/license.ts` Ed25519 verifier is **removed**; the `commercial/` entitlement *gate* + registry seam stay (now reading the JWT claim). This **collapses E0.5** (no new keypair) and **simplifies E2.4** (a single entitlement source). Assumption: LoreGUI premium is never sold standalone *outside* a StudioBrain entitlement — if that ever changes, the long-lived license-JWT covers it without reviving Ed25519.

This is the **"federate, don't replace"** architecture: StudioBrain stays master of its own data, and *additively* indexes tenant lore — so lore changing/breaking can never corrupt StudioBrain's core.

## 3. Architecture (the five streams, woven)

### 3.1 Federation data plane (stream 1)
- A new `sb-lore-client` crate binds **lore's gRPC proto stubs directly** (thin-client `RevisionTree`/`RevisionDiff`, `StorageService.Get`, `NotificationService.Subscribe`, repo/revision/lock services) — **not** via `lore-vm` (which is working-copy/disk-based; the cloud indexer wants a working-copy-*less* reader).
- A `TenantLoreRegistry` (LRU per-tenant session pool, modeled on the existing `TenantDbPool`) owns connect / reconnect-backoff / health and a per-tenant **notification loop**.
- The existing `loregui-dam-connector`'s **output contract is reused wholesale** — `POST /api/entity/{type}` → `WriteThroughCache::write()` (YB + Garage + Qdrant + async `IndexEvent`), `source='lore'`. Only the *source* changes: local `walkdir` → a `RemoteEnumerator` over the gRPC client. `mapping.rs` (SDK-parity projection) and `studiobrain.rs` (HTTP sink) are untouched.
- **Sync:** initial full index → incremental via `BranchPushed` notifications → `RevisionDiff` → targeted single-entity reindex; **a periodic reconcile/catch-up diff is load-bearing** because lore's notification stream has no replay/cursor (lagged events drop silently).
- **Offline:** the YB index stays **authoritative for search**, previews stay served from Garage; only full-res fetch + freshness degrade when the tenant machine is off.

### 3.2 Identity, grant & entitlement (stream 2)
- **The "Connect StudioBrain" grant flow** mints the **LSG** (accounts-issued RS256, `aud: lore-service` ≠ the user token's `aud: citybrains-app`, `scope: [lore:repo:read, lore:asset:read]`, `tenant_id`-pinned, ~1h TTL + refresh, revocable via the existing token blacklist + a Valkey revoke push). Consent UI is served **only** by the accounts iframe (SBAI-1935) — no auth UI in any satellite. studiobrain-cloud holds only the short-lived LSG (in Valkey), never storage creds, never PII.
- **Entitlement unlock — convergence (decision §2.5):** rather than bridge `plan→tier`, all parties adopt the canonical `{tier:int, features[]}` model. accounts emits it on the user JWT; LoreGUI's `commercial/entitlement.ts` is refactored to key off the canonical `tier` ordinal (`tier >= MIN` for monotonic features) ∪ `features[]` (for add-ons), with the StudioBrain JWT as the primary source and the offline license as a same-shape fallback (resolved as a union so "hooked in" only ever *adds* unlocks). The `commercial/` *seam* (registry + gate) is unchanged — only the resolution vocabulary converges.
- model-manager gets a **net-new runtime entitlement layer** (today it gates only by compile-time Cargo feature), reading the same canonical `tier` per-request (one gateway serves many accounts) to gate Tier-5 cloud burst / managed cluster / BYOK.

### 3.3 DAM content engine & data model (stream 5)
- Lore-sourced rows live in the **same `entities`/`assets` tables** as native rows, distinguished by the **`source` column** (`V3592`, written, **not yet applied**). The rebuildable-cache invariant becomes *per-source*: rebuild `source='lore'` rows from lore at the keyed revision, `source='studiobrain'` rows from Garage — a full rebuild must **partition by source**, never blind-recrawl Garage (which would orphan lore rows).
- **New schema:** promote `lore_path`/`lore_rev` to first-class `assets` columns (replace today's description-string hack); add `assets.preview_only`; add an append-only **`entity_revisions`** snapshot table.
- **The flagship — entity-aware versioning:** a lore entity field (e.g. a character's eye-color material param) drives an asset; `entity_revisions` snapshots the field+link per `lore_rev` so the DAM can diff "what did `eye_material` resolve to at rev N vs N+1, and which asset did it drive," extending the existing image-compare to `(asset_id, lore_rev)` pairs. StudioBrain stores thin per-revision snapshots over lore's authoritative history — **it never becomes the VCS.** This is the screen the SBAI-4079 UE plugin drives bidirectionally.

### 3.4 model-manager inference plane (stream 4)
- An OpenAI-compatible **local-first inference gateway**; the LAN "your own GPU box" mDNS mesh is **fully open-core**. It's deliberately **tier-agnostic** — gating happens at the cloud/account layer (aligns with §3.2).
- It is the inference engine under the DAM's AI: image embeddings + vision tags (`/api/vision/embed`, implemented), text embeddings, auto-caption/transcription (stubbed). The DAM calls it over one env-var URL, so embeddings can run on the tenant's **own** gateway.
- **Highest-leverage technical decision:** today text (384-d bge) and vision (512-d CLIP) live in **separate vector spaces** and aren't comparable; adopting the shipped **Qwen3-VL 2560-d shared space** unlocks real cross-modal "find images by text." This is a head-of-workstream decision.

### 3.5 Auto-deploy & customization (stream 3)
- Bundle both satellites as **`externalBin` sidecars fetched in CI, pinned by tag** (the model-manager pattern is already live). Promote the hardcoded version to a `satellites.lock.json` manifest. Two release trains joined only by the pin file; satellites keep their own open releases.
- **One binary, three injection channels:** config (host wiring + branding), the **signed-license entitlement seam** (port LoreGUI's `commercial/` model to the gateway), and a "host present" signal (injected token + JWT verify; no device-pairing needed for a co-installed sidecar).
- **Recommendation:** ship a *StudioBrain-composed* LoreGUI build (the `loregui-cloud` overlay applied in CI) for the bundled case so premium panels physically exist (dark by default, lit by the injected license); the standalone open release stays overlay-free. Same source, two compose recipes — no fork.

### 3.6 Cross-stream convergences (where the streams are the same work)

```
          ┌─────────────────────────── accounts (sole token minter, PII boundary) ──────────────────────────┐
          │   LSG (aud=lore-service, scope=lore:read, tenant-pinned)        user JWT (plan claim)            │
          └───────────────┬───────────────────────────────────────────────────────┬──────────────────────-─┘
                          │ (stream 1 "service token" == stream 2 Epic E1)          │ (stream 2 E2/E3 == stream 3 injection)
                          ▼                                                          ▼
 tenant LoreGUI ──host loreserver──► sb-relay (bore, TCP/gRPC) ──► studiobrain-cloud per-tenant lore CLIENT
   (their storage)        (SBAI-4072, built)                          │  index markdown + asset META (source='lore')
                                                                      │  NEVER stores bytes; preview cached, full-res lazy
                                                                      ▼
                                              WriteThroughCache → studiobrain_content (YB) + Garage + Qdrant
                                                                      │  (V3592 `source` column = the universal gate)
                          ┌───────────────────────────────────────────┤
                          ▼                                            ▼
         model-manager gateway (tenant's OWN GPU box, LAN-mDNS)   DAM product: provenance views, unified
           embeds lore assets locally → bytes+embeddings          semantic search (Qwen3-VL shared space),
           never leave the studio  (stream 1 + stream 4)          entity-aware revision diff (flagship → UE 4079)
```

Five convergences drive the sequencing:
1. **LSG = stream 1's "service token" = stream 2's Epic E1.** One deliverable (accounts).
2. **Applying the `V3592` `source` column is the universal gate** for all index-side work (streams 1 + 5).
3. **The embedding-space decision** (streams 4 + 5) heads the DAM workstream.
4. **The entity_revisions flagship** (stream 5) is the capstone that unblocks the UE plugin (SBAI-4079).
5. **The injection/entitlement seam** (stream 2's E2/E3) *is* stream 3's customization mechanism — and porting it to model-manager is shared.

## 4. Consequences

**Positive:** no asset-storage cost or scale liability for StudioBrain; perfect fit with the existing BYO-storage model; free versioning/provenance; a strong privacy story (bytes *and* embeddings can stay on studio hardware); open-core credibility (the satellites are genuinely useful standalone); StudioBrain's core is insulated from lore's pre-1.0 churn.

**Costs / risks (all designed-around, none blocking):** the LSG grant primitive is net-new; lore has four real gaps (no headless service-token issuance, no notification replay, no chunked blob reads, metadata-thin tree) — mitigated by the accounts-minted LSG + a periodic reconcile loop + size-gated previews, and filed as upstream tickets; per-tenant connection scale through one relay needs sizing; the placeholder license key must be replaced with a real keypair before GA.

## 5. Phased implementation plan

- **Phase 0 — Foundations / unblock** (mostly parallel): apply `V3592` (needs sign-off); LSG grant primitive in accounts; `sb-lore-client` gRPC crate; `tenant_lore_configs` schema; real license keypair + minting service; **embedding-space decision**.
- **Phase 1 — Federation data plane:** `TenantLoreRegistry`, `RemoteEnumerator`, initial full index, notification loop + reconcile, preview-cache + lazy full-res route, offline UX + metrics.
- **Phase 2 — Identity, grant & entitlement wiring:** LSG end-to-end (consent UI, refresh, revoke), the `plan→tier` entitlement bridge in LoreGUI (union resolution), model-manager runtime entitlement, "Connect StudioBrain" button.
- **Phase 3 — The DAM product:** first-class `lore_path`/`lore_rev` columns, `/assets/{id}` deep-link route, provenance badges + filter, entity-aware grouped views, **embedding unification → cross-modal semantic search**, **`entity_revisions` + revision-diff (flagship)**, sub-feature tier matrix + BrainBits metering.
- **Phase 4 — Auto-deploy & customization:** `satellites.lock.json`, `download-loregui.sh` + externalBin, `loregui_sidecar.rs` spawn/supervise, gateway `[cloud.studiobrain]` config-writer, host license/branding injection, CI compose+bundle, first-run/registration UX.
- **Phase 5 — Hardening:** threat-model tests, boundary CI guard, source-partitioned rebuild, observability, file the lore upstream gap tickets, UE-plugin bridge (SBAI-4079) on the revision model.

## 6. Epic / Story breakdown (deduped & dependency-sequenced)

Top Epic: **Federated content engine — StudioBrain over tenant lore.** Sub-epics map to the phases. (`⚠ sign-off` = needs explicit human go; `⛓ <id>` = dependency.)

**E0 — Foundations**
- E0.1 ⚠ Apply `V3592__lore_source_tag.sql` on BOTH YB sites + verify (`\d entities`/`\d assets`) — *gates all index work* (studiobrain-core PR #958).
- E0.2 LSG grant primitive in accounts: `lore_grant` table (both sites), `create_lore_service_token` (aud=lore-service), `/api/lore-grants` + `/internal/.../{refresh,revoke,status}`, revoke→blacklist+Valkey. *(= stream-1 service-token gap)*
- E0.3 `sb-lore-client` crate: tonic stubs from lore-proto, `LoreClient::connect` + auth interceptor + tree/read/subscribe.
- E0.4 `tenant_lore_configs` schema (both YB sites + sqlite mirror, `db_compat`).
- E0.5 **Unified offline entitlement (revised per §2.6):** reuse studiobrain-app's existing cached-accounts-JWT + offline-grace mechanism in LoreGUI; **remove `commercial/license.ts` (Ed25519)**; add a long-lived "license JWT" issuance path (same accounts signing key + JWKS) for standalone/air-gap. No new keypair.
- E0.6 **Decision/spike:** unify the DAM embedding space (keep dual vs adopt Qwen3-VL 2560-d shared) → collection-schema + migration plan.
- E0.7 ⚠ **Canonical entitlement model (global, pre-beta, prevents tech debt):** define `{tier:int ordinal (0/10/20/30…90/99) + stable string id, features[], role separate}` in a shared spec; change accounts to emit it on the JWT (and re-map existing plans: free→0, indie→10, team→20, enterprise→30); make the offline license emit the identical shape. *All of E2 keys off this.* Touches accounts + LoreGUI + model-manager; do it before release.

**E1 — Federation data plane** ⛓E0.2,E0.3,E0.4
- E1.1 `TenantLoreRegistry` (per-tenant LRU session pool, reconnect/backoff, health).
- E1.2 `RemoteEnumerator` replacing the connector's local enumerator (async build_plan/run, bytes over gRPC).
- E1.3 Initial full index wired to the existing `IndexClient` HTTP sink (`source='lore'`). ⛓E1.1,E1.2,E0.1
- E1.4 `source_hint="lore"` on `IndexEvent` + pipeline. *(small)*
- E1.5 Notification subscribe loop (`Subscribe`→`RevisionDiff`→reindex). ⛓E1.3
- E1.6 Periodic reconcile / catch-up diff (mitigates no-replay; also the reconnect path). ⛓E1.5
- E1.7 Desktop commit-nudge fallback (LoreGUI → existing `/api/internal/indexer/reindex`). ⛓E1.3
- E1.8 Preview generation + preview-only upload (reuse sb-thumbnail-worker); stop sending originals. ⛓E1.2,E1.3
- E1.9 Lazy `GET /api/assets/{id}/original` streaming from `LoreClient::read_file` (online-only). ⛓E1.1
- E1.10 Metrics + health surfacing + "tenant offline → preview-only/search-works" UX. ⛓E1.1,E1.5,E1.9

**E2 — Identity, grant & entitlement** ⛓E0.2,E0.5
- E2.1 Connector requires an LSG (not user JWT, not service secret) for content reads; tighten DAM-CONNECTOR auth wording.
- E2.2 lore server: JWKS-verify LSGs (`aud=lore-service`) + scope enforcement on read ops.
- E2.3 LoreGUI: refactor `entitlement.ts` to the canonical `tier` ordinal + `features[]` (per E0.7); `bootstrapAccountsEntitlements()` reads the SB JWT's canonical claim directly (no `plan→tier` translation — convergence removed it). ⛓E0.7
- E2.4 LoreGUI: `resolveEntitlements()` reads the single canonical source — the cached accounts JWT honored within `tenant_policy` offline-grace (no separate license path). ⛓E0.7,E0.5
- E2.5 LoreGUI: "Connect to StudioBrain" button mounting the accounts iframe (no new auth UI). ⛓E0.2
- E2.6 model-manager: per-request JWT `plan` extraction + runtime gating (Tier-5/cluster/BYOK); fail-open for pure self-host.

**E3 — DAM product** ⛓E0.1,E0.6
- E3.1 Promote `lore_path`+`lore_rev` to first-class `assets` columns (both sites) + accept in upload; connector sends them. ⛓E0.1
- E3.2 Stable deep-link route `/assets/{id}` (today detail is a local modal).
- E3.3 Provenance badge + lore/native/all filter (uses the partial indexes).
- E3.4 Entity-aware grouped view + cross-ref graph. ⛓E3.2
- E3.5 Route text embeddings through the gateway; cross-modal Qdrant search. ⛓E0.6
- E3.6 DAM semantic-search API + UI (tenant-scoped, BrainBits-gated). ⛓E3.5
- E3.7 `entity_revisions` table + revision snapshot on reindex. ⛓E0.1
- E3.8 **Entity-aware revision-diff UI (flagship)** — per-field timeline + `(asset_id, lore_rev)` compare + revision scrubber. ⛓E3.4,E3.7
- E3.9 Auto-caption/transcription wired through the gateway into the index (via WriteThroughCache). ⛓E0.6
- E3.10 Sub-feature tier matrix + BrainBits metering (split the coarse `dam` boolean). ⛓E3.3,E3.6
- E3.11 Source-partitioned rebuild (fan out by `source`; guard the legacy walker). ⛓E0.1,E3.7

**E4 — Auto-deploy & customization** ⛓E0.5,E2.3
- E4.1 `satellites.lock.json` pin manifest; refactor download scripts to read it; "About → Components".
- E4.2 `download-loregui.sh` + externalBin wiring + build.rs placeholder. ⛓E4.1
- E4.3 `loregui_sidecar.rs` spawn/supervise (fix the model-manager resolver-name mismatch). ⛓E4.2
- E4.4 Gateway `[cloud.studiobrain]` + `[auth] jwt` config-writer (app side, first-run, idempotent).
- E4.5 Gateway runtime-entitlement layer (port the LoreGUI `commercial/` seam). ⛓E0.5 *(= E2.6 impl home)*
- E4.6 LoreGUI host-injection of branding + entitlements/license. ⛓E4.3,E0.5
- E4.7 CI compose+bundle in `desktop-build.yml` (download satellites, overlay-compose LoreGUI, codesign sidecars, GPU-variant match). ⛓E4.1,E4.2,E4.6
- E4.8 First-run/registration UX (health-poll discovery, register gateway as LLM upstream, Components status). ⛓E4.3,E4.4

**E5 — Hardening & upstream**
- E5.1 Threat-model tests (audience-confusion, cross-tenant LSG, revoke latency, offline-grace expiry).
- E5.2 Boundary CI guard (fail if accounts/PII/proprietary symbols leak into the open repos).
- E5.3 File lore upstream gap tickets: notification cursor/replay, chunked/range blob read, TreeNode size/mtime/content-type, non-interactive service token.
- E5.4 UE-plugin bridge (SBAI-4079) consuming the entity-aware revision model. ⛓E3.8

**Critical path:** E0.2 → E0.3 → E1.1 → E1.3 → E1.5 → E1.6 (with E0.1 gating all index writes, E0.6 gating DAM search). Phases 2/3/4 parallelize off the data plane once E0 lands.

## 7. What's already built (this initiative starts from working foundations)
- The lore-source findings + the **federate-don't-replace** decision (this ADR).
- The **DAM federated connector** (`loregui-cloud/crates/loregui-dam-connector`, merged) — local-source today, evolves to remote per E1.
- The **`source` provenance column** (studiobrain-core PR #958, open, awaiting E0.1 apply).
- **sb-relay / bore cross-network** (SBAI-4072, shipped) — the transport for E1.
- **LoreGUI host-a-server + sidecar** (SBAI-4065/4069, shipped + install-verified) — the tenant side.
- The **`commercial/` entitlement + overlay seam** (shipped) — the model for E2/E4.
- **model-manager** mDNS LAN mesh + OpenAI-compatible gateway (shipped) — the inference plane for E3/E4.
