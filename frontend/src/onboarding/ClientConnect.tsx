import { useCallback, useState } from "react";
import { api, type UserInfo } from "../api";

type Step = "input" | "authenticating" | "success" | "error";

export default function ClientConnect() {
  const [remoteUrl, setRemoteUrl] = useState("");
  const [step, setStep] = useState<Step>("input");
  const [userInfo, setUserInfo] = useState<UserInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

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
            placeholder="https://api.studiobrain.ai"
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
