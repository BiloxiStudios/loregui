import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { chooseDirectory } from "../platform/directoryPicker";
import type { StepStateProps } from "./stepResult";

export type ClientRepositoryMode = "choice" | "clone" | "open" | "create";

interface ClientCloneProps extends StepStateProps<string> {
  initialMode?: ClientRepositoryMode;
  /** Exact validated server URL from the Connect step. */
  initialCloneUrl?: string;
}

/**
 * Onboarding component: clone a repository or open an existing working tree.
 * Wired into the onboarding shell by the integration manager.
 */
export default function ClientClone({
  initialMode = "choice",
  initialCloneUrl,
  onStateChange,
}: ClientCloneProps = {}) {
  const [mode, setMode] = useState<ClientRepositoryMode>(initialMode);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);

  // Clone state
  const [cloneUrl, setCloneUrl] = useState(initialCloneUrl ?? "");
  const [cloneDest, setCloneDest] = useState("");

  // Open state
  const [openPath, setOpenPath] = useState("");

  // Create state
  const [createName, setCreateName] = useState("");
  const [createPath, setCreatePath] = useState("");

  useEffect(() => {
    onStateChange?.({ status: "idle" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (initialCloneUrl !== undefined) setCloneUrl(initialCloneUrl);
  }, [initialCloneUrl]);

  const invalidate = useCallback(() => {
    setDone(false);
    setError(null);
    onStateChange?.({ status: "idle" });
  }, [onStateChange]);

  const run = useCallback(async (path: string, fn: () => Promise<void>) => {
    try {
      setError(null);
      setDone(false);
      onStateChange?.({ status: "working" });
      await fn();
      setDone(true);
      onStateChange?.({ status: "success", value: path });
    } catch (e) {
      const message =
        typeof e === "string"
          ? e
          : e instanceof Error
            ? e.message
            : JSON.stringify(e);
      setError(message);
      onStateChange?.({ status: "error", message });
    }
  }, [onStateChange]);

  const handleClone = async () => {
    if (!cloneUrl.trim() || !cloneDest.trim()) return;
    const url = initialCloneUrl ?? cloneUrl.trim();
    const destination = cloneDest.trim();
    await run(destination, async () => {
      await api.repositoryClone(url, destination);
      // After clone, open the repository
      await api.openRepository(destination);
    });
  };

  const browseFor = async (
    title: string,
    currentPath: string,
    setPath: (path: string) => void,
  ) => {
    const selected = await chooseDirectory({
      title,
      defaultPath: currentPath || undefined,
    });
    if (selected !== null) {
      setPath(selected);
      invalidate();
    }
  };

  const handleOpen = async () => {
    if (!openPath.trim()) return;
    const path = openPath.trim();
    await run(path, async () => {
      await api.openRepository(path);
    });
  };

  const handleCreate = async () => {
    if (!createName.trim() || !createPath.trim()) return;
    await run(createPath.trim(), async () => {
      const path = createPath.trim();
      await api.repositoryCreate(path, createName.trim());
      // Validate the exact path through Task 1's open_repository guard before
      // treating the new project as active shell context.
      await api.openRepository(path);
    });
  };

  const reset = () => {
    setMode("choice");
    setError(null);
    setDone(false);
    setCloneUrl("");
    setCloneDest("");
    setOpenPath("");
    setCreateName("");
    setCreatePath("");
    invalidate();
  };

  const chooseMode = (nextMode: ClientRepositoryMode) => {
    setMode(nextMode);
    invalidate();
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
            <button onClick={() => chooseMode("clone")}>
              Clone Repository
            </button>
            <button onClick={() => chooseMode("open")}>
              Open Working Tree
            </button>
            <button onClick={() => chooseMode("create")}>
              Create Local Project
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
              readOnly={initialCloneUrl !== undefined}
              onChange={(e) => {
                setCloneUrl(e.target.value);
                invalidate();
              }}
            />
            {initialCloneUrl !== undefined && (
              <span className="onboarding-field-hint">
                Server selected and verified in the previous step.
              </span>
            )}
          </div>
          <div className="field">
            <span>Destination Path</span>
            <button
              type="button"
              onClick={() =>
                void browseFor(
                  "Choose clone destination",
                  cloneDest,
                  setCloneDest,
                )
              }
            >
              Browse…
            </button>
            <code>{cloneDest || "No directory selected"}</code>
            <details>
              <summary>Advanced path entry</summary>
              <label htmlFor="clone-dest">Destination Path</label>
              <input
                id="clone-dest"
                type="text"
                placeholder="/path/to/local/clone"
                value={cloneDest}
                onChange={(e) => {
                  setCloneDest(e.target.value);
                  invalidate();
                }}
              />
            </details>
          </div>
          <div className="actions">
            <button
              disabled={!cloneUrl.trim() || !cloneDest.trim()}
              onClick={() => void handleClone()}
            >
              Clone
            </button>
            <button onClick={() => chooseMode("choice")}>
              Back
            </button>
          </div>
        </div>
      )}

      {mode === "open" && (
        <div className="step">
          <h3>Open Working Tree</h3>
          <div className="field">
            <span>Repository Path</span>
            <button
              type="button"
              onClick={() =>
                void browseFor(
                  "Choose an existing repository",
                  openPath,
                  setOpenPath,
                )
              }
            >
              Browse…
            </button>
            <code>{openPath || "No directory selected"}</code>
            <details>
              <summary>Advanced path entry</summary>
              <label htmlFor="open-path">Repository Path</label>
              <input
                id="open-path"
                type="text"
                placeholder="/path/to/existing/repository"
                value={openPath}
                onChange={(e) => {
                  setOpenPath(e.target.value);
                  invalidate();
                }}
              />
            </details>
          </div>
          <div className="actions">
            <button
              disabled={!openPath.trim()}
              onClick={() => void handleOpen()}
            >
              Open
            </button>
            <button onClick={() => chooseMode("choice")}>
              Back
            </button>
          </div>
        </div>
      )}

      {mode === "create" && (
        <div className="step">
          <h3>Create Local Project</h3>
          <div className="field">
            <label htmlFor="create-name">Project name</label>
            <input
              id="create-name"
              type="text"
              value={createName}
              onChange={(e) => {
                setCreateName(e.target.value);
                invalidate();
              }}
              placeholder="world-bible"
            />
          </div>
          <div className="field">
            <span>Local project path</span>
            <button
              type="button"
              onClick={() =>
                void browseFor(
                  "Choose local project directory",
                  createPath,
                  setCreatePath,
                )
              }
            >
              Browse…
            </button>
            <code>{createPath || "No directory selected"}</code>
            <details>
              <summary>Advanced path entry</summary>
              <label htmlFor="create-path">Local project path</label>
              <input
                id="create-path"
                type="text"
                value={createPath}
                onChange={(e) => {
                  setCreatePath(e.target.value);
                  invalidate();
                }}
                placeholder="/path/to/new/project"
              />
            </details>
          </div>
          <div className="actions">
            <button
              disabled={!createName.trim() || !createPath.trim()}
              onClick={() => void handleCreate()}
            >
              Create project
            </button>
            <button onClick={() => chooseMode("choice")}>Back</button>
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
