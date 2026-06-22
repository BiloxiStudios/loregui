import { useCallback, useEffect, useMemo, useState } from "react";
import {
  revisionActivityReportApi,
  revisionRevertLocalApi,
  fileWriteApi,
  type ActivityReportResult,
  type ActivityEntry,
} from "./api";
import { isEntitled, isDevDefaultEntitlement } from "./commercial/entitlement";

/**
 * Reporting & Insights panel (top-bar nav, commercial add-on).
 *
 * SBAI-4061 (reporting) under epic SBAI-4068 (commercial gating). This is a
 * PREMIUM surface: it is only mounted when `isEntitled("reporting")` is true
 * (App.tsx gates the nav button and renders a locked upsell otherwise). The
 * panel itself also re-checks entitlement defensively so it can never render its
 * controls un-gated.
 *
 * The data comes from the `revision_activity_report` op (PR #279): an aggregated
 * "who did what when" rollup over the revision chain. The panel turns that into:
 *  - **Per-user activity report** — pick an author + time window, get a rollup of
 *    commits / files changed / revisions ("everything Bob touched last week").
 *  - **History timeline** — a simple visual who-did-what graph of the matching
 *    revisions, colored by author, newest first.
 *  - **Multi-grain restore** — entry points to restore a whole revision (wired to
 *    `revision_revert_local`, the registered undo op), restore a single file from
 *    a revision (wired to `file_write`), and restore an individual change within
 *    a revision (NOT yet supported by any op — clearly marked as coming soon).
 *
 * Themed entirely via `--surface-*` tokens, reusing the shared overlay-panel
 * classes (storage-*). Esc closes. Empty / loading / error / locked states all
 * handled.
 */

function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    const o = e as { message?: unknown; kind?: unknown };
    if (typeof o.message === "string") return o.message;
    if (typeof o.kind === "string") return o.kind;
  }
  return JSON.stringify(e);
}

/** Parse a <input type="date"> value (YYYY-MM-DD) to a Unix-seconds timestamp. */
function dateToUnix(value: string, endOfDay: boolean): number {
  if (!value) return 0;
  const ms = Date.parse(endOfDay ? `${value}T23:59:59` : `${value}T00:00:00`);
  return Number.isFinite(ms) ? Math.floor(ms / 1000) : 0;
}

function fmtTimestamp(unix: number): string {
  if (!unix) return "—";
  return new Date(unix * 1000).toLocaleString();
}

interface PerAuthor {
  author: string;
  commits: number;
  filesChanged: number;
  revisions: string[];
}

/** Roll up entries into per-author totals, sorted by commit count desc. */
function rollupByAuthor(entries: ActivityEntry[]): PerAuthor[] {
  const map = new Map<string, PerAuthor>();
  for (const e of entries) {
    const key = e.author || "(unknown)";
    const cur =
      map.get(key) ?? { author: key, commits: 0, filesChanged: 0, revisions: [] };
    cur.commits += 1;
    cur.filesChanged += e.files_changed.length;
    cur.revisions.push(e.revision);
    map.set(key, cur);
  }
  return [...map.values()].sort((a, b) => b.commits - a.commits);
}

export default function ReportingPanel({ onClose }: { onClose: () => void }) {
  const entitled = isEntitled("reporting");

  // --- query controls ---
  const [author, setAuthor] = useState("");
  const [branch, setBranch] = useState("");
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");
  const [length, setLength] = useState("200");
  const [filePath, setFilePath] = useState("");

  // --- report data ---
  const [report, setReport] = useState<ActivityReportResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // --- multi-grain restore state ---
  const [restoreMsg, setRestoreMsg] = useState<string | null>(null);
  const [restoreErr, setRestoreErr] = useState<string | null>(null);
  const [confirmRevision, setConfirmRevision] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const runReport = useCallback(async () => {
    if (!entitled) return;
    setLoading(true);
    setError(null);
    try {
      const len = parseInt(length, 10);
      const res = await revisionActivityReportApi.report(
        "",
        branch.trim(),
        Number.isFinite(len) && len > 0 ? len : 0,
        author.trim(),
        dateToUnix(fromDate, false),
        dateToUnix(toDate, true),
        filePath.trim(),
      );
      setReport(res);
    } catch (e) {
      setReport(null);
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, [entitled, branch, author, fromDate, toDate, length, filePath]);

  // Load an initial report when entitled so the panel isn't blank.
  useEffect(() => {
    if (entitled) void runReport();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entitled]);

  // Esc closes (DESIGN-SYSTEM: overlays dismiss on Esc).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const entries = report?.entries ?? [];
  const perAuthor = useMemo(() => rollupByAuthor(entries), [entries]);

  // --- restore: whole revision (wired to revision_revert_local, the undo op) ---
  const restoreRevision = useCallback(
    async (revision: string) => {
      setBusy(true);
      setRestoreErr(null);
      setRestoreMsg(null);
      try {
        const res = await revisionRevertLocalApi.revertLocal(
          revision,
          `Restore: undo ${revision.slice(0, 12)}`,
        );
        if (res.has_conflicts) {
          setRestoreErr(
            `Restore produced ${res.conflict_files.length} conflict(s). Resolve them in the History panel's revert flow.`,
          );
        } else {
          setRestoreMsg(
            `Restored revision ${revision.slice(0, 12)}${
              res.committed_revision
                ? ` · new revision ${res.committed_revision.slice(0, 12)}`
                : ""
            }.`,
          );
        }
      } catch (e) {
        setRestoreErr(errMsg(e));
      } finally {
        setBusy(false);
        setConfirmRevision(null);
      }
    },
    [],
  );

  // --- restore: a single file from a revision (wired to file_write) ---
  const restoreFile = useCallback(
    async (revision: string, path: string) => {
      setBusy(true);
      setRestoreErr(null);
      setRestoreMsg(null);
      try {
        // file_write resolves by path+revision and writes back to the same
        // working-copy path — i.e. "restore this file to how it was at <rev>".
        const res = await fileWriteApi.write(path, path, revision, "");
        setRestoreMsg(
          `Restored file ${res.path} to its content at ${revision.slice(0, 12)}.`,
        );
      } catch (e) {
        setRestoreErr(errMsg(e));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  // ----- LOCKED (upsell) state: never render controls if not entitled -----
  if (!entitled) {
    return (
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Reporting & Insights (locked)"
        className="storage-scrim"
        onClick={onClose}
      >
        <div className="storage-panel" onClick={(e) => e.stopPropagation()}>
          <header className="storage-panel-header">
            <h2>Reporting &amp; Insights</h2>
            <button onClick={onClose} title="Close (Esc)">
              Close
            </button>
          </header>
          <section className="storage-section">
            <h3>Premium add-on</h3>
            <p className="storage-help">
              Reporting &amp; Insights is a commercial LoreGUI add-on. It gives
              you per-contributor activity rollups ("everything Bob touched last
              week" — commits, files changed, revisions), a who-did-what history
              timeline, and multi-grain restore.
            </p>
            <p className="storage-help">
              Unlock it with a Team or Enterprise StudioBrain plan. The rest of
              LoreGUI stays fully functional without it.
            </p>
            <a
              className="storage-primary"
              href="https://studiobrain.ai/loregui/reporting"
              target="_blank"
              rel="noreferrer"
              style={{ display: "inline-block", textDecoration: "none" }}
            >
              Learn more &amp; upgrade
            </a>
          </section>
        </div>
      </div>
    );
  }

  // ----- ENTITLED: the full panel -----
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Reporting & Insights"
      className="storage-scrim"
      onClick={onClose}
    >
      <div className="storage-panel" onClick={(e) => e.stopPropagation()}>
        <header className="storage-panel-header">
          <h2>
            Reporting &amp; Insights <span className="badge">Premium</span>
          </h2>
          <button onClick={onClose} title="Close (Esc)">
            Close
          </button>
        </header>

        {isDevDefaultEntitlement() && (
          <p className="storage-help" style={{ marginBottom: 0 }}>
            Dev build: premium features default to unlocked. In production this
            panel unlocks from your StudioBrain plan tier.
          </p>
        )}

        {/* --- Query: who did what when --- */}
        <section className="storage-section">
          <h3>Activity report</h3>
          <p className="storage-help">
            Roll up "who did what when" over the revision history. Filter by
            contributor and a time window to answer questions like "everything
            Bob touched last week".
          </p>
          <div className="onboarding-field">
            <label htmlFor="rpt-author">Contributor (substring, empty = all)</label>
            <input
              id="rpt-author"
              type="text"
              value={author}
              onChange={(e) => setAuthor(e.target.value)}
              placeholder="e.g. bob"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="rpt-from">From date</label>
            <input
              id="rpt-from"
              type="date"
              value={fromDate}
              onChange={(e) => setFromDate(e.target.value)}
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="rpt-to">To date</label>
            <input
              id="rpt-to"
              type="date"
              value={toDate}
              onChange={(e) => setToDate(e.target.value)}
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="rpt-branch">Branch (empty = current)</label>
            <input
              id="rpt-branch"
              type="text"
              value={branch}
              onChange={(e) => setBranch(e.target.value)}
              placeholder="e.g. main"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="rpt-file">File path (empty = all files)</label>
            <input
              id="rpt-file"
              type="text"
              value={filePath}
              onChange={(e) => setFilePath(e.target.value)}
              placeholder="e.g. Content/Maps/main.umap"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="rpt-length">
              Max revisions to scan (0 = all; large repos may be slow)
            </label>
            <input
              id="rpt-length"
              type="number"
              min="0"
              value={length}
              onChange={(e) => setLength(e.target.value)}
              placeholder="200"
            />
          </div>
          {error && <div className="error storage-inline-error">{error}</div>}
          <button
            className="storage-primary"
            disabled={loading}
            onClick={() => void runReport()}
          >
            {loading ? "Building report…" : "Run report"}
          </button>
        </section>

        {/* --- Per-contributor rollup --- */}
        {report && !loading && (
          <section className="storage-section">
            <h3>Per-contributor rollup</h3>
            <p className="storage-help">
              {report.total_after_filter} of {report.total_walked} scanned
              revision{report.total_walked === 1 ? "" : "s"} matched
              {author.trim() ? ` contributor "${author.trim()}"` : ""}.
            </p>
            {perAuthor.length === 0 ? (
              <p className="empty">
                No activity matched. Widen the date range or clear the
                contributor filter.
              </p>
            ) : (
              <ul className="storage-list">
                {perAuthor.map((a) => (
                  <li key={a.author}>
                    <span className="badge">{a.author}</span>
                    <span className="storage-status unknown">
                      ● {a.commits} commit{a.commits === 1 ? "" : "s"} ·{" "}
                      {a.filesChanged} file change
                      {a.filesChanged === 1 ? "" : "s"}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        )}

        {/* --- History timeline (visual who-did-what graph) --- */}
        {report && !loading && entries.length > 0 && (
          <section className="storage-section">
            <h3>History timeline</h3>
            <p className="storage-help">
              Who did what, newest first. Each row is a revision; the colored
              spine groups it by contributor. Use the restore actions to roll a
              file or a whole revision back.
            </p>
            <ul className="storage-list reporting-timeline">
              {entries.map((e) => {
                return (
                  <li
                    key={e.revision}
                    style={{
                      borderLeft: "3px solid var(--surface-primary-bg, var(--accent))",
                      paddingLeft: 8,
                      display: "block",
                    }}
                  >
                    <div>
                      <code>{e.revision.slice(0, 12)}</code>{" "}
                      <span className="badge">#{e.revision_number}</span>{" "}
                      <span className="badge">{e.author || "(unknown)"}</span>
                    </div>
                    <div className="storage-help" style={{ margin: "2px 0" }}>
                      {fmtTimestamp(e.timestamp)} ·{" "}
                      {e.files_changed.length} file
                      {e.files_changed.length === 1 ? "" : "s"}
                      {e.parents.length > 1 ? " · merge" : ""}
                    </div>
                    <div style={{ marginBottom: 4 }}>{e.message || "(no message)"}</div>
                    <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                      {confirmRevision === e.revision ? (
                        <span className="storage-confirm">
                          <span>
                            Restore (undo) revision {e.revision.slice(0, 12)}?
                            This modifies your working copy and creates a new
                            revision.
                          </span>
                          <button
                            className="storage-danger-btn"
                            disabled={busy}
                            onClick={() => void restoreRevision(e.revision)}
                          >
                            {busy ? "Restoring…" : "Yes, restore"}
                          </button>
                          <button
                            disabled={busy}
                            onClick={() => setConfirmRevision(null)}
                          >
                            Cancel
                          </button>
                        </span>
                      ) : (
                        <button
                          disabled={busy}
                          onClick={() => setConfirmRevision(e.revision)}
                          title="Restore the whole revision (undo it via revert)"
                        >
                          Restore revision…
                        </button>
                      )}
                    </div>
                    {e.files_changed.length > 0 && (
                      <details style={{ marginTop: 4 }}>
                        <summary className="storage-help">
                          Files in this revision ({e.files_changed.length})
                        </summary>
                        <ul className="storage-list">
                          {e.files_changed.map((f) => (
                            <li key={f.path}>
                              <span className="badge">{f.action}</span>{" "}
                              <code>{f.path}</code>
                              <button
                                disabled={busy}
                                onClick={() =>
                                  void restoreFile(e.revision, f.path)
                                }
                                title="Restore just this file to its content at this revision"
                              >
                                Restore file
                              </button>
                              <button
                                disabled
                                title="Restoring an individual change (single hunk) within a revision is not yet supported by the lore op surface."
                              >
                                Restore change (soon)
                              </button>
                            </li>
                          ))}
                        </ul>
                      </details>
                    )}
                  </li>
                );
              })}
            </ul>
          </section>
        )}

        {/* --- Restore status + multi-grain explainer --- */}
        <section className="storage-section">
          <h3>Restore</h3>
          <p className="storage-help">
            Multi-grain restore lets you roll back at three levels:
          </p>
          <ul className="storage-help" style={{ marginTop: 0 }}>
            <li>
              <strong>File</strong> — restore one file to its content at a
              revision (wired to <code>file_write</code>).
            </li>
            <li>
              <strong>Revision</strong> — undo a whole revision (wired to{" "}
              <code>revision_revert_local</code>); conflicts are resolved in the
              History panel.
            </li>
            <li>
              <strong>Individual change</strong> — restoring a single hunk within
              a revision is <em>not yet supported</em> by the lore op surface and
              is marked "soon".
            </li>
          </ul>
          {restoreErr && (
            <div className="error storage-inline-error">{restoreErr}</div>
          )}
          {restoreMsg && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> {restoreMsg}
            </div>
          )}
          {!restoreErr && !restoreMsg && (
            <p className="empty">
              Use the restore actions in the timeline above to roll a file or
              revision back.
            </p>
          )}
        </section>
      </div>
    </div>
  );
}
