import { useState } from "react";
import type { CSSProperties } from "react";
import { useTheme } from "./ThemeProvider";
import {
  PRESET_THEMES,
  SURFACE_META,
  SURFACE_NAMES,
} from "./theme";
import type {
  FontSize,
  SemanticTheme,
  SurfaceName,
  ThemeMode,
  ThemeSurface,
} from "./theme";

// ---------------------------------------------------------------------------
// Shared inline style helpers (the editor themes itself via CSS variables).
// ---------------------------------------------------------------------------

const containerStyle: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 20,
  maxWidth: 720,
  padding: 20,
  maxHeight: "100%",
  overflowY: "auto",
  background: "var(--surface-overlay-bg)",
  color: "var(--surface-overlay-text)",
  border: "1px solid var(--surface-overlay-border)",
  borderRadius: 10,
  boxShadow: "var(--surface-overlay-shadow)",
  fontFamily: "var(--font-family)",
  fontSize: "var(--base-font-size)",
  boxSizing: "border-box",
};

const sectionStyle: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 10,
  padding: 14,
  background: "var(--surface-elevated-bg)",
  color: "var(--surface-elevated-text)",
  border: "1px solid var(--surface-elevated-border)",
  borderRadius: 8,
};

const sectionTitleStyle: CSSProperties = {
  margin: 0,
  fontSize: "1.05em",
  fontWeight: 600,
};

const mutedStyle: CSSProperties = {
  color: "var(--surface-base-text-secondary)",
  fontSize: "0.85em",
};

const labelStyle: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 4,
  fontSize: "0.85em",
  fontWeight: 500,
};

const textInputStyle: CSSProperties = {
  background: "var(--surface-input-bg)",
  color: "var(--surface-input-text)",
  border: "1px solid var(--surface-input-border)",
  borderRadius: 6,
  padding: "6px 8px",
  fontFamily: "inherit",
  fontSize: "0.9em",
  boxSizing: "border-box",
};

const colorInputStyle: CSSProperties = {
  width: 38,
  height: 32,
  padding: 0,
  border: "1px solid var(--surface-input-border)",
  borderRadius: 6,
  background: "var(--surface-input-bg)",
  cursor: "pointer",
  flexShrink: 0,
};

const primaryButtonStyle: CSSProperties = {
  background: "var(--surface-primary-bg)",
  color: "var(--surface-primary-text)",
  border: "1px solid var(--surface-primary-border)",
  borderRadius: 6,
  padding: "7px 12px",
  cursor: "pointer",
  fontFamily: "inherit",
  fontSize: "0.9em",
  fontWeight: 500,
};

const secondaryButtonStyle: CSSProperties = {
  background: "var(--surface-secondary-bg)",
  color: "var(--surface-secondary-text)",
  border: "1px solid var(--surface-secondary-border)",
  borderRadius: 6,
  padding: "7px 12px",
  cursor: "pointer",
  fontFamily: "inherit",
  fontSize: "0.9em",
};

const rowStyle: CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: 8,
  alignItems: "center",
};

const errorStyle: CSSProperties = {
  background: "var(--surface-error-bg)",
  color: "var(--surface-error-text)",
  border: "1px solid var(--surface-error-border)",
  borderRadius: 6,
  padding: "6px 8px",
  fontSize: "0.85em",
};

/** The seven slots, ordered. Color-like slots get a color picker; shadow is text-only. */
const COLOR_SLOTS = [
  "background",
  "text",
  "textSecondary",
  "border",
  "hover",
  "active",
] as const;

const SLOT_LABELS: Record<keyof ThemeSurface, string> = {
  background: "Background",
  text: "Text",
  textSecondary: "Text (secondary)",
  border: "Border",
  hover: "Hover",
  active: "Active",
  shadow: "Shadow",
};

/** Whether a value is a usable #hex that the native color input can adopt. */
function isHexColor(value: string): boolean {
  return /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.test(value.trim());
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface SlotRowProps {
  variant: "light" | "dark";
  surface: SurfaceName;
  slot: keyof ThemeSurface;
  value: string;
  onChange: (value: string) => void;
}

function SlotRow({ slot, value, onChange }: SlotRowProps) {
  const isColor = (COLOR_SLOTS as readonly string[]).includes(slot);
  return (
    <label style={labelStyle}>
      <span>{SLOT_LABELS[slot]}</span>
      <div style={rowStyle}>
        {isColor && (
          <input
            type="color"
            style={colorInputStyle}
            // The text value is the source of truth; the color input is a
            // convenience that only reflects a valid #hex.
            value={isHexColor(value) ? value : "#000000"}
            onChange={(e) => onChange(e.target.value)}
            aria-label={`${SLOT_LABELS[slot]} color picker`}
          />
        )}
        <input
          type="text"
          style={{ ...textInputStyle, flex: 1, minWidth: 120 }}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          spellCheck={false}
          aria-label={`${SLOT_LABELS[slot]} value`}
        />
      </div>
    </label>
  );
}

interface SurfaceSectionProps {
  variant: "light" | "dark";
  surface: SurfaceName;
  theme: SemanticTheme;
  onSlotChange: (slot: keyof ThemeSurface, value: string) => void;
}

function SurfaceSection({
  variant,
  surface,
  theme,
  onSlotChange,
}: SurfaceSectionProps) {
  const [open, setOpen] = useState(false);
  const meta = SURFACE_META[surface];
  const slots = theme[surface];

  const previewStyle: CSSProperties = {
    marginLeft: "auto",
    width: 64,
    height: 28,
    borderRadius: 6,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    fontSize: "0.7em",
    fontWeight: 600,
    backgroundColor: slots.background,
    color: slots.text,
    border: `1px solid ${slots.border}`,
  };

  return (
    <div style={sectionStyle}>
      <div
        style={{ ...rowStyle, cursor: "pointer", flexWrap: "nowrap" }}
        onClick={() => setOpen((o) => !o)}
      >
        <span style={{ fontWeight: 600 }}>
          {open ? "▾" : "▸"} {meta.label}
        </span>
        <span style={mutedStyle}>{meta.description}</span>
        <div style={previewStyle}>Aa</div>
      </div>
      {open && (
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
            gap: 10,
          }}
        >
          {(["background", "text", "textSecondary", "border", "hover", "active", "shadow"] as (keyof ThemeSurface)[]).map(
            (slot) => (
              <SlotRow
                key={slot}
                variant={variant}
                surface={surface}
                slot={slot}
                value={slots[slot]}
                onChange={(value) => onSlotChange(slot, value)}
              />
            ),
          )}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function ThemeEditor() {
  const {
    settings,
    isDark,
    setMode,
    setFontSize,
    setFontFamily,
    setCustomCSS,
    updateSurfaceSlot,
    replaceSettings,
    resetToDefaults,
    exportBundle,
    downloadBundle,
    importBundle,
  } = useTheme();

  const [variant, setVariantTab] = useState<"light" | "dark">(
    isDark ? "dark" : "light",
  );
  const [themeName, setThemeName] = useState("My Theme");
  const [importText, setImportText] = useState("");
  const [importError, setImportError] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<string | null>(null);

  const editedTheme: SemanticTheme =
    variant === "dark" ? settings.darkTheme : settings.lightTheme;

  function handleCopy() {
    setImportError(null);
    const json = exportBundle(themeName.trim() || "My Theme");
    if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
      navigator.clipboard
        .writeText(json)
        .then(() => setCopyStatus("Copied JSON to clipboard"))
        .catch(() => setCopyStatus("Clipboard unavailable"));
    } else {
      setCopyStatus("Clipboard unavailable");
    }
  }

  function handleApplyImport() {
    setImportError(null);
    setCopyStatus(null);
    try {
      importBundle(importText);
    } catch (err) {
      setImportError(err instanceof Error ? err.message : String(err));
    }
  }

  const tabStyle = (active: boolean): CSSProperties => ({
    ...(active ? primaryButtonStyle : secondaryButtonStyle),
    fontWeight: active ? 600 : 500,
  });

  return (
    <div style={containerStyle}>
      <h2 style={{ margin: 0, fontSize: "1.3em" }}>Theme Editor</h2>

      {/* Variant tabs */}
      <div style={rowStyle}>
        <span style={mutedStyle}>Editing variant:</span>
        <button
          type="button"
          style={tabStyle(variant === "light")}
          onClick={() => setVariantTab("light")}
        >
          Light
        </button>
        <button
          type="button"
          style={tabStyle(variant === "dark")}
          onClick={() => setVariantTab("dark")}
        >
          Dark
        </button>
      </div>

      {/* Mode selector */}
      <div style={sectionStyle}>
        <h3 style={sectionTitleStyle}>Appearance Mode</h3>
        <p style={mutedStyle}>
          Controls which variant is shown live (system follows the OS).
        </p>
        <div style={rowStyle}>
          {(["light", "dark", "system"] as ThemeMode[]).map((m) => (
            <button
              type="button"
              key={m}
              style={tabStyle(settings.mode === m)}
              onClick={() => setMode(m)}
            >
              {m[0].toUpperCase() + m.slice(1)}
            </button>
          ))}
        </div>
      </div>

      {/* Surfaces */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <h3 style={sectionTitleStyle}>Surfaces ({variant})</h3>
        {SURFACE_NAMES.map((surface) => (
          <SurfaceSection
            key={surface}
            variant={variant}
            surface={surface}
            theme={editedTheme}
            onSlotChange={(slot, value) =>
              updateSurfaceSlot(variant, surface, slot, value)
            }
          />
        ))}
      </div>

      {/* Typography */}
      <div style={sectionStyle}>
        <h3 style={sectionTitleStyle}>Typography</h3>
        <div style={rowStyle}>
          <span style={mutedStyle}>Font size:</span>
          {(["small", "medium", "large"] as FontSize[]).map((size) => (
            <button
              type="button"
              key={size}
              style={tabStyle(settings.fontSize === size)}
              onClick={() => setFontSize(size)}
            >
              {size[0].toUpperCase() + size.slice(1)}
            </button>
          ))}
        </div>
        <label style={labelStyle}>
          <span>Font family</span>
          <input
            type="text"
            style={textInputStyle}
            value={settings.fontFamily}
            onChange={(e) => setFontFamily(e.target.value)}
            spellCheck={false}
          />
        </label>
      </div>

      {/* Presets */}
      <div style={sectionStyle}>
        <h3 style={sectionTitleStyle}>Presets</h3>
        <div style={rowStyle}>
          {PRESET_THEMES.map((preset) => (
            <button
              type="button"
              key={preset.name}
              style={secondaryButtonStyle}
              onClick={() => replaceSettings(preset.settings())}
            >
              {preset.name}
            </button>
          ))}
          <button
            type="button"
            style={secondaryButtonStyle}
            onClick={() => resetToDefaults()}
          >
            Reset to defaults
          </button>
        </div>
      </div>

      {/* Custom CSS */}
      <div style={sectionStyle}>
        <h3 style={sectionTitleStyle}>Custom CSS</h3>
        <p style={mutedStyle}>Appended last; for power users.</p>
        <textarea
          style={{ ...textInputStyle, minHeight: 90, resize: "vertical" }}
          value={settings.customCSS}
          onChange={(e) => setCustomCSS(e.target.value)}
          spellCheck={false}
          placeholder=":root { /* overrides */ }"
        />
      </div>

      {/* Share / Save */}
      <div style={sectionStyle}>
        <h3 style={sectionTitleStyle}>Share &amp; Save</h3>
        <label style={labelStyle}>
          <span>Theme name</span>
          <input
            type="text"
            style={textInputStyle}
            value={themeName}
            onChange={(e) => {
              setThemeName(e.target.value);
              setCopyStatus(null);
            }}
            spellCheck={false}
          />
        </label>
        <div style={rowStyle}>
          <button
            type="button"
            style={primaryButtonStyle}
            onClick={() => downloadBundle(themeName.trim() || "My Theme")}
          >
            Export / Download
          </button>
          <button type="button" style={secondaryButtonStyle} onClick={handleCopy}>
            Copy JSON
          </button>
        </div>
        {copyStatus && <p style={mutedStyle}>{copyStatus}</p>}

        <label style={labelStyle}>
          <span>Import (paste theme JSON)</span>
          <textarea
            style={{ ...textInputStyle, minHeight: 90, resize: "vertical" }}
            value={importText}
            onChange={(e) => {
              setImportText(e.target.value);
              setImportError(null);
            }}
            spellCheck={false}
            placeholder='{ "kind": "loregui-theme", ... }'
          />
        </label>
        <div style={rowStyle}>
          <button
            type="button"
            style={primaryButtonStyle}
            onClick={handleApplyImport}
          >
            Apply import
          </button>
        </div>
        {importError && <div style={errorStyle}>{importError}</div>}
      </div>
    </div>
  );
}
