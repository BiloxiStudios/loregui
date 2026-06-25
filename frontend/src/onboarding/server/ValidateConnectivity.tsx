import { useCallback, useState } from "react";
import { api, type StorageBackendConfig } from "../../api";

type Step = "idle" | "testing" | "pass" | "fail";

interface ValidateConnectivityProps {
  /** Storage backend config provided by the onboarding shell. */
  config: StorageBackendConfig;
}

export default function ValidateConnectivity({
  config,
}: ValidateConnectivityProps) {
  const [step, setStep] = useState<Step>("idle");
  const [error, setError] = useState<string | null>(null);

  const TEST_KEY = "__lore_connectivity_check__";
  const TEST_DATA = [79, 75]; // "OK" as bytes

  const handleTest = useCallback(async () => {
    try {
      setStep("testing");
      setError(null);

      // A local-FS host store is a plain directory the loreserver fills at host
      // time — NOT a lore repository — so its connectivity check is a real
      // filesystem round-trip (write → read → delete a probe file) rather than
      // the content-store put/get/obliterate, which would require an existing
      // `.lore`. The S3 backend keeps the storage round-trip below.
      if (config.kind === "local") {
        await api.hostStoreProbe(config.path ?? "");
        setStep("pass");
        return;
      }

      // Open the configured storage backend (S3-compatible)
      await api.storageOpen(config);

      // Put a test key
      await api.storagePut(TEST_KEY, TEST_DATA);

      // Get it back and verify round-trip
      const got = await api.storageGet(TEST_KEY);
      const ok =
        got.length === TEST_DATA.length &&
        got.every((b, i) => b === TEST_DATA[i]);

      if (!ok) {
        throw new Error(
          `Round-trip mismatch: wrote [${TEST_DATA}], got [${got}]`,
        );
      }

      // Clean up the test key
      await api.storageObliterate(TEST_KEY);

      setStep("pass");
    } catch (e) {
      // Best-effort cleanup if put succeeded but get/obliterate failed (S3 only).
      if (config.kind !== "local") {
        try {
          await api.storageObliterate(TEST_KEY);
        } catch {
          // ignore — the key may not exist or storage may be unreachable
        }
      }
      const msg = typeof e === "string" ? e : JSON.stringify(e);
      setError(msg);
      setStep("fail");
    }
  }, [config]);

  const handleRetry = useCallback(() => {
    setStep("idle");
    setError(null);
  }, []);

  return (
    <div className="onboarding-card">
      <h2>Validate Backend Connectivity</h2>
      <p className="onboarding-description">
        Run a storage round-trip test (Put → Get → Obliterate) to verify the
        configured backend is reachable and functioning correctly.
      </p>

      {error && <div className="error">{error}</div>}

      {step === "idle" && (
        <button
          className="onboarding-button onboarding-button--primary"
          onClick={() => void handleTest()}
        >
          Run Connectivity Test
        </button>
      )}

      {step === "testing" && (
        <button className="onboarding-button onboarding-button--primary" disabled>
          Testing&hellip;
        </button>
      )}

      {step === "pass" && (
        <div className="onboarding-success">
          <span className="success-icon">&#10003;</span>
          <span>Storage round-trip passed — backend is reachable.</span>
          <button className="onboarding-button" onClick={handleRetry}>
            Test Again
          </button>
        </div>
      )}

      {step === "fail" && (
        <div>
          <div className="onboarding-fail">
            <span className="fail-icon">&#10007;</span>
            <span>Connectivity test failed.</span>
          </div>
          <button
            className="onboarding-button onboarding-button--primary"
            onClick={() => void handleTest()}
          >
            Retry
          </button>
        </div>
      )}
    </div>
  );
}
