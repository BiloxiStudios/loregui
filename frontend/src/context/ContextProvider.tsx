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
  const selectionGeneration = useRef(0);

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
      const generation = ++selectionGeneration.current;
      const current = settings;
      const records = current ? recordsForProject(current, projectId) : null;
      if (!current || !records) {
        setValidationError("Selected project is not available in saved context.");
        return;
      }

      let result;
      try {
        result = await contextApi.select(
          { kind: "project", project_id: projectId },
          generation,
        );
      } catch {
        if (generation === selectionGeneration.current) {
          setValidationError("Could not save selected project.");
        }
        return;
      }

      if (generation !== selectionGeneration.current) return;
      const selectedProjectId = result.context.active.project_id;
      const selectedRecords = selectedProjectId
        ? recordsForProject(result.context, selectedProjectId)
        : null;
      if (selectedProjectId !== projectId || !selectedRecords || !result.status) {
        setValidationError("Saved project context could not be resolved.");
        return;
      }
      setSettings(result.context);
      setSnapshot(snapshotForProject(selectedRecords, result.status));
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
      const generation = ++selectionGeneration.current;
      const current = settings;
      const server = current?.servers.find((item) => item.id === serverId);
      if (!current || !server) {
        setValidationError("Selected server is not available in saved context.");
        return;
      }

      try {
        const result = await contextApi.select(
          { kind: "server", server_id: serverId },
          generation,
        );
        if (generation !== selectionGeneration.current) return;
        const selectedServerId = result.context.active.server_id;
        const selectedServer =
          result.context.active.project_id === null && selectedServerId === serverId
            ? result.context.servers.find((item) => item.id === selectedServerId) ??
              null
            : null;
        if (!selectedServer || result.active_repository !== null || result.status !== null) {
          setValidationError("Saved server context could not be resolved.");
          return;
        }
        setSettings(result.context);
        setSnapshot({
          ...EMPTY_CONTEXT_SNAPSHOT,
          server: selectedServer,
          authMode: selectedServer.auth_mode,
        });
        setValidationError(null);
      } catch {
        if (generation === selectionGeneration.current) {
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
