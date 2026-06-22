import { useCallback, useEffect, useState } from "react";
import { api } from "../../api";
import type { HostStatus } from "../../api";

type Step = "idle" | "starting" | "running" | "stopping" | "error";

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
 * Final host-flow step (SBAI-4065): launch a REAL `loreserver` over the store
 * the previous step created and show the `lore://` URL clients connect to.
 *
 * This replaces the old `serviceStart` call, which mapped to an upstream stub
 * that hosted nothing. If the previous step's store path isn't available, the
 * user can enter one manually.
 */
export default function ServiceSetup({
  storePath,
  repoName,
}: ServiceSetupProps = {}) {
  const [step, setStep] = useState<Step>("idle");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<HostStatus | null>(null);
  const [storeDir, setStoreDir] = useState(storePath ?? "");
  const [copied, setCopied] = useState(false);

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

  const handleStart = useCallback(async () => {
    if (!storeDir.trim()) return;
    try {
      setStep("starting");
      setError(null);
      const s = await api.hostServerStart({
        storeDir: storeDir.trim(),
        repositoryName: repoName,
      });
      setStatus(s);
      setStep("running");
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      setStep("error");
    }
  }, [storeDir, repoName]);

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

  const handleCopy = useCallback(async () => {
    if (!status?.url) return;
    try {
      await navigator.clipboard.writeText(status.url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      /* clipboard may be unavailable; ignore */
    }
  }, [status]);

  return (
    <div className="onboarding-card">
      <h2>Host Server</h2>
      <p className="onboarding-description">
        Start a Lore server over your store so other people can connect to it.
        The server runs on this machine and listens on <code>127.0.0.1</code>.
        Share the <code>lore://</code> URL below with your team to let them clone
        and push.
      </p>

      {error && <div className="error">{error}</div>}

      {step !== "running" && step !== "stopping" && (
        <div className="onboarding-field">
          <label htmlFor="host-store-dir">Store directory to serve</label>
          <input
            id="host-store-dir"
            type="text"
            placeholder="/path/to/shared/store"
            value={storeDir}
            onChange={(e) => setStoreDir(e.target.value)}
            disabled={step === "starting"}
          />
          <p className="onboarding-description">
            Use the same shared-store path you created on the previous step.
          </p>
        </div>
      )}

      {step === "idle" && (
        <button
          className="onboarding-button onboarding-button--primary"
          disabled={!storeDir.trim()}
          onClick={() => void handleStart()}
        >
          Start Hosting
        </button>
      )}

      {step === "starting" && (
        <button className="onboarding-button onboarding-button--primary" disabled>
          Starting&hellip;
        </button>
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
            <label htmlFor="host-url">Connection URL (give this to clients)</label>
            <div className="onboarding-url-row">
              <input id="host-url" type="text" readOnly value={status.url ?? ""} />
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
          <button
            className="onboarding-button onboarding-button--danger"
            onClick={() => void handleStop()}
          >
            Stop Hosting
          </button>
        </div>
      )}

      {step === "error" && (
        <button
          className="onboarding-button onboarding-button--primary"
          disabled={!storeDir.trim()}
          onClick={() => void handleStart()}
        >
          Retry
        </button>
      )}
    </div>
  );
}
