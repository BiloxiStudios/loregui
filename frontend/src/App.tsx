import { useCallback, useEffect, useState } from "react";
import {
  api,
  branchArchiveApi,
  branchInfoApi,
  branchMergeIntoApi,
  branchMergeUnresolveApi,
  branchProtectApi,
  fileInfoApi,
  fileObliterateApi,
  repositoryMetadataGetApi,
  repositoryStoreImmutableQueryApi,
  repositoryVerifyStateApi,
  revisionDiffApi,
  revisionRevertLocalApi,
  type Branch,
  type BranchInfoResult,
  type FileChange,
  type FileInfoEntry,
  type MetadataEntry,
  type RepoStatus,
  type RepositoryMetadataGetResult,
  type Revision,
  type RevisionDiffResult,
  type StoreImmutableQueryResult,
  type VerifyStateResult,
} from "./api";

function useAsyncError() {
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(async (fn: () => Promise<void>) => {
    try {
      setError(null);
      await fn();
    } catch (e) {
      setError(typeof e === "string" ? e : JSON.stringify(e));
    }
  }, []);
  return { error, run, setError };
}

export default function App() {
  const [repo, setRepo] = useState<string>("");
  const [status, setStatus] = useState<RepoStatus | null>(null);
  const [branches, setBranches] = useState<Branch[]>([]);
  const [history, setHistory] = useState<Revision[]>([]);
  const [message, setMessage] = useState("");
  const { error, run } = useAsyncError();

  // --- branch info state ---
  const [branchInfoData, setBranchInfoData] = useState<BranchInfoResult | null>(null);
  const [branchInfoLoading, setBranchInfoLoading] = useState(false);

  // --- file info state ---
  const [fileInfoData, setFileInfoData] = useState<FileInfoEntry | null>(null);
  const [fileInfoPath, setFileInfoPath] = useState<string | null>(null);
  const [fileInfoLoading, setFileInfoLoading] = useState(false);

  // --- repository metadata ---
  const [metadataData, setMetadataData] = useState<RepositoryMetadataGetResult | null>(null);
  const [metadataLoading, setMetadataLoading] = useState(false);

  // --- store immutable query ---
  const [storeQueryData, setStoreQueryData] = useState<StoreImmutableQueryResult | null>(null);
  const [storeQueryLoading, setStoreQueryLoading] = useState(false);

  // --- verify state ---
  const [verifyData, setVerifyData] = useState<VerifyStateResult | null>(null);
  const [verifyLoading, setVerifyLoading] = useState(false);

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

  const runStoreImmutableQuery = useCallback(async () => {
    const address = window.prompt("Fragment address to query:");
    if (address == null || !address.trim()) return;
    setStoreQueryLoading(true);
    try {
      const data = await repositoryStoreImmutableQueryApi.storeImmutableQuery(address.trim(), false);
      setStoreQueryData(data);
    } catch {
      setStoreQueryData(null);
    } finally {
      setStoreQueryLoading(false);
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

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          Lore<span>GUI</span>
        </div>
        <div className="repo">{repo || "no repository open"}</div>
        <div className="actions">
          <button onClick={() => void run(async () => { await api.sync(); await refresh(); })}>
            Sync
          </button>
          <button onClick={() => void run(async () => { await api.push(); await refresh(); })}>
            Push
          </button>
          <button onClick={() => void runVerifyState(false)} title="Verify repository integrity">
            Verify
          </button>
          <button onClick={() => void fetchMetadata()} title="View repository metadata">
            Metadata
          </button>
          <button onClick={() => void runStoreImmutableQuery()} title="Query immutable store by fragment address">
            Store Query
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

      {storeQueryLoading && <p className="store-query-loading">Querying immutable store...</p>}
      {storeQueryData && !storeQueryLoading && (
        <div className="store-query-panel">
          <h3>
            Store Immutable Query
            <button className="meta-close" onClick={() => setStoreQueryData(null)}>x</button>
          </h3>
          {storeQueryData.entries.length === 0 && <p className="empty">No fragments found</p>}
          {storeQueryData.entries.length > 0 && (
            <ul className="store-query-list">
              {storeQueryData.entries.map((entry, i) => (
                <li key={`${entry.address}-${i}`} className="store-query-entry">
                  <dl className="store-query-dl">
                    <dt>Address</dt>
                    <dd><code>{entry.address}</code>{entry.remote && <span className="badge">remote</span>}{entry.subfragment && <span className="badge">subfragment</span>}</dd>
                    <dt>Status</dt>
                    <dd>{entry.status_label} ({entry.status})</dd>
                    <dt>Payload</dt>
                    <dd>{entry.payload_size} bytes{entry.payload && " (cached)"}</dd>
                    <dt>Content</dt>
                    <dd>{entry.content_size} bytes</dd>
                    <dt>Flags</dt>
                    <dd><code>0x{entry.flags.toString(16)}</code></dd>
                  </dl>
                </li>
              ))}
            </ul>
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
