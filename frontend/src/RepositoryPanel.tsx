import { useCallback, useEffect, useState, type ReactNode } from "react";
import {
  api,
  repositoryDeleteApi,
  repositoryDumpApi,
  repositoryFlushApi,
  repositoryGcApi,
  repositoryInstanceListApi,
  repositoryListApi,
  repositoryMetadataGetApi,
  repositoryMetadataSetApi,
  repositoryVerifyStateApi,
  type DeleteResult,
  type InstanceListResult,
  type MetadataEntry,
  type MetadataFormat,
  type RepositoryDumpResult,
  type RepositoryListResult,
  type RepositoryMetadataGetResult,
  type VerifyStateResult,
} from "./api";

/**
 * Repository panel (Settings/Manage admin surface) — the rich home for the
 * repository domain's occasional/admin ops, per
 * `docs/INFORMATION-ARCHITECTURE.md` (repository row: "top bar (open/clone) +
 * Settings/Manage panel"). The daily status loop lives in the Changes panel;
 * this is the management/maintenance home.
 *
 * Surfaces the registered repository_* management commands: identity (current
 * repo id/path/status), instances, flush, gc (destructive), verify_state (with
 * heal), metadata get/set, dump, remote list, and delete (destructive →
 * confirm). Every section handles empty / loading / error / success and is
 * themed entirely via `--surface-*` tokens (reusing the shared overlay-panel
 * classes from StoragePanel). Esc closes; one primary action per section.
 */

function errMsg(e: unknown): string {
  return typeof e === "string" ? e : JSON.stringify(e);
}

export default function RepositoryPanel({
  onClose,
  hostedServerCard,
}: {
  onClose: () => void;
  hostedServerCard?: ReactNode;
}) {
  // --- current repository identity ---
  const [repo, setRepo] = useState<string | null>(null);
  const [repoId, setRepoId] = useState<string>("");
  const [branch, setBranch] = useState<string>("");
  const [revision, setRevision] = useState<string>("");
  const [identityLoading, setIdentityLoading] = useState(true);
  const [identityError, setIdentityError] = useState<string | null>(null);

  // --- instances ---
  const [instances, setInstances] = useState<InstanceListResult | null>(null);
  const [instancesLoading, setInstancesLoading] = useState(false);
  const [instancesError, setInstancesError] = useState<string | null>(null);

  // --- flush ---
  const [flushing, setFlushing] = useState(false);
  const [flushDone, setFlushDone] = useState(false);
  const [flushError, setFlushError] = useState<string | null>(null);

  // --- gc (destructive) ---
  const [gcRunning, setGcRunning] = useState(false);
  const [gcDone, setGcDone] = useState(false);
  const [gcError, setGcError] = useState<string | null>(null);
  const [confirmGc, setConfirmGc] = useState(false);

  // --- verify_state (with heal) ---
  const [verify, setVerify] = useState<VerifyStateResult | null>(null);
  const [verifying, setVerifying] = useState(false);
  const [verifyError, setVerifyError] = useState<string | null>(null);

  // --- metadata get/set ---
  const [metaKey, setMetaKey] = useState("");
  const [metaGet, setMetaGet] = useState<RepositoryMetadataGetResult | null>(
    null,
  );
  const [metaGetLoading, setMetaGetLoading] = useState(false);
  const [metaGetError, setMetaGetError] = useState<string | null>(null);
  const [setKey, setSetKey] = useState("");
  const [setValue, setSetValue] = useState("");
  const [setFormat, setSetFormat] = useState<MetadataFormat>("string");
  const [metaSetting, setMetaSetting] = useState(false);
  const [metaSetDone, setMetaSetDone] = useState(false);
  const [metaSetError, setMetaSetError] = useState<string | null>(null);

  // --- dump ---
  const [dumpRevision, setDumpRevision] = useState("");
  const [dumpPath, setDumpPath] = useState("");
  const [dump, setDump] = useState<RepositoryDumpResult | null>(null);
  const [dumpLoading, setDumpLoading] = useState(false);
  const [dumpError, setDumpError] = useState<string | null>(null);

  // --- remote list ---
  const [listUrl, setListUrl] = useState("");
  const [list, setList] = useState<RepositoryListResult | null>(null);
  const [listLoading, setListLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);

  // --- delete (destructive) ---
  const [deleteUrl, setDeleteUrl] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [deleteResult, setDeleteResult] = useState<DeleteResult | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const loadIdentity = useCallback(async () => {
    setIdentityLoading(true);
    setIdentityError(null);
    try {
      const [r, s] = await Promise.all([api.currentRepository(), api.status()]);
      setRepo(r);
      setRepoId(s.repo_id);
      setBranch(s.branch);
      setRevision(s.revision);
    } catch (e) {
      setIdentityError(errMsg(e));
    } finally {
      setIdentityLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadIdentity();
  }, [loadIdentity]);

  // Esc closes the panel (DESIGN-SYSTEM: overlays dismiss on Esc).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const loadInstances = useCallback(async () => {
    setInstancesLoading(true);
    setInstancesError(null);
    try {
      setInstances(await repositoryInstanceListApi.instanceList());
    } catch (e) {
      setInstances(null);
      setInstancesError(errMsg(e));
    } finally {
      setInstancesLoading(false);
    }
  }, []);

  const runFlush = useCallback(async () => {
    setFlushing(true);
    setFlushDone(false);
    setFlushError(null);
    try {
      await repositoryFlushApi.flush();
      setFlushDone(true);
    } catch (e) {
      setFlushError(errMsg(e));
    } finally {
      setFlushing(false);
    }
  }, []);

  const runGc = useCallback(async () => {
    setGcRunning(true);
    setGcDone(false);
    setGcError(null);
    try {
      await repositoryGcApi.gc();
      setGcDone(true);
    } catch (e) {
      setGcError(errMsg(e));
    } finally {
      setGcRunning(false);
      setConfirmGc(false);
    }
  }, []);

  const runVerify = useCallback(async (heal: boolean) => {
    setVerifying(true);
    setVerifyError(null);
    try {
      setVerify(await repositoryVerifyStateApi.verifyState("", heal));
    } catch (e) {
      setVerify(null);
      setVerifyError(errMsg(e));
    } finally {
      setVerifying(false);
    }
  }, []);

  const runMetaGet = useCallback(async () => {
    setMetaGetLoading(true);
    setMetaGetError(null);
    setMetaGet(null);
    try {
      setMetaGet(await repositoryMetadataGetApi.metadataGet(metaKey.trim()));
    } catch (e) {
      setMetaGetError(errMsg(e));
    } finally {
      setMetaGetLoading(false);
    }
  }, [metaKey]);

  const runMetaSet = useCallback(async () => {
    if (!setKey.trim()) return;
    setMetaSetting(true);
    setMetaSetDone(false);
    setMetaSetError(null);
    try {
      await repositoryMetadataSetApi.metadataSet(
        [setKey.trim()],
        [setValue],
        [setFormat],
      );
      setMetaSetDone(true);
      // Refresh the get view if it's showing the same key (or all).
      if (!metaKey.trim() || metaKey.trim() === setKey.trim()) {
        void runMetaGet();
      }
    } catch (e) {
      setMetaSetError(errMsg(e));
    } finally {
      setMetaSetting(false);
    }
  }, [setKey, setValue, setFormat, metaKey, runMetaGet]);

  const runDump = useCallback(async () => {
    setDumpLoading(true);
    setDumpError(null);
    setDump(null);
    try {
      setDump(
        await repositoryDumpApi.dump(dumpRevision.trim(), dumpPath.trim(), 0),
      );
    } catch (e) {
      setDumpError(errMsg(e));
    } finally {
      setDumpLoading(false);
    }
  }, [dumpRevision, dumpPath]);

  const runList = useCallback(async () => {
    if (!listUrl.trim()) return;
    setListLoading(true);
    setListError(null);
    setList(null);
    try {
      setList(await repositoryListApi.list(listUrl.trim()));
    } catch (e) {
      setListError(errMsg(e));
    } finally {
      setListLoading(false);
    }
  }, [listUrl]);

  const runDelete = useCallback(async () => {
    if (!deleteUrl.trim()) return;
    setDeleting(true);
    setDeleteError(null);
    setDeleteResult(null);
    try {
      setDeleteResult(await repositoryDeleteApi.delete(deleteUrl.trim()));
    } catch (e) {
      setDeleteError(errMsg(e));
    } finally {
      setDeleting(false);
      setConfirmDelete(false);
    }
  }, [deleteUrl]);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Repository management"
      className="storage-scrim"
      onClick={onClose}
    >
      <div className="storage-panel" onClick={(e) => e.stopPropagation()}>
        <header className="storage-panel-header">
          <h2>Repository</h2>
          <button onClick={onClose} title="Close (Esc)">
            Close
          </button>
        </header>

        {hostedServerCard}

        {/* --- Current repository identity --- */}
        <section className="storage-section">
          <h3>Current repository</h3>
          {identityLoading && <p className="storage-help">Loading…</p>}
          {!identityLoading && identityError && (
            <>
              <div className="error storage-inline-error">{identityError}</div>
              <button onClick={() => void loadIdentity()}>Retry</button>
            </>
          )}
          {!identityLoading && !identityError && !repo && (
            <p className="empty">
              No repository open — open or clone one from the top bar to manage
              it here.
            </p>
          )}
          {!identityLoading && !identityError && repo && (
            <dl className="metadata-dl">
              <span>
                <dt>Path / URL</dt>
                <dd>
                  <code>{repo}</code>
                </dd>
              </span>
              <span>
                <dt>Repository ID</dt>
                <dd>
                  <code>{repoId.slice(0, 16) || "—"}</code>
                </dd>
              </span>
              <span>
                <dt>Branch</dt>
                <dd>{branch || "—"}</dd>
              </span>
              <span>
                <dt>Revision</dt>
                <dd>
                  <code>{revision.slice(0, 16) || "—"}</code>
                </dd>
              </span>
            </dl>
          )}
        </section>

        {/* --- Instances (working copies) --- */}
        <section className="storage-section">
          <h3>Instances</h3>
          <p className="storage-help">
            Working copies registered against this repository's shared store.
          </p>
          {instancesError && (
            <div className="error storage-inline-error">{instancesError}</div>
          )}
          {instances && !instancesLoading && (
            <>
              {instances.instances.length === 0 ? (
                <p className="empty">No instances registered.</p>
              ) : (
                <ul className="storage-list">
                  {instances.instances.map((inst) => (
                    <li key={inst.instance_id}>
                      <code>{inst.path}</code>
                      <span
                        className={`storage-status ${inst.stale ? "bad" : "ok"}`}
                      >
                        {inst.stale
                          ? `● stale · ${inst.branch_name}`
                          : `● ${inst.branch_name}`}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
          <button disabled={instancesLoading} onClick={() => void loadInstances()}>
            {instancesLoading ? "Listing…" : "List instances"}
          </button>
        </section>

        {/* --- Verify state (with heal) --- */}
        <section className="storage-section">
          <h3>Verify integrity</h3>
          <p className="storage-help">
            Check fragment integrity across the repository; heal repairs any
            detected inconsistencies.
          </p>
          {verifyError && (
            <div className="error storage-inline-error">{verifyError}</div>
          )}
          {verify && !verifying && (
            <dl className="metadata-dl">
              <span>
                <dt>Fragments checked</dt>
                <dd>{verify.fragments.length}</dd>
              </span>
              <span>
                <dt>Local errors</dt>
                <dd>{verify.error_count}</dd>
              </span>
              <span>
                <dt>Remote fragments</dt>
                <dd>{verify.remote_fragments.length}</dd>
              </span>
              <span>
                <dt>Corrupted (remote)</dt>
                <dd>{verify.corrupted_count}</dd>
              </span>
            </dl>
          )}
          {verify &&
            !verifying &&
            verify.error_count === 0 &&
            verify.corrupted_count === 0 && (
              <div className="storage-ok">
                <span className="success-icon">&#10003;</span> No
                inconsistencies found.
              </div>
            )}
          <button
            className="storage-primary"
            disabled={verifying}
            onClick={() => void runVerify(false)}
          >
            {verifying ? "Verifying…" : "Verify"}
          </button>
          {verify &&
            !verifying &&
            (verify.error_count > 0 || verify.corrupted_count > 0) && (
              <button
                style={{ marginLeft: 8 }}
                disabled={verifying}
                onClick={() => void runVerify(true)}
                title="Repair detected inconsistencies"
              >
                Heal
              </button>
            )}
        </section>

        {/* --- Metadata get/set --- */}
        <section className="storage-section">
          <h3>Metadata</h3>
          <p className="storage-help">
            Read or write repository metadata key-value pairs.
          </p>
          <div className="onboarding-field">
            <label htmlFor="repo-meta-key">Key (empty reads all)</label>
            <input
              id="repo-meta-key"
              type="text"
              value={metaKey}
              onChange={(e) => setMetaKey(e.target.value)}
              placeholder="leave empty for all entries"
            />
          </div>
          {metaGetError && (
            <div className="error storage-inline-error">{metaGetError}</div>
          )}
          {metaGet && !metaGetLoading && (
            <>
              {metaGet.entries.length === 0 ? (
                <p className="empty">No metadata entries.</p>
              ) : (
                <dl className="metadata-dl">
                  {metaGet.entries.map((entry: MetadataEntry) => (
                    <span key={entry.key}>
                      <dt>
                        {entry.key}{" "}
                        <span className="badge">{entry.value_type}</span>
                      </dt>
                      <dd>
                        <code>{entry.value}</code>
                      </dd>
                    </span>
                  ))}
                </dl>
              )}
            </>
          )}
          <button disabled={metaGetLoading} onClick={() => void runMetaGet()}>
            {metaGetLoading ? "Reading…" : "Get metadata"}
          </button>

          <div style={{ marginTop: 14 }}>
            <div className="onboarding-field">
              <label htmlFor="repo-set-key">Set key</label>
              <input
                id="repo-set-key"
                type="text"
                value={setKey}
                onChange={(e) => setSetKey(e.target.value)}
                placeholder="key to write"
              />
            </div>
            <div className="onboarding-field">
              <label htmlFor="repo-set-value">Set value</label>
              <input
                id="repo-set-value"
                type="text"
                value={setValue}
                onChange={(e) => setSetValue(e.target.value)}
                placeholder="value"
              />
            </div>
            <div className="onboarding-field">
              <label htmlFor="repo-set-format">Format</label>
              <select
                id="repo-set-format"
                value={setFormat}
                onChange={(e) =>
                  setSetFormat(e.target.value as MetadataFormat)
                }
              >
                <option value="string">string</option>
                <option value="numeric">numeric</option>
                <option value="binary">binary</option>
              </select>
            </div>
            {metaSetError && (
              <div className="error storage-inline-error">{metaSetError}</div>
            )}
            {metaSetDone && !metaSetting && (
              <div className="storage-ok">
                <span className="success-icon">&#10003;</span> Metadata set.
              </div>
            )}
            <button
              disabled={!setKey.trim() || metaSetting}
              onClick={() => void runMetaSet()}
            >
              {metaSetting ? "Setting…" : "Set metadata"}
            </button>
          </div>
        </section>

        {/* --- Dump tree --- */}
        <section className="storage-section">
          <h3>Dump tree</h3>
          <p className="storage-help">
            Inspect the repository tree structure at a revision and path (no
            files written).
          </p>
          <div className="onboarding-field">
            <label htmlFor="repo-dump-rev">Revision (empty = current)</label>
            <input
              id="repo-dump-rev"
              type="text"
              value={dumpRevision}
              onChange={(e) => setDumpRevision(e.target.value)}
              placeholder="current revision"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="repo-dump-path">Path (empty = root)</label>
            <input
              id="repo-dump-path"
              type="text"
              value={dumpPath}
              onChange={(e) => setDumpPath(e.target.value)}
              placeholder="/"
            />
          </div>
          {dumpError && (
            <div className="error storage-inline-error">{dumpError}</div>
          )}
          {dump && !dumpLoading && (
            <>
              <p className="storage-help">
                {dump.nodes.length} node{dump.nodes.length === 1 ? "" : "s"}
                {dump.state ? ` · rev #${dump.state.revision_number}` : ""}
              </p>
              {dump.nodes.length === 0 ? (
                <p className="empty">No nodes at this path/revision.</p>
              ) : (
                <ul className="storage-list">
                  {dump.nodes.slice(0, 50).map((n) => (
                    <li key={n.id}>
                      <code>{n.name || "(root)"}</code>
                      <span className="storage-status unknown">
                        {n.size} bytes
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
          <button disabled={dumpLoading} onClick={() => void runDump()}>
            {dumpLoading ? "Dumping…" : "Dump tree"}
          </button>
        </section>

        {/* --- List remote repositories --- */}
        <section className="storage-section">
          <h3>List remote repositories</h3>
          <p className="storage-help">
            Enumerate repositories hosted at a remote URL.
          </p>
          <div className="onboarding-field">
            <label htmlFor="repo-list-url">Remote URL</label>
            <input
              id="repo-list-url"
              type="text"
              value={listUrl}
              onChange={(e) => setListUrl(e.target.value)}
              placeholder="lore://example.com"
            />
          </div>
          {listError && (
            <div className="error storage-inline-error">{listError}</div>
          )}
          {list && !listLoading && (
            <>
              {list.entries.length === 0 ? (
                <p className="empty">No repositories found at {list.url}.</p>
              ) : (
                <ul className="storage-list">
                  {list.entries.map((entry) => (
                    <li key={entry.id}>
                      <code>{entry.name}</code>
                      <span className="storage-status unknown">
                        {entry.id.slice(0, 12)}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
          <button
            disabled={!listUrl.trim() || listLoading}
            onClick={() => void runList()}
          >
            {listLoading ? "Listing…" : "List"}
          </button>
        </section>

        {/* --- Maintenance: flush --- */}
        <section className="storage-section">
          <h3>Flush pending writes</h3>
          <p className="storage-help">
            Force outstanding repository writes to persistent storage.
          </p>
          {flushError && (
            <div className="error storage-inline-error">{flushError}</div>
          )}
          {flushDone && !flushing && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Flushed.
            </div>
          )}
          <button disabled={flushing} onClick={() => void runFlush()}>
            {flushing ? "Flushing…" : "Flush"}
          </button>
        </section>

        {/* --- Danger zone: gc + delete --- */}
        <section className="storage-section storage-danger">
          <h3>Danger zone</h3>

          <p className="storage-help">
            Garbage collection permanently reclaims unreferenced data. This
            cannot be undone.
          </p>
          {gcError && (
            <div className="error storage-inline-error">{gcError}</div>
          )}
          {gcDone && !gcRunning && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Garbage collection
              complete.
            </div>
          )}
          {!confirmGc ? (
            <button
              className="storage-danger-btn"
              disabled={gcRunning}
              onClick={() => setConfirmGc(true)}
            >
              Run garbage collection
            </button>
          ) : (
            <div className="storage-confirm">
              <span>
                Permanently reclaim unreferenced data? This cannot be undone.
              </span>
              <button
                className="storage-danger-btn"
                disabled={gcRunning}
                onClick={() => void runGc()}
              >
                {gcRunning ? "Collecting…" : "Yes, collect"}
              </button>
              <button disabled={gcRunning} onClick={() => setConfirmGc(false)}>
                Cancel
              </button>
            </div>
          )}

          <div style={{ marginTop: 16 }}>
            <p className="storage-help">
              Delete permanently destroys a repository at the given URL,
              including all of its revisions and data. This cannot be undone.
            </p>
            <div className="onboarding-field">
              <label htmlFor="repo-delete-url">Repository URL to delete</label>
              <input
                id="repo-delete-url"
                type="text"
                value={deleteUrl}
                onChange={(e) => {
                  setDeleteUrl(e.target.value);
                  setConfirmDelete(false);
                }}
                placeholder="lore://localhost/my-repo"
              />
            </div>
            {deleteError && (
              <div className="error storage-inline-error">{deleteError}</div>
            )}
            {deleteResult && !deleting && (
              <div className="storage-ok">
                <span className="success-icon">&#10003;</span> Repository
                deleted.
              </div>
            )}
            {!confirmDelete ? (
              <button
                className="storage-danger-btn"
                disabled={!deleteUrl.trim() || deleting}
                onClick={() => setConfirmDelete(true)}
              >
                Delete repository
              </button>
            ) : (
              <div className="storage-confirm">
                <span>
                  Permanently delete <code>{deleteUrl.trim()}</code> and all its
                  data? This cannot be undone.
                </span>
                <button
                  className="storage-danger-btn"
                  disabled={deleting}
                  onClick={() => void runDelete()}
                >
                  {deleting ? "Deleting…" : "Yes, delete"}
                </button>
                <button
                  disabled={deleting}
                  onClick={() => setConfirmDelete(false)}
                >
                  Cancel
                </button>
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
