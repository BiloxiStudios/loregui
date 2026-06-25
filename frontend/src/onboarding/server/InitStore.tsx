import { useCallback, useEffect, useState } from "react";
import { api, type StorageBackendConfig } from "../../api";

type Step = "form" | "done" | "error";

export interface InitStoreResult {
  /** The store directory created — this is what the host server serves. */
  storePath: string;
  /** Optional repository name advertised in the host server's lore:// URL. */
  repoName: string;
}

interface InitStoreProps {
  /**
   * Storage backend config from step 1, used to prefill the store path so the
   * user doesn't retype it. Optional so the component still renders if the user
   * jumped here.
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
 * local host with "no remote URL". An optional repository name is collected to
 * advertise in the connection URL the next step shows clients.
 */
export default function InitStore({ config, onInitialized }: InitStoreProps = {}) {
  const [storePath, setStorePath] = useState(config?.path ?? "");
  const [repoName, setRepoName] = useState("");
  const [step, setStep] = useState<Step>("form");
  const [error, setError] = useState<string | null>(null);
  const [resolvedPath, setResolvedPath] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Keep the store path in sync if step 1 reports/updates a resolved path.
  useEffect(() => {
    if (config?.path) setStorePath(config.path);
  }, [config?.path]);

  const handleCreate = useCallback(async () => {
    if (!storePath.trim()) return;
    try {
      setIsSubmitting(true);
      setError(null);
      // Ensure the local store directory exists. Idempotent — step 1 may have
      // already created it. No remote URL, no repository-create here.
      const resolved = await api.hostStorePrepare(storePath.trim());
      setResolvedPath(resolved);
      setStep("done");
      onInitialized?.({ storePath: resolved, repoName: repoName.trim() });
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      setStep("error");
    } finally {
      setIsSubmitting(false);
    }
  }, [storePath, repoName, onInitialized]);

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
          <div className="onboarding-field">
            <label htmlFor="store-path">Store Path</label>
            <input
              id="store-path"
              type="text"
              placeholder="/path/to/store"
              value={storePath}
              onChange={(e) => setStorePath(e.target.value)}
              disabled={isSubmitting}
            />
            <span className="onboarding-field-hint">
              Prefilled from the storage backend you chose. The directory is
              created if it doesn&rsquo;t exist.
            </span>
          </div>
          <div className="onboarding-field onboarding-field--optional">
            <label htmlFor="repo-name">Repository Name (optional)</label>
            <input
              id="repo-name"
              type="text"
              placeholder="my-repository"
              value={repoName}
              onChange={(e) => setRepoName(e.target.value)}
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
