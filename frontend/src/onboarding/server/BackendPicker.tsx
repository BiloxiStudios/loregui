import { useCallback, useState } from "react";
import { api, type StorageBackendConfig } from "../../api";

// lore has exactly two real store backends:
//   - "local": a filesystem store (packfiles on disk).
//   - "s3": an S3-compatible object store (lore's `aws` store mode).
// AWS S3, MinIO, Garage, Ceph/RGW, Backblaze B2, etc. are all the SAME `s3`
// backend — they differ only by endpoint URL (and whether path-style addressing
// is required). They are offered below as non-binding *presets* that prefill the
// endpoint placeholder, never as separate backends.
type BackendKind = "local" | "s3";
type Step = "idle" | "connecting" | "success" | "error";

/** Non-binding endpoint presets for common S3-compatible providers. */
interface S3Preset {
  id: string;
  label: string;
  /** Placeholder shown in the endpoint field; the user can type any endpoint. */
  endpointHint: string;
  /** Hint copy shown under the preset row. */
  note: string;
}

const S3_PRESETS: S3Preset[] = [
  {
    id: "aws",
    label: "AWS S3",
    endpointHint: "https://s3.us-east-1.amazonaws.com",
    note: "Amazon's managed object storage. Leave the endpoint blank to use the default AWS S3 endpoint for your region.",
  },
  {
    id: "minio",
    label: "MinIO",
    endpointHint: "https://minio.example.com:9000",
    note: "Self-hosted S3-compatible server. Path-style addressing is enabled automatically.",
  },
  {
    id: "garage",
    label: "Garage",
    endpointHint: "http://garage.example.com:3900",
    note: "Lightweight self-hosted S3-compatible storage. Path-style addressing is enabled automatically.",
  },
  {
    id: "other",
    label: "Other S3-compatible",
    endpointHint: "https://object-store.example.com",
    note: "Any S3-compatible provider (Ceph/RGW, Backblaze B2, Wasabi, …). Enter its endpoint URL.",
  },
];

/** Presets that need path-style addressing (everything except real AWS S3). */
const PATH_STYLE_PRESETS = new Set(["minio", "garage", "other"]);

interface FormState {
  // local
  path: string;
  // object storage (s3)
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
  const [presetId, setPresetId] = useState<string>(S3_PRESETS[0].id);
  const [form, setForm] = useState<FormState>({ ...EMPTY_FORM });
  const [step, setStep] = useState<Step>("idle");
  const [error, setError] = useState<string | null>(null);

  const preset =
    S3_PRESETS.find((p) => p.id === presetId) ?? S3_PRESETS[0];

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
    // S3-compatible object storage: endpoint + bucket required.
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
      kind: "s3",
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
        Pick where your Lore data is stored. A <strong>local</strong> filesystem
        store is simplest for a single machine.{" "}
        <strong>S3-compatible object storage</strong> scales to teams and works
        with any S3 provider — AWS S3, MinIO, Garage, and others are the same
        backend, differing only by endpoint URL.
      </p>

      {/* Backend type selector — two honest options. */}
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
            <span className="onboarding-radio-label">Local filesystem</span>
            <span className="onboarding-radio-desc">
              Store data in a local directory. Simplest — no external services.
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
            <span className="onboarding-radio-label">
              S3-compatible object storage
            </span>
            <span className="onboarding-radio-desc">
              One backend, any S3 provider (AWS S3, MinIO, Garage, …). Scales to
              teams.
            </span>
          </label>
        </div>
      )}

      {/* Error display */}
      {error && (
        <div className="error" role="alert">
          {error}
        </div>
      )}

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

      {/* S3-compatible object storage form fields */}
      {kind === "s3" && step !== "success" && (
        <>
          {/* Provider presets — non-binding endpoint hints. */}
          <div className="onboarding-field">
            <label htmlFor="backend-preset">Provider preset (optional)</label>
            <select
              id="backend-preset"
              value={presetId}
              onChange={(e) => setPresetId(e.target.value)}
              disabled={step === "connecting"}
            >
              {S3_PRESETS.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
            </select>
            <span className="onboarding-field-hint">{preset.note}</span>
          </div>

          <div className="onboarding-field">
            <label htmlFor="backend-endpoint">Endpoint URL</label>
            <input
              id="backend-endpoint"
              type="text"
              placeholder={preset.endpointHint}
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
              placeholder={presetId === "aws" ? "us-east-1" : ""}
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
              placeholder="AKIA…"
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
          {PATH_STYLE_PRESETS.has(presetId) && (
            <p className="onboarding-field-hint">
              Path-style addressing will be used automatically for this provider.
            </p>
          )}
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
            Storage opened —{" "}
            {kind === "local" ? "Local filesystem" : "S3-compatible"} backend
            ready
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
