import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api";
import type { HostAdvancedOptions, HostStatus } from "../../api";
import AdvancedServerConfig from "./AdvancedServerConfig";
import PathField from "./PathField";
import { isEntitled } from "../../commercial/entitlement";
import { getRelayControl } from "../../commercial/relay-registry";
import type { StepStateProps } from "../stepResult";

type Step = "idle" | "starting" | "running" | "stopping" | "error";
type Mode = "basic" | "expert";

interface ServiceSetupProps extends StepStateProps<string> {
  /**
   * The store directory the previous step created (chosen ONCE in step 1,
   * SBAI-5560). Displayed read-only here — never re-asked. The hosted server
   * serves exactly this store so the repository just created is actually
   * reachable.
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
  onStateChange,
}: ServiceSetupProps = {}) {
  const [step, setStep] = useState<Step>("idle");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<HostStatus | null>(null);
  // The store to serve comes from step 1 verbatim (via the init step) — it is
  // displayed read-only and never edited here.
  const storeDir = storePath ?? "";
  const [copied, setCopied] = useState(false);
  const operationGeneration = useRef(0);
  const refreshGeneration = useRef(0);
  const lifecycleInFlight = useRef(false);
  const previousStorePath = useRef(storePath);

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

  useEffect(() => {
    onStateChange?.({ status: "idle" });
    return () => {
      operationGeneration.current += 1;
      refreshGeneration.current += 1;
      lifecycleInFlight.current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const invalidateOperation = useCallback(() => {
    operationGeneration.current += 1;
    refreshGeneration.current += 1;
    lifecycleInFlight.current = false;
    setStatus(null);
    setError(null);
    setStep("idle");
    onStateChange?.({ status: "idle" });
  }, [onStateChange]);

  // A new store path from the previous step invalidates any in-flight
  // operation against the old store.
  useEffect(() => {
    if (storePath !== previousStorePath.current) {
      previousStorePath.current = storePath;
      invalidateOperation();
    }
  }, [storePath, invalidateOperation]);

  // Reflect any already-running server (e.g. user navigated back and forth).
  useEffect(() => {
    let cancelled = false;
    const generation = ++operationGeneration.current;
    void api
      .hostServerStatus()
      .then((s) => {
        if (
          !cancelled &&
          generation === operationGeneration.current &&
          s.running
        ) {
          const expectedStore = storePath?.trim() ?? "";
          const runningStore = s.storeDir?.trim() ?? "";
          if (!expectedStore || runningStore !== expectedStore) {
            const message = `A Lore server is already running from ${
              runningStore || "an unknown store"
            }, not this flow's store ${
              expectedStore || "an unspecified store"
            }. Stop it before continuing.`;
            setStatus(null);
            setError(message);
            setStep("error");
            onStateChange?.({ status: "error", message });
            return;
          }
          setStatus(s);
          setStep("running");
          onStateChange?.({
            status: "success",
            value: s.advertisedUrl ?? s.url ?? "",
          });
        }
      })
      .catch(() => {
        /* status is best-effort; ignore */
      });
    return () => {
      cancelled = true;
      operationGeneration.current += 1;
    };
  }, [storePath, onStateChange]);

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
    const generation = ++operationGeneration.current;
    refreshGeneration.current += 1;
    lifecycleInFlight.current = true;
    const requestedStore = storeDir.trim();
    try {
      setStep("starting");
      setError(null);
      onStateChange?.({ status: "working" });
      const s = await api.hostServerStart(buildOptions());
      if (generation !== operationGeneration.current) return;
      lifecycleInFlight.current = false;
      const startedStore = s.storeDir?.trim() ?? "";
      if (!s.running) {
        throw new Error(
          `Lore server did not start for this flow's store ${requestedStore} (server is not running).`,
        );
      }
      if (startedStore !== requestedStore) {
        throw new Error(
          `Lore server did not start for this flow's store ${requestedStore} (reported ${
            startedStore || "an unknown store"
          }).`,
        );
      }
      setStatus(s);
      setStep("running");
      onStateChange?.({
        status: "success",
        value: s.advertisedUrl ?? s.url ?? "",
      });
    } catch (e) {
      if (generation !== operationGeneration.current) return;
      lifecycleInFlight.current = false;
      const message =
        typeof e === "string"
          ? e
          : e instanceof Error
            ? e.message
            : JSON.stringify(e);
      setError(message);
      setStep("error");
      onStateChange?.({ status: "error", message });
    }
  }, [storeDir, hasErrors, buildOptions, onStateChange]);

  const handleStop = useCallback(async () => {
    const generation = ++operationGeneration.current;
    refreshGeneration.current += 1;
    lifecycleInFlight.current = true;
    try {
      setStep("stopping");
      setError(null);
      onStateChange?.({ status: "working" });
      await api.hostServerStop();
      if (generation !== operationGeneration.current) return;
      lifecycleInFlight.current = false;
      setStatus(null);
      setStep("idle");
      onStateChange?.({ status: "idle" });
    } catch (e) {
      if (generation !== operationGeneration.current) return;
      lifecycleInFlight.current = false;
      const message =
        typeof e === "string"
          ? e
          : e instanceof Error
            ? e.message
            : JSON.stringify(e);
      setError(message);
      setStep("error");
      onStateChange?.({ status: "error", message });
    }
  }, [onStateChange]);

  const handlePreview = useCallback(async () => {
    if (!storeDir.trim()) {
      setPreviewError("No store yet — create one on the previous step first.");
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
    if (lifecycleInFlight.current) return;
    const generation = ++refreshGeneration.current;
    const lifecycleGeneration = operationGeneration.current;
    const expectedStore = storeDir.trim();
    const isCurrent = () =>
      generation === refreshGeneration.current &&
      lifecycleGeneration === operationGeneration.current &&
      !lifecycleInFlight.current;
    try {
      const s = await api.hostServerStatus();
      if (!isCurrent()) return;
      if (!s.running) {
        setStatus(null);
        setError(null);
        setStep("idle");
        onStateChange?.({ status: "idle" });
        return;
      }
      const runningStore = s.storeDir?.trim() ?? "";
      if (!expectedStore || runningStore !== expectedStore) {
        const message = `A Lore server is already running from ${
          runningStore || "an unknown store"
        }, not this flow's store ${
          expectedStore || "an unspecified store"
        }. Stop it before continuing.`;
        setStatus(null);
        setError(message);
        setStep("error");
        onStateChange?.({ status: "error", message });
        return;
      }
      setStatus(s);
      setError(null);
      setStep("running");
      onStateChange?.({
        status: "success",
        value: s.advertisedUrl ?? s.url ?? "",
      });
    } catch (e) {
      if (!isCurrent()) return;
      const message =
        typeof e === "string"
          ? e
          : e instanceof Error
            ? e.message
            : JSON.stringify(e);
      setStatus(null);
      setError(message);
      setStep("error");
      onStateChange?.({ status: "error", message });
    }
  }, [storeDir, onStateChange]);

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
              onClick={() => {
                setMode("basic");
                invalidateOperation();
              }}
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
              onClick={() => {
                setMode("expert");
                invalidateOperation();
              }}
              disabled={inputsDisabled}
            >
              Expert
            </button>
          </div>

          <PathField
            id="host-store-dir"
            label="Serving store"
            value={storeDir}
            readOnly
            hint={
              storeDir
                ? "Chosen in step 1 (Storage backend) — the server hosts exactly this store."
                : "No store yet — go back and complete the previous steps first."
            }
          />

          {mode === "expert" && (
            <AdvancedServerConfig
              value={advanced}
              bindHost={bindHost}
              onChange={(value) => {
                setAdvanced(value);
                invalidateOperation();
              }}
              onBindHostChange={(value) => {
                setBindHost(value);
                invalidateOperation();
              }}
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
