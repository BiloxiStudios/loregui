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
 * identically here and in model-manager. There are two axes:
 *
 *   - `tier`: an **integer ordinal**, spaced for future insertion, paired with a
 *     stable string id (`tier_id`). LOCKED scheme:
 *       0 free · 10 indie · 20 team · 30 enterprise · 40–89 reserved ·
 *       90 staff · 99 superadmin
 *     The tier is read for DISPLAY only (the `tier_id` label) — **never for
 *     gating**. LoreGUI does NOT re-derive a feature→tier mapping. See the gating
 *     rule below.
 *   - `features[]`: the FULL, fully-resolved set of capability feature ids for
 *     this session. As of SBAI-4167, accounts' `config/entitlement.py` is the
 *     single canonical feature→tier registry (`FEATURE_MIN_TIER`, 35 entries) and
 *     `features_for_tier(tier)` returns EVERY feature whose `min_tier <= tier` —
 *     so the resolved superset (e.g. `reporting`, `lore_relay`, `dam`, `byok` at
 *     enterprise) is minted onto `features[]` directly. LoreGUI gates EVERY
 *     gateable feature PURELY off `features.includes("<id>")`.
 *   - `role` (owner/admin/member): a SEPARATE within-tenant axis — never folded
 *     into `tier`. This module does not read `role`.
 *
 * ## The ONE gating rule (CLAUDE.md "Entitlement vs Plan", SBAI-4165)
 *
 * **Gate on `features[]` ONLY. Never re-derive the feature→tier mapping in any
 * service or frontend.** Read the minted `features[]` from the JWT. The canonical
 * registry is accounts' `config/entitlement.py` (SBAI-4167), which already
 * resolves tier → the full feature set. There is intentionally NO local min-tier
 * map of any kind in this module.
 *
 * A note on history: an interim pass added a local `MONOTONIC_FEATURE_MIN_TIER`
 * hybrid (gate `reporting`/`relay`/`dam` off a local tier map, only `byok` off
 * `features[]`). That was based on a STALE accounts checkout in which
 * `features_for_tier()` returned only the enterprise add-ons. The LIVE accounts
 * (post-SBAI-4167) mints the FULL resolved feature set into `features[]`, so the
 * hybrid is wrong and has been removed: gate `features[]`-only.
 *
 * `plan` (the legacy billing SKU string) is retained as `tier`-normalisation
 * back-compat (see {@link PLAN_TO_TIER}) but is NEVER gated on. SBAI-4168 (which
 * would have removed the legacy `plan` string) was DROPPED — plan stays as the
 * billing SKU; entitlement gating uses `features[]` exclusively.
 *
 * {@link bootstrapAccountsEntitlements} unions `claim.features[]` VERBATIM into
 * `window.__LOREGUI_ENTITLEMENTS__` (path 2 above) — so NO call site here changes.
 * Resolution is a UNION across sources so "hooked into StudioBrain" only ever
 * *adds* unlocks. This module never parses or stores the JWT itself; the
 * host/auth layer extracts the claim and hands it in, per the StudioBrain
 * accounts security boundary.
 */

import { resolveLicensedFeatures } from "./license.ts";

/**
 * A gateable premium feature id. Each value is a canonical feature id minted by
 * accounts into the JWT's `features[]` claim (`config/entitlement.py`,
 * SBAI-4167) and gated here PURELY off `features.includes(id)`. The id
 * `lore_relay` is deliberately distinct from accounts' free-tier
 * `fileserver_relay` to avoid a namespace collision (a user-facing "Relay" LABEL
 * is fine; the entitlement id must be `lore_relay` to match the minted claim).
 */
export type Feature = "reporting" | "lore_relay" | "dam" | "byok";

/**
 * Canonical tier ordinals — the LOCKED scheme (ADR-0001 §2.5). Integer, spaced
 * for future insertion, paired with a stable string id (see {@link TIER_ID}).
 * Mirrors accounts' `config/entitlement.py`.
 *
 * Used for the `tier_id` DISPLAY label only. **Never used for feature gating** —
 * gating is `features[]`-only (CLAUDE.md SBAI-4165).
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
 * The id of the bring-your-own-key add-on. Like every gateable feature it is
 * gated off `features.includes(FEATURE_BYOK)` — accounts mints it into
 * `features[]` (at tier>=enterprise today) via `features_for_tier()`.
 */
export const FEATURE_BYOK = "byok";

/**
 * Legacy plan string → canonical tier ordinal (back-compat for old claims).
 *
 * `plan` is the billing SKU; it is RETAINED for `tier` normalisation only and is
 * NEVER gated on (gating is `features[]`-only — CLAUDE.md SBAI-4165).
 * SBAI-4168, which would have removed the legacy `plan` string ecosystem-wide,
 * was DROPPED — so `tierOrdinal` still accepts a legacy string tier via this map
 * to render the `tier_id` display label.
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
 * Used ONLY to derive the `tier_id` DISPLAY label and to normalise a legacy
 * string tier. It is NOT used for any feature gating — gating reads the minted
 * `features[]` claim exclusively (CLAUDE.md SBAI-4165).
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
   * Canonical integer tier ordinal (or a legacy string tier). Read for DISPLAY
   * only (the `tier_id` label) — NEVER for gating. Gating reads `features[]`.
   */
  tier?: number | string | null;
  /** Stable string id for the tier (informational; logs/display). */
  tier_id?: string | null;
  /**
   * The FULL, fully-resolved set of canonical feature ids for this session.
   * accounts' `features_for_tier()` (`config/entitlement.py`, SBAI-4167) returns
   * EVERY feature whose `min_tier <= tier` (e.g. `reporting`, `lore_relay`,
   * `dam`, `byok` at enterprise). LoreGUI gates off this list VERBATIM and never
   * re-derives capability from `tier`.
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
 * Gating is `features[]`-ONLY (CLAUDE.md SBAI-4165): accounts already resolved
 * tier → the full feature set (`features_for_tier`, SBAI-4167), so this unions
 * `claim.features` VERBATIM. `claim.tier`/`tier_id` are read for DISPLAY only,
 * never to derive a feature. There is NO local feature→tier map.
 *
 * Safe with a null/garbage claim: it simply contributes nothing.
 */
export function bootstrapAccountsEntitlements(
  claim: AccountsEntitlementClaim | null | undefined,
): string[] {
  const resolved = new Set<string>();
  if (claim && Array.isArray(claim.features)) {
    // Union the minted features[] VERBATIM — accounts already resolved tier →
    // the full feature set (features_for_tier, SBAI-4167). NO local min-tier map.
    for (const f of claim.features) if (typeof f === "string" && f) resolved.add(f);
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
 * Gating is `features[]`-only (CLAUDE.md SBAI-4165): the resolved entitlement set
 * is the minted `features[]` (unioned with any offline license), so
 * membership-in-set is the right check. No tier threshold is consulted.
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
