import { useCallback, useEffect, useState } from "react";
import OnboardingFlow from "./onboarding/OnboardingFlow";
import ThemeEditor from "./theme/ThemeEditor";
import StoragePanel from "./StoragePanel";
import RepositoryPanel from "./RepositoryPanel";
import LocksPanel from "./LocksPanel";
import DependenciesPanel from "./DependenciesPanel";
import HistoryPanel from "./HistoryPanel";
import BranchesPanel from "./BranchesPanel";
import AccountPanel from "./AccountPanel";
import CommandPalette, { OPEN_PALETTE_EVENT } from "./palette/CommandPalette";
import {
  api,
  branchArchiveApi,
  branchCreateApi,
  branchInfoApi,
  branchMergeIntoApi,
  branchMetadataGetApi,
  branchMergeAbortApi,
  branchMergeUnresolveApi,
  branchProtectApi,
  branchUnprotectApi,
  fileInfoApi,
  fileObliterateApi,
  repositoryFlushApi,
  repositoryGcApi,
  repositoryListApi,
  repositoryMetadataGetApi,
  repositoryVerifyStateApi,
  revisionDiffApi,
  revisionFindApi,
  revisionRevertLocalApi,
  revisionSyncApi,
  type Branch,
  type BranchInfoResult,
  type BranchMetadataEntry,
  type BranchMetadataGetResult,
  type FileChange,
  type FileInfoEntry,
  type MetadataEntry,
  type RepoStatus,
  type RepositoryEntry,
  type RepositoryListResult,
  type RepositoryMetadataGetResult,
  type Revision,
  type RevisionDiffResult,
  type RevisionFindEntry,
  type RevisionFindResult,
  type RevisionSyncResult,
  type VerifyStateResult,
} from "./api";

/** Extract a human-readable message from a thrown value (LoreError is
 * serialized as `{ kind, message }`; plain strings and Errors pass through). */
function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    const o = e as { message?: unknown; kind?: unknown };
    if (typeof o.message === "string") return o.message;
    if (typeof o.kind === "string") return o.kind;
  }
  return JSON.stringify(e);
}

function useAsyncError() {
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(async (fn: () => Promise<void>) => {
    try {
      setError(null);
      await fn();
    } catch (e) {
      setError(errText(e));
    }
  }, []);
  return { error, run, setError };
}

export default function App() {
  const [repo, setRepo] = useState<string>("");
  const [onboarded, setOnboarded] = useState<boolean>(
    () => localStorage.getItem("loregui.onboarded") === "true",
  );
  const [themeOpen, setThemeOpen] = useState(false);
  const [storageOpen, setStorageOpen] = useState(false);
  const [repoPanelOpen, setRepoPanelOpen] = useState(false);
  const [locksPanelOpen, setLocksPanelOpen] = useState(false);
  const [depsPanelOpen, setDepsPanelOpen] = useState(false);
  const [historyPanelOpen, setHistoryPanelOpen] = useState(false);
  const [branchesPanelOpen, setBranchesPanelOpen] = useState(false);
  const [accountPanelOpen, setAccountPanelOpen] = useState(false);
  const [status, setStatus] = useState<RepoStatus | null>(null);
  const [branches, setBranches] = useState<Branch[]>([]);
  const [history, setHistory] = useState<Revision[]>([]);
  const [message, setMessage] = useState("");
  const { error, run } = useAsyncError();

  // --- branch info state ---
  const [branchInfoData, setBranchInfoData] = useState<BranchInfoResult | null>(null);
  const [branchInfoLoading, setBranchInfoLoading] = useState(false);

  // --- branch metadata state ---
  const [branchMetaData, setBranchMetaData] = useState<BranchMetadataGetResult | null>(null);
  const [branchMetaLoading, setBranchMetaLoading] = useState(false);

  // --- file info state ---
  const [fileInfoData, setFileInfoData] = useState<FileInfoEntry | null>(null);
  const [fileInfoPath, setFileInfoPath] = useState<string | null>(null);
  const [fileInfoLoading, setFileInfoLoading] = useState(false);

  // --- repository metadata ---
  const [metadataData, setMetadataData] = useState<RepositoryMetadataGetResult | null>(null);
  const [metadataLoading, setMetadataLoading] = useState(false);

  // --- repository list state ---
  const [repoListData, setRepoListData] = useState<RepositoryListResult | null>(null);
  const [repoListLoading, setRepoListLoading] = useState(false);

  // --- flush state ---
  const [flushLoading, setFlushLoading] = useState(false);
  const [flushDone, setFlushDone] = useState(false);

  // --- gc state ---
  const [gcLoading, setGcLoading] = useState(false);
  const [gcDone, setGcDone] = useState(false);

  // --- verify state ---
  const [verifyData, setVerifyData] = useState<VerifyStateResult | null>(null);
  const [verifyLoading, setVerifyLoading] = useState(false);

  // --- revision sync state ---
  const [syncData, setSyncData] = useState<RevisionSyncResult | null>(null);
  const [syncLoading, setSyncLoading] = useState(false);

  // --- revision find state ---
  const [findData, setFindData] = useState<RevisionFindResult | null>(null);
  const [findLoading, setFindLoading] = useState(false);
  const [findKey, setFindKey] = useState("");
  const [findValue, setFindValue] = useState("");
  const [findNumber, setFindNumber] = useState("");

  // --- revision diff state ---
  const [diffData, setDiffData] = useState<RevisionDiffResult | null>(null);
  const [diffRevision, setDiffRevision] = useState<string | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);

  const refresh = useCallback(async () => {
    await run(async () => {
      setRepo(await api.currentRepository());
      setStatus(await api.status());
      setBranches(await api.branches());
      setHistory(await api.log(50));
    });
  }, [run]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // If a REAL repository is open (status resolved with a repo id), the user has
  // been set up before — skip onboarding and remember it. Note: currentRepository
  // always returns a default working-dir path, so it can't be the signal — we gate
  // on a successful status() with a repo_id instead.
  useEffect(() => {
    if (status?.repo_id && !onboarded) {
      localStorage.setItem("loregui.onboarded", "true");
      setOnboarded(true);
    }
  }, [status, onboarded]);

  const completeOnboarding = useCallback(() => {
    localStorage.setItem("loregui.onboarded", "true");
    setOnboarded(true);
    void refresh();
  }, [refresh]);

  const staged = status?.changes.filter((c) => c.staged) ?? [];
  const unstaged = status?.changes.filter((c) => !c.staged) ?? [];

  const fetchBranchInfo = useCallback(
    async (name: string) => {
      if (branchInfoData?.name === name) {
        setBranchInfoData(null);
        return;
      }
      setBranchInfoLoading(true);
      try {
        const data = await branchInfoApi.info(name);
        setBranchInfoData(data);
      } catch {
        setBranchInfoData(null);
      } finally {
        setBranchInfoLoading(false);
      }
    },
    [branchInfoData],
  );

  const fetchBranchMetadata = useCallback(
    async (name: string) => {
      if (branchMetaData?.branch === name) {
        setBranchMetaData(null);
        return;
      }
      setBranchMetaLoading(true);
      try {
        const data = await branchMetadataGetApi.metadataGet(name);
        setBranchMetaData(data);
      } catch {
        setBranchMetaData(null);
      } finally {
        setBranchMetaLoading(false);
      }
    },
    [branchMetaData],
  );

  const fetchFileInfo = useCallback(
    async (path: string) => {
      if (fileInfoPath === path) {
        setFileInfoData(null);
        setFileInfoPath(null);
        return;
      }
      setFileInfoLoading(true);
      setFileInfoPath(path);
      try {
        const result = await fileInfoApi.info([path], "", true, false);
        setFileInfoData(result.entries[0] ?? null);
      } catch {
        setFileInfoData(null);
      } finally {
        setFileInfoLoading(false);
      }
    },
    [fileInfoPath],
  );

  const fetchRevisionDiff = useCallback(
    async (hash: string) => {
      if (diffRevision === hash) {
        setDiffData(null);
        setDiffRevision(null);
        return;
      }
      setDiffLoading(true);
      setDiffRevision(hash);
      try {
        const data = await revisionDiffApi.diff(hash);
        setDiffData(data);
      } catch {
        setDiffData(null);
      } finally {
        setDiffLoading(false);
      }
    },
    [diffRevision],
  );

  const runVerifyState = useCallback(
    async (heal: boolean = false) => {
      setVerifyLoading(true);
      try {
        const data = await repositoryVerifyStateApi.verifyState("", heal);
        setVerifyData(data);
      } catch {
        setVerifyData(null);
      } finally {
        setVerifyLoading(false);
      }
    },
    [],
  );

  const runRepositoryList = useCallback(async () => {
    const url = window.prompt("Remote URL to list repositories from:");
    if (!url) return;
    setRepoListLoading(true);
    try {
      const data = await repositoryListApi.list(url);
      setRepoListData(data);
    } catch {
      setRepoListData(null);
    } finally {
      setRepoListLoading(false);
    }
  }, []);

  const runFlush = useCallback(async () => {
    setFlushLoading(true);
    setFlushDone(false);
    try {
      await repositoryFlushApi.flush();
      setFlushDone(true);
    } catch {
      setFlushDone(false);
    } finally {
      setFlushLoading(false);
    }
  }, []);

  const runGc = useCallback(async () => {
    setGcLoading(true);
    setGcDone(false);
    try {
      await repositoryGcApi.gc();
      setGcDone(true);
    } catch {
      setGcDone(false);
    } finally {
      setGcLoading(false);
    }
  }, []);

  const fetchMetadata = useCallback(async () => {
    setMetadataLoading(true);
    try {
      const data = await repositoryMetadataGetApi.metadataGet("");
      setMetadataData(data);
    } catch {
      setMetadataData(null);
    } finally {
      setMetadataLoading(false);
    }
  }, []);

  if (!onboarded) {
    return <OnboardingFlow onComplete={completeOnboarding} />;
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          Lore<span>GUI</span>
        </div>
        <div className="repo">{repo || "no repository open"}</div>
        <div className="actions">
          <button
            onClick={() => window.dispatchEvent(new Event(OPEN_PALETTE_EVENT))}
            title="Command palette (Ctrl/Cmd-K)"
          >
            ⌘K
          </button>
          <button onClick={() => setThemeOpen(true)} title="Customize theme">
            Theme
          </button>
          <button
            onClick={() => setAccountPanelOpen(true)}
            title="Account: signed-in identity, local device accounts, connect to a server"
          >
            Account
          </button>
          <button
            onClick={() => setBranchesPanelOpen(true)}
            title="Branches: list, create, switch, info, protect, archive, reset, merge"
          >
            Branches
          </button>
          <button
            onClick={() => setHistoryPanelOpen(true)}
            title="Revision history: revisions, info, diff, commit, amend, find, revert"
          >
            History
          </button>
          <button
            onClick={() => setLocksPanelOpen(true)}
            title="File locks: query, status, acquire, release"
          >
            Locks
          </button>
          <button
            onClick={() => setStorageOpen(true)}
            title="Storage backend, connectivity, shared stores"
          >
            Storage
          </button>
          <button
            onClick={() => setRepoPanelOpen(true)}
            title="Manage repository: instances, integrity, metadata, gc, delete"
          >
            Manage
          </button>
          <button
            onClick={() => setDepsPanelOpen(true)}
            title="File dependencies: view, add, remove per-file edges"
          >
            Dependencies
          </button>
          <button disabled={syncLoading} onClick={() => {
            setSyncLoading(true);
            void run(async () => {
              try {
                const result = await revisionSyncApi.sync();
                setSyncData(result);
              } finally {
                setSyncLoading(false);
              }
              await refresh();
            });
          }}>
            {syncLoading ? "Syncing..." : "Sync"}
          </button>
          <button onClick={() => void run(async () => { await api.push(); await refresh(); })}>
            Push
          </button>
          <button onClick={() => void runVerifyState(false)} title="Verify repository integrity">
            Verify
          </button>
          <button onClick={() => void runRepositoryList()} disabled={repoListLoading} title="List repositories at a remote URL">
            {repoListLoading ? "Listing..." : "List Repos"}
          </button>
          <button onClick={() => void runFlush()} disabled={flushLoading} title="Flush outstanding async tasks">
            {flushLoading ? "Flushing..." : "Flush"}
          </button>
          <button onClick={() => void runGc()} disabled={gcLoading} title="Run garbage collection to reclaim space">
            {gcLoading ? "GC..." : "GC"}
          </button>
          <button onClick={() => void fetchMetadata()} title="View repository metadata">
            Metadata
          </button>
          <button onClick={() => void refresh()}>Refresh</button>
        </div>
      </header>

      {error && <div className="error">{error}</div>}

      {verifyLoading && <p className="verify-loading">Verifying repository state...</p>}
      {verifyData && !verifyLoading && (
        <div className="verify-panel">
          <h3>
            Verify State
            <button className="meta-close" onClick={() => setVerifyData(null)}>x</button>
          </h3>
          <dl className="verify-dl">
            <dt>Fragments checked</dt>
            <dd>{verifyData.fragments.length}</dd>
            <dt>Local errors</dt>
            <dd>{verifyData.error_count}</dd>
            <dt>Remote fragments</dt>
            <dd>{verifyData.remote_fragments.length}</dd>
            <dt>Corrupted (remote)</dt>
            <dd>{verifyData.corrupted_count}</dd>
            <dt>Healed state</dt>
            <dd><code>{verifyData.healed_staged_state.slice(0, 16) || "none"}</code></dd>
          </dl>
          {verifyData.error_count > 0 && (
            <button onClick={() => void runVerifyState(true)}>Heal</button>
          )}
        </div>
      )}

      {flushLoading && <p className="verify-loading">Flushing outstanding tasks...</p>}
      {flushDone && !flushLoading && (
        <div className="verify-panel">
          <h3>
            Flush
            <button className="meta-close" onClick={() => setFlushDone(false)}>x</button>
          </h3>
          <p>All outstanding async tasks flushed successfully.</p>
        </div>
      )}

      {syncData && !syncLoading && (
        <div className="verify-panel">
          <h3>
            Sync Result
            <button className="meta-close" onClick={() => setSyncData(null)}>x</button>
          </h3>
          <dl className="verify-dl">
            <dt>Files updated</dt>
            <dd>{syncData.files_updated}</dd>
            <dt>Files deleted</dt>
            <dd>{syncData.files_deleted}</dd>
            <dt>Files changed</dt>
            <dd>{syncData.files.length}</dd>
          </dl>
          {syncData.revisions.length > 0 && (
            <ul>
              {syncData.revisions.map((r, i) => (
                <li key={i}>
                  <code>{r.revision.slice(0, 12)}</code> rev#{r.revision_number} on {r.branch}
                  {r.is_merge && <span className="badge"> merge</span>}
                  {r.has_conflicts && <span className="badge conflict"> conflicts</span>}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {gcLoading && <p className="verify-loading">Running garbage collection...</p>}
      {gcDone && !gcLoading && (
        <div className="verify-panel">
          <h3>
            Garbage Collection
            <button className="meta-close" onClick={() => setGcDone(false)}>x</button>
          </h3>
          <p>Garbage collection completed successfully.</p>
        </div>
      )}

      {repoListLoading && <p className="verify-loading">Listing remote repositories...</p>}
      {repoListData && !repoListLoading && (
        <div className="verify-panel">
          <h3>
            Repositories at {repoListData.url}
            <button className="meta-close" onClick={() => setRepoListData(null)}>x</button>
          </h3>
          {repoListData.entries.length === 0 && <p className="empty">No repositories found</p>}
          {repoListData.entries.length > 0 && (
            <ul>
              {repoListData.entries.map((entry: RepositoryEntry) => (
                <li key={entry.id}>
                  <code>{entry.id.slice(0, 12)}</code> — {entry.name}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {metadataLoading && <p className="metadata-loading">Loading repository metadata...</p>}
      {metadataData && !metadataLoading && (
        <div className="metadata-panel">
          <h3>
            Repository Metadata
            <button className="meta-close" onClick={() => setMetadataData(null)}>x</button>
          </h3>
          {metadataData.entries.length === 0 && <p className="empty">No metadata entries</p>}
          {metadataData.entries.length > 0 && (
            <dl className="metadata-dl">
              {metadataData.entries.map((entry: MetadataEntry) => (
                <span key={entry.key}>
                  <dt>{entry.key} <span className="badge">{entry.value_type}</span></dt>
                  <dd><code>{entry.value}</code></dd>
                </span>
              ))}
            </dl>
          )}
        </div>
      )}

      <div className="cols">
        <aside className="branches">
          <h2>
            Branches
            {status && <span className="badge">{status.branch}</span>}
          </h2>
          <ul>
            {branches.map((b) => (
              <li key={b.id || b.name} className={b.is_current ? "current" : ""}>
                <span>{b.name}</span>
                <button
                  className="info-btn"
                  onClick={() => void fetchBranchInfo(b.name)}
                  title="Branch info"
                >
                  info
                </button>
                <button
                  className="meta-btn"
                  onClick={() => void fetchBranchMetadata(b.name)}
                  title="Branch metadata"
                >
                  meta
                </button>
                <button
                  className="protect-btn"
                  onClick={() =>
                    void run(async () => {
                      await branchProtectApi.protect(b.name);
                      await refresh();
                    })
                  }
                  title="Protect branch"
                >
                  protect
                </button>
                <button
                  className="unprotect-btn"
                  onClick={() =>
                    void run(async () => {
                      await branchUnprotectApi.unprotect(b.name);
                      await refresh();
                    })
                  }
                  title="Unprotect branch"
                >
                  unprotect
                </button>
                {!b.is_current && (
                  <button
                    className="archive-btn"
                    onClick={() =>
                      void run(async () => {
                        await branchArchiveApi.archive(b.name);
                        await refresh();
                      })
                    }
                    title="Archive branch"
                  >
                    archive
                  </button>
                )}
                {!b.is_current && (
                  <button
                    className="merge-into-btn"
                    onClick={() => {
                      const msg = window.prompt(
                        `Merge staged changes into "${b.name}".\nCommit message:`,
                        `Merge into ${b.name}`,
                      );
                      if (msg != null) {
                        void run(async () => {
                          await branchMergeIntoApi.mergeInto(b.name, msg);
                          await refresh();
                        });
                      }
                    }}
                    title="Merge staged changes into this branch"
                  >
                    merge into
                  </button>
                )}
                {!b.is_current && (
                  <button
                    onClick={() =>
                      void run(async () => {
                        await api.switchBranch(b.name);
                        await refresh();
                      })
                    }
                  >
                    switch
                  </button>
                )}
              </li>
            ))}
            {branches.length === 0 && <li className="empty">no branches</li>}
          </ul>
          {status && (
            <p className="ahead-behind">
              ↑{status.ahead} ↓{status.behind} · rev {status.revision.slice(0, 10) || "—"}
            </p>
          )}

          <button
            className="new-branch-btn"
            onClick={() => {
              const name = window.prompt("New branch name:");
              if (name) {
                void run(async () => {
                  await branchCreateApi.create(name);
                  await refresh();
                });
              }
            }}
            title="Create a new branch"
          >
            New Branch
          </button>

          <button
            className="abort-merge-btn"
            onClick={() => {
              if (window.confirm("Abort the current merge? This will revert the working directory to its pre-merge state.")) {
                void run(async () => {
                  await branchMergeAbortApi.mergeAbort();
                  await refresh();
                });
              }
            }}
            title="Abort an in-progress merge, reverting to the pre-merge state"
          >
            Abort Merge
          </button>

          {/* --- branch info panel --- */}
          {branchInfoLoading && <p className="branch-info-loading">Loading...</p>}
          {branchInfoData && !branchInfoLoading && (
            <div className="branch-info-panel">
              <h3>
                Branch: {branchInfoData.name}
                {branchInfoData.archived && <span className="badge archived">archived</span>}
                <button
                  className="meta-close"
                  onClick={() => setBranchInfoData(null)}
                >
                  x
                </button>
              </h3>
              <dl className="branch-info-dl">
                <dt>ID</dt>
                <dd><code>{branchInfoData.id.slice(0, 12)}</code></dd>
                <dt>Category</dt>
                <dd>{branchInfoData.category || "---"}</dd>
                <dt>Creator</dt>
                <dd>{branchInfoData.creator || "---"}</dd>
                <dt>Created</dt>
                <dd>{branchInfoData.created ? new Date(branchInfoData.created * 1000).toLocaleString() : "---"}</dd>
                <dt>Latest (local)</dt>
                <dd><code>{branchInfoData.latest.slice(0, 12) || "---"}</code></dd>
                <dt>Latest (remote)</dt>
                <dd><code>{branchInfoData.latest_remote.slice(0, 12) || "---"}</code></dd>
                <dt>Parent</dt>
                <dd><code>{branchInfoData.parent.slice(0, 12) || "---"}</code></dd>
                <dt>Branch point</dt>
                <dd><code>{branchInfoData.branch_point.slice(0, 12) || "---"}</code></dd>
              </dl>
            </div>
          )}

          {/* --- branch metadata panel --- */}
          {branchMetaLoading && <p className="branch-info-loading">Loading metadata...</p>}
          {branchMetaData && !branchMetaLoading && (
            <div className="branch-info-panel">
              <h3>
                Metadata: {branchMetaData.branch || "(current)"}
                <button className="meta-close" onClick={() => setBranchMetaData(null)}>x</button>
              </h3>
              {branchMetaData.entries.length === 0 && <p className="empty">No metadata entries</p>}
              {branchMetaData.entries.length > 0 && (
                <dl className="metadata-dl">
                  {branchMetaData.entries.map((entry: BranchMetadataEntry) => (
                    <span key={entry.key}>
                      <dt>{entry.key} <span className="badge">{entry.value_type}</span></dt>
                      <dd><code>{entry.value}</code></dd>
                    </span>
                  ))}
                </dl>
              )}
            </div>
          )}
        </aside>

        <main className="changes">
          <Section
            title="Staged"
            items={staged}
            action="unstage"
            onAction={(paths) => void run(async () => { await api.unstage(paths); await refresh(); })}
            onFileInfo={(path) => void fetchFileInfo(path)}
            extraAction={{
              label: "unresolve",
              onAction: (paths) =>
                void run(async () => {
                  await branchMergeUnresolveApi.mergeUnresolve(paths);
                  await refresh();
                }),
            }}
          />
          <Section
            title="Changes"
            items={unstaged}
            action="stage"
            onAction={(paths) => void run(async () => { await api.stage(paths); await refresh(); })}
            onFileInfo={(path) => void fetchFileInfo(path)}
            extraAction={{
              label: "obliterate",
              onAction: (paths) =>
                void run(async () => {
                  for (const p of paths) {
                    await fileObliterateApi.obliterate(p);
                  }
                  await refresh();
                }),
            }}
          />
          {fileInfoLoading && <p className="file-info-loading">Loading file info...</p>}
          {fileInfoData && !fileInfoLoading && (
            <div className="file-info-panel">
              <h3>
                File: {fileInfoPath}
                <button className="meta-close" onClick={() => { setFileInfoData(null); setFileInfoPath(null); }}>x</button>
              </h3>
              <dl className="file-info-dl">
                <dt>Type</dt>
                <dd>{fileInfoData.is_file ? "file" : fileInfoData.is_dir ? "directory" : "other"}</dd>
                <dt>Hash</dt>
                <dd><code>{fileInfoData.hash.slice(0, 12) || "---"}</code></dd>
                <dt>Context</dt>
                <dd><code>{fileInfoData.context.slice(0, 12) || "---"}</code></dd>
                <dt>Size</dt>
                <dd>{fileInfoData.size}</dd>
                <dt>Local size</dt>
                <dd>{fileInfoData.local_size}</dd>
                <dt>Filter size</dt>
                <dd>{fileInfoData.filter_size}</dd>
                <dt>Mode</dt>
                <dd>{fileInfoData.mode}</dd>
                <dt>Local hash</dt>
                <dd><code>{fileInfoData.local_hash.slice(0, 12) || "---"}</code></dd>
                <dt>Status</dt>
                <dd>
                  {fileInfoData.flag_conflict && <span className="badge conflict">conflict</span>}
                  {fileInfoData.flag_modified && <span className="badge modified">modified</span>}
                  {fileInfoData.flag_added && <span className="badge added">added</span>}
                  {fileInfoData.flag_deleted && <span className="badge deleted">deleted</span>}
                  {!fileInfoData.flag_conflict && !fileInfoData.flag_modified && !fileInfoData.flag_added && !fileInfoData.flag_deleted && <span>clean</span>}
                </dd>
              </dl>
            </div>
          )}
          <div className="commit">
            <textarea
              placeholder="Commit message"
              value={message}
              onChange={(e) => setMessage(e.target.value)}
            />
            <button
              disabled={!message.trim() || staged.length === 0}
              onClick={() =>
                void run(async () => {
                  await api.commit(message.trim());
                  setMessage("");
                  await refresh();
                })
              }
            >
              Commit {staged.length} file{staged.length === 1 ? "" : "s"}
            </button>
          </div>
        </main>

        <section className="history">
          <h2>History</h2>

          {/* --- revision find panel --- */}
          <div className="find-revision-panel">
            <details>
              <summary>Find Revision</summary>
              <div className="find-form">
                <label>
                  Key <input type="text" value={findKey} onChange={(e) => setFindKey(e.target.value)} placeholder="metadata key" />
                </label>
                <label>
                  Value <input type="text" value={findValue} onChange={(e) => setFindValue(e.target.value)} placeholder="metadata value" />
                </label>
                <label>
                  Number <input type="number" value={findNumber} onChange={(e) => setFindNumber(e.target.value)} placeholder="revision #" min="0" />
                </label>
                <button
                  disabled={findLoading || (!findKey && !findNumber)}
                  onClick={() => {
                    setFindLoading(true);
                    setFindData(null);
                    void run(async () => {
                      try {
                        const result = await revisionFindApi.find(findKey, findValue, findNumber ? parseInt(findNumber, 10) : 0);
                        setFindData(result);
                      } finally {
                        setFindLoading(false);
                      }
                    });
                  }}
                >
                  {findLoading ? "Searching..." : "Find"}
                </button>
              </div>
              {findData && (
                <div className="find-results">
                  <h4>Found {findData.revisions.length} revision{findData.revisions.length === 1 ? "" : "s"}
                    <button className="meta-close" onClick={() => setFindData(null)}>x</button>
                  </h4>
                  {findData.revisions.length === 0 && <p className="empty">No matching revisions</p>}
                  <ul>
                    {findData.revisions.map((r: RevisionFindEntry) => (
                      <li key={r.signature}>
                        <code>{r.signature.slice(0, 12)}</code>
                        <button className="diff-btn" onClick={() => void fetchRevisionDiff(r.signature)} title="Show diff">diff</button>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </details>
          </div>

          <ul>
            {history.map((r) => (
              <li key={r.hash}>
                <code>{r.hash.slice(0, 8)}</code>
                <span className="msg">{r.message || "(no message)"}</span>
                <span className="meta">{r.author}</span>
                <button
                  className="diff-btn"
                  onClick={() => void fetchRevisionDiff(r.hash)}
                  title="Show diff for this revision"
                >
                  diff
                </button>
                <button
                  className="revert-btn"
                  onClick={() => {
                    const msg = window.prompt(
                      `Revert revision ${r.hash.slice(0, 8)}?\nCommit message:`,
                      `Revert "${r.message || r.hash.slice(0, 8)}"`,
                    );
                    if (msg != null) {
                      void run(async () => {
                        const result = await revisionRevertLocalApi.revertLocal(
                          r.hash,
                          msg,
                        );
                        if (result.has_conflicts) {
                          window.alert(
                            `Revert produced conflicts in ${result.conflict_files.length} file(s):\n${result.conflict_files.map((f) => f.path).join("\n")}`,
                          );
                        }
                        await refresh();
                      });
                    }
                  }}
                  title="Revert this revision"
                >
                  revert
                </button>
              </li>
            ))}
            {history.length === 0 && <li className="empty">no revisions</li>}
          </ul>
          {diffLoading && <p className="diff-loading">Loading diff...</p>}
          {diffData && !diffLoading && diffRevision && (
            <div className="diff-panel">
              <h3>
                Diff: {diffRevision.slice(0, 8)}
                <button className="meta-close" onClick={() => { setDiffData(null); setDiffRevision(null); }}>x</button>
              </h3>
              {diffData.files.length === 0 && <p className="empty">No file changes</p>}
              <ul className="diff-files">
                {diffData.files.map((f) => (
                  <li key={f.path} className={`diff-file ${f.action}`}>
                    <span className="diff-action">{f.action_short}</span>
                    <span className="diff-path">{f.path}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </section>
      </div>

      {themeOpen && (
        <div
          role="dialog"
          aria-modal="true"
          aria-label="Theme settings"
          onClick={() => setThemeOpen(false)}
          style={{
            position: "fixed",
            inset: 0,
            background: "rgba(0,0,0,0.5)",
            display: "flex",
            alignItems: "flex-start",
            justifyContent: "center",
            padding: "32px 16px",
            overflowY: "auto",
            zIndex: 1000,
          }}
        >
          <div onClick={(e) => e.stopPropagation()} style={{ width: "100%", maxWidth: 720 }}>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                background: "var(--surface-overlay-bg)",
                color: "var(--surface-base-text)",
                border: "1px solid var(--surface-overlay-border)",
                borderBottom: "none",
                borderRadius: "8px 8px 0 0",
                padding: "12px 16px",
              }}
            >
              <strong>Theme</strong>
              <button onClick={() => setThemeOpen(false)} title="Close">
                Close
              </button>
            </div>
            <ThemeEditor />
          </div>
        </div>
      )}

      {branchesPanelOpen && (
        <BranchesPanel onClose={() => setBranchesPanelOpen(false)} />
      )}
      {historyPanelOpen && (
        <HistoryPanel onClose={() => setHistoryPanelOpen(false)} />
      )}

      {locksPanelOpen && (
        <LocksPanel onClose={() => setLocksPanelOpen(false)} />
      )}

      {accountPanelOpen && (
        <AccountPanel onClose={() => setAccountPanelOpen(false)} />
      )}

      {storageOpen && <StoragePanel onClose={() => setStorageOpen(false)} />}

      {repoPanelOpen && (
        <RepositoryPanel onClose={() => setRepoPanelOpen(false)} />
      )}

      {depsPanelOpen && (
        <DependenciesPanel onClose={() => setDepsPanelOpen(false)} />
      )}

      <CommandPalette />
    </div>
  );
}

function Section({
  title,
  items,
  action,
  onAction,
  onFileInfo,
  extraAction,
}: {
  title: string;
  items: FileChange[];
  action: string;
  onAction: (paths: string[]) => void;
  onFileInfo?: (path: string) => void;
  extraAction?: { label: string; onAction: (paths: string[]) => void };
}) {
  return (
    <div className="section">
      <h3>
        {title} <span className="count">{items.length}</span>
        {items.length > 0 && (
          <button className="all" onClick={() => onAction(items.map((i) => i.path))}>
            {action} all
          </button>
        )}
      </h3>
      <ul>
        {items.map((c) => (
          <li key={c.path}>
            <span className={`kind ${c.kind}`}>{c.kind[0].toUpperCase()}</span>
            <span className="path">{c.path}</span>
            {onFileInfo && (
              <button className="info-btn" onClick={() => onFileInfo(c.path)}>info</button>
            )}
            <button onClick={() => onAction([c.path])}>{action}</button>
            {extraAction && (
              <button onClick={() => extraAction.onAction([c.path])}>
                {extraAction.label}
              </button>
            )}
          </li>
        ))}
        {items.length === 0 && <li className="empty">nothing</li>}
      </ul>
    </div>
  );
}
