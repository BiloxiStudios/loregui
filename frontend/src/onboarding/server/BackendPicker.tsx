import { useCallback, useEffect, useState } from "react";
import { api, type StorageBackendConfig } from "../../api";
import PathField from "./PathField";
import type { StepStateProps } from "../stepResult";

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

interface BackendPickerProps extends StepStateProps<StorageBackendConfig> {
  /**
   * Called with the validated config once the backend opens successfully.
   * Lets the onboarding shell forward the config to later steps
   * (e.g. connectivity validation) without re-entering it.
   */
  onConfigured?: (config: StorageBackendConfig) => void;
}

export default function BackendPicker({
  onConfigured,
  onStateChange,
}: BackendPickerProps = {}) {
  const [kind, setKind] = useState<BackendKind>("local");
  const [presetId, setPresetId] = useState<string>(S3_PRESETS[0].id);
  const [form, setForm] = useState<FormState>({ ...EMPTY_FORM });
  const [step, setStep] = useState<Step>("idle");
  const [error, setError] = useState<string | null>(null);

  const preset =
    S3_PRESETS.find((p) => p.id === presetId) ?? S3_PRESETS[0];

  useEffect(() => {
    onStateChange?.({ status: "idle" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const updateField = useCallback(
    (field: keyof FormState) =>
      (e: React.ChangeEvent<HTMLInputElement>) => {
        setForm((prev) => ({ ...prev, [field]: e.target.value }));
        onStateChange?.({ status: "idle" });
      },
    [onStateChange],
  );

  /** Value-based setter for PathField (manual edits and picker selections). */
  const setField = useCallback(
    (field: keyof FormState) => (value: string) => {
      setForm((prev) => ({ ...prev, [field]: value }));
      onStateChange?.({ status: "idle" });
    },
    [onStateChange],
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
      onStateChange?.({ status: "working" });
      const config = buildConfig();
      if (config.kind === "local") {
        // A local-FS host store is a plain directory the loreserver populates
        // when it starts (step 4) — NOT an existing lore repository. Prepare
        // (create) the directory here instead of opening a `.lore` repo, which
        // would fail for a brand-new host with "missing .lore". Forward the
        // resolved absolute path so later steps serve exactly this directory.
        const resolved = await api.hostStorePrepare(
          config.path ?? "",
          config.mutableStore,
        );
        config.path = resolved;
      } else {
        await api.storageOpen(config);
      }
      setStep("success");
      onConfigured?.(config);
      onStateChange?.({ status: "success", value: config });
    } catch (e) {
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
  }, [isValid, buildConfig, onConfigured, onStateChange]);

  const handleReset = useCallback(() => {
    setStep("idle");
    setError(null);
    setForm({ ...EMPTY_FORM });
    onStateChange?.({ status: "idle" });
  }, [onStateChange]);

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
              onChange={() => {
                setKind("local");
                onStateChange?.({ status: "idle" });
              }}
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
              onChange={() => {
                setKind("s3");
                onStateChange?.({ status: "idle" });
              }}
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

      {/* Local form fields — the store path is asked ONCE, right here (SBAI-5560).
          Steps 3 and 4 display it read-only with its role. */}
      {kind === "local" && step !== "success" && (
        <PathField
          id="backend-path"
          label="Local Storage Path"
          value={form.path}
          onChange={setField("path")}
          placeholder="/path/to/lore/data"
          dialogTitle="Choose local storage directory"
          disabled={step === "connecting"}
          hint="The directory is created if it doesn't exist — no existing repository required. Your server fills it with its content store when it starts."
        />
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
              onChange={(e) => {
                setPresetId(e.target.value);
                onStateChange?.({ status: "idle" });
              }}
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
        <PathField
          id="backend-mutable"
          label="Mutable Store Path (optional)"
          optional
          value={form.mutableStore}
          onChange={setField("mutableStore")}
          placeholder="/path/to/mutable/store (branch pointers)"
          dialogTitle="Choose mutable store directory"
          disabled={step === "connecting"}
        />
      )}

      {/* Action buttons */}
      {step === "idle" && (
        <button
          className="onboarding-button onboarding-button--primary"
          disabled={!isValid()}
          onClick={handleConnect}
        >
          {kind === "local" ? "Prepare Store" : "Open Storage"}
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
