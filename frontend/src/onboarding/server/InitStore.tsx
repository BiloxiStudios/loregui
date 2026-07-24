import { useCallback, useEffect, useState } from "react";
import { api, type StorageBackendConfig } from "../../api";
import PathField from "./PathField";
import type { StepStateProps } from "../stepResult";

type Step = "form" | "done" | "error";

export interface InitStoreResult {
  /** The store directory created — this is what the host server serves. */
  storePath: string;
  /** Optional repository name advertised in the host server's lore:// URL. */
  repoName: string;
}

interface InitStoreProps extends StepStateProps<InitStoreResult> {
  /**
   * Storage backend config from step 1 — the store path is displayed read-only
   * from here and is never re-asked (SBAI-5560). Optional so the component
   * still renders if the user jumped here.
   */
  config?: StorageBackendConfig;
  /**
   * Reports the created store path + (optional) repo name up to the onboarding
   * shell so the next step ("Host server") serves exactly this store and
   * advertises this repo in its lore:// URL.
   */
  onInitialized?: (result: InitStoreResult) => void;
}

/**
 * Onboarding step 3: create the local store the server will host.
 *
 * The host flow's store is a plain directory the standalone `loreserver` fills
 * with its content-addressed immutable/ + mutable/ layout when it starts
 * (step 4). It is NOT a lore *repository* (no `.lore` marker) and needs NO
 * remote service, so this step simply ensures the directory exists via
 * `api.hostStorePrepare` — it does not call `shared_store_create` /
 * `repository_create`, which require a remote URL and would fail for a fresh
 * local host with "no remote URL". The store path itself is chosen ONCE in
 * step 1 (Choose Storage Backend) and shown here as a read-only summary; an
 * optional repository name is collected to advertise in the connection URL the
 * next step shows clients.
 */
export default function InitStore({
  config,
  onInitialized,
  onStateChange,
}: InitStoreProps = {}) {
  // The store path comes from step 1 verbatim — never edited here.
  const storePath = config?.path ?? "";
  const [repoName, setRepoName] = useState("");
  const [step, setStep] = useState<Step>("form");
  const [error, setError] = useState<string | null>(null);
  const [resolvedPath, setResolvedPath] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    onStateChange?.({ status: "idle" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleCreate = useCallback(async () => {
    if (!storePath.trim()) return;
    try {
      setIsSubmitting(true);
      setError(null);
      onStateChange?.({ status: "working" });
      // Ensure the local store directory exists. Idempotent — step 1 may have
      // already created it. No remote URL, no repository-create here.
      const resolved = await api.hostStorePrepare(storePath.trim());
      setResolvedPath(resolved);
      setStep("done");
      const result = { storePath: resolved, repoName: repoName.trim() };
      onInitialized?.(result);
      onStateChange?.({ status: "success", value: result });
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
    } finally {
      setIsSubmitting(false);
    }
  }, [storePath, repoName, onInitialized, onStateChange]);

  return (
    <div className="onboarding-card">
      <h2>Initialize Server</h2>
      <p className="onboarding-description">
        Create the local store your server will host. The server fills it with
        its content store when it starts — there&rsquo;s nothing else to set up.
        Optionally name the first repository to advertise in the connection URL.
      </p>

      {error && <div className="error">{error}</div>}

      {(step === "form" || step === "error") && (
        <>
          <PathField
            id="store-path"
            label="Shared store — created in step 1"
            value={storePath}
            readOnly
            hint={
              storePath
                ? "The directory is created if it doesn't exist. To change it, go back to the storage backend step."
                : "No store path yet — go back to step 1 and choose a storage location."
            }
          />
          <div className="onboarding-field onboarding-field--optional">
            <label htmlFor="repo-name">Repository Name (optional)</label>
            <input
              id="repo-name"
              type="text"
              placeholder="my-repository"
              value={repoName}
              onChange={(e) => {
                setRepoName(e.target.value);
                onStateChange?.({ status: "idle" });
              }}
              disabled={isSubmitting}
            />
            <span className="onboarding-field-hint">
              Advertised in the <code>lore://</code> URL clients clone. Leave
              blank to set it up later.
            </span>
          </div>
          <button
            className="onboarding-button onboarding-button--primary"
            disabled={!storePath.trim() || isSubmitting}
            onClick={() => void handleCreate()}
          >
            {step === "error" ? "Retry" : "Create Store"}
          </button>
        </>
      )}

      {step === "done" && (
        <div className="onboarding-success">
          <div className="success-message">
            <span className="success-icon">&#10003;</span>
            <span>Store ready</span>
          </div>
          <div className="onboarding-description">
            Store created at <code>{resolvedPath}</code>
            {repoName.trim() ? (
              <>
                {" — repository "}
                <code>{repoName.trim()}</code>
              </>
            ) : null}
            . Continue to host it.
          </div>
        </div>
      )}
    </div>
  );
}
