import { useCallback, useState } from "react";
import { api } from "../../api";

/**
 * Onboarding component: initialize a shared store + repository.
 * Wired into the onboarding shell by the integration manager.
 */
export default function InitStore() {
  const [storePath, setStorePath] = useState("");
  const [repoPath, setRepoPath] = useState("");
  const [repoName, setRepoName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [storeResult, setStoreResult] = useState<string | null>(null);
  const [repoResult, setRepoResult] = useState<string | null>(null);
  const [step, setStep] = useState<"store" | "repo" | "done">("store");

  const run = useCallback(async (fn: () => Promise<void>) => {
    try {
      setError(null);
      await fn();
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
    }
  }, []);

  const handleCreateStore = async () => {
    if (!storePath.trim()) return;
    await run(async () => {
      const id = await api.sharedStoreCreate(storePath.trim());
      setStoreResult(id);
      setStep("repo");
    });
  };

  const handleCreateRepo = async () => {
    if (!repoPath.trim() || !repoName.trim()) return;
    await run(async () => {
      const id = await api.repositoryCreate(repoPath.trim(), repoName.trim());
      setRepoResult(id);
      setStep("done");
    });
  };

  return (
    <div className="onboarding-init-store">
      <h2>Initialize Server</h2>
      <p className="subtitle">
        Set up a shared storage backend and create your first repository.
      </p>

      {error && <div className="error">{error}</div>}

      {step === "store" && (
        <div className="step">
          <h3>Step 1: Create Shared Store</h3>
          <div className="field">
            <label htmlFor="store-path">Store Path</label>
            <input
              id="store-path"
              type="text"
              placeholder="/path/to/shared/store"
              value={storePath}
              onChange={(e) => setStorePath(e.target.value)}
            />
          </div>
          <button
            disabled={!storePath.trim()}
            onClick={() => void handleCreateStore()}
          >
            Create Store
          </button>
        </div>
      )}

      {step === "repo" && (
        <div className="step">
          <div className="success">
            ✓ Shared store created (ID: {storeResult})
          </div>
          <h3>Step 2: Create Repository</h3>
          <div className="field">
            <label htmlFor="repo-path">Repository Path</label>
            <input
              id="repo-path"
              type="text"
              placeholder="/path/to/repository"
              value={repoPath}
              onChange={(e) => setRepoPath(e.target.value)}
            />
          </div>
          <div className="field">
            <label htmlFor="repo-name">Repository Name</label>
            <input
              id="repo-name"
              type="text"
              placeholder="my-repository"
              value={repoName}
              onChange={(e) => setRepoName(e.target.value)}
            />
          </div>
          <button
            disabled={!repoPath.trim() || !repoName.trim()}
            onClick={() => void handleCreateRepo()}
          >
            Create Repository
          </button>
        </div>
      )}

      {step === "done" && (
        <div className="step done">
          <div className="success">
            ✓ Shared store created (ID: {storeResult})
          </div>
          <div className="success">
            ✓ Repository created (ID: {repoResult})
          </div>
          <h3>Setup Complete</h3>
          <p>Your server is ready. Continue with the next setup step.</p>
        </div>
      )}
    </div>
  );
}
