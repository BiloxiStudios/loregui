import { useCallback, useEffect, useState } from "react";
import { api, desktopSettingsApi } from "../api";

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function SettingsPanel({ open, onClose }: Props) {
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
    if (open) void loadSettings();
  }, [open, loadSettings]);

  const handleAutostart = useCallback(
    async (val: boolean) => {
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
    },
    [],
  );

  const handleCloseToTray = useCallback(
    async (val: boolean) => {
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
    },
    [],
  );

  if (!open) return null;

  return (
    <div className="settings-panel-overlay" onClick={onClose}>
      <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Desktop Settings</h2>
          <button className="settings-close" onClick={onClose}>
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
                  computer.
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
                  quitting the application.
                </p>
              </div>
              <div className="settings-row-action">
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={closeToTray}
                    onChange={(e) =>
                      void handleCloseToTray(e.target.checked)
                    }
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
