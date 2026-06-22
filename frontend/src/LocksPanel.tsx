import { useCallback, useEffect, useState } from "react";
import {
  lockFileQueryApi,
  lockFileStatusApi,
  lockFileAcquireApi,
  lockFileAcquireAsOwnerApi,
  lockFileReleaseApi,
  type LockEntry,
  type LockStatus,
} from "./api";
import LockRequestButton from "./locks/LockRequestButton";

/**
 * Locks panel (sidebar/topbar nav, daily domain) — the rich home for the lock
 * domain, per `docs/INFORMATION-ARCHITECTURE.md` (lock row: "Locks panel + file
 * row menu", a daily core-loop domain).
 *
 * Surfaces every registered lock_* command:
 *  - query  → list held locks on a branch, optionally filtered by owner/path.
 *  - status → check who holds the lock for specific files.
 *  - acquire → claim exclusive locks for the current user.
 *  - acquire_as_owner → claim locks on behalf of another user (owner action).
 *  - release → release locks; releasing another user's lock is an owner action
 *    and confirms before proceeding.
 *
 * Each section handles empty / loading / error / success and is themed entirely
 * via `--surface-*` tokens, reusing the shared overlay-panel classes from
 * StoragePanel/RepositoryPanel (no new styles needed). Esc closes; one primary
 * action per section.
 */

function errMsg(e: unknown): string {
  return typeof e === "string" ? e : JSON.stringify(e);
}

function fmtTime(secs: number): string {
  if (!secs) return "—";
  try {
    return new Date(secs * 1000).toLocaleString();
  } catch {
    return String(secs);
  }
}

/** Split a textarea value into trimmed non-empty path lines. */
function splitPaths(raw: string): string[] {
  return raw
    .split("\n")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

export default function LocksPanel({ onClose }: { onClose: () => void }) {
  // --- query (list held locks) ---
  const [qBranch, setQBranch] = useState("");
  const [qOwner, setQOwner] = useState("");
  const [qPath, setQPath] = useState("");
  const [locks, setLocks] = useState<LockEntry[] | null>(null);
  const [queryLoading, setQueryLoading] = useState(false);
  const [queryError, setQueryError] = useState<string | null>(null);

  // --- status (per-file lock state) ---
  const [sPaths, setSPaths] = useState("");
  const [sBranch, setSBranch] = useState("");
  const [statusLocks, setStatusLocks] = useState<LockStatus[] | null>(null);
  const [statusLoading, setStatusLoading] = useState(false);
  const [statusError, setStatusError] = useState<string | null>(null);

  // --- acquire (for current user) ---
  const [aPaths, setAPaths] = useState("");
  const [aBranch, setABranch] = useState("");
  const [acquiring, setAcquiring] = useState(false);
  const [acquireResult, setAcquireResult] = useState<{
    acquired: string[];
    ignored: string[];
  } | null>(null);
  const [acquireError, setAcquireError] = useState<string | null>(null);

  // --- acquire as owner (on behalf of another user) ---
  const [aoPaths, setAoPaths] = useState("");
  const [aoBranch, setAoBranch] = useState("");
  const [aoOwner, setAoOwner] = useState("");
  const [aoAcquiring, setAoAcquiring] = useState(false);
  const [aoResult, setAoResult] = useState<{
    acquired: string[];
    ignored: string[];
  } | null>(null);
  const [aoError, setAoError] = useState<string | null>(null);

  // --- release (owner action) ---
  const [rPaths, setRPaths] = useState("");
  const [rBranch, setRBranch] = useState("");
  const [rOwner, setROwner] = useState("");
  const [rOwnerId, setROwnerId] = useState("");
  const [confirmRelease, setConfirmRelease] = useState(false);
  const [releasing, setReleasing] = useState(false);
  const [releaseResult, setReleaseResult] = useState<{
    released: string[];
    not_found: boolean;
  } | null>(null);
  const [releaseError, setReleaseError] = useState<string | null>(null);

  // Esc closes the panel (DESIGN-SYSTEM: overlays dismiss on Esc).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const runQuery = useCallback(async () => {
    setQueryLoading(true);
    setQueryError(null);
    try {
      const res = await lockFileQueryApi.fileQuery(
        qBranch.trim(),
        qOwner.trim(),
        qPath.trim(),
      );
      setLocks(res.locks);
    } catch (e) {
      setLocks(null);
      setQueryError(errMsg(e));
    } finally {
      setQueryLoading(false);
    }
  }, [qBranch, qOwner, qPath]);

  // Load all locks on first open so the panel isn't blank.
  useEffect(() => {
    void runQuery();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const runStatus = useCallback(async () => {
    const paths = splitPaths(sPaths);
    if (paths.length === 0) return;
    setStatusLoading(true);
    setStatusError(null);
    setStatusLocks(null);
    try {
      const res = await lockFileStatusApi.fileStatus(paths, sBranch.trim());
      setStatusLocks(res.locks);
    } catch (e) {
      setStatusError(errMsg(e));
    } finally {
      setStatusLoading(false);
    }
  }, [sPaths, sBranch]);

  const runAcquire = useCallback(async () => {
    const paths = splitPaths(aPaths);
    if (paths.length === 0) return;
    setAcquiring(true);
    setAcquireError(null);
    setAcquireResult(null);
    try {
      setAcquireResult(
        await lockFileAcquireApi.fileAcquire(paths, aBranch.trim()),
      );
      void runQuery();
    } catch (e) {
      setAcquireError(errMsg(e));
    } finally {
      setAcquiring(false);
    }
  }, [aPaths, aBranch, runQuery]);

  const runAcquireAsOwner = useCallback(async () => {
    const paths = splitPaths(aoPaths);
    if (paths.length === 0 || !aoOwner.trim()) return;
    setAoAcquiring(true);
    setAoError(null);
    setAoResult(null);
    try {
      setAoResult(
        await lockFileAcquireAsOwnerApi.fileAcquireAsOwner(
          paths,
          aoBranch.trim(),
          aoOwner.trim(),
        ),
      );
      void runQuery();
    } catch (e) {
      setAoError(errMsg(e));
    } finally {
      setAoAcquiring(false);
    }
  }, [aoPaths, aoBranch, aoOwner, runQuery]);

  const runRelease = useCallback(async () => {
    const paths = splitPaths(rPaths);
    if (paths.length === 0 || !rOwner.trim() || !rOwnerId.trim()) return;
    setReleasing(true);
    setReleaseError(null);
    setReleaseResult(null);
    try {
      setReleaseResult(
        await lockFileReleaseApi.fileRelease(
          paths,
          rBranch.trim(),
          rOwner.trim(),
          rOwnerId.trim(),
        ),
      );
      void runQuery();
    } catch (e) {
      setReleaseError(errMsg(e));
    } finally {
      setReleasing(false);
      setConfirmRelease(false);
    }
  }, [rPaths, rBranch, rOwner, rOwnerId, runQuery]);

  // Prefill the release form from a held lock row (one-click owner action).
  const prefillRelease = useCallback((lock: LockEntry) => {
    setRPaths(lock.path);
    setRBranch(lock.branch);
    setROwner(lock.owner);
    setROwnerId("");
    setConfirmRelease(false);
    setReleaseResult(null);
    setReleaseError(null);
  }, []);

  const releasePaths = splitPaths(rPaths);
  const releaseReady =
    releasePaths.length > 0 && !!rOwner.trim() && !!rOwnerId.trim();

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Locks"
      className="storage-scrim"
      onClick={onClose}
    >
      <div className="storage-panel" onClick={(e) => e.stopPropagation()}>
        <header className="storage-panel-header">
          <h2>Locks</h2>
          <button onClick={onClose} title="Close (Esc)">
            Close
          </button>
        </header>

        {/* --- Held locks (query) --- */}
        <section className="storage-section">
          <h3>Held locks</h3>
          <p className="storage-help">
            List the file locks currently held on a branch. Filter by owner or
            path prefix, or leave fields empty for everything on the current
            branch.
          </p>
          <div className="onboarding-field">
            <label htmlFor="lock-q-branch">Branch (empty = current)</label>
            <input
              id="lock-q-branch"
              type="text"
              value={qBranch}
              onChange={(e) => setQBranch(e.target.value)}
              placeholder="e.g. main"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="lock-q-owner">Owner (empty = all)</label>
            <input
              id="lock-q-owner"
              type="text"
              value={qOwner}
              onChange={(e) => setQOwner(e.target.value)}
              placeholder="e.g. user@example.com"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="lock-q-path">Path prefix (empty = all)</label>
            <input
              id="lock-q-path"
              type="text"
              value={qPath}
              onChange={(e) => setQPath(e.target.value)}
              placeholder="e.g. Content/Maps/"
            />
          </div>
          {queryError && (
            <div className="error storage-inline-error">{queryError}</div>
          )}
          {locks && !queryLoading && (
            <>
              {locks.length === 0 ? (
                <p className="empty">
                  No locks held — acquire one from a file or below.
                </p>
              ) : (
                <ul className="storage-list">
                  {locks.map((lock, i) => (
                    <li key={`${lock.branch}:${lock.path}:${i}`}>
                      <code>{lock.path}</code>
                      <span className="storage-status bad">
                        ● {lock.owner || "(unknown)"} · {lock.branch} ·{" "}
                        {fmtTime(lock.locked_at)}
                      </span>
                      <button
                        onClick={() => prefillRelease(lock)}
                        title="Fill the release form below with this lock"
                      >
                        Release…
                      </button>
                      {/* Ask the holder to check in — renders only when the
                          lock is held by someone else (SBAI-4044). */}
                      <LockRequestButton
                        path={lock.path}
                        branch={lock.branch}
                        compact
                      />
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
          <button
            className="storage-primary"
            disabled={queryLoading}
            onClick={() => void runQuery()}
          >
            {queryLoading ? "Querying…" : "Query locks"}
          </button>
        </section>

        {/* --- File status (who holds it) --- */}
        <section className="storage-section">
          <h3>File status</h3>
          <p className="storage-help">
            Check the lock state of specific files — who holds each one, and
            when it was locked.
          </p>
          <div className="onboarding-field">
            <label htmlFor="lock-s-paths">Paths (one per line)</label>
            <textarea
              id="lock-s-paths"
              value={sPaths}
              onChange={(e) => setSPaths(e.target.value)}
              placeholder={
                "Content/Characters/hero.uasset\nContent/Maps/main.umap"
              }
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="lock-s-branch">Branch (empty = current)</label>
            <input
              id="lock-s-branch"
              type="text"
              value={sBranch}
              onChange={(e) => setSBranch(e.target.value)}
              placeholder="e.g. main"
            />
          </div>
          {statusError && (
            <div className="error storage-inline-error">{statusError}</div>
          )}
          {statusLocks && !statusLoading && (
            <>
              {statusLocks.length === 0 ? (
                <p className="empty">
                  None of those files are locked — they're free to edit.
                </p>
              ) : (
                <ul className="storage-list">
                  {statusLocks.map((lock, i) => (
                    <li key={`${lock.path}:${i}`}>
                      <code>{lock.path}</code>
                      <span className="storage-status bad">
                        ● held by {lock.owner || "(unknown)"} ·{" "}
                        {fmtTime(lock.locked_at)}
                      </span>
                      <LockRequestButton
                        path={lock.path}
                        branch={sBranch.trim()}
                        compact
                      />
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
          <button
            disabled={splitPaths(sPaths).length === 0 || statusLoading}
            onClick={() => void runStatus()}
          >
            {statusLoading ? "Checking…" : "Check status"}
          </button>
        </section>

        {/* --- Acquire (current user) --- */}
        <section className="storage-section">
          <h3>Acquire locks</h3>
          <p className="storage-help">
            Claim exclusive locks on one or more files for yourself, so no one
            else can edit them until you release.
          </p>
          <div className="onboarding-field">
            <label htmlFor="lock-a-paths">Paths (one per line)</label>
            <textarea
              id="lock-a-paths"
              value={aPaths}
              onChange={(e) => setAPaths(e.target.value)}
              placeholder={
                "Content/Characters/hero.uasset\nContent/Maps/main.umap"
              }
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="lock-a-branch">Branch (empty = current)</label>
            <input
              id="lock-a-branch"
              type="text"
              value={aBranch}
              onChange={(e) => setABranch(e.target.value)}
              placeholder="e.g. main"
            />
          </div>
          {acquireError && (
            <div className="error storage-inline-error">{acquireError}</div>
          )}
          {acquireResult && !acquiring && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Acquired{" "}
              {acquireResult.acquired.length}
              {acquireResult.ignored.length > 0
                ? ` · ${acquireResult.ignored.length} already locked`
                : ""}
              .
            </div>
          )}
          <button
            disabled={splitPaths(aPaths).length === 0 || acquiring}
            onClick={() => void runAcquire()}
          >
            {acquiring ? "Acquiring…" : "Acquire locks"}
          </button>
        </section>

        {/* --- Acquire as owner (owner/admin) --- */}
        <section className="storage-section">
          <h3>Acquire on behalf of another user</h3>
          <p className="storage-help">
            Owner action: acquire locks on behalf of a specified user (e.g. to
            reserve files for a teammate). Requires the owner's user ID.
          </p>
          <div className="onboarding-field">
            <label htmlFor="lock-ao-paths">Paths (one per line)</label>
            <textarea
              id="lock-ao-paths"
              value={aoPaths}
              onChange={(e) => setAoPaths(e.target.value)}
              placeholder={
                "Content/Characters/hero.uasset\nContent/Maps/main.umap"
              }
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="lock-ao-owner">Owner (user ID) *</label>
            <input
              id="lock-ao-owner"
              type="text"
              value={aoOwner}
              onChange={(e) => setAoOwner(e.target.value)}
              placeholder="e.g. user@example.com"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="lock-ao-branch">Branch (empty = current)</label>
            <input
              id="lock-ao-branch"
              type="text"
              value={aoBranch}
              onChange={(e) => setAoBranch(e.target.value)}
              placeholder="e.g. main"
            />
          </div>
          {aoError && (
            <div className="error storage-inline-error">{aoError}</div>
          )}
          {aoResult && !aoAcquiring && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Acquired{" "}
              {aoResult.acquired.length} for {aoOwner.trim()}
              {aoResult.ignored.length > 0
                ? ` · ${aoResult.ignored.length} already locked`
                : ""}
              .
            </div>
          )}
          <button
            disabled={
              splitPaths(aoPaths).length === 0 ||
              !aoOwner.trim() ||
              aoAcquiring
            }
            onClick={() => void runAcquireAsOwner()}
          >
            {aoAcquiring ? "Acquiring…" : "Acquire as owner"}
          </button>
        </section>

        {/* --- Release (owner action, confirms) --- */}
        <section className="storage-section storage-danger">
          <h3>Release locks</h3>
          <p className="storage-help">
            Release the lock for the given paths. Releasing a lock you don't
            hold — someone else's lock — is an owner action that frees their
            reservation, so it's confirmed first. Use a held-lock row above to
            prefill the owner.
          </p>
          <div className="onboarding-field">
            <label htmlFor="lock-r-paths">Paths (one per line)</label>
            <textarea
              id="lock-r-paths"
              value={rPaths}
              onChange={(e) => {
                setRPaths(e.target.value);
                setConfirmRelease(false);
              }}
              placeholder={
                "Content/Characters/hero.uasset\nContent/Maps/main.umap"
              }
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="lock-r-owner">Owner *</label>
            <input
              id="lock-r-owner"
              type="text"
              value={rOwner}
              onChange={(e) => {
                setROwner(e.target.value);
                setConfirmRelease(false);
              }}
              placeholder="e.g. user@example.com"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="lock-r-owner-id">Owner ID *</label>
            <input
              id="lock-r-owner-id"
              type="text"
              value={rOwnerId}
              onChange={(e) => {
                setROwnerId(e.target.value);
                setConfirmRelease(false);
              }}
              placeholder="e.g. 12345"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="lock-r-branch">Branch (empty = current)</label>
            <input
              id="lock-r-branch"
              type="text"
              value={rBranch}
              onChange={(e) => setRBranch(e.target.value)}
              placeholder="e.g. main"
            />
          </div>
          {releaseError && (
            <div className="error storage-inline-error">{releaseError}</div>
          )}
          {releaseResult && !releasing && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Released{" "}
              {releaseResult.released.length}
              {releaseResult.not_found
                ? " · some requested locks were not found"
                : ""}
              .
            </div>
          )}
          {!confirmRelease ? (
            <button
              className="storage-danger-btn"
              disabled={!releaseReady || releasing}
              onClick={() => setConfirmRelease(true)}
            >
              Release locks
            </button>
          ) : (
            <div className="storage-confirm">
              <span>
                Release {releasePaths.length} lock
                {releasePaths.length === 1 ? "" : "s"} held by{" "}
                <code>{rOwner.trim()}</code>? If this isn't your own lock, you're
                freeing another user's reservation.
              </span>
              <button
                className="storage-danger-btn"
                disabled={releasing}
                onClick={() => void runRelease()}
              >
                {releasing ? "Releasing…" : "Yes, release"}
              </button>
              <button
                disabled={releasing}
                onClick={() => setConfirmRelease(false)}
              >
                Cancel
              </button>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
