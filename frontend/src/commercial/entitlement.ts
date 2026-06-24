/**
 * Commercial entitlement gate (SBAI-4068).
 *
 * LoreGUI is open core (MIT). A small set of premium surfaces — the first being
 * the Reporting & Insights add-on (SBAI-4061) — are *gated*: present in the same
 * binary, but dark (hidden / locked behind an upsell) unless the running studio
 * is entitled. The open core stays fully functional with NO entitlements.
 *
 * This module is the single source of truth for "is feature X unlocked?". It is
 * deliberately tiny and dependency-free so every surface can call `isEntitled()`
 * synchronously while rendering.
 *
 * ## Where entitlements come from (resolved in priority order)
 *
 * 1. **Signed license key** — an offline Ed25519-signed license token (see
 *    `license.ts`). This is the AUTHORITATIVE production unlock: a real, portable
 *    license that only Biloxi can mint (we hold the private signing key) and that
 *    carries an expiry. Resolved once at {@link bootstrapEntitlements} and cached
 *    into the runtime slot below. Highest precedence.
 * 2. **Runtime injection** — `window.__LOREGUI_ENTITLEMENTS__`, a string[] of
 *    feature ids. The host shell (or, later, a StudioBrain accounts session
 *    bootstrap) can write this before React mounts. This is ALSO where the
 *    license bootstrap writes the verified license features, so call sites stay
 *    synchronous.
 * 3. **Local override** — `localStorage["loregui.entitlements"]`, a JSON array
 *    or comma-separated list. Lets a developer or QA toggle features without a
 *    rebuild. Also how the in-app dev affordance persists a choice.
 * 4. **Build-time env** — `import.meta.env.VITE_LOREGUI_ENTITLEMENTS` (a.k.a.
 *    `LOREGUI_ENTITLEMENTS` exported at build), comma-separated. Lets a
 *    commercial build ship pre-entitled.
 * 5. **Dev default** — in a dev build (`import.meta.env.DEV`) with none of the
 *    above set, ALL features default to ON so contributors see the full UI. In a
 *    production build with nothing configured, features default to OFF (locked).
 *
 * In production, the effective resolution collapses to the contract from
 * SBAI-4068: **valid signed license → (dev override, dev builds only) → empty**.
 * The license is signature-verified, not anti-tamper DRM — the app and verify key
 * are public open core; only the private signing key (in Vaultwarden / Azure KV)
 * is secret. See `license.ts` and `docs/COMMERCIAL-ADDONS.md`.
 *
 * ## Canonical entitlement model (SBAI-4089 / E0.7, ADR-0001 §2.5)
 *
 * The StudioBrain accounts JWT (RS256, issued by accounts.studiobrain.ai) is the
 * PRIMARY entitlement source. accounts emits the *canonical* ecosystem-wide model
 * verbatim as TOP-LEVEL claims (`tier`, `tier_id`, `features`) — adopted
 * identically here and in model-manager. There are TWO axes, and they are gated
 * DIFFERENTLY (the hybrid contract — see {@link MONOTONIC_FEATURE_MIN_TIER}):
 *
 *   - `tier`: an **integer ordinal**, spaced for future insertion, paired with a
 *     stable string id (`tier_id`). LOCKED scheme:
 *       0 free · 10 indie · 20 team · 30 enterprise · 40–89 reserved ·
 *       90 staff · 99 superadmin
 *     A higher tier is a strict superset. **Monotonic** features (the ones a
 *     higher tier strictly includes — `reporting`/`relay`/`dam`) are gated by
 *     `tierOrdinal(tier) >= MIN`. accounts deliberately does NOT mint monotonic
 *     features into `features[]`; it tier-gates them. accounts also does not
 *     centralize their min-tiers, so LoreGUI owns that map locally (kept in sync
 *     with accounts' `config/entitlement.py`).
 *   - `features[]`: the minted set of **non-monotonic add-on** feature ids for
 *     this session (today: just `byok`, gated at tier>=enterprise). These are
 *     unlocks that do NOT follow the tier ladder, so accounts cannot express them
 *     as a simple `tier >= MIN` and instead mints the resolved id onto the array.
 *     LoreGUI gates these PURELY off `features.includes(...)`. accounts'
 *     `features_for_tier()` returns ONLY these add-ons (e.g. `["byok"]` at
 *     tier>=enterprise) — it never returns `reporting`/`relay`/`dam`.
 *   - `role` (owner/admin/member): a SEPARATE within-tenant axis — never folded
 *     into `tier`. This module does not read `role`.
 *
 * The tier ordinals are a cross-repo contract: they mirror accounts'
 * `config/entitlement.py` and model-manager's per-request gating. Don't renumber
 * without changing all three.
 *
 * ## Hybrid gating (SBAI-4170 — correcting the over-broad SBAI-4170 first pass)
 *
 * An earlier pass DELETED the local feature→tier map and tried to gate
 * `reporting`/`relay`/`dam` off `features.includes(...)`. That was WRONG: accounts
 * never mints those into `features[]` (its `features_for_tier()` returns only the
 * non-monotonic add-ons, e.g. `["byok"]`), so they became un-unlockable. The
 * correct, canonical model is HYBRID and is restored here:
 *
 *   - reporting → `tierOrdinal(tier) >= TIER.team`        (monotonic, tier-gated)
 *   - relay     → `tierOrdinal(tier) >= TIER.enterprise`  (monotonic, tier-gated)
 *   - dam       → `tierOrdinal(tier) >= TIER.enterprise`  (monotonic, tier-gated)
 *   - byok      → `features.includes("byok")`             (non-monotonic, from features[])
 *
 * {@link bootstrapAccountsEntitlements} reads BOTH top-level claims (`tier` AND
 * `features`) and unions both axes into `window.__LOREGUI_ENTITLEMENTS__` (path 2
 * above) — so NO call site here changes. Resolution is a UNION across sources so
 * "hooked into StudioBrain" only ever *adds* unlocks. This module never parses or
 * stores the JWT itself; the host/auth layer extracts the claim and hands it in,
 * per the StudioBrain accounts security boundary.
 */

import { resolveLicensedFeatures } from "./license.ts";

/**
 * A gateable premium feature id. Two kinds, gated differently (the hybrid
 * contract): **monotonic** features (`reporting`/`relay`/`dam`) are tier-gated via
 * {@link MONOTONIC_FEATURE_MIN_TIER}; the **non-monotonic** add-on `byok` is gated
 * off the minted `features[]` claim. See the module docs.
 */
export type Feature = "reporting" | "relay" | "dam" | "byok";

/**
 * Canonical tier ordinals — the LOCKED scheme (ADR-0001 §2.5). Integer, spaced
 * for future insertion, paired with a stable string id (see {@link TIER_ID}).
 * Mirrors accounts' `config/entitlement.py`.
 */
export const TIER = {
  free: 0,
  indie: 10,
  team: 20,
  enterprise: 30,
  // 40–89 reserved for future paid tiers — leave the gaps.
  staff: 90,
  superadmin: 99,
} as const;

/** Stable string id for each canonical tier ordinal (logs / display). */
export const TIER_ID: Readonly<Record<number, string>> = {
  [TIER.free]: "free",
  [TIER.indie]: "indie",
  [TIER.team]: "team",
  [TIER.enterprise]: "enterprise",
  [TIER.staff]: "staff",
  [TIER.superadmin]: "superadmin",
};

/**
 * Minimum canonical tier ordinal that unlocks each **monotonic** feature.
 *
 * These min-tiers live HERE (not in accounts' minted `features[]`) because the
 * canonical accounts model intentionally tier-gates monotonic features rather
 * than minting them into `features[]`: accounts' `features_for_tier()` returns
 * ONLY the non-monotonic add-ons (today `["byok"]` at tier>=enterprise) and never
 * `reporting`/`relay`/`dam`. accounts does not centralize the monotonic
 * min-tiers, so each product owns the map. Keep it in sync with accounts'
 * `config/entitlement.py` — these mirror its monotonic thresholds.
 */
export const MONOTONIC_FEATURE_MIN_TIER: Readonly<Record<string, number>> = {
  reporting: TIER.team,
  relay: TIER.enterprise,
  dam: TIER.enterprise,
};

/**
 * The id of the non-monotonic add-on `byok` (bring-your-own-key). Non-monotonic
 * features do NOT follow the tier ladder, so accounts mints the resolved id onto
 * the JWT's `features[]` (at tier>=enterprise today) and LoreGUI gates off
 * `features.includes(FEATURE_BYOK)` — never off a tier threshold.
 */
export const FEATURE_BYOK = "byok";

/**
 * The set of NON-MONOTONIC add-on feature ids — gated off the minted `features[]`
 * claim, not off the tier ordinal. Today this is just `byok`. accounts'
 * `features_for_tier()` is the authority for what lands in `features[]`.
 *
 * Resolution doesn't need to consult this set (the resolved entitlement set
 * already encodes both axes — see {@link bootstrapAccountsEntitlements}), but it
 * documents the contract and is the canonical list of ids gated off `features[]`.
 */
export const NON_MONOTONIC_FEATURES: ReadonlySet<string> = new Set<string>([FEATURE_BYOK]);

/**
 * Legacy plan string → canonical tier ordinal (back-compat for old claims).
 *
 * TODO(SBAI-4168): REMOVE once the legacy `plan` string is deprecated
 * ecosystem-wide. Phase 1 migrates all readers to `tier`/`tier_id`/`features[]`
 * while accounts still emits `plan`; Phase 2 stops emitting `plan`. Until that
 * lands, `tierOrdinal` still accepts a legacy string tier via this map.
 */
const PLAN_TO_TIER: Record<string, number> = {
  free: TIER.free,
  indie: TIER.indie,
  team: TIER.team,
  enterprise: TIER.enterprise,
  staff: TIER.staff,
  superadmin: TIER.superadmin,
};

/**
 * Normalise a `tier` claim to a canonical integer ordinal. Accepts the canonical
 * integer directly, a numeric string, or a legacy plan/tier string
 * (`free`/`indie`/`team`/`enterprise`/…). Unknown/missing → `free` (0), i.e.
 * least privilege.
 *
 * Used for monotonic feature gating (`tier >= MIN`, see
 * {@link MONOTONIC_FEATURE_MIN_TIER}) and to normalise a legacy string tier. It
 * is NOT used to derive non-monotonic unlocks (those come minted in `features[]`).
 */
export function tierOrdinal(tier: number | string | null | undefined): number {
  if (typeof tier === "number" && Number.isFinite(tier)) return tier;
  if (typeof tier === "string") {
    const trimmed = tier.trim();
    if (/^-?\d+$/.test(trimmed)) return parseInt(trimmed, 10);
    return PLAN_TO_TIER[trimmed.toLowerCase()] ?? TIER.free;
  }
  return TIER.free;
}

const LOCAL_STORAGE_KEY = "loregui.entitlements";

declare global {
  interface Window {
    /** Runtime-injected entitlement feature ids (highest precedence). */
    __LOREGUI_ENTITLEMENTS__?: string[];
  }
}

/** Parse a comma/whitespace/JSON-array list of feature ids into a clean array. */
function parseList(raw: string | null | undefined): string[] {
  if (!raw) return [];
  const trimmed = raw.trim();
  if (!trimmed) return [];
  if (trimmed.startsWith("[")) {
    try {
      const parsed = JSON.parse(trimmed);
      if (Array.isArray(parsed)) return parsed.map(String);
    } catch {
      /* fall through to delimiter parsing */
    }
  }
  return trimmed
    .split(/[,\s]+/)
    .map((s) => s.trim())
    .filter(Boolean);
}

function isDev(): boolean {
  try {
    return Boolean(import.meta.env?.DEV);
  } catch {
    return false;
  }
}

function envEntitlements(): string[] {
  try {
    return parseList(import.meta.env?.VITE_LOREGUI_ENTITLEMENTS as string);
  } catch {
    return [];
  }
}

function localOverride(): string[] | null {
  try {
    const raw = window.localStorage.getItem(LOCAL_STORAGE_KEY);
    return raw == null ? null : parseList(raw);
  } catch {
    return null;
  }
}

/** Sentinel meaning "every feature" (dev default), kept internal. */
const ALL = "*";

/**
 * Features unlocked by a verified signed license, resolved once by
 * {@link bootstrapEntitlements}. `null` = bootstrap hasn't run / no valid
 * license. This is the authoritative production source and takes precedence over
 * everything except an explicit runtime injection by the host shell.
 */
let licensedFeatures: string[] | null = null;

/**
 * Resolve, verify, and cache the offline signed license (SBAI-4068). Call this
 * ONCE before React mounts. After it resolves, `isEntitled()` reflects the
 * license synchronously (the verified features are also mirrored into the
 * runtime injection slot so any late readers agree).
 *
 * `loadFile` is an optional reader for an on-disk `license.key` (e.g. a thin
 * wrapper over a `read_license_file` Tauri command). When omitted, only the env
 * and localStorage token sources are consulted.
 *
 * Safe to call even with no license present: it simply leaves entitlements at
 * their non-license defaults, so the open core stays fully functional.
 */
export async function bootstrapEntitlements(
  loadFile?: () => Promise<string | null>,
): Promise<string[] | null> {
  try {
    licensedFeatures = await resolveLicensedFeatures(loadFile);
  } catch {
    licensedFeatures = null;
  }
  if (licensedFeatures && typeof window !== "undefined") {
    // Mirror into the runtime slot so any synchronous reader (and the future
    // accounts-JWT bootstrap, which also writes here) sees a single source.
    window.__LOREGUI_ENTITLEMENTS__ = [...licensedFeatures];
  }
  return licensedFeatures;
}

/** @internal — for tests only. Reset the cached license resolution. */
export function __resetLicensedFeaturesForTests(): void {
  licensedFeatures = null;
}

/**
 * The canonical entitlement claim carried on the StudioBrain accounts user JWT
 * (SBAI-4089), as TOP-LEVEL claims. The host/auth layer extracts these from the
 * verified JWT and hands them in — this module never parses or stores the token
 * itself, per the accounts security boundary. `role` is intentionally absent: it
 * is a separate within-tenant axis, not a subscription capability.
 */
export interface AccountsEntitlementClaim {
  /**
   * Canonical integer tier ordinal (or a legacy string tier). Gates the
   * **monotonic** features (`reporting`/`relay`/`dam`) via
   * {@link MONOTONIC_FEATURE_MIN_TIER}.
   */
  tier?: number | string | null;
  /** Stable string id for the tier (informational; logs/display). */
  tier_id?: string | null;
  /**
   * The minted set of **non-monotonic** add-on feature ids (today: `byok`).
   * accounts' `features_for_tier()` (`config/entitlement.py`, SBAI-4089) returns
   * ONLY these add-ons — never `reporting`/`relay`/`dam`, which are tier-gated.
   */
  features?: readonly string[] | null;
}

/**
 * Resolve the canonical accounts JWT entitlement claim (SBAI-4089 / E2.3) into
 * concrete LoreGUI feature ids and UNION them into the runtime injection slot
 * (`window.__LOREGUI_ENTITLEMENTS__`). Call this when the StudioBrain auth bridge
 * provides a verified claim; it is additive, so "hooked into StudioBrain" only
 * ever *adds* unlocks on top of any offline license already bootstrapped.
 *
 * Resolution is HYBRID across BOTH top-level claims (SBAI-4170 correction):
 *   - For each **monotonic** feature in {@link MONOTONIC_FEATURE_MIN_TIER}, unlock
 *     it iff `tierOrdinal(claim.tier) >= its min`. accounts tier-gates these and
 *     does NOT mint them into `features[]`.
 *   - PLUS every id in `claim.features` (the **non-monotonic** add-ons, e.g.
 *     `byok`), gated verbatim off the minted array.
 *
 * Safe with a null/garbage claim: it simply contributes nothing.
 */
export function bootstrapAccountsEntitlements(
  claim: AccountsEntitlementClaim | null | undefined,
): string[] {
  const resolved = new Set<string>();
  if (claim) {
    // Axis 1 — monotonic features tier-gated locally (accounts does not mint
    // these into features[]; their min-tiers mirror config/entitlement.py).
    const ordinal = tierOrdinal(claim.tier);
    for (const [feature, min] of Object.entries(MONOTONIC_FEATURE_MIN_TIER)) {
      if (ordinal >= min) resolved.add(feature);
    }
    // Axis 2 — non-monotonic add-ons minted onto features[] (e.g. byok).
    if (Array.isArray(claim.features)) {
      for (const f of claim.features) if (typeof f === "string" && f) resolved.add(f);
    }
  }
  if (typeof window !== "undefined") {
    const existing = Array.isArray(window.__LOREGUI_ENTITLEMENTS__)
      ? window.__LOREGUI_ENTITLEMENTS__
      : [];
    // Union with whatever is already injected (e.g. a verified offline license).
    window.__LOREGUI_ENTITLEMENTS__ = [...new Set([...existing.map(String), ...resolved])];
    return [...window.__LOREGUI_ENTITLEMENTS__];
  }
  return [...resolved];
}

/**
 * The resolved set of entitled feature ids for this session. Returns `["*"]`
 * when everything is unlocked (dev default). Order matters: see module docs.
 */
function resolveEntitlements(): string[] {
  const injected =
    typeof window !== "undefined" && Array.isArray(window.__LOREGUI_ENTITLEMENTS__)
      ? window.__LOREGUI_ENTITLEMENTS__.map(String)
      : null;

  // 1. verified signed license (authoritative production unlock) ∪ runtime
  //    injection. Per ADR-0001 §2.5 the sources resolve as a UNION so that being
  //    "hooked into StudioBrain" (accounts JWT → injection slot) only ever ADDS
  //    unlocks on top of the offline license, never removes them.
  if (licensedFeatures != null) {
    return [...new Set([...licensedFeatures, ...(injected ?? [])])];
  }

  // 2. runtime injection (host shell / accounts JWT bootstrap)
  if (injected != null) return injected;

  // 3. local override (dev / QA / in-app toggle)
  const override = typeof window !== "undefined" ? localOverride() : null;
  if (override != null) return override;

  // 4. build-time env
  const env = envEntitlements();
  if (env.length) return env;

  // 5. defaults: dev = all on, prod = none.
  return isDev() ? [ALL] : [];
}

/**
 * Is the given premium feature unlocked for this session?
 *
 * The resolved entitlement set already encodes the hybrid contract: a monotonic
 * feature lands in the set only when `tierOrdinal(tier) >= its min` (resolved in
 * {@link bootstrapAccountsEntitlements}); a non-monotonic add-on (`byok`) lands in
 * the set only when minted into `features[]`. So membership-in-set is the right
 * check for BOTH kinds.
 *
 * The open core never calls this for its own surfaces — only premium add-ons do.
 * A locked feature must render an upsell affordance, never a broken control.
 */
export function isEntitled(feature: Feature): boolean {
  const set = resolveEntitlements();
  return set.includes(ALL) || set.includes(feature);
}

/** True when entitlements are coming from the dev "all on" default. */
export function isDevDefaultEntitlement(): boolean {
  return resolveEntitlements().includes(ALL);
}

/**
 * Persist a dev/QA override of the entitlement set to localStorage, then reload
 * so every surface re-resolves. `null` clears the override (back to defaults).
 * Only intended for the in-app dev affordance — production unlock comes from the
 * accounts JWT, not this.
 */
export function setDevEntitlements(features: Feature[] | null): void {
  try {
    if (features == null) {
      window.localStorage.removeItem(LOCAL_STORAGE_KEY);
    } else {
      window.localStorage.setItem(LOCAL_STORAGE_KEY, JSON.stringify(features));
    }
  } catch {
    /* ignore storage failures (private mode, etc.) */
  }
}
