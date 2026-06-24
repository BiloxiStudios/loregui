import { useCallback, useEffect, useState } from "react";
import {
  api,
  lockFileReleaseApi,
  lockMessagingApi,
  LOCK_REQUEST_EVENT,
  type LockRequest,
} from "../api";
import { listen } from "@tauri-apps/api/event";

/**
 * Lock-request inbox drawer (SBAI-4044).
 *
 * The holder's surface for incoming "please check in <file>" requests. Each row
 * shows who is asking and which file, with two actions:
 *   - Release: calls `lock.file_release` for the file, then dismisses the row.
 *   - Dismiss: drops the request without releasing.
 *
 * It listens live for the `lock/request` Tauri event (fired when a request lands
 * locally) so the list updates without a reopen, and refetches on open. Themed
 * via `--surface-*` (reuses the shared overlay-panel classes); Esc closes.
 */

function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    const o = e as { message?: unknown };
    if (typeof o.message === "string") return o.message;
  }
  return JSON.stringify(e);
}

function fmtTime(ms: number): string {
  if (!ms) return "";
  try {
    return new Date(ms).toLocaleString();
  } catch {
    return "";
  }
}

export default function LockInbox({ onClose }: { onClose: () => void }) {
  const [requests, setRequests] = useState<LockRequest[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setRequests(await lockMessagingApi.inboxList());
    } catch (e) {
      setError(errMsg(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Live updates: a new request arriving locally fires `lock/request`.
  useEffect(() => {
    // Capture the promise so cleanup awaits listen() — a synchronous unlisten
    // no-ops (leaking a duplicate handler) if torn down before listen resolves.
    const p = listen<LockRequest>(LOCK_REQUEST_EVENT, () => {
      void refresh();
    });
    return () => {
      void p.then((un) => un());
    };
  }, [refresh]);

  // Esc closes (DESIGN-SYSTEM: overlays dismiss on Esc).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const dismiss = useCallback(
    async (id: string) => {
      setBusyId(id);
      setError(null);
      try {
        await lockMessagingApi.inboxDismiss(id);
        await refresh();
      } catch (e) {
        setError(errMsg(e));
      } finally {
        setBusyId(null);
      }
    },
    [refresh],
  );

  const release = useCallback(
    async (req: LockRequest) => {
      setBusyId(req.id);
      setError(null);
      try {
        const user = await api.authUserInfo().catch(() => null);
        // Release our own lock — owner is the signed-in user.
        await lockFileReleaseApi.fileRelease(
          [req.path],
          req.branch,
          user?.name || user?.id || req.holder,
          user?.id || req.toUserId,
        );
        await lockMessagingApi.inboxDismiss(req.id);
        await refresh();
      } catch (e) {
        setError(errMsg(e));
      } finally {
        setBusyId(null);
      }
    },
    [refresh],
  );

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Lock requests"
      className="storage-scrim"
      onClick={onClose}
    >
      <div className="storage-panel" onClick={(e) => e.stopPropagation()}>
        <header className="storage-panel-header">
          <h2>Lock requests</h2>
          <button onClick={onClose} title="Close (Esc)">
            Close
          </button>
        </header>

        <section className="storage-section">
          <p className="storage-help">
            Teammates asking you to check in (release) files you have locked.
            Release frees the lock for them; Dismiss clears the request without
            releasing.
          </p>

          {error && (
            <div className="error storage-inline-error">{error}</div>
          )}

          {requests.length === 0 ? (
            <p className="empty">No pending requests.</p>
          ) : (
            <ul className="storage-list">
              {requests.map((req) => (
                <li key={req.id}>
                  <code>{req.path}</code>
                  <span className="storage-status">
                    {req.from} wants you to check this in
                    {req.branch ? ` · ${req.branch}` : ""}
                    {fmtTime(req.createdAt) ? ` · ${fmtTime(req.createdAt)}` : ""}
                  </span>
                  {req.note && <span className="lock-req-note">“{req.note}”</span>}
                  <button
                    className="storage-primary"
                    disabled={busyId === req.id}
                    onClick={() => void release(req)}
                    title={`Release the lock on ${req.path}`}
                  >
                    {busyId === req.id ? "Releasing…" : "Release"}
                  </button>
                  <button
                    disabled={busyId === req.id}
                    onClick={() => void dismiss(req.id)}
                    title="Clear this request without releasing"
                  >
                    Dismiss
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
