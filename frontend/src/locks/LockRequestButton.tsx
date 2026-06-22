import { useCallback, useEffect, useState } from "react";
import { api, lockFileStatusApi, lockMessagingApi } from "../api";

/**
 * "Request check-in" action for a single file (SBAI-4044).
 *
 * Reusable across surfaces that show one file — the Locks panel row and the
 * content/file workspace header. On mount it checks the file's lock status; it
 * renders an action ONLY when the file is locked by **someone else** (you can't
 * meaningfully ask yourself to release your own lock). Clicking sends a check-in
 * request to the holder, who receives a tray notification + inbox entry.
 *
 * Transport: core build delivers locally (same machine); cross-network delivery
 * is the premium relay (SBAI-4072). See docs/lock-messaging-spike.md.
 *
 * Themed via `--surface-*` (reuses shared button classes); no hardcoded colors.
 */

function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    const o = e as { message?: unknown };
    if (typeof o.message === "string") return o.message;
  }
  return JSON.stringify(e);
}

type Holder = { owner: string } | null;

export default function LockRequestButton({
  path,
  branch,
  className = "",
  compact = false,
}: {
  path: string;
  /** Branch the lock is on; empty = current. */
  branch?: string;
  className?: string;
  /** Render as a tight inline control (file row) vs a labelled button. */
  compact?: boolean;
}) {
  const [holder, setHolder] = useState<Holder>(null);
  const [me, setMe] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [res, user] = await Promise.all([
          lockFileStatusApi.fileStatus([path], branch ?? ""),
          api.authUserInfo().catch(() => null),
        ]);
        if (cancelled) return;
        const lock = res.locks.find((l) => l.path === path) ?? null;
        setHolder(lock ? { owner: lock.owner } : null);
        setMe(user ? user.id || user.name : null);
      } catch {
        // Lock status unavailable (e.g. local-only repo) — render nothing.
        if (!cancelled) setHolder(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [path, branch]);

  const send = useCallback(async () => {
    if (!holder) return;
    setSending(true);
    setError(null);
    try {
      const user = await api.authUserInfo().catch(() => null);
      const from = user ? user.name || user.id : "A teammate";
      await lockMessagingApi.requestCheckin({
        path,
        branch: branch ?? "",
        from,
        toUserId: holder.owner,
        holder: holder.owner,
      });
      setSent(true);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setSending(false);
    }
  }, [holder, path, branch]);

  // Nothing to do: file isn't locked, or it's locked by you.
  if (!holder) return null;
  const ownLock = me != null && holder.owner === me;
  if (ownLock) return null;

  if (sent) {
    return (
      <span className={`lock-req-sent ${className}`} role="status">
        Requested check-in from {holder.owner}.
      </span>
    );
  }

  const label = compact ? "Request…" : "Request check-in";
  return (
    <span className={`lock-req ${className}`}>
      <button
        type="button"
        className="lock-req-btn"
        disabled={sending}
        onClick={() => void send()}
        title={`Ask ${holder.owner} to check in this file`}
        aria-label={`Request check-in of ${path} from ${holder.owner}`}
      >
        {sending ? "Sending…" : label}
      </button>
      {error && (
        <span className="lock-req-error error" role="alert">
          {error}
        </span>
      )}
    </span>
  );
}
