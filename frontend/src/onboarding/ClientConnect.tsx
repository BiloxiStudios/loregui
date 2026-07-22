import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  isNoAuthConfigured,
  LAN_DISCOVERED_EVENT,
  type DiscoveredServer,
  type UserInfo,
} from "../api";
import { listen } from "@tauri-apps/api/event";
import type { StepStateProps } from "./stepResult";

type Step = "input" | "authenticating" | "success" | "error";
type DiscoveryStep = "loading" | "ready" | "error";
type ConnectionMode = "authenticated" | "no-auth";

/**
 * Connect-to-server onboarding (SBAI-3841 + SBAI-4073).
 *
 * Two ways in:
 *   1. "Servers on your network" — LAN auto-discovery (mDNS). Lists lore servers
 *      hosted by peers on the same network; one click prefills the `lore://` URL.
 *      Open-core, not gated. Themed via `--surface-*`; loading/empty/error states.
 *   2. Manual URL — the always-available fallback for remote servers or when mDNS
 *      is blocked (firewall / VPN / corporate switch dropping multicast).
 */
export default function ClientConnect({
  onStateChange,
}: StepStateProps<string> = {}) {
  const [remoteUrl, setRemoteUrl] = useState("");
  const [step, setStep] = useState<Step>("input");
  const [userInfo, setUserInfo] = useState<UserInfo | null>(null);
  const [connectionMode, setConnectionMode] = useState<ConnectionMode | null>(null);
  const [error, setError] = useState<string | null>(null);

  // --- LAN discovery state ---
  const [servers, setServers] = useState<DiscoveredServer[]>([]);
  const [discovery, setDiscovery] = useState<DiscoveryStep>("loading");
  const [discoveryError, setDiscoveryError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const errMsg = (e: unknown): string =>
    typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);

  // Start (or reuse) a LAN browse on mount; subscribe to live updates; stop the
  // browse on unmount so we are not discovering for the whole app lifetime.
  useEffect(() => {
    onStateChange?.({ status: "idle" });
    // Each mount is a fresh attempt. Subsequent transitions are reported at
    // the same point as the backend state changes below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    let unlisten: undefined | (() => void);
    let cancelled = false;
    void (async () => {
      try {
        setDiscovery("loading");
        setDiscoveryError(null);
        const initial = await api.lanDiscoverBrowse();
        if (!cancelled) {
          setServers(initial);
          setDiscovery("ready");
        }
        unlisten = await listen<DiscoveredServer[]>(
          LAN_DISCOVERED_EVENT,
          (event) => {
            setServers(event.payload);
            setDiscovery("ready");
          },
        );
      } catch (e) {
        if (!cancelled) {
          setDiscoveryError(errMsg(e));
          setDiscovery("error");
        }
      }
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
      // Best-effort: stop the browse when leaving the connect flow.
      void api.lanDiscoverStop().catch(() => {});
    };
  }, []);

  const refreshDiscovery = useCallback(async () => {
    try {
      setDiscovery("loading");
      setDiscoveryError(null);
      const list = await api.lanDiscoverBrowse();
      setServers(list);
      setDiscovery("ready");
    } catch (e) {
      setDiscoveryError(errMsg(e));
      setDiscovery("error");
    }
  }, []);

  const pickServer = useCallback((server: DiscoveredServer) => {
    // One-click connect prefills the lore:// URL; the user confirms with Connect
    // (so they can review the target before authenticating).
    setRemoteUrl(server.url);
    setStep("input");
    setError(null);
    onStateChange?.({ status: "idle" });
    inputRef.current?.focus();
  }, [onStateChange]);

  const handleAuth = useCallback(async () => {
    if (!remoteUrl.trim()) return;

    const selectedUrl = remoteUrl.trim();
    try {
      setStep("authenticating");
      setError(null);
      onStateChange?.({ status: "working" });
      const user = await api.authLoginInteractive(selectedUrl);
      setUserInfo(user);
      setConnectionMode("authenticated");
      setStep("success");
      onStateChange?.({ status: "success", value: selectedUrl });
    } catch (e) {
      // Epic Lore uses this exact result after successfully contacting a server
      // whose administrator intentionally disabled authentication. Connectivity
      // succeeded; there is no identity to synthesize or token flow to retry.
      if (isNoAuthConfigured(e)) {
        setUserInfo(null);
        setConnectionMode("no-auth");
        setStep("success");
        onStateChange?.({ status: "success", value: selectedUrl });
        return;
      }
      const message = errMsg(e);
      setError(message);
      setStep("error");
      onStateChange?.({ status: "error", message });
    }
  }, [remoteUrl, onStateChange]);

  const handleRetry = useCallback(() => {
    setStep("input");
    setError(null);
    setConnectionMode(null);
    onStateChange?.({ status: "idle" });
  }, [onStateChange]);

  return (
    <div className="onboarding-card">
      <h2>Connect to Server</h2>
      <p className="onboarding-description">
        Pick a server discovered on your network, or enter the URL of a remote
        StudioBrain server. You will be prompted to authenticate when required.
      </p>

      {error && <div className="error">{error}</div>}

      {/* --- Servers on your network (LAN auto-discovery, SBAI-4073) --- */}
      <section className="lan-discovery" aria-label="Servers on your network">
        <div className="lan-discovery-header">
          <h3>Servers on your network</h3>
          <button
            type="button"
            className="lan-discovery-refresh"
            onClick={() => void refreshDiscovery()}
            disabled={discovery === "loading"}
            title="Re-scan the local network for lore servers"
          >
            {discovery === "loading" ? "Scanning…" : "Refresh"}
          </button>
        </div>

        {discovery === "error" ? (
          <div className="error lan-discovery-error">
            Network discovery unavailable: {discoveryError}. You can still connect
            using the server URL below.
          </div>
        ) : discovery === "loading" && servers.length === 0 ? (
          <p className="empty lan-discovery-empty">Scanning your network…</p>
        ) : servers.length === 0 ? (
          <p className="empty lan-discovery-empty">
            No servers found yet. Make sure a host is running on your network, or
            enter a server URL below.
          </p>
        ) : (
          <ul className="lan-discovery-list">
            {servers.map((server) => (
              <li key={server.id} className="lan-discovery-item">
                <div className="lan-discovery-meta">
                  <span className="lan-discovery-name">{server.name}</span>
                  <span className="lan-discovery-sub">
                    {server.repo ? `${server.repo} · ` : ""}
                    {server.host}
                    {server.port ? `:${server.port}` : ""}
                  </span>
                </div>
                <button
                  type="button"
                  className="onboarding-button onboarding-button--primary lan-discovery-connect"
                  onClick={() => pickServer(server)}
                  title={`Use ${server.url}`}
                >
                  Connect
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      {step === "input" && (
        <div className="onboarding-field">
          <label htmlFor="remote-url">Remote Server URL</label>
          <input
            id="remote-url"
            ref={inputRef}
            type="text"
            placeholder="lore://host:port/repo or https://api.studiobrain.ai"
            value={remoteUrl}
            onChange={(e) => {
              setRemoteUrl(e.target.value);
              onStateChange?.({ status: "idle" });
            }}
          />
          <button
            className="onboarding-button onboarding-button--primary"
            disabled={!remoteUrl.trim()}
            onClick={() => void handleAuth()}
          >
            Connect
          </button>
        </div>
      )}

      {step === "authenticating" && (
        <div className="onboarding-authenticating">
          <button className="onboarding-button onboarding-button--primary" disabled>
            Connecting&hellip;
          </button>
        </div>
      )}

      {step === "success" && connectionMode === "no-auth" && (
        <div className="onboarding-success">
          <div className="success-message">
            <span className="success-icon">&#10003;</span>
            <span>Connected without authentication</span>
          </div>
        </div>
      )}

      {step === "success" && connectionMode === "authenticated" && userInfo && (
        <div className="onboarding-success">
          <div className="success-message">
            <span className="success-icon">&#10003;</span>
            <span>Connected as:</span>
          </div>
          <div className="user-info">
            <div className="user-info-field">
              <span className="user-info-label">Name:</span>
              <span className="user-info-value">{userInfo.name}</span>
            </div>
            <div className="user-info-field">
              <span className="user-info-label">ID:</span>
              <span className="user-info-value code">{userInfo.id}</span>
            </div>
          </div>
        </div>
      )}

      {step === "error" && (
        <button
          className="onboarding-button onboarding-button--primary"
          onClick={handleRetry}
        >
          Retry
        </button>
      )}
    </div>
  );
}
