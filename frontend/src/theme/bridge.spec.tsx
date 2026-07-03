/**
 * SBAI-4605 — StudioBrain → LoreGUI theme bridge tests.
 *
 * Covers: payload round-trip incl. user-customized colors, allowed-origin
 * gating (postMessage), fragment hand-off, cookie-shape conversion, and the
 * standalone fallback to LoreGUI's own theme.
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { render, cleanup, act } from "@testing-library/react";
import { ThemeProvider, useTheme } from "./ThemeProvider";
import {
  BRIDGE_MESSAGE_TYPE,
  BRIDGE_FRAGMENT_KEY,
  cookieJsonToPayload,
  isAllowedBridgeOrigin,
  parseBridgePayload,
  payloadToSettings,
  varMapToVariant,
  type SbThemeBridgePayload,
} from "./bridge";
import {
  DEFAULT_THEME_SETTINGS,
  SURFACE_NAMES,
  cloneTheme,
} from "./theme";
import type { SemanticTheme } from "./theme";

// ---------------------------------------------------------------------------
// Helpers — simulate StudioBrain's export side (surfaces → CSS-var map)
// ---------------------------------------------------------------------------

const SLOT_TO_VAR: [string, string][] = [
  ["background", "bg"],
  ["text", "text"],
  ["textSecondary", "text-secondary"],
  ["border", "border"],
  ["hover", "hover"],
  ["active", "active"],
  ["shadow", "shadow"],
];

function exportVarMap(variant: SemanticTheme): Record<string, string> {
  const vars: Record<string, string> = {};
  for (const name of SURFACE_NAMES) {
    for (const [slot, suffix] of SLOT_TO_VAR) {
      vars[`--surface-${name}-${suffix}`] = (
        variant[name] as unknown as Record<string, string>
      )[slot];
    }
  }
  return vars;
}

function makeStudioBrainPayload(): SbThemeBridgePayload {
  // A StudioBrain user's CUSTOMIZED theme: start from defaults, override.
  const dark = cloneTheme(DEFAULT_THEME_SETTINGS.darkTheme);
  const light = cloneTheme(DEFAULT_THEME_SETTINGS.lightTheme);
  dark.primary.background = "#ff0088"; // user-picked brand color
  dark.base.background = "#101820";
  light.accent.text = "#123456";
  return {
    kind: "sb-theme-bridge",
    version: 1,
    base: "studiobrain",
    mode: "dark",
    fontSize: "large",
    fontFamily: "Inter, sans-serif",
    light: exportVarMap(light),
    dark: exportVarMap(dark),
  };
}

function encodeFragment(payload: SbThemeBridgePayload): string {
  return btoa(unescape(encodeURIComponent(JSON.stringify(payload))))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

function Probe({ onCtx }: { onCtx: (ctx: ReturnType<typeof useTheme>) => void }) {
  onCtx(useTheme());
  return null;
}

beforeEach(() => {
  localStorage.clear();
  window.location.hash = "";
  // Wipe any CSS vars left by a previous applyTheme.
  document.documentElement.removeAttribute("style");
});

afterEach(() => {
  cleanup();
  window.location.hash = "";
});

// ---------------------------------------------------------------------------
// Round-trip: export → import → tokens match (incl. custom overrides)
// ---------------------------------------------------------------------------

describe("payload round-trip", () => {
  it("reproduces StudioBrain's customized tokens exactly", () => {
    const payload = makeStudioBrainPayload();
    const parsed = parseBridgePayload(payload);
    expect(parsed).not.toBeNull();
    const settings = payloadToSettings(parsed!);
    expect(settings.mode).toBe("dark");
    expect(settings.fontSize).toBe("large");
    expect(settings.fontFamily).toBe("Inter, sans-serif");
    // Custom overrides survive.
    expect(settings.darkTheme.primary.background).toBe("#ff0088");
    expect(settings.darkTheme.base.background).toBe("#101820");
    expect(settings.lightTheme.accent.text).toBe("#123456");
    // Non-overridden tokens equal StudioBrain's export bit-for-bit.
    expect(settings.darkTheme.info).toEqual(DEFAULT_THEME_SETTINGS.darkTheme.info);
    // Remote CSS injection is never accepted.
    expect(settings.customCSS).toBe("");
  });

  it("applies bridged tokens to the DOM via the provider (fragment boot)", () => {
    const payload = makeStudioBrainPayload();
    window.location.hash = `#${BRIDGE_FRAGMENT_KEY}=${encodeFragment(payload)}`;
    let ctx!: ReturnType<typeof useTheme>;
    render(
      <ThemeProvider>
        <Probe onCtx={(c) => (ctx = c)} />
      </ThemeProvider>,
    );
    const style = document.documentElement.style;
    expect(style.getPropertyValue("--surface-primary-bg")).toBe("#ff0088");
    expect(style.getPropertyValue("--surface-base-bg")).toBe("#101820");
    expect(ctx.bridgeActive).toBe(true);
    expect(ctx.isDark).toBe(true);
    // Fragment is stripped after being read (accounts #token= discipline).
    expect(window.location.hash).not.toContain(BRIDGE_FRAGMENT_KEY);
    // Bridged theme is never persisted over the user's local theme.
    expect(localStorage.getItem("loregui.theme.v1")).toBeNull();
  });

  it("merges partial var maps over LoreGUI defaults", () => {
    const variant = varMapToVariant(
      { "--surface-primary-bg": "#abcdef", "--unknown-var": "#000" },
      DEFAULT_THEME_SETTINGS.darkTheme,
    );
    expect(variant.primary.background).toBe("#abcdef");
    expect(variant.primary.text).toBe(
      DEFAULT_THEME_SETTINGS.darkTheme.primary.text,
    );
  });
});

// ---------------------------------------------------------------------------
// Allowed-origin gating
// ---------------------------------------------------------------------------

describe("origin gating", () => {
  it("accepts only StudioBrain origins (+ loopback in dev builds)", () => {
    expect(isAllowedBridgeOrigin("https://app.studiobrain.ai")).toBe(true);
    expect(isAllowedBridgeOrigin("https://studiobrain.ai")).toBe(true);
    expect(isAllowedBridgeOrigin("https://foo.studiobrain.ai")).toBe(true);
    expect(isAllowedBridgeOrigin("tauri://localhost")).toBe(true);
    expect(isAllowedBridgeOrigin("capacitor://localhost")).toBe(true);
    expect(isAllowedBridgeOrigin("https://evil.example.com")).toBe(false);
    expect(isAllowedBridgeOrigin("https://studiobrain.ai.evil.com")).toBe(false);
    expect(isAllowedBridgeOrigin("http://studiobrain.ai", { dev: false })).toBe(
      false, // http (not https) is not a StudioBrain origin
    );
    expect(isAllowedBridgeOrigin("http://localhost:1420", { dev: false })).toBe(
      false,
    );
    expect(isAllowedBridgeOrigin("http://localhost:1420", { dev: true })).toBe(
      true,
    );
    expect(isAllowedBridgeOrigin("")).toBe(false);
  });

  it("applies postMessage themes from allowed origins and drops others", () => {
    let ctx!: ReturnType<typeof useTheme>;
    render(
      <ThemeProvider>
        <Probe onCtx={(c) => (ctx = c)} />
      </ThemeProvider>,
    );
    const payload = makeStudioBrainPayload();

    // Disallowed origin → ignored.
    act(() => {
      window.dispatchEvent(
        new MessageEvent("message", {
          data: { type: BRIDGE_MESSAGE_TYPE, payload },
          origin: "https://evil.example.com",
        }),
      );
    });
    expect(ctx.bridgeActive).toBe(false);
    expect(
      document.documentElement.style.getPropertyValue("--surface-primary-bg"),
    ).not.toBe("#ff0088");

    // Allowed origin → applied live.
    act(() => {
      window.dispatchEvent(
        new MessageEvent("message", {
          data: { type: BRIDGE_MESSAGE_TYPE, payload },
          origin: "https://app.studiobrain.ai",
        }),
      );
    });
    expect(ctx.bridgeActive).toBe(true);
    expect(
      document.documentElement.style.getPropertyValue("--surface-primary-bg"),
    ).toBe("#ff0088");
  });

  it("rejects malformed payloads even from allowed origins", () => {
    let ctx!: ReturnType<typeof useTheme>;
    render(
      <ThemeProvider>
        <Probe onCtx={(c) => (ctx = c)} />
      </ThemeProvider>,
    );
    act(() => {
      window.dispatchEvent(
        new MessageEvent("message", {
          data: { type: BRIDGE_MESSAGE_TYPE, payload: { kind: "nope" } },
          origin: "https://app.studiobrain.ai",
        }),
      );
    });
    expect(ctx.bridgeActive).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Cookie-shape conversion (SettingsContext's sb_theme_sync_v1)
// ---------------------------------------------------------------------------

describe("cookie conversion", () => {
  it("converts the SettingsContext cookie JSON (surface objects) to a payload", () => {
    const dark = cloneTheme(DEFAULT_THEME_SETTINGS.darkTheme);
    dark.accent.background = "#00ffcc";
    const cookieJson = JSON.stringify({
      theme: {
        mode: "dark",
        fontSize: "small",
        fontFamily: "JetBrains Mono",
        lightTheme: DEFAULT_THEME_SETTINGS.lightTheme,
        darkTheme: { ...dark, primaryColor: "#legacy-ignored" },
      },
      display: { uiScale: 1.25 },
    });
    const payload = cookieJsonToPayload(cookieJson);
    expect(payload).not.toBeNull();
    expect(payload!.dark["--surface-accent-bg"]).toBe("#00ffcc");
    expect(payload!.fontFamily).toBe("JetBrains Mono");
    const settings = payloadToSettings(payload!);
    expect(settings.darkTheme.accent.background).toBe("#00ffcc");
  });

  it("returns null for junk cookies", () => {
    expect(cookieJsonToPayload("not json")).toBeNull();
    expect(cookieJsonToPayload("{}")).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Standalone fallback
// ---------------------------------------------------------------------------

describe("standalone fallback", () => {
  it("uses LoreGUI's own theme when no bridge payload is present", () => {
    let ctx!: ReturnType<typeof useTheme>;
    render(
      <ThemeProvider>
        <Probe onCtx={(c) => (ctx = c)} />
      </ThemeProvider>,
    );
    expect(ctx.bridgeActive).toBe(false);
    // matchMedia stub says light; system mode resolves to LIGHT defaults.
    expect(
      document.documentElement.style.getPropertyValue("--surface-base-bg"),
    ).toBe(DEFAULT_THEME_SETTINGS.lightTheme.base.background);
  });

  it("local edits take over from a bridged theme and persist locally only", () => {
    const payload = makeStudioBrainPayload();
    window.location.hash = `#${BRIDGE_FRAGMENT_KEY}=${encodeFragment(payload)}`;
    let ctx!: ReturnType<typeof useTheme>;
    render(
      <ThemeProvider>
        <Probe onCtx={(c) => (ctx = c)} />
      </ThemeProvider>,
    );
    expect(ctx.bridgeActive).toBe(true);
    act(() => {
      ctx.updateSurfaceSlot("light", "primary", "background", "#111111");
    });
    expect(ctx.bridgeActive).toBe(false);
    const saved = JSON.parse(localStorage.getItem("loregui.theme.v1")!);
    expect(saved.lightTheme.primary.background).toBe("#111111");
    // The bridged StudioBrain color was never written to storage.
    expect(saved.darkTheme.primary.background).not.toBe("#ff0088");
  });
});
