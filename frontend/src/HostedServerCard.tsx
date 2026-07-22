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
  onBrowseRepositories: (url: string) => void;
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
  const actionGeneration = useRef(0);
  const lifecycleInFlight = useRef(false);

  const applyStatus = useCallback((next: HostStatus) => {
    if (next.running) {
      const nextContext = contextFromRunningStatus(next);
      if (!nextContext) {
        setStatus(null);
        setError("Hosted server status is incomplete.");
        return;
      }
      sessionLastContext = nextContext;
      setContext(nextContext);
      setStatus(next);
      setError(null);
      return;
    }
    setStatus(next);
    setError(null);
  }, []);

  const refresh = useCallback(async () => {
    if (lifecycleInFlight.current) return;
    const generation = ++pollGeneration.current;
    try {
      const next = await api.hostServerStatus();
      if (!mounted.current || generation !== pollGeneration.current) return;
      applyStatus(next);
    } catch (caught) {
      if (!mounted.current || generation !== pollGeneration.current) return;
      setStatus(null);
      setError(messageFrom(caught));
    }
  }, [applyStatus]);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    const interval = window.setInterval(() => void refresh(), pollIntervalMs);
    return () => {
      mounted.current = false;
      pollGeneration.current += 1;
      actionGeneration.current += 1;
      lifecycleInFlight.current = false;
      window.clearInterval(interval);
    };
  }, [pollIntervalMs, refresh]);

  const handleCopy = useCallback(async () => {
    if (!status?.running || !context?.clientUrl) return;
    const generation = ++actionGeneration.current;
    setCopyResult(null);
    try {
      await navigator.clipboard.writeText(context.clientUrl);
      if (!mounted.current || generation !== actionGeneration.current) return;
      setCopyResult("success");
    } catch {
      if (!mounted.current || generation !== actionGeneration.current) return;
      setCopyResult("error");
    }
  }, [context?.clientUrl, status?.running]);

  const handleStop = useCallback(async () => {
    if (!status?.running || lifecycleInFlight.current) return;
    lifecycleInFlight.current = true;
    pollGeneration.current += 1;
    const generation = ++actionGeneration.current;
    setStopping(true);
    setError(null);
    try {
      const next = await api.hostServerStop();
      if (!mounted.current || generation !== actionGeneration.current) return;
      applyStatus(next);
    } catch (caught) {
      if (!mounted.current || generation !== actionGeneration.current) return;
      setStatus(null);
      setError(messageFrom(caught));
    } finally {
      if (mounted.current && generation === actionGeneration.current) {
        lifecycleInFlight.current = false;
        setStopping(false);
      }
    }
  }, [applyStatus, status?.running]);

  const running = status?.running === true;
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
          disabled={!running || !context?.clientUrl}
          onClick={() => context && onBrowseRepositories(context.clientUrl)}
        >
          Browse repositories
        </button>
        <button
          type="button"
          disabled={!running || !context?.clientUrl}
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
