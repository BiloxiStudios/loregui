/**
 * Semantic theme model for LoreGUI.
 *
 * Ported from the StudioBrain desktop app theming system so that themes are
 * cross-compatible: a theme exported from StudioBrain can be imported here and
 * vice-versa. The model is a set of named *surfaces*, each carrying the colors
 * needed to render any UI region, applied to the DOM as CSS custom properties
 * (`--surface-{name}-{prop}`) on `:root`. Plain CSS consumes them via `var()`.
 */

/** Every surface carries the same seven slots. */
export interface ThemeSurface {
  /** Background fill. */
  background: string;
  /** Primary (high-contrast) text. */
  text: string;
  /** Secondary (muted) text. */
  textSecondary: string;
  /** Border / divider color. */
  border: string;
  /** Hover-state background. */
  hover: string;
  /** Active / selected background. */
  active: string;
  /** CSS box-shadow value for this surface. */
  shadow: string;
}

/** The canonical, ordered list of surfaces. */
export const SURFACE_NAMES = [
  // Layout surfaces (neutral, hierarchical)
  "base",
  "elevated",
  "overlay",
  // Brand surfaces (colorful, interactive)
  "primary",
  "secondary",
  "accent",
  // Status surfaces (semantic meaning)
  "success",
  "warning",
  "error",
  "info",
  // Specialized surfaces
  "navigation",
  "input",
] as const;

export type SurfaceName = (typeof SURFACE_NAMES)[number];

/** Human-readable labels + descriptions, used by the theme editor. */
export const SURFACE_META: Record<
  SurfaceName,
  { label: string; description: string }
> = {
  base: { label: "Base", description: "Page background and primary content" },
  elevated: { label: "Elevated", description: "Cards, panels, raised content" },
  overlay: { label: "Overlay", description: "Modals, dropdowns, tooltips" },
  primary: { label: "Primary", description: "Primary buttons and CTAs" },
  secondary: { label: "Secondary", description: "Secondary actions" },
  accent: { label: "Accent", description: "Highlights, badges, emphasis" },
  success: { label: "Success", description: "Success states, confirmations" },
  warning: { label: "Warning", description: "Warnings and cautions" },
  error: { label: "Error", description: "Errors and destructive actions" },
  info: { label: "Info", description: "Informational messages" },
  navigation: { label: "Navigation", description: "Top bar, sidebars, menus" },
  input: { label: "Input", description: "Text fields, selects, text areas" },
};

export type SemanticTheme = Record<SurfaceName, ThemeSurface>;

export type ThemeMode = "light" | "dark" | "system";
export type FontSize = "small" | "medium" | "large";

/** The full, persisted theme configuration. */
export interface ThemeSettings {
  mode: ThemeMode;
  lightTheme: SemanticTheme;
  darkTheme: SemanticTheme;
  fontSize: FontSize;
  fontFamily: string;
  /** Free-form CSS appended last, for power users. */
  customCSS: string;
}

/** A shareable, named theme bundle (export/import unit). */
export interface ThemeBundle {
  /** Schema marker so importers can validate. */
  kind: "loregui-theme";
  version: 1;
  name: string;
  author?: string;
  lightTheme: SemanticTheme;
  darkTheme: SemanticTheme;
  fontFamily?: string;
}

export const FONT_SIZE_PX: Record<FontSize, number> = {
  small: 13,
  medium: 14,
  large: 16,
};

export const DEFAULT_FONT_FAMILY =
  'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif';

// ---------------------------------------------------------------------------
// Preset themes
// ---------------------------------------------------------------------------

const DARK: SemanticTheme = {
  base: {
    background: "#0d1117",
    text: "#e6edf3",
    textSecondary: "#8b949e",
    border: "#2a3340",
    hover: "#161b22",
    active: "#1f2630",
    shadow: "0 1px 2px rgba(0,0,0,0.4)",
  },
  elevated: {
    background: "#161b22",
    text: "#e6edf3",
    textSecondary: "#8b949e",
    border: "#2a3340",
    hover: "#1f2630",
    active: "#262d38",
    shadow: "0 4px 12px rgba(0,0,0,0.5)",
  },
  overlay: {
    background: "#1c2230",
    text: "#e6edf3",
    textSecondary: "#8b949e",
    border: "#39414e",
    hover: "#242b3a",
    active: "#2c3444",
    shadow: "0 12px 32px rgba(0,0,0,0.6)",
  },
  primary: {
    background: "#3b82f6",
    text: "#ffffff",
    textSecondary: "#dbeafe",
    border: "#3b82f6",
    hover: "#2f6fe0",
    active: "#2861c9",
    shadow: "0 2px 8px rgba(59,130,246,0.35)",
  },
  secondary: {
    background: "#21262d",
    text: "#e6edf3",
    textSecondary: "#8b949e",
    border: "#2a3340",
    hover: "#2a313a",
    active: "#323a45",
    shadow: "none",
  },
  accent: {
    background: "#8b5cf6",
    text: "#ffffff",
    textSecondary: "#ede9fe",
    border: "#8b5cf6",
    hover: "#7c4ddb",
    active: "#6d3fc4",
    shadow: "0 2px 8px rgba(139,92,246,0.35)",
  },
  success: {
    background: "rgba(63,185,80,0.15)",
    text: "#3fb950",
    textSecondary: "#56d364",
    border: "rgba(63,185,80,0.4)",
    hover: "rgba(63,185,80,0.22)",
    active: "rgba(63,185,80,0.3)",
    shadow: "none",
  },
  warning: {
    background: "rgba(210,153,34,0.15)",
    text: "#d29922",
    textSecondary: "#e3b341",
    border: "rgba(210,153,34,0.4)",
    hover: "rgba(210,153,34,0.22)",
    active: "rgba(210,153,34,0.3)",
    shadow: "none",
  },
  error: {
    background: "rgba(248,81,73,0.15)",
    text: "#f85149",
    textSecondary: "#ff7b72",
    border: "rgba(248,81,73,0.4)",
    hover: "rgba(248,81,73,0.22)",
    active: "rgba(248,81,73,0.3)",
    shadow: "none",
  },
  info: {
    background: "rgba(59,130,246,0.15)",
    text: "#58a6ff",
    textSecondary: "#79c0ff",
    border: "rgba(59,130,246,0.4)",
    hover: "rgba(59,130,246,0.22)",
    active: "rgba(59,130,246,0.3)",
    shadow: "none",
  },
  navigation: {
    background: "#161b22",
    text: "#e6edf3",
    textSecondary: "#8b949e",
    border: "#2a3340",
    hover: "#1f2630",
    active: "#262d38",
    shadow: "0 1px 0 rgba(0,0,0,0.4)",
  },
  input: {
    background: "#0d1117",
    text: "#e6edf3",
    textSecondary: "#8b949e",
    border: "#2a3340",
    hover: "#161b22",
    active: "#1f2630",
    shadow: "none",
  },
};

const LIGHT: SemanticTheme = {
  base: {
    background: "#ffffff",
    text: "#1f2328",
    textSecondary: "#656d76",
    border: "#d0d7de",
    hover: "#f6f8fa",
    active: "#eef1f4",
    shadow: "0 1px 2px rgba(31,35,40,0.08)",
  },
  elevated: {
    background: "#f6f8fa",
    text: "#1f2328",
    textSecondary: "#656d76",
    border: "#d0d7de",
    hover: "#eef1f4",
    active: "#e6eaef",
    shadow: "0 4px 12px rgba(31,35,40,0.1)",
  },
  overlay: {
    background: "#ffffff",
    text: "#1f2328",
    textSecondary: "#656d76",
    border: "#d0d7de",
    hover: "#f6f8fa",
    active: "#eef1f4",
    shadow: "0 12px 32px rgba(31,35,40,0.18)",
  },
  primary: {
    background: "#2563eb",
    text: "#ffffff",
    textSecondary: "#dbeafe",
    border: "#2563eb",
    hover: "#1d54cf",
    active: "#1a49b5",
    shadow: "0 2px 8px rgba(37,99,235,0.25)",
  },
  secondary: {
    background: "#eef1f4",
    text: "#1f2328",
    textSecondary: "#656d76",
    border: "#d0d7de",
    hover: "#e6eaef",
    active: "#dde2e8",
    shadow: "none",
  },
  accent: {
    background: "#7c3aed",
    text: "#ffffff",
    textSecondary: "#ede9fe",
    border: "#7c3aed",
    hover: "#6d2fd6",
    active: "#5f28bd",
    shadow: "0 2px 8px rgba(124,58,237,0.25)",
  },
  success: {
    background: "rgba(26,127,55,0.12)",
    text: "#1a7f37",
    textSecondary: "#1a7f37",
    border: "rgba(26,127,55,0.35)",
    hover: "rgba(26,127,55,0.18)",
    active: "rgba(26,127,55,0.26)",
    shadow: "none",
  },
  warning: {
    background: "rgba(154,103,0,0.12)",
    text: "#9a6700",
    textSecondary: "#9a6700",
    border: "rgba(154,103,0,0.35)",
    hover: "rgba(154,103,0,0.18)",
    active: "rgba(154,103,0,0.26)",
    shadow: "none",
  },
  error: {
    background: "rgba(207,34,46,0.12)",
    text: "#cf222e",
    textSecondary: "#cf222e",
    border: "rgba(207,34,46,0.35)",
    hover: "rgba(207,34,46,0.18)",
    active: "rgba(207,34,46,0.26)",
    shadow: "none",
  },
  info: {
    background: "rgba(37,99,235,0.1)",
    text: "#0969da",
    textSecondary: "#0969da",
    border: "rgba(37,99,235,0.32)",
    hover: "rgba(37,99,235,0.16)",
    active: "rgba(37,99,235,0.24)",
    shadow: "none",
  },
  navigation: {
    background: "#f6f8fa",
    text: "#1f2328",
    textSecondary: "#656d76",
    border: "#d0d7de",
    hover: "#eef1f4",
    active: "#e6eaef",
    shadow: "0 1px 0 rgba(31,35,40,0.06)",
  },
  input: {
    background: "#ffffff",
    text: "#1f2328",
    textSecondary: "#656d76",
    border: "#d0d7de",
    hover: "#f6f8fa",
    active: "#eef1f4",
    shadow: "none",
  },
};

/** Built-in presets, selectable from the editor. */
export const PRESET_THEMES: { name: string; settings: () => ThemeSettings }[] = [
  {
    name: "LoreGUI Dark",
    settings: () => ({ ...DEFAULT_THEME_SETTINGS, mode: "dark" }),
  },
  {
    name: "LoreGUI Light",
    settings: () => ({ ...DEFAULT_THEME_SETTINGS, mode: "light" }),
  },
];

export const DEFAULT_THEME_SETTINGS: ThemeSettings = {
  mode: "system",
  lightTheme: LIGHT,
  darkTheme: DARK,
  fontSize: "medium",
  fontFamily: DEFAULT_FONT_FAMILY,
  customCSS: "",
};

export const PRESET_DARK = DARK;
export const PRESET_LIGHT = LIGHT;

// ---------------------------------------------------------------------------
// Apply to DOM
// ---------------------------------------------------------------------------

export function resolveIsDark(mode: ThemeMode): boolean {
  if (mode === "dark") return true;
  if (mode === "light") return false;
  return (
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches
  );
}

/**
 * Writes the active theme to `document.documentElement` as CSS custom
 * properties. Idempotent — safe to call on every settings change.
 */
export function applyTheme(settings: ThemeSettings): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  const isDark = resolveIsDark(settings.mode);
  root.classList.toggle("dark", isDark);

  const active = isDark ? settings.darkTheme : settings.lightTheme;

  for (const name of SURFACE_NAMES) {
    const s = active[name];
    if (!s) continue;
    root.style.setProperty(`--surface-${name}-bg`, s.background);
    root.style.setProperty(`--surface-${name}-text`, s.text);
    root.style.setProperty(`--surface-${name}-text-secondary`, s.textSecondary);
    root.style.setProperty(`--surface-${name}-border`, s.border);
    root.style.setProperty(`--surface-${name}-hover`, s.hover);
    root.style.setProperty(`--surface-${name}-active`, s.active);
    root.style.setProperty(`--surface-${name}-shadow`, s.shadow);
  }

  // Shadow ladder derived from layout surfaces.
  root.style.setProperty("--shadow-sm", active.base.shadow);
  root.style.setProperty("--shadow-md", active.elevated.shadow);
  root.style.setProperty("--shadow-lg", active.overlay.shadow);

  // Typography.
  const px = FONT_SIZE_PX[settings.fontSize];
  root.style.setProperty("--base-font-size", `${px}px`);
  root.style.setProperty("--font-family", settings.fontFamily);
  root.style.fontSize = `${px}px`;
  root.style.fontFamily = settings.fontFamily;

  // Custom CSS override (injected once, kept in sync).
  let el = document.getElementById("loregui-custom-theme");
  if (settings.customCSS.trim()) {
    if (!el) {
      el = document.createElement("style");
      el.id = "loregui-custom-theme";
      document.head.appendChild(el);
    }
    el.textContent = settings.customCSS;
  } else if (el) {
    el.remove();
  }
}

// ---------------------------------------------------------------------------
// Serialization helpers (deep clone + import validation)
// ---------------------------------------------------------------------------

export function cloneTheme(t: SemanticTheme): SemanticTheme {
  return JSON.parse(JSON.stringify(t));
}

export function toBundle(
  name: string,
  settings: ThemeSettings,
  author?: string,
): ThemeBundle {
  return {
    kind: "loregui-theme",
    version: 1,
    name,
    author,
    lightTheme: cloneTheme(settings.lightTheme),
    darkTheme: cloneTheme(settings.darkTheme),
    fontFamily: settings.fontFamily,
  };
}

/** Parse + validate an imported bundle. Throws on malformed input. */
export function parseBundle(json: string): ThemeBundle {
  const parsed = JSON.parse(json) as Partial<ThemeBundle>;
  const hasSurfaces = (t: unknown): t is SemanticTheme =>
    !!t &&
    typeof t === "object" &&
    SURFACE_NAMES.every(
      (n) =>
        (t as Record<string, unknown>)[n] &&
        typeof (t as Record<string, ThemeSurface>)[n].background === "string",
    );
  if (!hasSurfaces(parsed.lightTheme) || !hasSurfaces(parsed.darkTheme)) {
    throw new Error(
      "Invalid theme: expected lightTheme and darkTheme with all surfaces.",
    );
  }
  return {
    kind: "loregui-theme",
    version: 1,
    name: parsed.name ?? "Imported theme",
    author: parsed.author,
    lightTheme: cloneTheme(parsed.lightTheme),
    darkTheme: cloneTheme(parsed.darkTheme),
    fontFamily: parsed.fontFamily,
  };
}
