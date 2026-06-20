import { useCallback, useState } from "react";
import { api, type StorageBackendConfig } from "../../api";

type BackendKind = "local" | "s3" | "minio" | "garage";
type Step = "configuring" | "connecting" | "connected" | "error";

const BACKEND_KINDS: { value: BackendKind; label: string }[] = [
  { value: "local", label: "Local Filesystem" },
  { value: "s3", label: "Amazon S3" },
  { value: "minio", label: "MinIO" },
  { value: "garage", label: "Garage" },
];

export default function BackendConfig() {
  const [step, setStep] = useState<Step>("configuring");
  const [kind, setKind] = useState<BackendKind>("local");
  const [error, setError] = useState<string | null>(null);

  // Local backend fields
  const [path, setPath] = useState("");

  // Remote backend fields (s3/minio/garage)
  const [endpoint, setEndpoint] = useState("");
  const [bucket, setBucket] = useState("");
  const [region, setRegion] = useState("");
  const [accessKeyId, setAccessKeyId] = useState("");
  const [secretAccessKey, setSecretAccessKey] = useState("");

  // Mutable KV store location (all backends)
  const [mutableStore, setMutableStore] = useState("");

  const buildConfig = useCallback((): StorageBackendConfig => {
    if (kind === "local") {
      return {
        kind: "local",
        path: path.trim() || undefined,
        mutableStore: mutableStore.trim() || undefined,
      };
    }
    return {
      kind,
      endpoint: endpoint.trim() || undefined,
      bucket: bucket.trim() || undefined,
      region: region.trim() || undefined,
      accessKeyId: accessKeyId.trim() || undefined,
      secretAccessKey: secretAccessKey.trim() || undefined,
      mutableStore: mutableStore.trim() || undefined,
    };
  }, [kind, path, endpoint, bucket, region, accessKeyId, secretAccessKey, mutableStore]);

  const isFormValid = useCallback((): boolean => {
    if (kind === "local") {
      return path.trim().length > 0;
    }
    return endpoint.trim().length > 0 && bucket.trim().length > 0;
  }, [kind, path, endpoint, bucket]);

  const handleConnect = useCallback(async () => {
    if (!isFormValid()) return;

    try {
      setStep("connecting");
      setError(null);
      const config = buildConfig();
      await api.storageOpen(config);
      setStep("connected");
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      setStep("error");
    }
  }, [isFormValid, buildConfig]);

  const handleReset = useCallback(() => {
    setStep("configuring");
    setError(null);
  }, []);

  const isRemote = kind !== "local";

  return (
    <div className="onboarding-card">
      <h2>Configure Backend Storage</h2>
      <p className="onboarding-description">
        Choose a storage backend for your Lore repositories. The backend stores
        packfiles and revision data. A separate mutable KV store holds branch
        pointers and bookkeeping data.
      </p>

      {step !== "connected" && (
        <>
          <div className="onboarding-field">
            <label htmlFor="backend-kind">Storage Type</label>
            <select
              id="backend-kind"
              value={kind}
              onChange={(e) => setKind(e.target.value as BackendKind)}
              disabled={step === "connecting"}
            >
              {BACKEND_KINDS.map((k) => (
                <option key={k.value} value={k.value}>
                  {k.label}
                </option>
              ))}
            </select>
          </div>

          {kind === "local" ? (
            <div className="onboarding-field">
              <label htmlFor="local-path">Packfiles Path</label>
              <input
                id="local-path"
                type="text"
                placeholder="/path/to/packfiles"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                disabled={step === "connecting"}
              />
            </div>
          ) : (
            <>
              <div className="onboarding-field">
                <label htmlFor="endpoint">Endpoint</label>
                <input
                  id="endpoint"
                  type="text"
                  placeholder="https://s3.amazonaws.com"
                  value={endpoint}
                  onChange={(e) => setEndpoint(e.target.value)}
                  disabled={step === "connecting"}
                />
              </div>
              <div className="onboarding-field">
                <label htmlFor="bucket">Bucket</label>
                <input
                  id="bucket"
                  type="text"
                  placeholder="lore-storage"
                  value={bucket}
                  onChange={(e) => setBucket(e.target.value)}
                  disabled={step === "connecting"}
                />
              </div>
              <div className="onboarding-field">
                <label htmlFor="region">Region</label>
                <input
                  id="region"
                  type="text"
                  placeholder="us-east-1"
                  value={region}
                  onChange={(e) => setRegion(e.target.value)}
                  disabled={step === "connecting"}
                />
              </div>
              <div className="onboarding-field">
                <label htmlFor="access-key-id">Access Key ID</label>
                <input
                  id="access-key-id"
                  type="text"
                  placeholder="AKIA..."
                  value={accessKeyId}
                  onChange={(e) => setAccessKeyId(e.target.value)}
                  disabled={step === "connecting"}
                />
              </div>
              <div className="onboarding-field">
                <label htmlFor="secret-access-key">Secret Access Key</label>
                <input
                  id="secret-access-key"
                  type="password"
                  placeholder="••••••••"
                  value={secretAccessKey}
                  onChange={(e) => setSecretAccessKey(e.target.value)}
                  disabled={step === "connecting"}
                />
              </div>
            </>
          )}

          <div className="onboarding-field">
            <label htmlFor="mutable-store">Mutable KV Store Path {isRemote && "(Optional)"}</label>
            <input
              id="mutable-store"
              type="text"
              placeholder={isRemote ? "/path/to/kv-store (optional)" : "/path/to/kv-store"}
              value={mutableStore}
              onChange={(e) => setMutableStore(e.target.value)}
              disabled={step === "connecting"}
            />
          </div>

          {error && <div className="error">{error}</div>}

          {step === "error" && (
            <button
              className="onboarding-button onboarding-button--primary"
              onClick={handleReset}
            >
              Back to Form
            </button>
          )}

          {step === "configuring" && (
            <button
              className="onboarding-button onboarding-button--primary"
              disabled={!isFormValid()}
              onClick={() => void handleConnect()}
            >
              Connect
            </button>
          )}

          {step === "connecting" && (
            <button className="onboarding-button onboarding-button--primary" disabled>
              Connecting&hellip;
            </button>
          )}
        </>
      )}

      {step === "connected" && (
        <div className="onboarding-success">
          <span className="success-icon">&#10003;</span>
          <span>Backend storage connected</span>
          <button className="onboarding-button" onClick={handleReset}>
            Back
          </button>
        </div>
      )}
    </div>
  );
}
