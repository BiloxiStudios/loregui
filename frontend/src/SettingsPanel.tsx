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
export default function SettingsPanel({ onClose }: Props) {
  const [autostart, setAutostart] = useState(false);
  const [closeToTray, setCloseToTray] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState<string | null>(null);

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
          </div>
        )}
      </div>
    </div>
  );
}
