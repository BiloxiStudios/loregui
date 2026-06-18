import { useCallback, useState } from "react";
import { api } from "../../api";

type Step = "idle" | "starting" | "running" | "error";

export default function ServiceSetup() {
  const [step, setStep] = useState<Step>("idle");
  const [installAutorun, setInstallAutorun] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const handleStart = useCallback(async () => {
    try {
      setStep("starting");
      setError(null);
      await api.serviceStart(installAutorun);
      setStep("running");
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      setStep("error");
    }
  }, [installAutorun]);

  const handleReset = useCallback(() => {
    setStep("idle");
    setError(null);
  }, []);

  return (
    <div className="onboarding-card">
      <h2>Start Lore Service</h2>
      <p className="onboarding-description">
        The Lore service handles repository synchronization and background operations.
        Start the service now to continue setup.
      </p>

      {step !== "running" && (
        <label className="onboarding-checkbox">
          <input
            type="checkbox"
            checked={installAutorun}
            onChange={(e) => setInstallAutorun(e.target.checked)}
            disabled={step === "starting"}
          />
          <span>Install as Windows service / autorun on login</span>
        </label>
      )}

      {error && <div className="error">{error}</div>}

      {step === "idle" && (
        <button
          className="onboarding-button onboarding-button--primary"
          onClick={handleStart}
        >
          Start Service
        </button>
      )}

      {step === "starting" && (
        <button className="onboarding-button onboarding-button--primary" disabled>
          Starting&hellip;
        </button>
      )}

      {step === "running" && (
        <div className="onboarding-success">
          <span className="success-icon">&#10003;</span>
          <span>Service is running</span>
          <button className="onboarding-button" onClick={handleReset}>
            Back
          </button>
        </div>
      )}

      {step === "error" && (
        <button
          className="onboarding-button onboarding-button--primary"
          onClick={handleStart}
        >
          Retry
        </button>
      )}
    </div>
  );
}
