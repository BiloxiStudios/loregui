import { useCallback, useEffect, useMemo, useState } from "react";
import { api, desktopSettingsApi } from "../../api";
import type { HostAdvancedOptions, HostStatus } from "../../api";
import AdvancedServerConfig from "./AdvancedServerConfig";
import { isEntitled } from "../../commercial/entitlement";
import { getRelayControl } from "../../commercial/relay-registry";

type Step = "idle" | "starting" | "running" | "stopping" | "error";
type Mode = "basic" | "expert";

interface ServiceSetupProps {
  /**
   * The store directory the previous step created. The hosted server serves
   * exactly this store so the repository just created is actually reachable.
   */
  storePath?: string;
  /** Repository name created in that store — advertised in the lore:// URL. */
  repoName?: string;
}

/**
 * Final host-flow step (SBAI-4065 / SBAI-4075): launch a REAL `loreserver` over
 * the store the previous step created and show the `lore://` URL clients connect
 * to. A Basic ↔ Expert toggle reveals the full lore-server configuration surface
 * (network, storage, topology, telemetry, runtime, notifications, features,
 * timeouts) plus a "View generated config" TOML preview.
 *
 * Basic mode produces exactly the working local config it always did — every
 * advanced field defaults to lore's own value when left untouched.
 */
export default function ServiceSetup({
  storePath,
  repoName,
}: ServiceSetupProps = {}) {
  const [step, setStep] = useState<Step>("idle");
  const [installAutorun, setInstallAutorun] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<HostStatus | null>(null);
  const [storeDir, setStoreDir] = useState(storePath ?? "");
  const [copied, setCopied] = useState(false);

  // Basic vs Expert configuration surface.
  const [mode, setMode] = useState<Mode>("basic");
  const [bindHost, setBindHost] = useState("");
  const [advanced, setAdvanced] = useState<HostAdvancedOptions>({});

  // Cross-network relay control (SBAI-4072). The open core registers nothing, so
  // `relayControl` is null and no relay UI renders. A commercial build's overlay
  // registers a control here; it is only mounted when the studio is entitled to
  // the "lore_relay" feature (otherwise a locked upsell shows).
  const relayControl = useMemo(() => getRelayControl(), []);
  const relayEntitled = relayControl ? isEntitled(relayControl.feature) : false;

  // "View generated config" preview state.
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewToml, setPreviewToml] = useState<string | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  // Keep the store-dir field in sync if the previous step reports a path.
  useEffect(() => {
    if (storePath) setStoreDir(storePath);
  }, [storePath]);

  // Reflect any already-running server (e.g. user navigated back and forth).
  useEffect(() => {
    let cancelled = false;
    void api
      .hostServerStatus()
      .then((s) => {
        if (!cancelled && s.running) {
          setStatus(s);
          setStep("running");
        }
      })
      .catch(() => {
        /* status is best-effort; ignore */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  /** Client-side validation, keyed by the same field ids the panel uses. */
  const validationErrors = useMemo(
    () => validateAdvanced(advanced),
    [advanced],
  );
  const hasErrors = Object.keys(validationErrors).length > 0;

  /** Build the full options object sent to start / preview. */
  const buildOptions = useCallback(
    () => ({
      storeDir: storeDir.trim(),
      repositoryName: repoName,
      bindHost: mode === "expert" && bindHost.trim() ? bindHost.trim() : undefined,
      advanced:
        mode === "expert" && Object.keys(advanced).length > 0
          ? advanced
          : undefined,
    }),
    [storeDir, repoName, mode, bindHost, advanced],
  );

  const handleStart = useCallback(async () => {
    if (!storeDir.trim() || hasErrors) return;
    try {
      setStep("starting");
      setError(null);
      // Start the base service with optional autorun install (SBAI-3848).
      await api.serviceStart(installAutorun);
      // Register with the desktop autostart plugin (SBAI-4043/3848).
      await desktopSettingsApi.setAutostart(installAutorun);
      // Launch the genuine standalone loreserver (SBAI-4065).
      const s = await api.hostServerStart(buildOptions());
      setStatus(s);
      setStep("running");
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      setStep("error");
    }
  }, [storeDir, hasErrors, buildOptions, installAutorun]);

  const handleStop = useCallback(async () => {
    try {
      setStep("stopping");
      setError(null);
      await api.hostServerStop();
      setStatus(null);
      setStep("idle");
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      setStep("error");
    }
  }, []);

  const handlePreview = useCallback(async () => {
    if (!storeDir.trim()) {
      setPreviewError("Enter a store directory first.");
      setPreviewOpen(true);
      return;
    }
    setPreviewOpen(true);
    setPreviewLoading(true);
    setPreviewError(null);
    try {
      const toml = await api.hostServerRenderConfig(buildOptions());
      setPreviewToml(toml);
    } catch (e) {
      setPreviewError(typeof e === "string" ? e : JSON.stringify(e));
      setPreviewToml(null);
    } finally {
      setPreviewLoading(false);
    }
  }, [storeDir, buildOptions]);

  // The URL we show clients: the relay's public `advertisedUrl` when a tunnel is
  // open (SBAI-4072), otherwise the real loopback `url`. The relay overlay sets
  // `advertisedUrl` through the core seam; with no relay it is always undefined.
  const displayUrl = status?.advertisedUrl ?? status?.url ?? "";

  // Re-fetch host status so a freshly-registered (or cleared) advertised URL is
  // reflected. Passed to the relay control so it can refresh after open/stop.
  const refreshStatus = useCallback(async () => {
    try {
      const s = await api.hostServerStatus();
      if (s.running) setStatus(s);
    } catch {
      /* best-effort; ignore */
    }
  }, []);

  const handleCopy = useCallback(async () => {
    if (!displayUrl) return;
    try {
      await navigator.clipboard.writeText(displayUrl);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      /* clipboard may be unavailable; ignore */
    }
  }, [displayUrl]);

  const editing = step === "idle" || step === "starting" || step === "error";
  const inputsDisabled = step === "starting";

  return (
    <div className="onboarding-card">
      <h2>Host Server</h2>
      <p className="onboarding-description">
        Start a Lore server over your store so other people can connect to it.
        The server runs on this machine and listens on <code>127.0.0.1</code> by
        default. Share the <code>lore://</code> URL below with your team to let
        them clone and push.
      </p>

      {error && <div className="error">{error}</div>}

      {editing && (
        <>
          <div
            className="server-config-modes"
            role="tablist"
            aria-label="Configuration detail"
          >
            <button
              type="button"
              role="tab"
              aria-selected={mode === "basic"}
              className={`server-config-mode${
                mode === "basic" ? " server-config-mode--active" : ""
              }`}
              onClick={() => setMode("basic")}
              disabled={inputsDisabled}
            >
              Basic
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={mode === "expert"}
              className={`server-config-mode${
                mode === "expert" ? " server-config-mode--active" : ""
              }`}
              onClick={() => setMode("expert")}
              disabled={inputsDisabled}
            >
              Expert
            </button>
          </div>

          <div className="onboarding-field">
            <label htmlFor="host-store-dir">Store directory to serve</label>
            <input
              id="host-store-dir"
              type="text"
              placeholder="/path/to/shared/store"
              value={storeDir}
              onChange={(e) => setStoreDir(e.target.value)}
              disabled={inputsDisabled}
            />
            <p className="onboarding-field-hint">
              Use the same shared-store path you created on the previous step.
            </p>
          </div>

          <div className="onboarding-field">
            <label className="onboarding-checkbox">
              <input
                type="checkbox"
                checked={installAutorun}
                onChange={(e) => setInstallAutorun(e.target.checked)}
                disabled={inputsDisabled}
              />
              <span>Install as Windows service / autorun on login</span>
            </label>
            <p className="onboarding-field-hint">
              Keeps the server running in the background even when LoreGUI is
              closed.
            </p>
          </div>

          {mode === "expert" && (
            <AdvancedServerConfig
              value={advanced}
              bindHost={bindHost}
              onChange={setAdvanced}
              onBindHostChange={setBindHost}
              disabled={inputsDisabled}
              errors={validationErrors}
            />
          )}

          <div className="server-config-actions">
            <button
              type="button"
              className="onboarding-button"
              onClick={() => void handlePreview()}
              disabled={inputsDisabled}
            >
              View generated config
            </button>

            {step === "idle" || step === "error" ? (
              <button
                className="onboarding-button onboarding-button--primary"
                disabled={!storeDir.trim() || hasErrors}
                onClick={() => void handleStart()}
              >
                {step === "error" ? "Retry" : "Start Hosting"}
              </button>
            ) : (
              <button
                className="onboarding-button onboarding-button--primary"
                disabled
              >
                Starting&hellip;
              </button>
            )}
          </div>

          {hasErrors && (
            <p className="server-config-field-error">
              Fix the highlighted fields before hosting.
            </p>
          )}

          {previewOpen && (
            <div className="server-config-preview">
              <div className="onboarding-url-row">
                <strong>Generated loreserver config (local.toml)</strong>
                <button
                  type="button"
                  className="onboarding-button"
                  onClick={() => setPreviewOpen(false)}
                >
                  Hide
                </button>
              </div>
              {previewLoading && (
                <p className="onboarding-field-hint">Rendering&hellip;</p>
              )}
              {previewError && <div className="error">{previewError}</div>}
              {previewToml && !previewLoading && (
                <pre aria-label="Generated config TOML">{previewToml}</pre>
              )}
            </div>
          )}
        </>
      )}

      {step === "stopping" && (
        <button className="onboarding-button" disabled>
          Stopping&hellip;
        </button>
      )}

      {step === "running" && status && (
        <div className="onboarding-success">
          <div className="success-message">
            <span className="success-icon">&#10003;</span>
            <span>Server is hosting</span>
          </div>
          <div className="onboarding-field">
            <label htmlFor="host-url">
              Connection URL (give this to clients)
              {status.advertisedUrl ? " — reachable across networks" : ""}
            </label>
            <div className="onboarding-url-row">
              <input id="host-url" type="text" readOnly value={displayUrl} />
              <button className="onboarding-button" onClick={() => void handleCopy()}>
                {copied ? "Copied" : "Copy"}
              </button>
            </div>
            <p className="onboarding-description">
              Clients run <strong>Connect to server</strong> with this URL, then
              clone the repository. The server keeps running while LoreGUI is
              open. Use <strong>Stop Hosting</strong> below to shut it down.
            </p>
          </div>

          {/* Cross-network relay (SBAI-4072). Premium + proprietary: present
              only when the loregui-cloud overlay registered a relay control. The
              open core registers nothing, so this whole block is absent. */}
          {relayControl &&
            (relayEntitled ? (
              <relayControl.component
                status={status}
                onAdvertisedUrlChange={() => void refreshStatus()}
              />
            ) : (
              <div className="onboarding-field">
                <p className="onboarding-field-hint">
                  <strong>{relayControl.label}</strong> — make this server
                  reachable across networks with no VPN. Premium add-on (locked).
                </p>
              </div>
            ))}

          <button
            className="onboarding-button onboarding-button--danger"
            onClick={() => void handleStop()}
          >
            Stop Hosting
          </button>
        </div>
      )}
    </div>
  );
}

/**
 * Client-side validation mirroring the backend's `resolve_advanced` checks, so
 * the user gets inline feedback before submitting. Returns a map of field id →
 * message; the AdvancedServerConfig panel reads it to mark/explain bad fields.
 */
function validateAdvanced(adv: HostAdvancedOptions): Record<string, string> {
  const errors: Record<string, string> = {};

  const inRange = (v: number | undefined) =>
    v === undefined || (v >= 0 && v <= 1);
  if (!inRange(adv.telemetry?.traceSampleRate)) {
    errors["telemetry.traceSampleRate"] = "Must be between 0.0 and 1.0.";
  }
  if (!inRange(adv.telemetry?.traceSampleRateLowTier)) {
    errors["telemetry.traceSampleRateLowTier"] = "Must be between 0.0 and 1.0.";
  }
  if (
    adv.telemetry?.logOutput === "file" &&
    !adv.telemetry?.logFile?.trim()
  ) {
    errors["telemetry.logFile"] = "A file path is required for file output.";
  }

  const listeners = adv.quic?.numListeners;
  if (listeners !== undefined && (listeners < 1 || listeners > 255)) {
    errors["quic.numListeners"] = "Must be between 1 and 255.";
  }

  const topo = adv.topology;
  if (topo && topo.provider && topo.provider !== "none") {
    const peers = topo.peers ?? [];
    peers.forEach((p, i) => {
      if (!p.address.trim()) {
        errors[`topology.peers.${i}.address`] = "Address is required.";
      }
    });
    if (
      topo.provider === "rotating_id_fixed" &&
      topo.rotationIntervalSeconds === undefined
    ) {
      errors["topology.rotationIntervalSeconds"] =
        "Rotation interval is required.";
    }
  }

  for (const [key, ep] of [
    ["quicInternal", adv.quicInternal],
    ["replicationEndpoint", adv.replicationEndpoint],
  ] as const) {
    if (ep?.enabled) {
      if (!ep.certFile?.trim()) {
        errors[`${key}.certFile`] = "Required when the endpoint is enabled.";
      }
      if (!ep.pkeyFile?.trim()) {
        errors[`${key}.pkeyFile`] = "Required when the endpoint is enabled.";
      }
    }
  }

  return errors;
}
