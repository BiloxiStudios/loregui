import { useCallback, useEffect, useState } from "react";
import {
  revisionHistoryApi,
  revisionInfoApi,
  revisionDiffApi,
  revisionCommitApi,
  revisionAmendApi,
  revisionFindApi,
  revisionFindLocalApi,
  revisionRevertLocalApi,
  revisionRevertResolveApi,
  type RevisionHistoryEntry,
  type RevisionInfoResult,
  type RevisionDiffResult,
  type RevertLocalResult,
} from "./api";

/**
 * History panel (top-bar nav, daily domain) — the rich home for the revision
 * domain, per `docs/INFORMATION-ARCHITECTURE.md` (revision row: "History panel
 * + revision row menus", a daily core-loop domain). The list of revisions is
 * the centerpiece; selecting one reveals its info + file changes (diff).
 *
 * Surfaces the common revision_* commands that have a registered
 * `#[tauri::command]` (verified in src-tauri/src/commands.rs + lib.rs
 * generate_handler!):
 *  - history       → list revisions (the centerpiece: hash/number/parents).
 *  - info          → a revision's details (parents, deltas, metadata).
 *  - diff          → a revision's file changes (reuses the App.tsx diff shape).
 *  - commit        → create a revision from staged changes.
 *  - amend         → rewrite the latest revision's message.
 *  - find          → search revisions by metadata key/value (repository-wide).
 *  - find_local    → search revisions by metadata key/value (local only).
 *  - revert_local  → revert a revision (destructive; confirms). A stateful flow:
 *                    if it conflicts, resolve the listed files, then
 *  - revert_resolve→ mark reverted conflicts resolved.
 *
 * Revert's `restart`/`abort`/`resolve_mine`/`resolve_theirs`/`unresolve`
 * variants exist as ops but have NO registered tauri command, so they are out
 * of scope for this panel (palette parity only covers registered commands).
 *
 * Each section handles empty / loading / error / success and is themed entirely
 * via `--surface-*` tokens, reusing the shared overlay-panel classes from
 * StoragePanel/RepositoryPanel/LocksPanel/DependenciesPanel (no new styles).
 * Esc closes; one primary action per section; destructive revert confirms.
 */

function errMsg(e: unknown): string {
  return typeof e === "string" ? e : JSON.stringify(e);
}

/** Split a textarea value into trimmed non-empty path lines. */
function splitPaths(raw: string): string[] {
  return raw
    .split("\n")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

export default function HistoryPanel({ onClose }: { onClose: () => void }) {
  // --- history (the centerpiece list) ---
  const [hBranch, setHBranch] = useState("");
  const [hLength, setHLength] = useState("");
  const [hOnlyBranch, setHOnlyBranch] = useState(false);
  const [entries, setEntries] = useState<RevisionHistoryEntry[] | null>(null);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);

  // --- selected revision (drives info + diff) ---
  const [selected, setSelected] = useState<string | null>(null);
  const [info, setInfo] = useState<RevisionInfoResult | null>(null);
  const [infoLoading, setInfoLoading] = useState(false);
  const [infoError, setInfoError] = useState<string | null>(null);
  const [diff, setDiff] = useState<RevisionDiffResult | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);

  // --- commit ---
  const [commitMessage, setCommitMessage] = useState("");
  const [committing, setCommitting] = useState(false);
  const [commitDone, setCommitDone] = useState<{ revision: string } | null>(
    null,
  );
  const [commitError, setCommitError] = useState<string | null>(null);

  // --- amend (rewrites the latest revision) ---
  const [amendMessage, setAmendMessage] = useState("");
  const [confirmAmend, setConfirmAmend] = useState(false);
  const [amending, setAmending] = useState(false);
  const [amendDone, setAmendDone] = useState<{ revision: string } | null>(null);
  const [amendError, setAmendError] = useState<string | null>(null);

  // --- find / find_local (search by metadata) ---
  const [fKey, setFKey] = useState("");
  const [fValue, setFValue] = useState("");
  const [fNumber, setFNumber] = useState("");
  const [fLocal, setFLocal] = useState(false);
  const [findResult, setFindResult] = useState<string[] | null>(null);
  const [finding, setFinding] = useState(false);
  const [findError, setFindError] = useState<string | null>(null);

  // --- revert_local (destructive, stateful) ---
  const [rvRevision, setRvRevision] = useState("");
  const [rvMessage, setRvMessage] = useState("");
  const [rvNoCommit, setRvNoCommit] = useState(false);
  const [confirmRevert, setConfirmRevert] = useState(false);
  const [reverting, setReverting] = useState(false);
  const [revertResult, setRevertResult] = useState<RevertLocalResult | null>(
    null,
  );
  const [revertError, setRevertError] = useState<string | null>(null);

  // --- revert_resolve (clears revert conflicts) ---
  const [resolvePaths, setResolvePaths] = useState("");
  const [resolving, setResolving] = useState(false);
  const [resolveDone, setResolveDone] = useState<string[] | null>(null);
  const [resolveError, setResolveError] = useState<string | null>(null);

  const runHistory = useCallback(async () => {
    setHistoryLoading(true);
    setHistoryError(null);
    try {
      const len = parseInt(hLength, 10);
      const res = await revisionHistoryApi.history(
        "",
        hBranch.trim(),
        0,
        Number.isFinite(len) && len > 0 ? len : 0,
        hOnlyBranch,
      );
      setEntries(res.entries);
    } catch (e) {
      setEntries(null);
      setHistoryError(errMsg(e));
    } finally {
      setHistoryLoading(false);
    }
  }, [hBranch, hLength, hOnlyBranch]);

  // Load history on first open so the panel's centerpiece isn't blank.
  useEffect(() => {
    void runHistory();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Esc closes the panel (DESIGN-SYSTEM: overlays dismiss on Esc).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Select a revision → load its info + diff (the per-revision detail view).
  const selectRevision = useCallback(
    async (revision: string) => {
      if (selected === revision) {
        setSelected(null);
        setInfo(null);
        setDiff(null);
        return;
      }
      setSelected(revision);
      setInfo(null);
      setInfoError(null);
      setDiff(null);
      setDiffError(null);

      setInfoLoading(true);
      setDiffLoading(true);
      try {
        setInfo(await revisionInfoApi.info(revision, true, true));
      } catch (e) {
        setInfoError(errMsg(e));
      } finally {
        setInfoLoading(false);
      }
      try {
        setDiff(await revisionDiffApi.diff(revision));
      } catch (e) {
        setDiffError(errMsg(e));
      } finally {
        setDiffLoading(false);
      }
    },
    [selected],
  );

  const runCommit = useCallback(async () => {
    if (!commitMessage.trim()) return;
    setCommitting(true);
    setCommitError(null);
    setCommitDone(null);
    try {
      const res = await revisionCommitApi.commit(commitMessage.trim());
      setCommitDone({ revision: res.revision });
      setCommitMessage("");
      void runHistory();
    } catch (e) {
      setCommitError(errMsg(e));
    } finally {
      setCommitting(false);
    }
  }, [commitMessage, runHistory]);

  const runAmend = useCallback(async () => {
    if (!amendMessage.trim()) return;
    setAmending(true);
    setAmendError(null);
    setAmendDone(null);
    try {
      const res = await revisionAmendApi.amend(amendMessage.trim());
      setAmendDone({ revision: res.revision });
      setAmendMessage("");
      void runHistory();
    } catch (e) {
      setAmendError(errMsg(e));
    } finally {
      setAmending(false);
      setConfirmAmend(false);
    }
  }, [amendMessage, runHistory]);

  const runFind = useCallback(async () => {
    setFinding(true);
    setFindError(null);
    setFindResult(null);
    try {
      const num = parseInt(fNumber, 10);
      const n = Number.isFinite(num) && num > 0 ? num : 0;
      if (fLocal) {
        const res = await revisionFindLocalApi.findLocal(
          fKey.trim(),
          fValue.trim(),
          n,
        );
        setFindResult(res.revisions.map((r) => r.signature));
      } else {
        const res = await revisionFindApi.find(fKey.trim(), fValue.trim(), n);
        setFindResult(res.revisions.map((r) => r.signature));
      }
    } catch (e) {
      setFindError(errMsg(e));
    } finally {
      setFinding(false);
    }
  }, [fKey, fValue, fNumber, fLocal]);

  const runRevert = useCallback(async () => {
    if (!rvRevision.trim()) return;
    setReverting(true);
    setRevertError(null);
    setRevertResult(null);
    try {
      const res = await revisionRevertLocalApi.revertLocal(
        rvRevision.trim(),
        rvMessage.trim(),
        rvNoCommit,
      );
      setRevertResult(res);
      // Prefill the resolve form if the revert produced conflicts.
      if (res.has_conflicts && res.conflict_files.length > 0) {
        setResolvePaths(res.conflict_files.map((f) => f.path).join("\n"));
      }
      void runHistory();
    } catch (e) {
      setRevertError(errMsg(e));
    } finally {
      setReverting(false);
      setConfirmRevert(false);
    }
  }, [rvRevision, rvMessage, rvNoCommit, runHistory]);

  const runResolve = useCallback(async () => {
    const paths = splitPaths(resolvePaths);
    if (paths.length === 0) return;
    setResolving(true);
    setResolveError(null);
    setResolveDone(null);
    try {
      const res = await revisionRevertResolveApi.revertResolve(paths);
      setResolveDone(res.paths);
    } catch (e) {
      setResolveError(errMsg(e));
    } finally {
      setResolving(false);
    }
  }, [resolvePaths]);

  // Prefill the revert form from a revision row (one-click destructive action).
  const prefillRevert = useCallback((revision: string) => {
    setRvRevision(revision);
    setRvMessage("");
    setConfirmRevert(false);
    setRevertResult(null);
    setRevertError(null);
  }, []);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="History"
      className="storage-scrim"
      onClick={onClose}
    >
      <div className="storage-panel" onClick={(e) => e.stopPropagation()}>
        <header className="storage-panel-header">
          <h2>History</h2>
          <button onClick={onClose} title="Close (Esc)">
            Close
          </button>
        </header>

        {/* --- Revisions (the centerpiece list) --- */}
        <section className="storage-section">
          <h3>Revisions</h3>
          <p className="storage-help">
            The revision history of the current branch. Select a revision to see
            its details and file changes, or revert it.
          </p>
          <div className="onboarding-field">
            <label htmlFor="hist-branch">Branch (empty = current)</label>
            <input
              id="hist-branch"
              type="text"
              value={hBranch}
              onChange={(e) => setHBranch(e.target.value)}
              placeholder="e.g. main"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="hist-length">Limit (0 = all)</label>
            <input
              id="hist-length"
              type="number"
              min="0"
              value={hLength}
              onChange={(e) => setHLength(e.target.value)}
              placeholder="0"
            />
          </div>
          <label
            htmlFor="hist-only-branch"
            style={{ display: "block", marginBottom: 6 }}
          >
            <input
              id="hist-only-branch"
              type="checkbox"
              checked={hOnlyBranch}
              onChange={(e) => setHOnlyBranch(e.target.checked)}
            />{" "}
            Only this branch (exclude merged-in history)
          </label>
          {historyError && (
            <div className="error storage-inline-error">{historyError}</div>
          )}
          {entries && !historyLoading && (
            <>
              {entries.length === 0 ? (
                <p className="empty">
                  No revisions yet. Stage changes and commit below to create the
                  first one.
                </p>
              ) : (
                <ul className="storage-list">
                  {entries.map((rev) => (
                    <li key={rev.revision}>
                      <code>{rev.revision.slice(0, 12)}</code>
                      <span className="storage-status unknown">
                        ● #{rev.revision_number}
                        {rev.parents.length > 1 ? " · merge" : ""}
                      </span>
                      <button
                        onClick={() => void selectRevision(rev.revision)}
                        title="Show this revision's details and file changes"
                      >
                        {selected === rev.revision ? "Hide" : "Details"}
                      </button>
                      <button
                        onClick={() => prefillRevert(rev.revision)}
                        title="Fill the revert form below with this revision"
                      >
                        Revert…
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
          <button
            className="storage-primary"
            disabled={historyLoading}
            onClick={() => void runHistory()}
          >
            {historyLoading ? "Loading…" : "Refresh history"}
          </button>
        </section>

        {/* --- Selected revision detail: info + diff --- */}
        {selected && (
          <section className="storage-section">
            <h3>
              Revision <code>{selected.slice(0, 12)}</code>
            </h3>

            {/* info */}
            {infoLoading && <p className="storage-help">Loading details…</p>}
            {infoError && (
              <div className="error storage-inline-error">{infoError}</div>
            )}
            {info && !infoLoading && info.info && (
              <dl className="metadata-dl">
                <span>
                  <dt>Revision</dt>
                  <dd>
                    <code>{info.info.revision.slice(0, 16)}</code>
                  </dd>
                </span>
                <span>
                  <dt>Number</dt>
                  <dd>#{info.info.revision_number}</dd>
                </span>
                <span>
                  <dt>Parents</dt>
                  <dd>
                    {info.info.parents.length === 0
                      ? "—"
                      : info.info.parents
                          .map((p) => p.slice(0, 12))
                          .join(", ")}
                  </dd>
                </span>
                <span>
                  <dt>Changed files</dt>
                  <dd>{info.deltas.length}</dd>
                </span>
              </dl>
            )}
            {info && !infoLoading && info.metadata.length > 0 && (
              <dl className="metadata-dl">
                {info.metadata.map((m) => (
                  <span key={m.key}>
                    <dt>{m.key}</dt>
                    <dd>
                      <code>{m.value}</code>
                    </dd>
                  </span>
                ))}
              </dl>
            )}

            {/* diff (file changes) — same shape as App.tsx's diff rendering */}
            <p className="storage-help" style={{ marginTop: 12 }}>
              File changes
            </p>
            {diffLoading && <p className="storage-help">Loading diff…</p>}
            {diffError && (
              <div className="error storage-inline-error">{diffError}</div>
            )}
            {diff && !diffLoading && (
              <>
                {diff.files.length === 0 ? (
                  <p className="empty">No file changes in this revision.</p>
                ) : (
                  <ul className="storage-list">
                    {diff.files.map((f) => (
                      <li key={f.path}>
                        <span className="badge">{f.action_short}</span>{" "}
                        <code>{f.path}</code>
                      </li>
                    ))}
                  </ul>
                )}
              </>
            )}
          </section>
        )}

        {/* --- Commit (create a revision) --- */}
        <section className="storage-section">
          <h3>Commit</h3>
          <p className="storage-help">
            Create a new revision from the currently staged changes.
          </p>
          <div className="onboarding-field">
            <label htmlFor="commit-msg">Message *</label>
            <textarea
              id="commit-msg"
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              placeholder="Describe what changed"
            />
          </div>
          {commitError && (
            <div className="error storage-inline-error">{commitError}</div>
          )}
          {commitDone && !committing && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Committed{" "}
              <code>{commitDone.revision.slice(0, 12)}</code>.
            </div>
          )}
          <button
            className="storage-primary"
            disabled={!commitMessage.trim() || committing}
            onClick={() => void runCommit()}
          >
            {committing ? "Committing…" : "Commit"}
          </button>
        </section>

        {/* --- Amend (rewrites the latest revision — confirms) --- */}
        <section className="storage-section">
          <h3>Amend latest revision</h3>
          <p className="storage-help">
            Rewrite the message of the most recent revision. This replaces the
            latest revision, so it's confirmed first.
          </p>
          <div className="onboarding-field">
            <label htmlFor="amend-msg">New message *</label>
            <textarea
              id="amend-msg"
              value={amendMessage}
              onChange={(e) => {
                setAmendMessage(e.target.value);
                setConfirmAmend(false);
              }}
              placeholder="Replacement message for the latest revision"
            />
          </div>
          {amendError && (
            <div className="error storage-inline-error">{amendError}</div>
          )}
          {amendDone && !amending && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Amended to{" "}
              <code>{amendDone.revision.slice(0, 12)}</code>.
            </div>
          )}
          {!confirmAmend ? (
            <button
              disabled={!amendMessage.trim() || amending}
              onClick={() => setConfirmAmend(true)}
            >
              Amend
            </button>
          ) : (
            <div className="storage-confirm">
              <span>
                Replace the latest revision's message? The revision hash will
                change.
              </span>
              <button
                disabled={amending}
                onClick={() => void runAmend()}
              >
                {amending ? "Amending…" : "Yes, amend"}
              </button>
              <button disabled={amending} onClick={() => setConfirmAmend(false)}>
                Cancel
              </button>
            </div>
          )}
        </section>

        {/* --- Find (search by metadata) --- */}
        <section className="storage-section">
          <h3>Find revisions</h3>
          <p className="storage-help">
            Search revisions by a metadata key/value. Enable <em>local</em> to
            search only revisions present in this working copy.
          </p>
          <div className="onboarding-field">
            <label htmlFor="find-key">Metadata key</label>
            <input
              id="find-key"
              type="text"
              value={fKey}
              onChange={(e) => setFKey(e.target.value)}
              placeholder="e.g. author"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="find-value">Metadata value</label>
            <input
              id="find-value"
              type="text"
              value={fValue}
              onChange={(e) => setFValue(e.target.value)}
              placeholder="e.g. user@example.com"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="find-number">Max results (0 = unlimited)</label>
            <input
              id="find-number"
              type="number"
              min="0"
              value={fNumber}
              onChange={(e) => setFNumber(e.target.value)}
              placeholder="0"
            />
          </div>
          <label
            htmlFor="find-local"
            style={{ display: "block", marginBottom: 6 }}
          >
            <input
              id="find-local"
              type="checkbox"
              checked={fLocal}
              onChange={(e) => setFLocal(e.target.checked)}
            />{" "}
            Local only — search just this working copy
          </label>
          {findError && (
            <div className="error storage-inline-error">{findError}</div>
          )}
          {findResult && !finding && (
            <>
              {findResult.length === 0 ? (
                <p className="empty">No matching revisions.</p>
              ) : (
                <ul className="storage-list">
                  {findResult.map((sig, i) => (
                    <li key={`${sig}:${i}`}>
                      <code>{sig.slice(0, 16)}</code>
                      <button
                        onClick={() => void selectRevision(sig)}
                        title="Show this revision's details and file changes"
                      >
                        Details
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
          <button disabled={finding} onClick={() => void runFind()}>
            {finding ? "Searching…" : "Find"}
          </button>
        </section>

        {/* --- Revert (destructive, stateful) --- */}
        <section className="storage-section storage-danger">
          <h3>Revert revision</h3>
          <p className="storage-help">
            Undo a revision by applying its inverse to the working copy. This
            modifies files and, unless <em>no-commit</em> is set, creates a new
            revision — so it's confirmed first. If the revert conflicts, resolve
            the listed files below and mark them resolved. Use a revision row
            above to prefill.
          </p>
          <div className="onboarding-field">
            <label htmlFor="revert-rev">Revision to revert *</label>
            <input
              id="revert-rev"
              type="text"
              value={rvRevision}
              onChange={(e) => {
                setRvRevision(e.target.value);
                setConfirmRevert(false);
              }}
              placeholder="revision hash"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="revert-msg">Commit message (empty = default)</label>
            <input
              id="revert-msg"
              type="text"
              value={rvMessage}
              onChange={(e) => setRvMessage(e.target.value)}
              placeholder="Revert <revision>"
            />
          </div>
          <label
            htmlFor="revert-no-commit"
            style={{ display: "block", marginBottom: 6 }}
          >
            <input
              id="revert-no-commit"
              type="checkbox"
              checked={rvNoCommit}
              onChange={(e) => setRvNoCommit(e.target.checked)}
            />{" "}
            No-commit — apply to the working copy without committing
          </label>
          {revertError && (
            <div className="error storage-inline-error">{revertError}</div>
          )}
          {revertResult && !reverting && !revertResult.has_conflicts && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Reverted
              {revertResult.committed_revision
                ? ` · new revision ${revertResult.committed_revision.slice(0, 12)}`
                : " · staged (not committed)"}
              .
            </div>
          )}
          {revertResult && !reverting && revertResult.has_conflicts && (
            <div className="error storage-inline-error">
              Revert produced {revertResult.conflict_files.length} conflict
              {revertResult.conflict_files.length === 1 ? "" : "s"}. Resolve the
              files below, then mark them resolved.
            </div>
          )}
          {!confirmRevert ? (
            <button
              className="storage-danger-btn"
              disabled={!rvRevision.trim() || reverting}
              onClick={() => setConfirmRevert(true)}
            >
              Revert revision
            </button>
          ) : (
            <div className="storage-confirm">
              <span>
                Revert <code>{rvRevision.trim().slice(0, 12)}</code>? This
                changes files in your working copy
                {rvNoCommit ? "" : " and creates a new revision"}.
              </span>
              <button
                className="storage-danger-btn"
                disabled={reverting}
                onClick={() => void runRevert()}
              >
                {reverting ? "Reverting…" : "Yes, revert"}
              </button>
              <button
                disabled={reverting}
                onClick={() => setConfirmRevert(false)}
              >
                Cancel
              </button>
            </div>
          )}

          {/* revert_resolve — second step of the stateful revert flow */}
          <div style={{ marginTop: 16 }}>
            <p className="storage-help">
              Resolve revert conflicts: once you've fixed the conflicting files,
              list them here to mark them resolved and continue the revert.
            </p>
            <div className="onboarding-field">
              <label htmlFor="revert-resolve-paths">
                Resolved paths (one per line)
              </label>
              <textarea
                id="revert-resolve-paths"
                value={resolvePaths}
                onChange={(e) => setResolvePaths(e.target.value)}
                placeholder={"Content/Maps/main.umap"}
              />
            </div>
            {resolveError && (
              <div className="error storage-inline-error">{resolveError}</div>
            )}
            {resolveDone && !resolving && (
              <div className="storage-ok">
                <span className="success-icon">&#10003;</span> Marked{" "}
                {resolveDone.length} path
                {resolveDone.length === 1 ? "" : "s"} resolved.
              </div>
            )}
            <button
              disabled={splitPaths(resolvePaths).length === 0 || resolving}
              onClick={() => void runResolve()}
            >
              {resolving ? "Resolving…" : "Mark resolved"}
            </button>
          </div>
        </section>
      </div>
    </div>
  );
}
