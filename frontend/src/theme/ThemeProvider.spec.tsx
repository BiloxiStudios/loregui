/**
 * Component tests for ThemeProvider — the store that persists theme settings to
 * localStorage, applies them to the DOM, and exposes mutators via useTheme().
 * Covers: default boot, persistence on change, restore-from-storage with a
 * forward-merge against defaults, mode switching, and import via the context.
 */
import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { ThemeProvider, useTheme } from "./ThemeProvider";
import { toBundle, DEFAULT_THEME_SETTINGS, cloneTheme } from "./theme";

const STORAGE_KEY = "loregui.theme.v1";

// A tiny consumer that surfaces context state + exposes the mutators to tests.
let ctx: ReturnType<typeof useTheme>;
function Probe() {
  ctx = useTheme();
  return (
    <div>
      <span data-testid="mode">{ctx.settings.mode}</span>
      <span data-testid="isDark">{String(ctx.isDark)}</span>
      <span data-testid="font">{ctx.settings.fontSize}</span>
    </div>
  );
}

function renderProvider() {
  return render(
    <ThemeProvider>
      <Probe />
    </ThemeProvider>,
  );
}

beforeEach(() => {
  localStorage.clear();
});

describe("ThemeProvider", () => {
  it("boots from defaults when storage is empty", () => {
    renderProvider();
    expect(screen.getByTestId("mode").textContent).toBe(
      DEFAULT_THEME_SETTINGS.mode,
    );
    expect(screen.getByTestId("font").textContent).toBe("medium");
  });

  it("does not write storage on the first (apply-only) pass", () => {
    renderProvider();
    expect(localStorage.getItem(STORAGE_KEY)).toBeNull();
  });

  it("persists to localStorage after a settings change", () => {
    renderProvider();
    act(() => ctx.setMode("dark"));
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    expect(saved.mode).toBe("dark");
    expect(screen.getByTestId("mode").textContent).toBe("dark");
    expect(screen.getByTestId("isDark").textContent).toBe("true");
  });

  it("restores a saved mode and forward-merges missing fields against defaults", () => {
    // Persist only a partial settings blob (older save without fontFamily).
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ mode: "light" }));
    renderProvider();
    expect(screen.getByTestId("mode").textContent).toBe("light");
    // The merge filled fontSize from defaults rather than crashing.
    expect(screen.getByTestId("font").textContent).toBe("medium");
    expect(ctx.settings.fontFamily).toBe(DEFAULT_THEME_SETTINGS.fontFamily);
  });

  it("setFontSize updates settings and the applied root font size", () => {
    renderProvider();
    act(() => ctx.setFontSize("large"));
    expect(screen.getByTestId("font").textContent).toBe("large");
    expect(document.documentElement.style.getPropertyValue("--base-font-size")).toBe(
      "16px",
    );
  });

  it("importBundle replaces both variants from a parsed bundle", () => {
    renderProvider();
    const custom = cloneTheme(DEFAULT_THEME_SETTINGS.darkTheme);
    custom.base.background = "#123456";
    const bundle = toBundle("Imported", {
      ...DEFAULT_THEME_SETTINGS,
      lightTheme: custom,
      darkTheme: custom,
    });
    act(() => ctx.importBundle(JSON.stringify(bundle)));
    expect(ctx.settings.lightTheme.base.background).toBe("#123456");
    expect(ctx.settings.darkTheme.base.background).toBe("#123456");
  });

  it("exportBundle produces a re-importable JSON bundle", () => {
    renderProvider();
    const json = ctx.exportBundle("RoundTrip");
    expect(() => ctx.importBundle(json)).not.toThrow();
    const parsed = JSON.parse(json);
    expect(parsed.kind).toBe("loregui-theme");
    expect(parsed.name).toBe("RoundTrip");
  });

  it("resetToDefaults returns settings to the default mode", () => {
    renderProvider();
    act(() => ctx.setMode("dark"));
    act(() => ctx.resetToDefaults());
    expect(screen.getByTestId("mode").textContent).toBe(
      DEFAULT_THEME_SETTINGS.mode,
    );
  });
});

describe("useTheme guard", () => {
  it("throws when used outside a ThemeProvider", () => {
    // Silence the React error boundary console noise for this expected throw.
    const orig = console.error;
    console.error = () => {};
    function Bare() {
      useTheme();
      return null;
    }
    expect(() => render(<Bare />)).toThrow(/within <ThemeProvider>/);
    console.error = orig;
  });
});
