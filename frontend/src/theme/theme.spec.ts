/**
 * Unit tests for the theme model: DOM application, serialization round-trip,
 * import validation, and the system/dark resolution rule. These are pure-ish
 * functions (applyTheme writes CSS custom properties onto documentElement,
 * which jsdom provides) so they run without React.
 */
import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  applyTheme,
  resolveIsDark,
  cloneTheme,
  toBundle,
  parseBundle,
  DEFAULT_THEME_SETTINGS,
  SURFACE_NAMES,
  FONT_SIZE_PX,
  PRESET_DARK,
  PRESET_LIGHT,
  type ThemeSettings,
} from "./theme";

function freshSettings(): ThemeSettings {
  return {
    ...DEFAULT_THEME_SETTINGS,
    lightTheme: cloneTheme(DEFAULT_THEME_SETTINGS.lightTheme),
    darkTheme: cloneTheme(DEFAULT_THEME_SETTINGS.darkTheme),
  };
}

describe("resolveIsDark", () => {
  it("is true for explicit dark, false for explicit light", () => {
    expect(resolveIsDark("dark")).toBe(true);
    expect(resolveIsDark("light")).toBe(false);
  });

  it("follows matchMedia for system mode", () => {
    const spy = vi
      .spyOn(window, "matchMedia")
      .mockReturnValue({ matches: true } as MediaQueryList);
    expect(resolveIsDark("system")).toBe(true);
    spy.mockReturnValue({ matches: false } as MediaQueryList);
    expect(resolveIsDark("system")).toBe(false);
  });
});

describe("applyTheme", () => {
  beforeEach(() => {
    document.documentElement.removeAttribute("style");
    document.documentElement.className = "";
    document.getElementById("loregui-custom-theme")?.remove();
  });

  it("writes every surface's seven CSS custom properties", () => {
    applyTheme({ ...freshSettings(), mode: "dark" });
    const root = document.documentElement;
    for (const name of SURFACE_NAMES) {
      const surf = PRESET_DARK[name];
      expect(root.style.getPropertyValue(`--surface-${name}-bg`)).toBe(
        surf.background,
      );
      expect(root.style.getPropertyValue(`--surface-${name}-text`)).toBe(
        surf.text,
      );
      expect(root.style.getPropertyValue(`--surface-${name}-border`)).toBe(
        surf.border,
      );
    }
  });

  it("toggles the `dark` class to match the resolved mode", () => {
    applyTheme({ ...freshSettings(), mode: "dark" });
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    applyTheme({ ...freshSettings(), mode: "light" });
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("derives the shadow ladder + font size from the active variant", () => {
    applyTheme({ ...freshSettings(), mode: "light", fontSize: "large" });
    const root = document.documentElement;
    expect(root.style.getPropertyValue("--shadow-lg")).toBe(
      PRESET_LIGHT.overlay.shadow,
    );
    expect(root.style.getPropertyValue("--base-font-size")).toBe(
      `${FONT_SIZE_PX.large}px`,
    );
  });

  it("injects custom CSS into a managed <style> and removes it when cleared", () => {
    applyTheme({ ...freshSettings(), customCSS: "body{color:red}" });
    const el = document.getElementById("loregui-custom-theme");
    expect(el?.textContent).toBe("body{color:red}");
    applyTheme({ ...freshSettings(), customCSS: "   " });
    expect(document.getElementById("loregui-custom-theme")).toBeNull();
  });
});

describe("serialization round-trip", () => {
  it("toBundle → JSON → parseBundle preserves both variants and stamps the schema", () => {
    const s = freshSettings();
    const bundle = toBundle("My Theme", s, "alice");
    expect(bundle.kind).toBe("loregui-theme");
    expect(bundle.version).toBe(1);
    const round = parseBundle(JSON.stringify(bundle));
    expect(round.name).toBe("My Theme");
    expect(round.lightTheme).toEqual(s.lightTheme);
    expect(round.darkTheme).toEqual(s.darkTheme);
  });

  it("cloneTheme deep-copies (mutating the clone never touches the source)", () => {
    const src = cloneTheme(PRESET_DARK);
    const copy = cloneTheme(src);
    copy.base.background = "#ffffff";
    expect(src.base.background).not.toBe("#ffffff");
  });
});

describe("parseBundle validation", () => {
  it("rejects JSON missing a theme variant", () => {
    expect(() => parseBundle(JSON.stringify({ lightTheme: PRESET_LIGHT }))).toThrow(
      /Invalid theme/,
    );
  });

  it("rejects a variant missing a surface", () => {
    const broken = cloneTheme(PRESET_DARK) as Record<string, unknown>;
    delete broken.input;
    expect(() =>
      parseBundle(
        JSON.stringify({ lightTheme: PRESET_LIGHT, darkTheme: broken }),
      ),
    ).toThrow(/Invalid theme/);
  });

  it("rejects non-JSON input", () => {
    expect(() => parseBundle("{not json")).toThrow();
  });

  it("defaults the name when absent", () => {
    const b = parseBundle(
      JSON.stringify({ lightTheme: PRESET_LIGHT, darkTheme: PRESET_DARK }),
    );
    expect(b.name).toBe("Imported theme");
  });
});
