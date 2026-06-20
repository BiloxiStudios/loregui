import { useCallback, useState } from "react";
import { api, type StorageBackendConfig } from "../../api";

type BackendKind = "local" | "s3" | "minio" | "garage";
type Step = "idle" | "connecting" | "success" | "error";

interface FormState {
  // local
  path: string;
  // object storage
  endpoint: string;
  bucket: string;
  region: string;
  accessKeyId: string;
  secretAccessKey: string;
  // optional mutable store
  mutableStore: string;
}

const EMPTY_FORM: FormState = {
  path: "",
  endpoint: "",
  bucket: "",
  region: "",
  accessKeyId: "",
  secretAccessKey: "",
  mutableStore: "",
};

function isObjectStorage(kind: BackendKind): boolean {
  return kind !== "local";
}

interface BackendPickerProps {
  /**
   * Called with the validated config once the backend opens successfully.
   * Lets the onboarding shell forward the config to later steps
   * (e.g. connectivity validation) without re-entering it.
   */
  onConfigured?: (config: StorageBackendConfig) => void;
}

export default function BackendPicker({ onConfigured }: BackendPickerProps = {}) {
  const [kind, setKind] = useState<BackendKind>("local");
  const [form, setForm] = useState<FormState>({ ...EMPTY_FORM });
  const [step, setStep] = useState<Step>("idle");
  const [error, setError] = useState<string | null>(null);

  const updateField = useCallback(
    (field: keyof FormState) =>
      (e: React.ChangeEvent<HTMLInputElement>) => {
        setForm((prev) => ({ ...prev, [field]: e.target.value }));
      },
    [],
  );

  const isValid = useCallback((): boolean => {
    if (kind === "local") {
      return form.path.trim().length > 0;
    }
    // object storage: endpoint + bucket required
    return (
      form.endpoint.trim().length > 0 && form.bucket.trim().length > 0
    );
  }, [kind, form]);

  const buildConfig = useCallback((): StorageBackendConfig => {
    if (kind === "local") {
      return {
        kind: "local",
        path: form.path.trim() || undefined,
        mutableStore: form.mutableStore.trim() || undefined,
      };
    }
    return {
      kind,
      endpoint: form.endpoint.trim() || undefined,
      bucket: form.bucket.trim() || undefined,
      region: form.region.trim() || undefined,
      accessKeyId: form.accessKeyId.trim() || undefined,
      secretAccessKey: form.secretAccessKey.trim() || undefined,
      mutableStore: form.mutableStore.trim() || undefined,
    };
  }, [kind, form]);

  const handleConnect = useCallback(async () => {
    if (!isValid()) return;

    try {
      setStep("connecting");
      setError(null);
      const config = buildConfig();
      await api.storageOpen(config);
      setStep("success");
      onConfigured?.(config);
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      setStep("error");
    }
  }, [isValid, buildConfig, onConfigured]);

  const handleReset = useCallback(() => {
    setStep("idle");
    setError(null);
    setForm({ ...EMPTY_FORM });
  }, []);

  return (
    <div className="onboarding-card">
      <h2>Choose Storage Backend</h2>
      <p className="onboarding-description">
        Pick where your Lore data will be stored. Local packfiles are simplest
        for single-user setups. Object storage (S3, MinIO, Garage) scales to teams.
      </p>

      {/* Backend type selector */}
      {step !== "success" && (
        <div className="onboarding-radio-group">
          <label
            className={`onboarding-radio ${
              kind === "local" ? "onboarding-radio--selected" : ""
            }`}
          >
            <input
              type="radio"
              name="backend-kind"
              value="local"
              checked={kind === "local"}
              onChange={() => setKind("local")}
              disabled={step === "connecting"}
            />
            <span className="onboarding-radio-label">Local Packfiles</span>
            <span className="onboarding-radio-desc">
              Store data in local directories. Simple, no external services needed.
            </span>
          </label>

          <label
            className={`onboarding-radio ${
              kind === "s3" ? "onboarding-radio--selected" : ""
            }`}
          >
            <input
              type="radio"
              name="backend-kind"
              value="s3"
              checked={kind === "s3"}
              onChange={() => setKind("s3")}
              disabled={step === "connecting"}
            />
            <span className="onboarding-radio-label">Amazon S3</span>
            <span className="onboarding-radio-desc">
              Managed object storage on AWS.
            </span>
          </label>

          <label
            className={`onboarding-radio ${
              kind === "minio" ? "onboarding-radio--selected" : ""
            }`}
          >
            <input
              type="radio"
              name="backend-kind"
              value="minio"
              checked={kind === "minio"}
              onChange={() => setKind("minio")}
              disabled={step === "connecting"}
            />
            <span className="onboarding-radio-label">MinIO</span>
            <span className="onboarding-radio-desc">
              Self-hosted S3-compatible object storage.
            </span>
          </label>

          <label
            className={`onboarding-radio ${
              kind === "garage" ? "onboarding-radio--selected" : ""
            }`}
          >
            <input
              type="radio"
              name="backend-kind"
              value="garage"
              checked={kind === "garage"}
              onChange={() => setKind("garage")}
              disabled={step === "connecting"}
            />
            <span className="onboarding-radio-label">Garage</span>
            <span className="onboarding-radio-desc">
              Lightweight S3-compatible storage for self-hosting.
            </span>
          </label>
        </div>
      )}

      {/* Error display */}
      {error && <div className="error">{error}</div>}

      {/* Local form fields */}
      {kind === "local" && step !== "success" && (
        <div className="onboarding-field">
          <label htmlFor="backend-path">Local Storage Path</label>
          <input
            id="backend-path"
            type="text"
            placeholder="/path/to/lore/data"
            value={form.path}
            onChange={updateField("path")}
            disabled={step === "connecting"}
          />
        </div>
      )}

      {/* Object storage form fields */}
      {isObjectStorage(kind) && step !== "success" && (
        <>
          <div className="onboarding-field">
            <label htmlFor="backend-endpoint">Endpoint URL</label>
            <input
              id="backend-endpoint"
              type="text"
              placeholder={
                kind === "s3"
                  ? "https://s3.amazonaws.com"
                  : "https://minio.example.com"
              }
              value={form.endpoint}
              onChange={updateField("endpoint")}
              disabled={step === "connecting"}
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="backend-bucket">Bucket Name</label>
            <input
              id="backend-bucket"
              type="text"
              placeholder="lore-data"
              value={form.bucket}
              onChange={updateField("bucket")}
              disabled={step === "connecting"}
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="backend-region">Region</label>
            <input
              id="backend-region"
              type="text"
              placeholder={kind === "s3" ? "us-east-1" : ""}
              value={form.region}
              onChange={updateField("region")}
              disabled={step === "connecting"}
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="backend-access-key">Access Key ID</label>
            <input
              id="backend-access-key"
              type="text"
              placeholder="AKIA..."
              value={form.accessKeyId}
              onChange={updateField("accessKeyId")}
              disabled={step === "connecting"}
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="backend-secret-key">Secret Access Key</label>
            <input
              id="backend-secret-key"
              type="password"
              placeholder="••••••••"
              value={form.secretAccessKey}
              onChange={updateField("secretAccessKey")}
              disabled={step === "connecting"}
            />
          </div>
        </>
      )}

      {/* Mutable store (optional for all backends) */}
      {step !== "success" && (
        <div className="onboarding-field onboarding-field--optional">
          <label htmlFor="backend-mutable">Mutable Store Path (optional)</label>
          <input
            id="backend-mutable"
            type="text"
            placeholder="/path/to/mutable/store (branch pointers)"
            value={form.mutableStore}
            onChange={updateField("mutableStore")}
            disabled={step === "connecting"}
          />
        </div>
      )}

      {/* Action buttons */}
      {step === "idle" && (
        <button
          className="onboarding-button onboarding-button--primary"
          disabled={!isValid()}
          onClick={handleConnect}
        >
          Open Storage
        </button>
      )}

      {step === "connecting" && (
        <button
          className="onboarding-button onboarding-button--primary"
          disabled
        >
          Connecting&hellip;
        </button>
      )}

      {step === "success" && (
        <div className="onboarding-success">
          <span className="success-icon">&#10003;</span>
          <span>
            Storage opened — {kind === "local" ? "Local" : kind} backend ready
          </span>
          <button className="onboarding-button" onClick={handleReset}>
            Back
          </button>
        </div>
      )}

      {step === "error" && (
        <button
          className="onboarding-button onboarding-button--primary"
          onClick={handleConnect}
        >
          Retry
        </button>
      )}
    </div>
  );
}
