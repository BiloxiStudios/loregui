import { useCallback, useEffect, useState } from "react";
import { api, type UserInfo } from "../api";
import type { LanDiscoveredServer } from "../api";
import { isEntitled } from "../commercial/entitlement";

type Step = "input" | "authenticating" | "success" | "error";

export default function ClientConnect() {
  const [remoteUrl, setRemoteUrl] = useState("");
  const [step, setStep] = useState<Step>("input");
  const [userInfo, setUserInfo] = useState<UserInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [discovered, setDiscovered] = useState<LanDiscoveredServer[]>([]);
  const [discovering, setDiscovering] = useState(false);
  const [discoveryError, setDiscoveryError] = useState<string | null>(null);
  const lanDiscoveryEntitled = isEntitled("lanDiscovery");

  const browseLan = useCallback(async () => {
    if (!lanDiscoveryEntitled) return;
    try {
      setDiscovering(true);
      setDiscoveryError(null);
      const servers = await api.lanServerDiscoveryBrowse();
      setDiscovered(servers);
    } catch (e) {
      setDiscoveryError(typeof e === "string" ? e : JSON.stringify(e));
    } finally {
      setDiscovering(false);
    }
  }, [lanDiscoveryEntitled]);

  const handleAuth = useCallback(async () => {
    if (!remoteUrl.trim()) return;

    try {
      setStep("authenticating");
      setError(null);
      const user = await api.authLoginInteractive(remoteUrl.trim());
      setUserInfo(user);
      setStep("success");
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      setStep("error");
    }
  }, [remoteUrl]);

  const handleRetry = useCallback(() => {
    setStep("input");
    setError(null);
  }, []);

  useEffect(() => {
    if (lanDiscoveryEntitled) {
      void browseLan();
    }
  }, [lanDiscoveryEntitled]);

  return (
    <div className="onboarding-card">
      <h2>Connect to Server</h2>
      <p className="onboarding-description">
        Enter the URL of the remote StudioBrain server you want to connect to.
        You will be prompted to authenticate.
      </p>

      {error && <div className="error">{error}</div>}

      {step === "input" && (
        <div className="onboarding-field">
          <label htmlFor="remote-url">Remote Server URL</label>
          <input
            id="remote-url"
            type="text"
            placeholder="lore://192.168.1.10:41337/myrepo"
            value={remoteUrl}
            onChange={(e) => setRemoteUrl(e.target.value)}
          />
          <button
            className="onboarding-button onboarding-button--primary"
            disabled={!remoteUrl.trim()}
            onClick={() => void handleAuth()}
          >
            Connect
          </button>

          {lanDiscoveryEntitled ? (
            <>
              <p className="onboarding-description">
                Or pick a server discovered on your LAN.
              </p>
              <button
                className="onboarding-button"
                onClick={() => void browseLan()}
                disabled={discovering}
              >
                {discovering ? "Discovering..." : "Refresh LAN servers"}
              </button>
              {discoveryError && (
                <p className="onboarding-description onboarding-description--error">
                  Discovery failed: {discoveryError}
                </p>
              )}
              {discovered.length > 0 ? (
                <div className="onboarding-field">
                  <label>Discovered servers</label>
                  {discovered.map((server) => (
                    <button
                      key={server.url}
                      className="onboarding-button"
                      onClick={() => setRemoteUrl(server.url)}
                      type="button"
                    >
                      {server.name} — {server.url}
                    </button>
                  ))}
                </div>
              ) : (
                <p className="onboarding-description">
                  {discovering
                    ? "Searching your local network..."
                    : "No LoreGUI hosts found. You can still paste a lore:// URL manually."}
                </p>
              )}
            </>
          ) : (
            <p className="onboarding-description">
              LAN auto-discovery is a Premium add-on. You can always connect by entering a server URL manually.
            </p>
          )}
        </div>
      )}

      {step === "authenticating" && (
        <div className="onboarding-authenticating">
          <button className="onboarding-button onboarding-button--primary" disabled>
            Connecting&hellip;
          </button>
        </div>
      )}

      {step === "success" && userInfo && (
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
