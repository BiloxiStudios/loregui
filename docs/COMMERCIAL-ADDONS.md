# Commercial Add-ons & Entitlement Gating

LoreGUI is **open core** (MIT). The full VCS surface — every domain, panel, and
command-palette op — is and stays free and functional. On top of that core,
Biloxi Studios ships a small set of **premium add-ons** that are present in the
same binary but *gated*: dark (hidden or locked behind an upsell) unless the
running studio is entitled.

- **Epic:** SBAI-4068 (commercial gating)
- **First add-on:** SBAI-4061 — Reporting & Insights

## Principles

1. **Open core never breaks.** No core surface ever calls the entitlement gate.
   Removing all entitlements yields a complete, working LoreGUI.
2. **One gate, checked at the UI seam.** A single module —
   `frontend/src/commercial/entitlement.ts` — answers `isEntitled(feature)`.
   Premium surfaces call it while rendering; a locked feature shows an upsell
   affordance, never a broken control.
3. **Same binary, runtime unlock.** We don't ship a separate "pro" build. The
   feature set is resolved at runtime, so the same artifact serves Free, Team,
   and Enterprise studios. This keeps CI, release, and supply chain singular.
4. **Backend stays open.** The Tauri command + lore-vm op behind a premium panel
   are *not* separately gated when they only expose data already reachable from
   open-core ops. (The Reporting op returns the same read-only revision history
   that `revision_history` / `revision_info` already expose.) Gating lives in the
   UI; the command-palette parity ratchet therefore still passes. If a future
   add-on exposes genuinely new privileged capability, gate it server-side too.

## The entitlement module

`frontend/src/commercial/entitlement.ts` exposes:

```ts
isEntitled(feature: "reporting" | "lanDiscovery"): boolean   // the one call sites use
featuresForTier(tier): Feature[]            // Free/Team/Enterprise → features
setDevEntitlements(features | null): void   // dev/QA localStorage override
isDevDefaultEntitlement(): boolean          // are we in dev "all on" mode?
```

`isEntitled` resolves the active feature set in this priority order:

| # | Source | Use |
|---|--------|-----|
| 1 | `window.__LOREGUI_ENTITLEMENTS__` (string[]) | Runtime injection by the host shell / accounts bootstrap. **Highest precedence.** |
| 2 | `localStorage["loregui.entitlements"]` (JSON array or CSV) | Dev / QA override; how an in-app toggle persists. |
| 3 | `VITE_LOREGUI_ENTITLEMENTS` / `LOREGUI_ENTITLEMENTS` (CSV, build time) | Ship a commercial build pre-entitled. |
| 4 | **default** | Dev build → **all features ON** (contributors see the full UI). Production build with nothing set → **all features OFF** (locked). |

### Tier → feature mapping

```
free        → []
team        → ["reporting", "lanDiscovery"]
enterprise  → ["reporting", "lanDiscovery"]
```

`featuresForTier()` encodes this. Adjust packaging there; call sites are
unaffected.

## How a studio unlocks an add-on

**Today (dev / self-serve):** set the entitlement via any source above, e.g.

- A commercial build: `LOREGUI_ENTITLEMENTS=reporting` at build time.
- Local trial / QA: in the browser console
  `localStorage.setItem("loregui.entitlements", '["reporting"]')` then reload,
  or call `setDevEntitlements(["reporting"])`.
- Dev builds already default to everything-on.

**Future (StudioBrain accounts tiers):** the durable source is the StudioBrain
accounts JWT (RS256, issued by `accounts.studiobrain.ai`). When the auth bridge
lands, a bootstrap step will:

1. Read the signed JWT's `tier` claim (`free` / `team` / `enterprise`).
2. Resolve it through `featuresForTier(tier)`.
3. Inject the result into `window.__LOREGUI_ENTITLEMENTS__` **before** React
   mounts (priority source #1).

No call site in the app changes — `isEntitled("reporting")` just starts
returning the tier's answer. Per the StudioBrain **accounts security boundary**,
LoreGUI never parses, stores, or mints the JWT here; token handling stays in the
auth/accounts layer. This module only consumes a resolved, already-trusted
feature list.

## Adding a new premium feature

1. Add the id to the `Feature` union and to `TIER_FEATURES` in
   `entitlement.ts`.
2. Guard the new surface with `isEntitled("<id>")`; render a locked/upsell state
   otherwise (see `ReportingPanel.tsx` for the pattern — the panel returns an
   upsell view when not entitled and re-checks defensively even though the nav
   already gates it).
3. Keep the underlying op/command open unless it grants new privileged
   capability; if it does, add a server-side check too.
4. Document the upsell copy and the tier it belongs to here.

## Shipped add-ons

### Reporting & Insights (SBAI-4061)

"Who did what when." Built on the `revision_activity_report` op (PR #279).

- **Surface:** `ReportingPanel.tsx`, top-bar **Reporting** nav entry (shows
  `Reporting 🔒` and opens an upsell when not entitled).
- **Capabilities:** per-contributor activity rollup (commits / files changed /
  revisions, filterable by author + date window + branch + file), a who-did-what
  history timeline colored by contributor, and **multi-grain restore**.
- **Restore grains:**
  - *File* — restore one file to its content at a revision. **Wired** to
    `file_write`.
  - *Revision* — undo a whole revision. **Wired** to `revision_revert_local`
    (conflicts resolved via the History panel's revert flow).
  - *Individual change* — restore a single hunk within a revision. **Stubbed /
    "soon"** — no lore op exposes hunk-level restore yet.
- **Feature id:** `reporting` (Team and Enterprise tiers).

### LAN server auto-discovery (SBAI-4073)

Auto-discovers LoreGUI-hosted lore servers on the same local network in the
onboarding "Connect to server" step.

- **Surface:** onboarding `ClientConnect.tsx` LAN discovered list (with manual URL
  entry always available as fallback).
- **Capabilities:** browse local-network server advertisements, pick one, then
  proceed through normal auth/clone.
- **Feature id:** `lanDiscovery` (Team and Enterprise tiers).
