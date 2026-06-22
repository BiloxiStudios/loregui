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
isEntitled(feature: "reporting"): boolean   // the one call sites use
featuresForTier(tier): Feature[]            // Free/Team/Enterprise → features
setDevEntitlements(features | null): void   // dev/QA localStorage override
isDevDefaultEntitlement(): boolean          // are we in dev "all on" mode?
```

`isEntitled` resolves the active feature set in this priority order:

| # | Source | Use |
|---|--------|-----|
| 1 | **Signed license key** (Ed25519, offline) | The authoritative production unlock. A real, portable, offline license token only Biloxi can mint. Verified at bootstrap, then mirrored into `window.__LOREGUI_ENTITLEMENTS__`. See [Offline signed license keys](#offline-signed-license-keys-sbai-4068). **Highest precedence.** |
| 2 | `window.__LOREGUI_ENTITLEMENTS__` (string[]) | Runtime injection by the host shell / accounts bootstrap (and where the license bootstrap writes its result). |
| 3 | `localStorage["loregui.entitlements"]` (JSON array or CSV) | Dev / QA override; how an in-app toggle persists. |
| 4 | `VITE_LOREGUI_ENTITLEMENTS` / `LOREGUI_ENTITLEMENTS` (CSV, build time) | Ship a commercial build pre-entitled. |
| 5 | **default** | Dev build → **all features ON** (contributors see the full UI). Production build with nothing set → **all features OFF** (locked). |

In production this collapses to the SBAI-4068 contract: **valid signed license →
(dev override, dev builds only) → empty**. The verifier and the embedded public
key are open-core public — this is *signature-verified entitlement, not
anti-tamper DRM* (a user with the source can patch the gate; honest licensing,
not copy protection). Only the private signing key, which is never in this repo,
is secret.

### Tier → feature mapping

```
free        → []
team        → ["reporting"]
enterprise  → ["reporting"]
```

`featuresForTier()` encodes this. Adjust packaging there; call sites are
unaffected.

## How a studio unlocks an add-on

**Production (today):** issue the studio an **offline signed license key** and
have them drop it into `LOREGUI_LICENSE`, `localStorage["loregui.license"]`, or a
`license.key` file. See [Offline signed license keys](#offline-signed-license-keys-sbai-4068)
below for the full mint/install flow.

**Dev / QA:** set the entitlement directly via any of the lower-priority sources, e.g.

- Local trial / QA: in the browser console
  `localStorage.setItem("loregui.entitlements", '["reporting"]')` then reload,
  or call `setDevEntitlements(["reporting"])`.
- A pre-entitled build: `LOREGUI_ENTITLEMENTS=reporting` at build time.
- Dev builds already default to everything-on.

**Future (StudioBrain accounts tiers):** the *second* durable source (after the
offline license key) is the StudioBrain accounts JWT (RS256, issued by
`accounts.studiobrain.ai`). When the auth bridge lands, a bootstrap step will:

1. Read the signed JWT's `tier` claim (`free` / `team` / `enterprise`).
2. Resolve it through `featuresForTier(tier)`.
3. Inject the result into `window.__LOREGUI_ENTITLEMENTS__` **before** React
   mounts (priority source #2).

No call site in the app changes — `isEntitled("reporting")` just starts
returning the tier's answer. Per the StudioBrain **accounts security boundary**,
LoreGUI never parses, stores, or mints the JWT here; token handling stays in the
auth/accounts layer. This module only consumes a resolved, already-trusted
feature list.

## Offline signed license keys (SBAI-4068)

The authoritative production unlock is an **offline, Ed25519-signed license
token**. It needs no account and no network: a studio that pays gets a portable
token they can drop on any machine.

> **This is signature-verified open-core entitlement, NOT anti-tamper DRM.**
> LoreGUI is MIT and public — the verifier *and* the embedded public key ship in
> the open. A determined user with the source can patch the gate out; that is
> fine and expected for honest open-core licensing. What the signature *does*
> guarantee is that a license cannot be **forged or self-minted** (only the
> holder of the private key can sign one) and that it **expires**. The future
> accounts-JWT tier (above) is the planned *second* entitlement source.

### Token format

A compact, JWT-like string — `payload.signature`, but using **Ed25519 / EdDSA**:

```
base64url(JSON payload) "." base64url(Ed25519 signature)
```

The signature is over the UTF-8 bytes of the base64url **payload segment** (the
part before the dot). The payload is:

```jsonc
{
  "licensee":  "Acme Studios",     // who it's for (informational)
  "tier":      "team",             // display label (informational)
  "features":  ["reporting"],      // entitlement ids granted (authoritative)
  "issuedAt":  1782148402,         // unix epoch seconds
  "expiresAt": 1813684402          // unix epoch seconds; past → rejected
}
```

### Verification (open, in the app)

`frontend/src/commercial/license.ts#verifyLicense(token)` verifies the Ed25519
signature with **WebCrypto** (`crypto.subtle`, native Ed25519) against the
embedded public key, then enforces `expiresAt`. On success it returns the
license's `features`; on any failure (malformed, bad signature, wrong key,
expired) it returns `null` and the app falls back to open core — an invalid or
absent license never breaks the free app, it only withholds premium surfaces.

`bootstrapEntitlements()` (called from `main.tsx` before React mounts) resolves
the token from `LOREGUI_LICENSE` env → `localStorage["loregui.license"]` →
`license.key` file (read via the `read_license_file` Tauri command, which looks
at `$LOREGUI_LICENSE_FILE` or `license.key` in the app config dir), verifies it,
and mirrors the granted features into `window.__LOREGUI_ENTITLEMENTS__` so every
`isEntitled()` call stays synchronous.

### The public key (embedded — safe to be public)

The Ed25519 **public** verify key is embedded as `LICENSE_PUBLIC_KEY_B64URL` in
`frontend/src/commercial/license.ts` (raw 32 bytes, base64url). A verify key can
only *check* signatures, so shipping it in a public MIT repo is safe.

> ⚠️ The value currently embedded is a **throwaway placeholder** (its private
> half was discarded). Before cutting a commercial build, generate the real
> keypair, embed its public half here, and store the private half in Vaultwarden
> (see below).

### The private key (THE SECRET — never in the repo)

The matching Ed25519 **private** signing key is the licensing secret. Anyone
holding it can mint a license that unlocks every premium surface. It MUST live
**only** in Vaultwarden (entry suggestion: *"LoreGUI license signing key
(Ed25519 private)"*) or Azure Key Vault. It is **never** committed, never in
`.env`, never in CI logs. The issuer tool reads it from an env var / file path at
mint time only.

### Tooling

Both scripts live in `frontend/scripts/` and use only Node's built-in `crypto`
(no deps):

1. **Generate a keypair** (one-time, when setting up commercial signing):

   ```bash
   node frontend/scripts/gen-license-keypair.mjs
   ```

   Prints the PUBLIC key (embed it as `LICENSE_PUBLIC_KEY_B64URL`) and the
   PRIVATE key (store it in Vaultwarden — do **not** commit it).

2. **Mint a license** for a studio (reads the private key from the environment,
   never from the repo):

   ```bash
   LOREGUI_LICENSE_PRIVATE_KEY=<private-key-from-vaultwarden> \
     node frontend/scripts/issue-license.mjs \
       --licensee "Acme Studios" \
       --tier team \
       --features reporting \
       --days 365
   ```

   The token is printed to **stdout** (the human-readable summary goes to
   stderr, so you can capture the token cleanly). Or supply
   `LOREGUI_LICENSE_PRIVATE_KEY_FILE=/path/to/key` instead of the env var, and
   `--expires <ISO-8601>` instead of `--days`.

3. **Deliver** the token to the studio. They install it via any of:
   - `LOREGUI_LICENSE=<token>` in the environment, or
   - `localStorage.setItem("loregui.license", "<token>")` (then reload), or
   - a `license.key` file in the app config dir (or at `$LOREGUI_LICENSE_FILE`).

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
