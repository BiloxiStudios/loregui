import { useCallback, useEffect, useState } from "react";
import {
  dependencyListApi,
  dependencyAddApi,
  dependencyRemoveApi,
  type FileDependencies,
} from "./api";

/**
 * Dependencies panel (Settings/Manage admin surface) — the rich home for the
 * dependency domain, per `docs/INFORMATION-ARCHITECTURE.md` (dependency row:
 * "file detail / Settings", an occasional/admin surface, not the main sidebar).
 *
 * The dependency domain is a per-file dependency graph: a *source* file path
 * depends-on one or more *target* paths, each edge optionally classified with
 * tags. This panel surfaces the three registered dependency_* commands:
 *  - list   → show one file's dependencies (or dependents, in reverse mode),
 *             optionally recursive / depth-limited / tag-filtered.
 *  - add    → record that a source file depends on a target path (+ tags).
 *  - remove → drop a dependency edge (or just specific tags) from a source.
 *
 * Each op is a per-file relationship, so every form is keyed on a single source
 * file path. Each section handles empty / loading / error / success and is
 * themed entirely via `--surface-*` tokens, reusing the shared overlay-panel
 * classes from StoragePanel/RepositoryPanel/LocksPanel (no new styles needed).
 * Esc closes; one primary action per section.
 */

function errMsg(e: unknown): string {
  return typeof e === "string" ? e : JSON.stringify(e);
}

/** Split a comma/newline-separated tag string into trimmed non-empty tags. */
function splitTags(raw: string): string[] {
  return raw
    .split(/[\n,]/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

export default function DependenciesPanel({
  onClose,
}: {
  onClose: () => void;
}) {
  // --- list (show a file's dependencies) ---
  const [lPath, setLPath] = useState("");
  const [lRevision, setLRevision] = useState("");
  const [lRecursive, setLRecursive] = useState(false);
  const [lReverse, setLReverse] = useState(false);
  const [lTags, setLTags] = useState("");
  const [lDepth, setLDepth] = useState("");
  const [listResult, setListResult] = useState<FileDependencies[] | null>(null);
  const [listLoading, setListLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);

  // --- add (file → depends-on path) ---
  const [aPath, setAPath] = useState("");
  const [aDependency, setADependency] = useState("");
  const [aTags, setATags] = useState("");
  const [aForce, setAForce] = useState(false);
  const [adding, setAdding] = useState(false);
  const [addResult, setAddResult] = useState<number | null>(null);
  const [addError, setAddError] = useState<string | null>(null);

  // --- remove (drop an edge / its tags) ---
  const [rPath, setRPath] = useState("");
  const [rDependency, setRDependency] = useState("");
  const [rTags, setRTags] = useState("");
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [removeResult, setRemoveResult] = useState<number | null>(null);
  const [removeError, setRemoveError] = useState<string | null>(null);

  // Esc closes the panel (DESIGN-SYSTEM: overlays dismiss on Esc).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const runList = useCallback(async () => {
    if (!lPath.trim()) return;
    setListLoading(true);
    setListError(null);
    setListResult(null);
    try {
      const depth = parseInt(lDepth, 10);
      const res = await dependencyListApi.list(
        [lPath.trim()],
        lRevision.trim(),
        lRecursive,
        lReverse,
        splitTags(lTags),
        Number.isFinite(depth) && depth > 0 ? depth : 0,
      );
      setListResult(res.files);
    } catch (e) {
      setListError(errMsg(e));
    } finally {
      setListLoading(false);
    }
  }, [lPath, lRevision, lRecursive, lReverse, lTags, lDepth]);

  const runAdd = useCallback(async () => {
    if (!aPath.trim() || !aDependency.trim()) return;
    setAdding(true);
    setAddError(null);
    setAddResult(null);
    try {
      const tags = splitTags(aTags);
      const res = await dependencyAddApi.add(
        [
          {
            path: aPath.trim(),
            dependencies: [
              {
                dependency: aDependency.trim(),
                ...(tags.length > 0 ? { tags } : {}),
              },
            ],
          },
        ],
        aForce,
      );
      setAddResult(res.added_count);
    } catch (e) {
      setAddError(errMsg(e));
    } finally {
      setAdding(false);
    }
  }, [aPath, aDependency, aTags, aForce]);

  const runRemove = useCallback(async () => {
    if (!rPath.trim() || !rDependency.trim()) return;
    setRemoving(true);
    setRemoveError(null);
    setRemoveResult(null);
    try {
      const tags = splitTags(rTags);
      const res = await dependencyRemoveApi.remove([
        {
          path: rPath.trim(),
          dependencies: [
            {
              dependency: rDependency.trim(),
              ...(tags.length > 0 ? { tags } : {}),
            },
          ],
        },
      ]);
      setRemoveResult(res.removed_count);
    } catch (e) {
      setRemoveError(errMsg(e));
    } finally {
      setRemoving(false);
      setConfirmRemove(false);
    }
  }, [rPath, rDependency, rTags]);

  const listNoun = lReverse ? "dependents" : "dependencies";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Dependencies"
      className="storage-scrim"
      onClick={onClose}
    >
      <div className="storage-panel" onClick={(e) => e.stopPropagation()}>
        <header className="storage-panel-header">
          <h2>Dependencies</h2>
          <button onClick={onClose} title="Close (Esc)">
            Close
          </button>
        </header>

        <p className="storage-help">
          Dependencies are per-file relationships: a source file depends on one
          or more target paths. Use this panel to view a file's dependencies and
          to add or remove individual edges.
        </p>

        {/* --- List a file's dependencies --- */}
        <section className="storage-section">
          <h3>File dependencies</h3>
          <p className="storage-help">
            Show the paths a file depends on. Enable <em>reverse</em> to list its
            dependents (the files that depend on it) instead.
          </p>
          <div className="onboarding-field">
            <label htmlFor="dep-l-path">File path *</label>
            <input
              id="dep-l-path"
              type="text"
              value={lPath}
              onChange={(e) => setLPath(e.target.value)}
              placeholder="e.g. Content/Meshes/hero.fbx"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="dep-l-revision">Revision (empty = current)</label>
            <input
              id="dep-l-revision"
              type="text"
              value={lRevision}
              onChange={(e) => setLRevision(e.target.value)}
              placeholder="current revision"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="dep-l-tags">Filter by tags (empty = all)</label>
            <input
              id="dep-l-tags"
              type="text"
              value={lTags}
              onChange={(e) => setLTags(e.target.value)}
              placeholder="e.g. texture, compile"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="dep-l-depth">Depth limit (0 = unlimited)</label>
            <input
              id="dep-l-depth"
              type="number"
              min="0"
              value={lDepth}
              onChange={(e) => setLDepth(e.target.value)}
              placeholder="0"
            />
          </div>
          <label
            htmlFor="dep-l-recursive"
            style={{ display: "block", marginBottom: 6 }}
          >
            <input
              id="dep-l-recursive"
              type="checkbox"
              checked={lRecursive}
              onChange={(e) => setLRecursive(e.target.checked)}
            />{" "}
            Follow transitive dependencies recursively
          </label>
          <label
            htmlFor="dep-l-reverse"
            style={{ display: "block", marginBottom: 6 }}
          >
            <input
              id="dep-l-reverse"
              type="checkbox"
              checked={lReverse}
              onChange={(e) => setLReverse(e.target.checked)}
            />{" "}
            Reverse — show dependents instead
          </label>
          {listError && (
            <div className="error storage-inline-error">{listError}</div>
          )}
          {listResult && !listLoading && (
            <>
              {listResult.length === 0 ||
              listResult.every((f) => f.entries.length === 0) ? (
                <p className="empty">
                  <code>{lPath.trim()}</code> has no {listNoun}.
                </p>
              ) : (
                listResult.map((file) => (
                  <div key={file.path} style={{ marginTop: 8 }}>
                    <p className="storage-help">
                      <code>{file.path}</code> — {file.entries.length}{" "}
                      {file.entries.length === 1
                        ? listNoun.replace(/s$/, "")
                        : listNoun}
                    </p>
                    {file.entries.length > 0 && (
                      <ul className="storage-list">
                        {file.entries.map((entry, i) => (
                          <li key={`${entry.path}:${i}`}>
                            <code>{entry.path}</code>
                            <span className="storage-status unknown">
                              {entry.tags.length > 0
                                ? `● ${entry.tags.join(", ")} · depth ${entry.depth}`
                                : `● depth ${entry.depth}`}
                            </span>
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                ))
              )}
            </>
          )}
          {!listResult && !listLoading && !listError && (
            <p className="empty">Enter a file path to view its dependencies.</p>
          )}
          <button
            className="storage-primary"
            disabled={!lPath.trim() || listLoading}
            onClick={() => void runList()}
          >
            {listLoading ? "Loading…" : `Show ${listNoun}`}
          </button>
        </section>

        {/* --- Add a dependency --- */}
        <section className="storage-section">
          <h3>Add dependency</h3>
          <p className="storage-help">
            Record that a source file depends on a target path. Tags classify the
            edge (e.g. <code>texture</code>, <code>compile</code>).
          </p>
          <div className="onboarding-field">
            <label htmlFor="dep-a-path">Source file *</label>
            <input
              id="dep-a-path"
              type="text"
              value={aPath}
              onChange={(e) => setAPath(e.target.value)}
              placeholder="e.g. Content/Meshes/hero.fbx"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="dep-a-dependency">Depends on (target path) *</label>
            <input
              id="dep-a-dependency"
              type="text"
              value={aDependency}
              onChange={(e) => setADependency(e.target.value)}
              placeholder="e.g. Content/Textures/hero_albedo.png"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="dep-a-tags">Tags (optional)</label>
            <input
              id="dep-a-tags"
              type="text"
              value={aTags}
              onChange={(e) => setATags(e.target.value)}
              placeholder="e.g. texture, compile"
            />
          </div>
          <label
            htmlFor="dep-a-force"
            style={{ display: "block", marginBottom: 6 }}
          >
            <input
              id="dep-a-force"
              type="checkbox"
              checked={aForce}
              onChange={(e) => setAForce(e.target.checked)}
            />{" "}
            Force — skip cycle detection
          </label>
          {addError && (
            <div className="error storage-inline-error">{addError}</div>
          )}
          {addResult != null && !adding && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Added {addResult}{" "}
              dependency edge{addResult === 1 ? "" : "s"}.
            </div>
          )}
          <button
            disabled={!aPath.trim() || !aDependency.trim() || adding}
            onClick={() => void runAdd()}
          >
            {adding ? "Adding…" : "Add dependency"}
          </button>
        </section>

        {/* --- Remove a dependency (confirms) --- */}
        <section className="storage-section storage-danger">
          <h3>Remove dependency</h3>
          <p className="storage-help">
            Drop a dependency edge from a source file. Leave tags empty to remove
            the whole edge, or list tags to remove only those classifications.
            Back-references on the target are updated automatically.
          </p>
          <div className="onboarding-field">
            <label htmlFor="dep-r-path">Source file *</label>
            <input
              id="dep-r-path"
              type="text"
              value={rPath}
              onChange={(e) => {
                setRPath(e.target.value);
                setConfirmRemove(false);
              }}
              placeholder="e.g. Content/Meshes/hero.fbx"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="dep-r-dependency">Depends-on path to remove *</label>
            <input
              id="dep-r-dependency"
              type="text"
              value={rDependency}
              onChange={(e) => {
                setRDependency(e.target.value);
                setConfirmRemove(false);
              }}
              placeholder="e.g. Content/Textures/hero_albedo.png"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="dep-r-tags">
              Tags to remove (empty = whole edge)
            </label>
            <input
              id="dep-r-tags"
              type="text"
              value={rTags}
              onChange={(e) => {
                setRTags(e.target.value);
                setConfirmRemove(false);
              }}
              placeholder="e.g. texture"
            />
          </div>
          {removeError && (
            <div className="error storage-inline-error">{removeError}</div>
          )}
          {removeResult != null && !removing && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Removed{" "}
              {removeResult} dependency edge{removeResult === 1 ? "" : "s"}.
            </div>
          )}
          {!confirmRemove ? (
            <button
              className="storage-danger-btn"
              disabled={!rPath.trim() || !rDependency.trim() || removing}
              onClick={() => setConfirmRemove(true)}
            >
              Remove dependency
            </button>
          ) : (
            <div className="storage-confirm">
              <span>
                Remove{" "}
                {splitTags(rTags).length > 0
                  ? `tags [${splitTags(rTags).join(", ")}] from the edge`
                  : "the dependency edge"}{" "}
                <code>{rPath.trim()}</code> → <code>{rDependency.trim()}</code>?
              </span>
              <button
                className="storage-danger-btn"
                disabled={removing}
                onClick={() => void runRemove()}
              >
                {removing ? "Removing…" : "Yes, remove"}
              </button>
              <button
                disabled={removing}
                onClick={() => setConfirmRemove(false)}
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
