import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { contextApi } from "./api";
import {
  EMPTY_CONTEXT_SNAPSHOT,
  recordsForProject,
  snapshotForProject,
  type ActiveContextSnapshot,
  type ContextSettings,
} from "./types";

const SAVED_PROJECT_UNAVAILABLE =
  "Saved project is unavailable. Choose another project or restore its local path.";

interface LoreContextValue {
  snapshot: ActiveContextSnapshot;
  unavailableProjectIds: ReadonlySet<string>;
  validationError: string | null;
  selectProject: (projectId: string) => Promise<void>;
  selectServer: (serverId: string) => Promise<void>;
  refresh: () => Promise<void>;
}

const LoreContext = createContext<LoreContextValue | null>(null);

export function ContextProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<ContextSettings | null>(null);
  const [snapshot, setSnapshot] = useState<ActiveContextSnapshot>(
    EMPTY_CONTEXT_SNAPSHOT,
  );
  const [unavailableProjectIds, setUnavailableProjectIds] = useState<
    ReadonlySet<string>
  >(() => new Set());
  const [validationError, setValidationError] = useState<string | null>(null);
  const operation = useRef(0);
  const snapshotRef = useRef(snapshot);

  useEffect(() => {
    snapshotRef.current = snapshot;
  }, [snapshot]);

  const refresh = useCallback(async () => {
    const generation = ++operation.current;
    let context: ContextSettings;
    try {
      context = await contextApi.get();
    } catch {
      if (generation === operation.current) {
        setValidationError("Could not read saved Lore context.");
      }
      return;
    }
    if (generation !== operation.current) return;
    setSettings(context);

    const projectId = context.active.project_id;
    if (!projectId) {
      setSnapshot(EMPTY_CONTEXT_SNAPSHOT);
      setUnavailableProjectIds(new Set());
      setValidationError(null);
      return;
    }

    const records = recordsForProject(context, projectId);
    if (!records) {
      setSnapshot(EMPTY_CONTEXT_SNAPSHOT);
      setUnavailableProjectIds(new Set([projectId]));
      setValidationError(SAVED_PROJECT_UNAVAILABLE);
      return;
    }

    try {
      const currentPath = await contextApi.currentRepository();
      if (currentPath !== records.project.local_path) {
        throw new Error("saved project is not the validated P0 repository");
      }
      const status = await contextApi.status();
      if (generation !== operation.current) return;
      setSnapshot(snapshotForProject(records, status));
      setUnavailableProjectIds(new Set());
      setValidationError(null);
    } catch {
      if (generation === operation.current) {
        setSnapshot(EMPTY_CONTEXT_SNAPSHOT);
        setUnavailableProjectIds(new Set([projectId]));
        setValidationError(SAVED_PROJECT_UNAVAILABLE);
      }
    }
  }, []);

  useEffect(() => {
    void refresh();
    return () => {
      operation.current += 1;
    };
  }, [refresh]);

  const selectProject = useCallback(
    async (projectId: string) => {
      const generation = ++operation.current;
      const current = settings;
      const records = current ? recordsForProject(current, projectId) : null;
      if (!current || !records) {
        setValidationError("Selected project is not available in saved context.");
        return;
      }

      const candidate: ContextSettings = {
        ...current,
        active: {
          project_id: projectId,
          server_id: records.repository.server_id,
          identity_ref:
            current.active.server_id === records.repository.server_id
              ? current.active.identity_ref
              : null,
        },
      };

      let validated: ContextSettings;
      try {
        validated = await contextApi.validate(candidate);
      } catch {
        if (generation === operation.current) {
          setValidationError("Selected project context is invalid.");
        }
        return;
      }

      let status;
      try {
        await contextApi.openRepository(records.project.local_path);
        const openedPath = await contextApi.currentRepository();
        if (openedPath !== records.project.local_path) {
          throw new Error("opened repository path did not match selection");
        }
        status = await contextApi.status();
      } catch {
        if (generation === operation.current) {
          setUnavailableProjectIds((previous) =>
            new Set([...previous, projectId]),
          );
          setValidationError("Could not open selected project.");
        }
        return;
      }

      let persisted: ContextSettings;
      try {
        persisted = await contextApi.update(validated);
      } catch {
        // Re-open the prior validated project when possible so backend runtime
        // state cannot silently diverge from the retained frontend snapshot.
        const previousPath = snapshotRef.current.project?.local_path;
        if (previousPath && previousPath !== records.project.local_path) {
          try {
            await contextApi.openRepository(previousPath);
          } catch {
            // Keep the public error non-secret; the next refresh remains closed
            // unless P0 independently validates a matching saved project.
          }
        }
        if (generation === operation.current) {
          setValidationError("Could not save selected project.");
        }
        return;
      }

      if (generation !== operation.current) return;
      const persistedRecords = recordsForProject(persisted, projectId);
      if (!persistedRecords) {
        setValidationError("Saved project context could not be resolved.");
        return;
      }
      setSettings(persisted);
      setSnapshot(snapshotForProject(persistedRecords, status));
      setUnavailableProjectIds((previous) => {
        const next = new Set(previous);
        next.delete(projectId);
        return next;
      });
      setValidationError(null);
    },
    [settings],
  );

  const selectServer = useCallback(
    async (serverId: string) => {
      const generation = ++operation.current;
      const current = settings;
      const server = current?.servers.find((item) => item.id === serverId);
      if (!current || !server) {
        setValidationError("Selected server is not available in saved context.");
        return;
      }
      const candidate: ContextSettings = {
        ...current,
        active: {
          project_id: null,
          server_id: serverId,
          identity_ref:
            current.active.server_id === serverId
              ? current.active.identity_ref
              : null,
        },
      };

      try {
        const validated = await contextApi.validate(candidate);
        const persisted = await contextApi.update(validated);
        if (generation !== operation.current) return;
        const persistedServer =
          persisted.servers.find((item) => item.id === serverId) ?? null;
        if (!persistedServer) {
          setValidationError("Saved server context could not be resolved.");
          return;
        }
        setSettings(persisted);
        setSnapshot({
          ...EMPTY_CONTEXT_SNAPSHOT,
          server: persistedServer,
          authMode: persistedServer.auth_mode,
        });
        setValidationError(null);
      } catch {
        if (generation === operation.current) {
          setValidationError("Could not save selected server.");
        }
      }
    },
    [settings],
  );

  return (
    <LoreContext.Provider
      value={{
        snapshot,
        unavailableProjectIds,
        validationError,
        selectProject,
        selectServer,
        refresh,
      }}
    >
      {children}
    </LoreContext.Provider>
  );
}

export function useLoreContext(): LoreContextValue {
  const context = useContext(LoreContext);
  if (!context) {
    throw new Error("useLoreContext must be used within ContextProvider");
  }
  return context;
}
