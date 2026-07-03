/**
 * StudioBrain → LoreGUI theme bridge (SBAI-4605).
 *
 * LoreGUI's surface model was ported from StudioBrain, so the two apps share
 * the same 12 surfaces × 7 slots and the same `--surface-{name}-{prop}` CSS
 * custom properties. This module lets LoreGUI *import* StudioBrain's resolved
 * theme — including user-customized colors — so it renders visually identical
 * whether launched standalone or alongside/inside StudioBrain.
 *
 * Accepted transports (in priority order):
 *  1. URL fragment  `#sbtheme=<base64url(JSON)>` — set by StudioBrain when it
 *     launches or embeds LoreGUI (same pattern as the accounts-iframe JWT
 *     fragment, SBAI-1935: read once, stripped from the URL, never logged).
 *  2. Cookie        `sb_theme_sync_v1` on `.studiobrain.ai` — present when the
 *     LoreGUI web build is served from a *.studiobrain.ai origin.
 *  3. postMessage   `sb-theme-update` — live updates while embedded; ONLY
 *     accepted from allowed StudioBrain origins (mirrors the accounts
 *     frame-ancestors allowlist). Arbitrary cross-origin injection is
 *     rejected.
 *
 * When no payload is present, LoreGUI falls back to its own saved/default
 * theme — standalone use is unaffected.
 */

import {
  DEFAULT_THEME_SETTINGS,
  SURFACE_NAMES,
  cloneTheme,
} from "./theme";
import type {
  FontSize,
  SemanticTheme,
  SurfaceName,
  ThemeMode,
  ThemeSettings,
  ThemeSurface,
} from "./theme";

// ---------------------------------------------------------------------------
// Wire format (shared contract with studiobrain-core `lib/theme-bridge.ts`)
// ---------------------------------------------------------------------------

export interface SbThemeBridgePayload {
  kind: "sb-theme-bridge";
  version: 1;
  /** Base theme identity (informational; colors are fully resolved). */
  base?: string;
  mode: ThemeMode;
  fontSize?: FontSize;
  fontFamily?: string;
  /** Flat CSS-var maps keyed `--surface-{surface}-{prop}`. */
  light: Record<string, string>;
  dark: Record<string, string>;
}

export const BRIDGE_MESSAGE_TYPE = "sb-theme-update";
export const BRIDGE_FRAGMENT_KEY = "sbtheme";
export const BRIDGE_COOKIE_NAME = "sb_theme_sync_v1";

/** CSS-var suffix → ThemeSurface slot. */
const VAR_TO_SLOT: [string, keyof ThemeSurface][] = [
  // Longest suffixes first so "text-secondary" wins over "text".
  ["text-secondary", "textSecondary"],
  ["bg", "background"],
  ["text", "text"],
  ["border", "border"],
  ["hover", "hover"],
  ["active", "active"],
  ["shadow", "shadow"],
];

// ---------------------------------------------------------------------------
// Origin allowlist (mirror of the accounts CSP frame-ancestors, SBAI-1952)
// ---------------------------------------------------------------------------

const ALLOWED_EXACT_ORIGINS = new Set([
  "https://app.studiobrain.ai",
  "https://studiobrain.ai",
  // Desktop (Tauri) and mobile (Capacitor) StudioBrain shells.
  "tauri://localhost",
  "https://tauri.localhost",
  "capacitor://localhost",
]);

/** Is `origin` an allowed StudioBrain host for theme injection? */
export function isAllowedBridgeOrigin(
  origin: string,
  opts: { dev?: boolean } = {},
): boolean {
  if (!origin) return false;
  if (ALLOWED_EXACT_ORIGINS.has(origin)) return true;
  try {
    const url = new URL(origin);
    if (
      url.protocol === "https:" &&
      (url.hostname === "studiobrain.ai" ||
        url.hostname.endsWith(".studiobrain.ai"))
    ) {
      return true;
    }
    // Local development only (vite dev / tauri dev): loopback hosts.
    const dev = opts.dev ?? isDevBuild();
    if (
      dev &&
      (url.hostname === "localhost" || url.hostname === "127.0.0.1")
    ) {
      return true;
    }
  } catch {
    return false;
  }
  return false;
}

function isDevBuild(): boolean {
  try {
    return Boolean(import.meta.env?.DEV);
  } catch {
    return false;
  }
}

// ---------------------------------------------------------------------------
// Payload validation + conversion to LoreGUI theme settings
// ---------------------------------------------------------------------------

function isVarMap(v: unknown): v is Record<string, string> {
  return (
    !!v &&
    typeof v === "object" &&
    !Array.isArray(v) &&
    Object.entries(v as Record<string, unknown>).every(
      ([k, val]) => k.startsWith("--") && typeof val === "string",
    )
  );
}

/** Validate an untrusted value as a bridge payload. Returns null on junk. */
export function parseBridgePayload(raw: unknown): SbThemeBridgePayload | null {
  if (!raw || typeof raw !== "object") return null;
  const p = raw as Partial<SbThemeBridgePayload>;
  if (p.kind !== "sb-theme-bridge" || p.version !== 1) return null;
  if (!isVarMap(p.light) || !isVarMap(p.dark)) return null;
  const mode: ThemeMode =
    p.mode === "light" || p.mode === "dark" || p.mode === "system"
      ? p.mode
      : "system";
  return {
    kind: "sb-theme-bridge",
    version: 1,
    base: typeof p.base === "string" ? p.base : undefined,
    mode,
    fontSize:
      p.fontSize === "small" || p.fontSize === "medium" || p.fontSize === "large"
        ? p.fontSize
        : undefined,
    fontFamily: typeof p.fontFamily === "string" ? p.fontFamily : undefined,
    light: { ...p.light },
    dark: { ...p.dark },
  };
}

/**
 * Merge a flat CSS-var map onto a variant. Unknown vars are ignored; missing
 * vars keep the fallback's value so partial payloads degrade gracefully.
 */
export function varMapToVariant(
  vars: Record<string, string>,
  fallback: SemanticTheme,
): SemanticTheme {
  const out = cloneTheme(fallback);
  for (const [key, value] of Object.entries(vars)) {
    if (!key.startsWith("--surface-") || typeof value !== "string" || !value)
      continue;
    const rest = key.slice("--surface-".length); // "{surface}-{suffix}"
    for (const name of SURFACE_NAMES) {
      if (!rest.startsWith(`${name}-`)) continue;
      const suffix = rest.slice(name.length + 1);
      const slot = VAR_TO_SLOT.find(([s]) => s === suffix)?.[1];
      if (slot) out[name as SurfaceName][slot] = value;
      break;
    }
  }
  return out;
}

/** Convert a validated payload into full LoreGUI theme settings. */
export function payloadToSettings(
  payload: SbThemeBridgePayload,
  base: ThemeSettings = DEFAULT_THEME_SETTINGS,
): ThemeSettings {
  return {
    mode: payload.mode,
    lightTheme: varMapToVariant(payload.light, base.lightTheme),
    darkTheme: varMapToVariant(payload.dark, base.darkTheme),
    fontSize: payload.fontSize ?? base.fontSize,
    fontFamily: payload.fontFamily ?? base.fontFamily,
    // Never accept remote CSS injection — customCSS stays local-only.
    customCSS: base.customCSS,
  };
}

// ---------------------------------------------------------------------------
// Cookie-shape conversion (SettingsContext writes surface OBJECTS, not vars)
// ---------------------------------------------------------------------------

interface CookieShape {
  theme?: {
    mode?: ThemeMode;
    fontSize?: FontSize;
    fontFamily?: string;
    lightTheme?: Partial<Record<SurfaceName, Partial<ThemeSurface>>>;
    darkTheme?: Partial<Record<SurfaceName, Partial<ThemeSurface>>>;
  };
}

const SLOT_TO_VAR: [keyof ThemeSurface, string][] = [
  ["background", "bg"],
  ["text", "text"],
  ["textSecondary", "text-secondary"],
  ["border", "border"],
  ["hover", "hover"],
  ["active", "active"],
  ["shadow", "shadow"],
];

function variantObjectToVarMap(
  variant: Partial<Record<SurfaceName, Partial<ThemeSurface>>>,
): Record<string, string> {
  const vars: Record<string, string> = {};
  for (const name of SURFACE_NAMES) {
    const s = variant[name];
    if (!s || typeof s !== "object") continue;
    for (const [slot, suffix] of SLOT_TO_VAR) {
      const v = (s as Record<string, unknown>)[slot];
      if (typeof v === "string" && v)
        vars[`--surface-${name}-${suffix}`] = v;
    }
  }
  return vars;
}

/** Convert the `sb_theme_sync_v1` cookie JSON into a bridge payload. */
export function cookieJsonToPayload(raw: string): SbThemeBridgePayload | null {
  try {
    const parsed = JSON.parse(raw) as CookieShape;
    const t = parsed?.theme;
    if (!t?.lightTheme || !t.darkTheme) return null;
    return parseBridgePayload({
      kind: "sb-theme-bridge",
      version: 1,
      base: "studiobrain",
      mode: t.mode ?? "system",
      fontSize: t.fontSize,
      fontFamily: t.fontFamily,
      light: variantObjectToVarMap(t.lightTheme),
      dark: variantObjectToVarMap(t.darkTheme),
    });
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Transport readers
// ---------------------------------------------------------------------------

function fromBase64Url(value: string): string {
  const b64 = value.replace(/-/g, "+").replace(/_/g, "/");
  const padded = b64 + "=".repeat((4 - (b64.length % 4)) % 4);
  return decodeURIComponent(escape(atob(padded)));
}

/**
 * Read `#sbtheme=...` from the current URL, then strip it (never logged,
 * never persisted in history — same discipline as the accounts `#token=`).
 */
export function readFragmentTheme(): SbThemeBridgePayload | null {
  if (typeof window === "undefined") return null;
  const hash = window.location.hash.replace(/^#/, "");
  if (!hash) return null;
  const params = new URLSearchParams(hash);
  const value = params.get(BRIDGE_FRAGMENT_KEY);
  if (!value) return null;
  params.delete(BRIDGE_FRAGMENT_KEY);
  try {
    const remaining = params.toString();
    window.history.replaceState(
      null,
      "",
      window.location.pathname +
        window.location.search +
        (remaining ? `#${remaining}` : ""),
    );
  } catch {
    /* history may be unavailable; payload still applies */
  }
  try {
    return parseBridgePayload(JSON.parse(fromBase64Url(value)));
  } catch {
    return null;
  }
}

/**
 * Read the StudioBrain theme-sync cookie. Only meaningful when LoreGUI is
 * served from a *.studiobrain.ai origin (the cookie is domain-scoped there,
 * so its presence already implies an allowed origin).
 */
export function readCookieTheme(): SbThemeBridgePayload | null {
  if (typeof document === "undefined" || typeof window === "undefined")
    return null;
  const host = window.location.hostname.toLowerCase();
  if (host !== "studiobrain.ai" && !host.endsWith(".studiobrain.ai"))
    return null;
  const match = document.cookie
    .split("; ")
    .find((c) => c.startsWith(`${BRIDGE_COOKIE_NAME}=`));
  if (!match) return null;
  try {
    return cookieJsonToPayload(
      decodeURIComponent(match.slice(BRIDGE_COOKIE_NAME.length + 1)),
    );
  } catch {
    return null;
  }
}

/** Boot-time detection: fragment first (explicit hand-off), then cookie. */
export function detectBridgeTheme(): SbThemeBridgePayload | null {
  return readFragmentTheme() ?? readCookieTheme();
}

/**
 * Listen for live `sb-theme-update` postMessages from an embedding
 * StudioBrain host. Messages from non-allowed origins are silently dropped.
 * Returns an unsubscribe function.
 */
export function listenForBridgeTheme(
  onTheme: (payload: SbThemeBridgePayload) => void,
): () => void {
  if (typeof window === "undefined") return () => {};
  const handler = (event: MessageEvent) => {
    if (!isAllowedBridgeOrigin(event.origin)) return;
    const data = event.data as { type?: string; payload?: unknown } | null;
    if (!data || data.type !== BRIDGE_MESSAGE_TYPE) return;
    const payload = parseBridgePayload(data.payload);
    if (payload) onTheme(payload);
  };
  window.addEventListener("message", handler);
  return () => window.removeEventListener("message", handler);
}
