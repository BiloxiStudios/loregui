import { useCallback, useEffect, useRef, useState } from "react";
import { api, type HostStatus } from "./api";

const DEFAULT_POLL_INTERVAL_MS = 5_000;
const RESTART_DISABLED_REASON =
  "Restart is unavailable because the backend does not retain a secret-free full launch configuration.";

interface HostedServerContext {
  serverName?: string;
  clientUrl: string;
  localUrl: string;
  storeDir: string;
  authRequired: boolean;
}

// Deliberately memory-only. This preserves non-secret launch context across
// shell card unmount/remounts without persisting credentials or depending on a
// repository/CWD/account. The running backend remains the source of truth.
let sessionLastContext: HostedServerContext | null = null;

function messageFrom(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") return message;
  }
  return "Hosted server status is unavailable.";
}

function contextFromRunningStatus(status: HostStatus): HostedServerContext | null {
  if (
    !status.running ||
    typeof status.pid !== "number" ||
    !status.url?.trim() ||
    !status.storeDir?.trim() ||
    typeof status.authRequired !== "boolean"
  ) {
    return null;
  }
  return {
    serverName: status.serverName?.trim() || undefined,
    clientUrl: status.advertisedUrl?.trim() || status.url,
    localUrl: status.url,
    storeDir: status.storeDir,
    authRequired: status.authRequired,
  };
}

export interface HostedServerCardProps {
  onBrowseRepositories: (url: string, signal: AbortSignal) => void | Promise<void>;
  pollIntervalMs?: number;
}

export default function HostedServerCard({
  onBrowseRepositories,
  pollIntervalMs = DEFAULT_POLL_INTERVAL_MS,
}: HostedServerCardProps) {
  const [status, setStatus] = useState<HostStatus | null>(null);
  const [context, setContext] = useState<HostedServerContext | null>(
    () => sessionLastContext,
  );
  const [error, setError] = useState<string | null>(null);
  const [copyResult, setCopyResult] = useState<"success" | "error" | null>(null);
  const [stopping, setStopping] = useState(false);
  const mounted = useRef(false);
  const pollGeneration = useRef(0);
  const stopGeneration = useRef(0);
  const copyGeneration = useRef(0);
  const browseController = useRef<AbortController | null>(null);
  const activeContextKey = useRef<string | null>(null);
  const lifecycleInFlight = useRef(false);

  const invalidateCopyAndBrowse = useCallback(() => {
    copyGeneration.current += 1;
    setCopyResult(null);
    browseController.current?.abort();
    browseController.current = null;
  }, []);

  const applyStatus = useCallback((next: HostStatus) => {
    if (next.running) {
      const nextContext = contextFromRunningStatus(next);
      if (!nextContext) {
        activeContextKey.current = null;
        invalidateCopyAndBrowse();
        setStatus(null);
        setError("Hosted server status is incomplete.");
        return;
      }
      const nextKey = [
        nextContext.clientUrl,
        nextContext.localUrl,
        nextContext.storeDir,
        nextContext.serverName ?? "",
        String(nextContext.authRequired),
      ].join("\u0000");
      if (activeContextKey.current !== nextKey) {
        invalidateCopyAndBrowse();
      }
      activeContextKey.current = nextKey;
      sessionLastContext = nextContext;
      setContext(nextContext);
      setStatus(next);
      setError(null);
      return;
    }
    activeContextKey.current = null;
    invalidateCopyAndBrowse();
    setStatus(next);
    setError(null);
  }, [invalidateCopyAndBrowse]);

  const refresh = useCallback(async () => {
    if (lifecycleInFlight.current) return;
    const generation = ++pollGeneration.current;
    try {
      const next = await api.hostServerStatus();
      if (!mounted.current || generation !== pollGeneration.current) return;
      applyStatus(next);
    } catch (caught) {
      if (!mounted.current || generation !== pollGeneration.current) return;
      activeContextKey.current = null;
      invalidateCopyAndBrowse();
      setStatus(null);
      setError(messageFrom(caught));
    }
  }, [applyStatus, invalidateCopyAndBrowse]);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    const interval = window.setInterval(() => void refresh(), pollIntervalMs);
    return () => {
      mounted.current = false;
      pollGeneration.current += 1;
      stopGeneration.current += 1;
      copyGeneration.current += 1;
      browseController.current?.abort();
      browseController.current = null;
      activeContextKey.current = null;
      lifecycleInFlight.current = false;
      window.clearInterval(interval);
    };
  }, [pollIntervalMs, refresh]);

  const handleCopy = useCallback(async () => {
    if (!status?.running || !context?.clientUrl || lifecycleInFlight.current) return;
    const generation = ++copyGeneration.current;
    setCopyResult(null);
    try {
      await navigator.clipboard.writeText(context.clientUrl);
      if (!mounted.current || generation !== copyGeneration.current) return;
      setCopyResult("success");
    } catch {
      if (!mounted.current || generation !== copyGeneration.current) return;
      setCopyResult("error");
    }
  }, [context?.clientUrl, status?.running]);

  const handleStop = useCallback(async () => {
    if (!status?.running || lifecycleInFlight.current) return;
    lifecycleInFlight.current = true;
    pollGeneration.current += 1;
    invalidateCopyAndBrowse();
    const generation = ++stopGeneration.current;
    setStopping(true);
    setError(null);
    try {
      const next = await api.hostServerStop();
      if (!mounted.current || generation !== stopGeneration.current) return;
      applyStatus(next);
    } catch (caught) {
      if (!mounted.current || generation !== stopGeneration.current) return;
      activeContextKey.current = null;
      invalidateCopyAndBrowse();
      setStatus(null);
      setError(messageFrom(caught));
    } finally {
      if (mounted.current && generation === stopGeneration.current) {
        lifecycleInFlight.current = false;
        setStopping(false);
      }
    }
  }, [applyStatus, invalidateCopyAndBrowse, status?.running]);

  const handleBrowse = useCallback(() => {
    if (!status?.running || !context?.clientUrl || lifecycleInFlight.current) return;
    browseController.current?.abort();
    const controller = new AbortController();
    browseController.current = controller;
    void Promise.resolve(
      onBrowseRepositories(context.clientUrl, controller.signal),
    )
      .catch(() => {
        // The shell owns repository-list error presentation. A rejected browse
        // must not become an unhandled promise rejection in the status card.
      })
      .finally(() => {
        if (browseController.current === controller) {
          browseController.current = null;
        }
      });
  }, [context?.clientUrl, onBrowseRepositories, status?.running]);

  const running = status?.running === true;
  const runningActionsEnabled = running && !stopping;
  const stateLabel = running
    ? "Hosted on this device"
    : error
      ? "Server status unavailable"
      : status
        ? "Server stopped"
        : "Checking hosted server…";

  return (
    <section className="hosted-server-card" aria-label="Hosted server">
      <div className="hosted-server-card__heading">
        <div>
          <p className="project-hub-eyebrow">Hosted server</p>
          <h2>{stateLabel}</h2>
        </div>
        {running && status.pid !== undefined && (
          <span className="hosted-server-card__process">
            PID {status.pid} · Process running
          </span>
        )}
      </div>

      {error && <div role="alert" className="error hosted-server-card__error">{error}</div>}

      {context ? (
        <dl className="hosted-server-card__details">
          <div>
            <dt>Name</dt>
            <dd>{context.serverName ?? "Unnamed server"}</dd>
          </div>
          <div>
            <dt>Client URL</dt>
            <dd><code>{context.clientUrl}</code></dd>
          </div>
          {context.clientUrl !== context.localUrl && (
            <div>
              <dt>Local URL</dt>
              <dd><code>{context.localUrl}</code></dd>
            </div>
          )}
          <div>
            <dt>Store</dt>
            <dd><code>{context.storeDir}</code></dd>
          </div>
          <div>
            <dt>Authentication</dt>
            <dd>
              Authentication: {context.authRequired ? "Required" : "Not required"}
            </dd>
          </div>
        </dl>
      ) : (
        status && (
          <p className="hosted-server-card__empty">
            No hosted server context is available in this app session.
          </p>
        )
      )}

      <div className="hosted-server-card__actions">
        <button
          type="button"
          disabled={!runningActionsEnabled || !context?.clientUrl}
          onClick={handleBrowse}
        >
          Browse repositories
        </button>
        <button
          type="button"
          disabled={!runningActionsEnabled || !context?.clientUrl}
          onClick={() => void handleCopy()}
        >
          Copy URL
        </button>
        <button type="button" disabled title={RESTART_DISABLED_REASON}>
          Restart
        </button>
        <button
          type="button"
          disabled={!running || stopping}
          onClick={() => void handleStop()}
        >
          {stopping ? "Stopping…" : "Stop"}
        </button>
      </div>
      <p className="hosted-server-card__restart-reason">
        {RESTART_DISABLED_REASON}
      </p>
      {copyResult === "success" && <p role="status">URL copied.</p>}
      {copyResult === "error" && (
        <p role="alert">Could not copy the server URL.</p>
      )}
    </section>
  );
}
