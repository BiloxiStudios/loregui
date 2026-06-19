import { useCallback, useState } from "react";
import { api } from "../api";

/**
 * Onboarding component: clone a repository or open an existing working tree.
 * Wired into the onboarding shell by the integration manager.
 */
export default function ClientClone() {
  const [mode, setMode] = useState<"choice" | "clone" | "open">("choice");
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);

  // Clone state
  const [cloneUrl, setCloneUrl] = useState("");
  const [cloneDest, setCloneDest] = useState("");

  // Open state
  const [openPath, setOpenPath] = useState("");

  const run = useCallback(async (fn: () => Promise<void>) => {
    try {
      setError(null);
      await fn();
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
    }
  }, []);

  const handleClone = async () => {
    if (!cloneUrl.trim() || !cloneDest.trim()) return;
    await run(async () => {
      await api.repositoryClone(cloneUrl.trim(), cloneDest.trim());
      // After clone, open the repository
      await api.openRepository(cloneDest.trim());
      setDone(true);
    });
  };

  const handleOpen = async () => {
    if (!openPath.trim()) return;
    await run(async () => {
      await api.openRepository(openPath.trim());
      setDone(true);
    });
  };

  const reset = () => {
    setMode("choice");
    setError(null);
    setDone(false);
    setCloneUrl("");
    setCloneDest("");
    setOpenPath("");
  };

  return (
    <div className="onboarding-client-clone">
      <h2>Get Repository</h2>
      <p className="subtitle">
        Clone a remote repository or open an existing working tree.
      </p>

      {error && <div className="error">{error}</div>}

      {mode === "choice" && (
        <div className="step choice">
          <h3>Choose an option</h3>
          <div className="choice-buttons">
            <button onClick={() => setMode("clone")}>
              Clone Repository
            </button>
            <button onClick={() => setMode("open")}>
              Open Working Tree
            </button>
          </div>
        </div>
      )}

      {mode === "clone" && (
        <div className="step">
          <h3>Clone Repository</h3>
          <div className="field">
            <label htmlFor="clone-url">Repository URL</label>
            <input
              id="clone-url"
              type="text"
              placeholder="https://example.com/repo.git"
              value={cloneUrl}
              onChange={(e) => setCloneUrl(e.target.value)}
            />
          </div>
          <div className="field">
            <label htmlFor="clone-dest">Destination Path</label>
            <input
              id="clone-dest"
              type="text"
              placeholder="/path/to/local/clone"
              value={cloneDest}
              onChange={(e) => setCloneDest(e.target.value)}
            />
          </div>
          <div className="actions">
            <button
              disabled={!cloneUrl.trim() || !cloneDest.trim()}
              onClick={() => void handleClone()}
            >
              Clone
            </button>
            <button onClick={() => setMode("choice")}>
              Back
            </button>
          </div>
        </div>
      )}

      {mode === "open" && (
        <div className="step">
          <h3>Open Working Tree</h3>
          <div className="field">
            <label htmlFor="open-path">Repository Path</label>
            <input
              id="open-path"
              type="text"
              placeholder="/path/to/existing/repository"
              value={openPath}
              onChange={(e) => setOpenPath(e.target.value)}
            />
          </div>
          <div className="actions">
            <button
              disabled={!openPath.trim()}
              onClick={() => void handleOpen()}
            >
              Open
            </button>
            <button onClick={() => setMode("choice")}>
              Back
            </button>
          </div>
        </div>
      )}

      {done && (
        <div className="step done">
          <div className="success">
            ✓ Repository ready
          </div>
          <h3>Setup Complete</h3>
          <p>Your repository is now open. Continue with the next setup step.</p>
          <div className="actions">
            <button onClick={reset}>
              Start Over
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
