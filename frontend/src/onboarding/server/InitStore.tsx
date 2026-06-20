import { useCallback, useState } from "react";
import { api } from "../../api";

type Step = "store" | "repo" | "done" | "error";

/**
 * Onboarding component: initialize a shared store + repository.
 * Wired into the onboarding shell by the integration manager.
 *
 * Uses `api.sharedStoreCreate` to create the shared storage backend,
 * then `api.repositoryCreate` to create the first repository.
 */
export default function InitStore() {
  const [storePath, setStorePath] = useState("");
  const [repoPath, setRepoPath] = useState("");
  const [repoName, setRepoName] = useState("");
  const [step, setStep] = useState<Step>("store");
  const [error, setError] = useState<string | null>(null);
  const [storeResult, setStoreResult] = useState<string | null>(null);
  const [repoResult, setRepoResult] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleCreateStore = useCallback(async () => {
    if (!storePath.trim()) return;
    try {
      setIsSubmitting(true);
      setError(null);
      const id = await api.sharedStoreCreate(storePath.trim());
      setStoreResult(id);
      setStep("repo");
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      setStep("error");
    } finally {
      setIsSubmitting(false);
    }
  }, [storePath]);

  const handleCreateRepo = useCallback(async () => {
    if (!repoPath.trim() || !repoName.trim()) return;
    try {
      setIsSubmitting(true);
      setError(null);
      const id = await api.repositoryCreate(repoPath.trim(), repoName.trim());
      setRepoResult(id);
      setStep("done");
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
      setStep("error");
    } finally {
      setIsSubmitting(false);
    }
  }, [repoPath, repoName]);

  const handleRetry = useCallback(() => {
    setError(null);
    // Retry from the step that failed
    if (storeResult) {
      setStep("repo");
    } else {
      setStep("store");
    }
  }, [storeResult]);

  return (
    <div className="onboarding-card">
      <h2>Initialize Server</h2>
      <p className="onboarding-description">
        Set up a shared storage backend and create your first repository.
      </p>

      {error && <div className="error">{error}</div>}

      {step === "store" && (
        <div className="onboarding-field">
          <label htmlFor="store-path">Shared Store Path</label>
          <input
            id="store-path"
            type="text"
            placeholder="/path/to/shared/store"
            value={storePath}
            onChange={(e) => setStorePath(e.target.value)}
            disabled={isSubmitting}
          />
          <button
            className="onboarding-button onboarding-button--primary"
            disabled={!storePath.trim() || isSubmitting}
            onClick={() => void handleCreateStore()}
          >
            Create Store
          </button>
        </div>
      )}

      {step === "repo" && (
        <>
          <div className="onboarding-success">
            <span className="success-icon">&#10003;</span>
            <span>Shared store created (ID: {storeResult})</span>
          </div>
          <div className="onboarding-field">
            <label htmlFor="repo-path">Repository Path</label>
            <input
              id="repo-path"
              type="text"
              placeholder="/path/to/repository"
              value={repoPath}
              onChange={(e) => setRepoPath(e.target.value)}
              disabled={isSubmitting}
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="repo-name">Repository Name</label>
            <input
              id="repo-name"
              type="text"
              placeholder="my-repository"
              value={repoName}
              onChange={(e) => setRepoName(e.target.value)}
              disabled={isSubmitting}
            />
          </div>
          <button
            className="onboarding-button onboarding-button--primary"
            disabled={!repoPath.trim() || !repoName.trim() || isSubmitting}
            onClick={() => void handleCreateRepo()}
          >
            Create Repository
          </button>
        </>
      )}

      {step === "done" && (
        <div className="onboarding-success">
          <div className="success-message">
            <span className="success-icon">&#10003;</span>
            <span>Server initialized</span>
          </div>
          <div className="onboarding-description">
            Shared store created (ID: {storeResult})
          </div>
          <div className="onboarding-description">
            Repository created (ID: {repoResult})
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
