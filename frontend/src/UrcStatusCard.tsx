import { useCallback, useState } from "react";
import {
  branchMergeAbortApi,
  branchMergeResolveMineApi,
  branchMergeResolveTheirsApi,
  repositoryRecoverApi,
  revisionSyncApi,
  type RepositoryRecoverLocalResult,
  type UrcStatus,
} from "./api";

/**
 * UrcStatusCard (SBAI-5499) — first-class local working-tree (URC) health in
 * the Changes panel.
 *
 * The App shell fetches `repository_urc_status` during `refresh()` and hands
 * the snapshot down; this card owns the rendering of every non-healthy state
 * and the actions that resolve them:
 *
 * - loading            → quiet "checking…" line
 * - healthy            → nothing (quiet success — the Changes panel is the status)
 * - pendingMerge       → incoming revision + resolve mine/theirs + abort (confirm)
 * - conflicts          → conflict list + resolve mine/theirs
 * - diverged           → local/remote revisions + reset-to-remote (confirm, destructive)
 * - error (status fetch failed, not NoRepository)
 *                      → "repository unreachable" + recover-local (confirm, destructive)
 *
 * Every action re-runs `onRefresh` on completion and surfaces the real backend
 * error message on failure. The card never touches staging/commit — the commit
 * box below it stays the only commit affordance.
 */

export interface UrcStatusCardProps {
  /** Latest health snapshot; null while unknown or when the fetch failed. */
  status: UrcStatus | null;
  /** Non-NoRepository failure from the status fetch — the tree may need recovery. */
  error: string | null;
  /** True while the first status fetch is in flight. */
  loading?: boolean;
  /** Re-run the shell refresh after an action completes. */
  onRefresh: () => void | Promise<void>;
}

function messageFrom(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") return message;
  }
  return "the action failed";
}

function shortRev(rev: string): string {
  return rev ? rev.slice(0, 12) : "—";
}

type UrcKind = "pendingMerge" | "conflicts" | "diverged" | "healthy";

function kindOf(status: UrcStatus): UrcKind {
  if (status.pendingMerge) return "pendingMerge";
  if (status.conflicts.length > 0) return "conflicts";
  if (status.diverged) return "diverged";
  return "healthy";
}

export default function UrcStatusCard({
  status,
  error,
  loading = false,
  onRefresh,
}: UrcStatusCardProps) {
  const [busy, setBusy] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [recoverResult, setRecoverResult] =
    useState<RepositoryRecoverLocalResult | null>(null);

  const runAction = useCallback(
    async (label: string, fn: () => Promise<void>) => {
      setBusy(label);
      setActionError(null);
      try {
        await fn();
        await onRefresh();
      } catch (e) {
        setActionError(messageFrom(e));
      } finally {
        setBusy(null);
      }
    },
    [onRefresh],
  );

  if (loading && !status && !error) {
    return (
      <p className="urc-status urc-status--loading">checking repository state…</p>
    );
  }

  // The status command itself failed (and it was NOT the expected first-run
  // NoRepository, which the shell filters out): the local tree is unreadable.
  // This is the "needs recovery" state — distinct from a fetched-but-unhealthy
  // status, which is "needs resolution".
  if (error) {
    return (
      <section className="urc-status urc-status--error" aria-label="repository status">
        <h3>repository unreachable</h3>
        <p>the local working tree could not be read and may need recovery.</p>
        <p className="urc-status__error-detail" role="alert">{error}</p>
        {recoverResult ? (
          <div className="urc-status__recover-result">
            <p>
              recovered to <code>{recoverResult.recoveredDir}</code>
            </p>
            {recoverResult.preservedDir && (
              <p>
                previous tree preserved at <code>{recoverResult.preservedDir}</code>
              </p>
            )}
          </div>
        ) : (
          <div className="urc-status__actions">
            <button
              className="urc-status__primary"
              disabled={busy !== null}
              onClick={() => {
                if (
                  window.confirm(
                    "Recover the local repository? The current working tree is preserved and a fresh local copy is checked out.",
                  )
                ) {
                  void runAction("recover", async () => {
                    setRecoverResult(await repositoryRecoverApi.recoverLocal());
                  });
                }
              }}
            >
              {busy === "recover" ? "recovering…" : "recover local repository"}
            </button>
            <button
              disabled={busy !== null}
              onClick={() => void runAction("retry", async () => {})}
            >
              retry
            </button>
          </div>
        )}
        {actionError && (
          <p className="urc-status__error-detail" role="alert">{actionError}</p>
        )}
      </section>
    );
  }

  // Quiet success: a healthy tree needs no banner — the Changes panel below
  // already IS the status view.
  if (!status) return null;
  const kind = kindOf(status);
  if (kind === "healthy") return null;

  const resolveMine = () =>
    void runAction("resolve-mine", async () => {
      await branchMergeResolveMineApi.mergeResolveMine(status.conflicts);
    });
  const resolveTheirs = () =>
    void runAction("resolve-theirs", async () => {
      await branchMergeResolveTheirsApi.mergeResolveTheirs(status.conflicts);
    });
  const abortMerge = () => {
    if (
      window.confirm(
        "Abort the current merge? This will revert the working directory to its pre-merge state.",
      )
    ) {
      void runAction("abort", async () => {
        await branchMergeAbortApi.mergeAbort();
      });
    }
  };
  const resetToRemote = () => {
    if (
      window.confirm(
        `Reset "${status.branch}" to the remote revision ${shortRev(status.remoteRev)}? Local changes will be discarded.`,
      )
    ) {
      void runAction("reset", async () => {
        await revisionSyncApi.sync(status.remoteRev, false, true);
      });
    }
  };

  return (
    <section
      className={`urc-status urc-status--${kind === "diverged" ? "error" : "warning"}`}
      aria-label="repository status"
    >
      {kind === "pendingMerge" && (
        <>
          <h3>
            merge in progress
            <span className="badge needs-resolution">needs resolution</span>
          </h3>
          <p>
            branch <strong>{status.branch}</strong> has an unfinished merge —
            incoming revision <code>{shortRev(status.remoteRev)}</code>.
          </p>
        </>
      )}
      {kind === "conflicts" && (
        <h3>
          conflicts
          <span className="badge needs-resolution">needs resolution</span>
        </h3>
      )}
      {kind === "diverged" && (
        <>
          <h3>
            branch diverged
            <span className="badge needs-resolution">needs resolution</span>
          </h3>
          <p>
            <strong>{status.branch}</strong> is at{" "}
            <code>{shortRev(status.currentRev)}</code> locally but the remote is
            at <code>{shortRev(status.remoteRev)}</code>.
          </p>
          {status.staged.length > 0 && (
            <p>
              {status.staged.length} staged file
              {status.staged.length === 1 ? "" : "s"} present — a reset discards
              them.
            </p>
          )}
        </>
      )}

      {status.conflicts.length > 0 && (
        <ul className="urc-status__conflicts">
          {status.conflicts.map((path) => (
            <li key={path}>
              <code>{path}</code>
            </li>
          ))}
        </ul>
      )}

      <div className="urc-status__actions">
        {kind === "diverged" ? (
          <button
            className="urc-status__primary"
            disabled={busy !== null}
            onClick={resetToRemote}
          >
            {busy === "reset" ? "resetting…" : "reset to remote"}
          </button>
        ) : (
          <>
            <button disabled={busy !== null} onClick={resolveMine}>
              {busy === "resolve-mine" ? "resolving…" : "resolve mine"}
            </button>
            <button disabled={busy !== null} onClick={resolveTheirs}>
              {busy === "resolve-theirs" ? "resolving…" : "resolve theirs"}
            </button>
            {kind === "pendingMerge" && (
              <button disabled={busy !== null} onClick={abortMerge}>
                {busy === "abort" ? "aborting…" : "abort merge"}
              </button>
            )}
          </>
        )}
      </div>

      {actionError && (
        <p className="urc-status__error-detail" role="alert">{actionError}</p>
      )}
    </section>
  );
}
