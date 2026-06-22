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
 * ## Future: StudioBrain accounts tiers (Free / Team / Enterprise)
 *
 * The offline signed license key is the FIRST real source (shipping now). The
 * StudioBrain accounts JWT (RS256, issued by accounts.studiobrain.ai) is the
 * planned SECOND source. Its `tier` claim maps to a feature set via
 * {@link TIER_FEATURES}. When the auth bridge lands, a bootstrap step will read
 * the JWT's `tier` claim and inject the resolved feature ids into
 * `window.__LOREGUI_ENTITLEMENTS__` (path 1 above) — so NO call site here
 * changes. `featuresForTier()` already encodes that mapping. This module never
 * parses or stores the JWT itself; that stays in the auth/accounts layer per the
 * StudioBrain accounts security boundary.
 */

import { resolveLicensedFeatures } from "./license";

/** A gateable premium feature id. Keep in sync with TIER_FEATURES below. */
export type Feature = "reporting" | "relay" | "dam";

/** Commercial tiers, as issued in the StudioBrain accounts JWT `tier` claim. */
export type Tier = "free" | "team" | "enterprise";

/**
 * The feature set unlocked by each tier. Reporting & Insights (SBAI-4061) is a
 * Team-and-up add-on; the cross-network Relay (SBAI-4072) — host a lore server
 * reachable across networks with no VPN, via the StudioBrain bore relay — is an
 * Enterprise add-on (it consumes shared relay infrastructure). The enhanced DAM
 * (SBAI-4077) — surface a lore repo's art/media in StudioBrain's entity-aware
 * Digital Asset Manager (semantic search, tagging, cross-refs) — is also an
 * Enterprise add-on (it consumes the shared StudioBrain content index). Adjust
 * here when packaging changes — call sites are unaffected.
 */
export const TIER_FEATURES: Record<Tier, readonly Feature[]> = {
  free: [],
  team: ["reporting"],
  enterprise: ["reporting", "relay", "dam"],
};

/** Resolve a tier name to its feature ids (unknown tier → no features). */
export function featuresForTier(tier: string | null | undefined): Feature[] {
  if (!tier) return [];
  const key = tier.toLowerCase() as Tier;
  return [...(TIER_FEATURES[key] ?? [])];
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
 * The resolved set of entitled feature ids for this session. Returns `["*"]`
 * when everything is unlocked (dev default). Order matters: see module docs.
 */
function resolveEntitlements(): string[] {
  // 1. verified signed license (authoritative production unlock)
  if (licensedFeatures != null) return [...licensedFeatures];

  // 2. runtime injection (host shell / future accounts bootstrap)
  const injected = typeof window !== "undefined" ? window.__LOREGUI_ENTITLEMENTS__ : undefined;
  if (Array.isArray(injected)) return injected.map(String);

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
