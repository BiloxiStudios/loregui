import { useCallback, useEffect, useState } from "react";
import { desktopSettingsApi } from "./api";

interface Props {
  onClose: () => void;
}

/**
 * Desktop integration settings (SBAI-4043): start-at-login (autostart) and
 * close-to-tray. Both toggles persist immediately and revert on failure.
 * Rendered by the parent only when open, so it has no `open` prop of its own.
 */
// Third-party attribution bundles staged into the frontend at build time by
// `scripts/gen-licenses.sh` (copied to frontend/public/licenses/). Fetched
// on demand so the app can show full notices offline.
const LICENSE_BUNDLES = [
  { key: "rust", label: "Rust crates & loreserver", file: "rust.md" },
  { key: "frontend", label: "Frontend (npm)", file: "frontend.md" },
] as const;

type LicenseKey = (typeof LICENSE_BUNDLES)[number]["key"];

export default function SettingsPanel({ onClose }: Props) {
  const [autostart, setAutostart] = useState(false);
  const [closeToTray, setCloseToTray] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState<string | null>(null);
  const [licenseView, setLicenseView] = useState<LicenseKey | null>(null);
  const [licenseText, setLicenseText] = useState<string>("");
  const [licenseLoading, setLicenseLoading] = useState(false);
  const [licenseError, setLicenseError] = useState<string | null>(null);

  const openLicenses = useCallback(async (key: LicenseKey, file: string) => {
    setLicenseView(key);
    setLicenseLoading(true);
    setLicenseError(null);
    setLicenseText("");
    try {
      const res = await fetch(`./licenses/${file}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setLicenseText(await res.text());
    } catch (e) {
      setLicenseError(
        `Could not load license bundle: ${
          e instanceof Error ? e.message : String(e)
        }`,
      );
    } finally {
      setLicenseLoading(false);
    }
  }, []);

  const loadSettings = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await desktopSettingsApi.get();
      setAutostart(s.autostart_enabled);
      setCloseToTray(s.close_to_tray);
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  const handleAutostart = useCallback(async (val: boolean) => {
    setSaving("autostart");
    setError(null);
    try {
      await desktopSettingsApi.setAutostart(val);
      setAutostart(val);
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      // Revert on failure.
      setAutostart(!val);
    } finally {
      setSaving(null);
    }
  }, []);

  const handleCloseToTray = useCallback(async (val: boolean) => {
    setSaving("closeToTray");
    setError(null);
    try {
      await desktopSettingsApi.setCloseToTray(val);
      setCloseToTray(val);
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      // Revert on failure.
      setCloseToTray(!val);
    } finally {
      setSaving(null);
    }
  }, []);

  return (
    <div className="settings-panel-overlay" onClick={onClose}>
      <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Desktop Settings</h2>
          <button
            className="settings-close"
            aria-label="Close settings"
            onClick={onClose}
          >
            ✕
          </button>
        </div>

        {error && <div className="error">{error}</div>}

        {loading ? (
          <p className="settings-loading">Loading settings…</p>
        ) : (
          <div className="settings-body">
            <div className="settings-row">
              <div className="settings-row-label">
                <strong>Start LoreGUI at login</strong>
                <p className="settings-row-desc">
                  Automatically launch the application when you log in to your
                  computer. It starts hidden in the system tray.
                </p>
              </div>
              <div className="settings-row-action">
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={autostart}
                    onChange={(e) => void handleAutostart(e.target.checked)}
                    disabled={saving === "autostart"}
                  />
                  <span className="toggle-slider" />
                </label>
                {saving === "autostart" && (
                  <span className="settings-saving">Saving…</span>
                )}
              </div>
            </div>

            <div className="settings-row">
              <div className="settings-row-label">
                <strong>Close to tray</strong>
                <p className="settings-row-desc">
                  When closing the window, hide to the system tray instead of
                  quitting. Reopen anytime from the tray icon.
                </p>
              </div>
              <div className="settings-row-action">
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={closeToTray}
                    onChange={(e) => void handleCloseToTray(e.target.checked)}
                    disabled={saving === "closeToTray"}
                  />
                  <span className="toggle-slider" />
                </label>
                {saving === "closeToTray" && (
                  <span className="settings-saving">Saving…</span>
                )}
              </div>
            </div>

            <div className="settings-row">
              <div className="settings-row-label">
                <strong>About &amp; third-party licenses</strong>
                <p className="settings-row-desc">
                  LoreGUI is open source under the MIT License. It bundles
                  Epic&apos;s upstream Lore, the loreserver sidecar, and many
                  open-source Rust and npm dependencies — all under permissive
                  licenses. View their attributions below.
                </p>
              </div>
              <div className="settings-row-action settings-license-actions">
                {LICENSE_BUNDLES.map((b) => (
                  <button
                    key={b.key}
                    className="settings-license-btn"
                    onClick={() => void openLicenses(b.key, b.file)}
                  >
                    {b.label}
                  </button>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>

      {licenseView && (
        <div
          className="settings-panel-overlay"
          onClick={() => setLicenseView(null)}
        >
          <div
            className="settings-panel settings-license-modal"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="settings-header">
              <h2>
                Third-party licenses —{" "}
                {LICENSE_BUNDLES.find((b) => b.key === licenseView)?.label}
              </h2>
              <button
                className="settings-close"
                aria-label="Close licenses"
                onClick={() => setLicenseView(null)}
              >
                ✕
              </button>
            </div>
            {licenseError && <div className="error">{licenseError}</div>}
            {licenseLoading ? (
              <p className="settings-loading">Loading licenses…</p>
            ) : (
              <pre className="settings-license-text">{licenseText}</pre>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
