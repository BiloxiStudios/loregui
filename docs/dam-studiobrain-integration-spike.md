# Enhanced Asset DAM for LoreGUI via licensed StudioBrain — integration spike

- **Epic / ticket:** SBAI-4077 (enhanced asset DAM), under SBAI-4068 (commercial gating)
- **Status:** SPIKE — design + a minimal gated seam. NOT a full build. No StudioBrain
  repo is touched this round.
- **Repos in scope:** `loregui` (open core, MIT) · `loregui-cloud` (proprietary overlay)
- **Referenced (read-only) repos:** StudioBrain `core` / `cloud` / `accounts` / `app`
  at `/opt/studiobrain-dev/`.

## 1. What exists today

### LoreGUI core (open, MIT)

- **Basic asset preview** (merged #301): the content workspace
  (`frontend/src/content/`) renders an image / 3D model / audio / video / text /
  binary preview for a selected file in the working copy. File-kind detection is
  `content/kinds.ts#kindOf(path)`; the workspace mounts from `App.tsx`
  (`workspaceFile` state, opened via `openWorkspace(path, …)`). This is a
  **preview of bytes on disk**, with no entity model, tagging, search, or
  cross-refs.
- **Commercial gating seam** (SBAI-4068): `commercial/entitlement.ts`
  (`isEntitled(feature)`), `commercial/premium-registry.ts`
  (`registerPremiumPanel` / `getPremiumPanels`, top-bar nav panels) and
  `commercial/relay-registry.ts` (a host-flow-embedded control). `App.tsx`
  renders premium nav buttons from `getPremiumPanels()`, each gated by
  `isEntitled(panel.feature)` with a `🔒` locked-upsell fallback. The open core
  registers nothing, so it ships zero premium UI.
- **The 2nd entitlement source is already designed** (`entitlement.ts` module
  docs): a StudioBrain accounts JWT (RS256, `accounts.studiobrain.ai`) whose
  `tier` claim maps through `featuresForTier()` and is injected into
  `window.__LOREGUI_ENTITLEMENTS__` before React mounts. No call site changes —
  `isEntitled("dam")` simply starts returning the tier's answer once the auth
  bridge lands.

### StudioBrain (the licensed platform — read-only reference)

StudioBrain is a schema-driven DAM. The relevant model:

- **Source of truth = markdown-in-Garage-S3**, path `{tenant}/{project}/…`. The
  `studiobrain_content` YugabyteDB tables (`entities`, `assets`,
  `entity_relationships`, `asset_relationships`) are a **cache/index** over that
  markdown, never authoritative. Schema:
  `core/crates/sb-server/src/schema_sqlite.sql` (sqlite) + the postgres migrations.
- **`assets` table** tracks rich per-asset metadata: `asset_id`, `filename`,
  `file_path`, `mime_type`, `checksum`, the linked `entity_type` / `entity_id`,
  `asset_category` / `asset_subcategory` (`images` / `audio` / `video` / `models`
  / `comfy_workflows` / `data`), media metadata (width/height/duration/
  polygon_count…), `tags` / `keywords` (JSON), `description`, `ai_analysis` (JSON,
  vision tags), licensing fields, and `tenant_id`.
- **`entities` table** holds entity frontmatter (`fields` JSON), `markdown_body`,
  an `assets` JSON list + `primary_asset`, an `embedding` blob, `search_text`,
  and the cross-ref `references` column. Cross-refs in markdown are
  `[[EntityType:entity_id]]` (SDK `extractCrossRefs`). Entity IDs are slugs from
  SDK `generateEntityId(name)`. Asset category is SDK
  `detectAssetCategory(mime, filename)`. All three SDK functions are the
  canonical source of truth (`core/packages/sdk/`).
- **Write path = `WriteThroughCache::write()`**
  (`cloud/crates/sb-cloud/src/write_through.rs`): a markdown write fans out
  atomically to YugabyteDB (`entities` upsert) → Garage S3 (`{tenant}/entities/…
  .md`) → Qdrant (embedding upsert), with rollback. Async re-indexing is driven
  by an `IndexEvent`; the internal entry point is
  **`POST /api/internal/indexer/reindex`**
  (`cloud/crates/sb-cloud/src/routes/indexer_routes.rs`, auth via
  `X-Service-Secret` or a tenant-scoped Bearer JWT), body
  `{ tenant_id, entity_type, entity_id }`.
- **Asset upload** is **`POST /api/assets/upload`** (multipart, fields `file`,
  `entity_type`, `entity_id`, `description`, `asset_category`;
  `core/crates/sb-server/src/routes/assets.rs`). It writes the binary to storage
  and upserts the `assets` row.
- **Semantic search** is **`GET /api/search/semantic`** /
  `GET /api/search/all` (`core/crates/sb-server/src/routes/ai.rs`): keyword (SQL
  ILIKE over `search_text`) + Qdrant vector search, grouped by entity type.
- **DAM UI** exists in StudioBrain's own frontend: an `/assets` page
  (`core/frontend/src/app/assets/page.tsx`, grid/list + filters + search) and an
  `AssetDetailModal` (preview, metadata, vision tags, comparison). **Caveat
  (important for deep-linking):** there is **no** stable `/assets/{id}` detail
  *route* today — the detail is a modal local to the `/assets` page. A deep-link
  target is a small follow-up on the StudioBrain side (see Phase plan).

## 2. The integration verdict: **federated index via a thin connector — NOT an importer, NOT a fork of the source of truth**

Three shapes were considered:

| Shape | What it means | Verdict |
|---|---|---|
| **Importer** | One-shot copy a lore repo's assets+markdown into StudioBrain's markdown-in-Garage as the new source of truth. | **Rejected.** It duplicates the source of truth. The lore repo (Epic's `lore` VCS) is already a versioned, locked, branch-aware store and IS the authoritative home of the studio's art/media. Copying it into Garage creates a second master, a sync/conflict problem, and violates "lore is source of truth." |
| **Live connector (write-through to Garage)** | LoreGUI streams every lore change into StudioBrain's `WriteThroughCache` so Garage mirrors the lore working copy. | **Rejected for v1.** Same duplicate-master problem plus a hard coupling: it would make StudioBrain's content pipeline a hot dependency of every lore commit, and force PII/tenant routing decisions into the open core. |
| **Federated index (recommended)** | Lore stays the source of truth. A **thin connector** projects lore asset *metadata* (path, kind, checksum, the owning entity's frontmatter-derived fields, cross-refs) into StudioBrain's `studiobrain_content` index **as a cache**, exactly as StudioBrain already treats its own markdown. The bytes are fetched on demand (or thumbnailed) — they are NOT re-mastered into Garage. | **Recommended.** ✔ |

**Why federated index wins.** StudioBrain's own architecture already says the YB
`studiobrain_content` tables are a *rebuildable cache over a markdown source of
truth*. A federated lore connector slots into exactly that contract: lore is the
markdown/asset source of truth, and StudioBrain indexes it — the same way
StudioBrain indexes its own Garage markdown. There is one master (lore), the index
is disposable and rebuildable, and the seam between the two systems is a small,
auditable connector rather than a fork of the data model.

Mechanically the connector reuses StudioBrain's existing internal entry point:
for each lore asset/entity it POSTs to **`/api/internal/indexer/reindex`** (or a
new sibling, see Phase 2) with the projected `{ tenant_id, entity_type,
entity_id }` and a metadata payload. No new write path; no `INSERT INTO entities`
from outside the indexer (which the StudioBrain CLAUDE.md forbids). The asset
bytes are served to the DAM either by a short-lived signed URL the connector
mints from the lore working copy, or by an on-demand thumbnail the connector
uploads via `POST /api/assets/upload` — the *binary* stays lore-owned; only a
*derived preview* and *metadata* live in the index.

## 3. The data mapping: lore ↔ StudioBrain

A lore repo holds asset files plus markdown (entity descriptions, frontmatter).
The projection:

| Lore concept | StudioBrain concept | Via |
|---|---|---|
| A lore **markdown entity file** (e.g. `Content/Lore/Characters/aria.md` with YAML frontmatter) | An `entities` row (`entity_type`, `entity_id`, `fields`, `markdown_body`, `search_text`, `embedding`) | `entity_id = generateEntityId(name)`; frontmatter → `fields` JSON; body → `markdown_body` + embedded for semantic search. SDK functions are canonical — the connector calls the SDK, it does NOT re-implement slug/label/category logic. |
| A lore **art/media file** (`.png` / `.glb` / `.wav` …) | An `assets` row (`asset_id`, `file_path`, `mime_type`, `checksum`, `asset_category`, linked `entity_type`/`entity_id`) | `asset_category = detectAssetCategory(mime, filename)`; `file_path` = the **lore repo path** (the connector's stable key back to the source); `checksum` = lore's content hash for idempotent re-index. |
| A frontmatter **field that names an asset** (e.g. a character's `eye_material: M_Eyes_Aria`, or `portrait: portraits/aria.png`) | The `assets.entity_id` link + the entity's `primary_asset` / `assets` JSON list | The connector resolves the field value to the asset file in the repo and writes the link, so "Aria's eye-color material" is reachable from the Aria entity in the DAM and vice-versa. |
| A lore **cross-reference** in markdown (`[[Location:tavern]]`) | An `entity_relationships` row | `extractCrossRefs(markdown)` → `{ entityType, entityId }` → relationship rows, so the DAM shows the entity graph (this character appears in these locations / scenes / items). |
| A lore **branch / revision** | (v1) the indexed snapshot is the working copy's current revision. Multi-revision DAM history is a later phase. | The connector indexes the checked-out revision; re-index on lore commit keeps the cache fresh. |

**Reconciling the two source-of-truth models.** Lore is the source of truth for
the *bytes and the markdown*. StudioBrain's `studiobrain_content` is, and stays, a
*cache/index* — but the upstream it indexes is **lore**, not Garage, for these
rows. The connector never writes lore data into Garage as a master; it only
projects metadata + a derived preview into the YB/Qdrant index. If the index
diverges, "rebuild from lore" is always safe — the identical invariant StudioBrain
already documents for "rebuild from markdown."

## 4. The entitlement flow

The unlock rides the *exact* path SBAI-4068 already built and SBAI-4077 reuses —
no new entitlement machinery:

1. A studio on a **StudioBrain Enterprise plan** (the tier that includes `dam`,
   per `TIER_FEATURES`) signs in. Two unlock sources, in priority order, already
   exist in `entitlement.ts`:
   - **Offline signed license key** (shipping now) whose `features` array
     includes `"dam"`; or
   - **StudioBrain accounts JWT** (planned 2nd source) whose `tier` claim resolves
     through `featuresForTier(tier)` to a feature set including `"dam"`, injected
     into `window.__LOREGUI_ENTITLEMENTS__` before React mounts.
2. `isEntitled("dam")` returns true. The DAM premium panel (registered by the
   `loregui-cloud` overlay through `registerPremiumPanel`) lights up its top-bar
   nav entry; when locked it shows `Digital Asset Manager 🔒` and an upsell.
3. In the entitled panel, the user picks the selected asset and clicks **"Open in
   StudioBrain DAM"** — a deep-link into `app.studiobrain.ai`'s DAM for that
   asset/entity (and, in the full build, triggers/refreshes the federated index
   entry). Enhanced tagging / semantic search / cross-ref views render from the
   StudioBrain content index.

The `dam` feature id is the **only** change the open core needs (added to the
`Feature` union + `TIER_FEATURES` in `entitlement.ts`) — exactly the one-line core
change the COMMERCIAL-ADDONS playbook prescribes for any new add-on. Everything
else lives in the proprietary overlay.

## 5. Security / boundary considerations (PII isolation)

The StudioBrain **accounts security boundary** is the governing constraint:

- **No PII into LoreGUI, ever.** LoreGUI never parses, stores, or mints the
  accounts JWT; it consumes only a *resolved, already-trusted feature list*
  (`window.__LOREGUI_ENTITLEMENTS__`). Email, billing, team membership, tenant
  policy, OAuth tokens — none of it enters loregui or loregui-cloud. This is
  already how reporting/relay work; DAM follows suit.
- **No accounts UI bundled.** The DAM panel never embeds auth/billing/team UI. The
  "Open in StudioBrain DAM" action is a plain deep-link/browser-out to
  `app.studiobrain.ai`; the StudioBrain DAM (and any auth it needs) renders on the
  StudioBrain origin, not inside the LoreGUI bundle.
- **No direct DB / Garage access from the connector.** The full-build connector
  reaches StudioBrain *only* through documented service entry points
  (`/api/internal/indexer/reindex`, `/api/assets/upload`, `/api/search/*`) with
  `X-Service-Secret` / tenant-scoped Bearer auth — never `INSERT INTO entities`,
  never a raw Garage write. This respects StudioBrain's "no direct YB writes
  outside the indexer" and "no direct DB access to auth tables" rules.
- **Tenant scoping is StudioBrain's job.** The connector passes the `tenant_id`
  the JWT/license already carries; it does not invent or cross tenants. LoreGUI
  holds no tenant table.
- **The asset bytes stay lore-owned.** Only metadata + a derived preview are
  projected. No re-mastering of studio IP into a second store.

## 6. Phased build plan + ticket stubs

**Phase 0 — this spike (DONE).** Design (this doc) + a minimal gated seam in the
overlay: a `dam/` premium module registered via `registerPremiumPanel`, gated
`isEntitled("dam")`, rendering a placeholder "Open in StudioBrain DAM" panel wired
to the selected asset path. Proves the seam end-to-end; core ships dark.
→ *SBAI-4077 (this), refs SBAI-4068.*

**Phase 1 — selected-asset core seam (small, open core).** Today the DAM panel
reads the selected path from a defensive `window.__LOREGUI_SELECTED_PATH__`
convention. Promote that to a proper read-only seam: a tiny
`commercial/selected-asset-registry.ts` (mirror of relay-registry) that `App.tsx`
writes the current `workspaceFile.path` + `kind` into, and that any premium panel
can read. Keeps the open/commercial seam clean and typed.
→ *stub SBAI-XXXX "LoreGUI: selected-asset seam for premium panels".*

**Phase 2 — the federated connector (proprietary, loregui-cloud + a StudioBrain
follow-up).** A connector that, for the selected entity/asset, projects lore
metadata into `studiobrain_content` via `/api/internal/indexer/reindex` and
uploads a derived thumbnail via `/api/assets/upload`. Reuses SDK
`generateEntityId` / `detectAssetCategory` / `extractCrossRefs` for parity.
→ *stub SBAI-XXXX "LoreGUI×SB: federated lore→content connector (metadata
projection)".*
→ *stub SBAI-XXXX (StudioBrain repo) "Indexer: accept federated lore-sourced
rows + lore repo path as stable key".*

**Phase 3 — deep-link target in StudioBrain (StudioBrain follow-up).** Add a
stable `/assets/{id}` (or `/dam/{entity_type}/{entity_id}`) route so "Open in
StudioBrain DAM" lands on a real detail page rather than the ephemeral
`/assets`-page modal.
→ *stub SBAI-XXXX (StudioBrain repo) "DAM: stable deep-link route for an asset /
entity detail".*

**Phase 4 — enhanced in-LoreGUI surfaces (proprietary).** Render entity-aware
tags, semantic-search results (`/api/search/semantic`), and the cross-ref graph
inline in the DAM panel for the selected asset — not just a deep-link out.
→ *stub SBAI-XXXX "LoreGUI×SB: in-panel tags / semantic search / cross-refs".*

**Phase 5 — reverse + revision awareness (proprietary, later).** Re-index on lore
commit (hook the lore-vm commit op), and project lore branch/revision context so
the DAM can show asset history per revision.
→ *stub SBAI-XXXX "LoreGUI×SB: re-index on commit + revision-aware DAM".*

## 7. The minimal gated seam shipped in this spike

In `loregui-cloud` (proprietary), mirroring the reporting/relay overlays exactly:

- `frontend-overlay/dam/index.ts` — `registerPremiumPanel({ id: "dam", … })`.
- `frontend-overlay/dam/DamPanel.tsx` — a `PremiumPanelProps` modal: locked-upsell
  when `!isEntitled("dam")`; when entitled, a placeholder that shows the selected
  asset path and a deep-link **"Open in StudioBrain DAM"** action (stub URL to
  `app.studiobrain.ai/assets?...`). Reads the selected path defensively from
  `window.__LOREGUI_SELECTED_PATH__` (Phase 1 promotes this to a typed seam).
- `frontend-overlay/overlay-entry.ts` — adds `import "../_overlay/dam/index";`.

Open-core change (the only one): `dam` added to the `Feature` union +
`TIER_FEATURES` (Enterprise) in `commercial/entitlement.ts`, and the tier table in
this doc set / COMMERCIAL-ADDONS.md. With no entitlement, the open core ships **no
DAM UI** — gate dark.
