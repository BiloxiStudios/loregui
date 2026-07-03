import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ReactNode } from "react";
import {
  applyTheme,
  cloneTheme,
  DEFAULT_THEME_SETTINGS,
  parseBundle,
  resolveIsDark,
  toBundle,
} from "./theme";
import {
  detectBridgeTheme,
  listenForBridgeTheme,
  payloadToSettings,
} from "./bridge";
import type {
  FontSize,
  SemanticTheme,
  SurfaceName,
  ThemeMode,
  ThemeSettings,
  ThemeSurface,
} from "./theme";

const STORAGE_KEY = "loregui.theme.v1";

interface ThemeContextValue {
  settings: ThemeSettings;
  /** Whether the *resolved* appearance is dark (accounts for system mode). */
  isDark: boolean;
  /** Which theme (light/dark) the editor is currently editing. */
  setMode: (mode: ThemeMode) => void;
  setFontSize: (size: FontSize) => void;
  setFontFamily: (family: string) => void;
  setCustomCSS: (css: string) => void;
  /** Patch a single slot of a single surface, on the given variant. */
  updateSurfaceSlot: (
    variant: "light" | "dark",
    surface: SurfaceName,
    slot: keyof ThemeSurface,
    value: string,
  ) => void;
  /** Replace a whole variant (used by presets / import). */
  setVariant: (variant: "light" | "dark", theme: SemanticTheme) => void;
  /** Replace the entire settings object. */
  replaceSettings: (next: ThemeSettings) => void;
  resetToDefaults: () => void;
  /** Serialize current themes to a shareable JSON bundle string. */
  exportBundle: (name: string, author?: string) => string;
  /** Trigger a file download of the current theme bundle. */
  downloadBundle: (name: string, author?: string) => void;
  /** Parse + apply an imported bundle JSON. Throws on bad input. */
  importBundle: (json: string) => void;
  /**
   * True when the active theme came from StudioBrain via the theme bridge
   * (SBAI-4605: URL fragment, studiobrain.ai cookie, or host postMessage).
   * Bridged themes are runtime-only — they are never persisted, so the
   * user's own standalone theme survives untouched.
   */
  bridgeActive: boolean;
  /** Drop the bridged StudioBrain theme and return to the local theme. */
  clearBridgeTheme: () => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

function loadSettings(): ThemeSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return structuredCloneSettings(DEFAULT_THEME_SETTINGS);
    const parsed = JSON.parse(raw) as Partial<ThemeSettings>;
    // Deep-merge against defaults so newly-added fields never crash old saves.
    return {
      mode: parsed.mode ?? DEFAULT_THEME_SETTINGS.mode,
      fontSize: parsed.fontSize ?? DEFAULT_THEME_SETTINGS.fontSize,
      fontFamily: parsed.fontFamily ?? DEFAULT_THEME_SETTINGS.fontFamily,
      customCSS: parsed.customCSS ?? DEFAULT_THEME_SETTINGS.customCSS,
      lightTheme: mergeVariant(
        DEFAULT_THEME_SETTINGS.lightTheme,
        parsed.lightTheme,
      ),
      darkTheme: mergeVariant(
        DEFAULT_THEME_SETTINGS.darkTheme,
        parsed.darkTheme,
      ),
    };
  } catch {
    return structuredCloneSettings(DEFAULT_THEME_SETTINGS);
  }
}

function mergeVariant(
  base: SemanticTheme,
  patch?: Partial<SemanticTheme>,
): SemanticTheme {
  if (!patch) return cloneTheme(base);
  const out = cloneTheme(base);
  for (const key of Object.keys(out) as SurfaceName[]) {
    if (patch[key]) out[key] = { ...out[key], ...patch[key] };
  }
  return out;
}

function structuredCloneSettings(s: ThemeSettings): ThemeSettings {
  return {
    ...s,
    lightTheme: cloneTheme(s.lightTheme),
    darkTheme: cloneTheme(s.darkTheme),
  };
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<ThemeSettings>(() => loadSettings());
  // SBAI-4605: theme handed off by StudioBrain (fragment / cookie /
  // postMessage). Runtime-only override — never written to localStorage —
  // so standalone launches keep the user's own LoreGUI theme.
  const [bridged, setBridged] = useState<ThemeSettings | null>(() => {
    const payload = detectBridgeTheme();
    return payload ? payloadToSettings(payload) : null;
  });
  const effective = bridged ?? settings;
  const [isDark, setIsDark] = useState<boolean>(() =>
    resolveIsDark(loadSettings().mode),
  );
  const firstRender = useRef(true);

  // Apply on mount + whenever the effective (bridged or local) theme changes.
  useEffect(() => {
    applyTheme(effective);
    setIsDark(resolveIsDark(effective.mode));
  }, [effective]);

  // Persist LOCAL settings only (skip the very first pass so we don't
  // rewrite identical storage on boot). Bridged themes are never persisted.
  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch {
      /* storage may be unavailable; theme still applies in-memory */
    }
  }, [settings]);

  // Live theme updates from an embedding StudioBrain host. Origin-gated in
  // listenForBridgeTheme (only *.studiobrain.ai / Tauri / Capacitor shells).
  useEffect(
    () =>
      listenForBridgeTheme((payload) => setBridged(payloadToSettings(payload))),
    [],
  );

  // React to OS dark-mode changes while in "system" mode.
  useEffect(() => {
    if (effective.mode !== "system" || !window.matchMedia) return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      applyTheme(effective);
      setIsDark(resolveIsDark(effective.mode));
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [effective]);

  const clearBridgeTheme = useCallback(() => setBridged(null), []);

  // Any explicit local theme interaction takes over from a bridged theme so
  // the user's edits are immediately visible.
  const setMode = useCallback((mode: ThemeMode) => {
    setBridged(null);
    setSettings((s) => ({ ...s, mode }));
  }, []);
  const setFontSize = useCallback((fontSize: FontSize) => {
    setBridged(null);
    setSettings((s) => ({ ...s, fontSize }));
  }, []);
  const setFontFamily = useCallback((fontFamily: string) => {
    setBridged(null);
    setSettings((s) => ({ ...s, fontFamily }));
  }, []);
  const setCustomCSS = useCallback((customCSS: string) => {
    setBridged(null);
    setSettings((s) => ({ ...s, customCSS }));
  }, []);

  const updateSurfaceSlot = useCallback(
    (
      variant: "light" | "dark",
      surface: SurfaceName,
      slot: keyof ThemeSurface,
      value: string,
    ) => {
      setBridged(null);
      setSettings((s) => {
        const key = variant === "dark" ? "darkTheme" : "lightTheme";
        const next = cloneTheme(s[key]);
        next[surface] = { ...next[surface], [slot]: value };
        return { ...s, [key]: next };
      });
    },
    [],
  );

  const setVariant = useCallback(
    (variant: "light" | "dark", theme: SemanticTheme) => {
      setBridged(null);
      setSettings((s) => ({
        ...s,
        [variant === "dark" ? "darkTheme" : "lightTheme"]: cloneTheme(theme),
      }));
    },
    [],
  );

  const replaceSettings = useCallback((next: ThemeSettings) => {
    setBridged(null);
    setSettings(structuredCloneSettings(next));
  }, []);

  const resetToDefaults = useCallback(() => {
    setBridged(null);
    setSettings(structuredCloneSettings(DEFAULT_THEME_SETTINGS));
  }, []);

  const exportBundle = useCallback(
    (name: string, author?: string) =>
      JSON.stringify(toBundle(name, settings, author), null, 2),
    [settings],
  );

  const downloadBundle = useCallback(
    (name: string, author?: string) => {
      const json = JSON.stringify(toBundle(name, settings, author), null, 2);
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      const safe = name.trim().replace(/[^a-z0-9-_]+/gi, "-") || "theme";
      a.download = `${safe}.loregui-theme.json`;
      a.click();
      URL.revokeObjectURL(url);
    },
    [settings],
  );

  const importBundle = useCallback((json: string) => {
    const bundle = parseBundle(json);
    setBridged(null);
    setSettings((s) => ({
      ...s,
      lightTheme: cloneTheme(bundle.lightTheme),
      darkTheme: cloneTheme(bundle.darkTheme),
      fontFamily: bundle.fontFamily ?? s.fontFamily,
    }));
  }, []);

  const value = useMemo<ThemeContextValue>(
    () => ({
      settings,
      isDark,
      setMode,
      setFontSize,
      setFontFamily,
      setCustomCSS,
      updateSurfaceSlot,
      setVariant,
      replaceSettings,
      resetToDefaults,
      exportBundle,
      downloadBundle,
      importBundle,
      bridgeActive: bridged !== null,
      clearBridgeTheme,
    }),
    [
      settings,
      isDark,
      bridged,
      setMode,
      setFontSize,
      setFontFamily,
      setCustomCSS,
      updateSurfaceSlot,
      setVariant,
      replaceSettings,
      resetToDefaults,
      exportBundle,
      downloadBundle,
      importBundle,
      clearBridgeTheme,
    ],
  );

  return (
    <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
  );
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used within <ThemeProvider>");
  return ctx;
}
