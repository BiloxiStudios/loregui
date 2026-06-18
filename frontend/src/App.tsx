import { useCallback, useEffect, useState } from "react";
import {
  api,
  branchInfoApi,
  fileDiffApi,
  type Branch,
  type BranchInfoResult,
  type FileChange,
  type FileDiffEntry,
  type RepoStatus,
  type Revision,
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

  // --- file diff state ---
  const [diffEntries, setDiffEntries] = useState<FileDiffEntry[]>([]);
  const [diffPath, setDiffPath] = useState<string | null>(null);
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

  const fetchDiff = useCallback(
    async (path: string) => {
      if (diffPath === path) {
        setDiffPath(null);
        setDiffEntries([]);
        return;
      }
      setDiffLoading(true);
      setDiffPath(path);
      try {
        const entries = await fileDiffApi.diff({ paths: [path] });
        setDiffEntries(entries);
      } catch {
        setDiffEntries([]);
      } finally {
        setDiffLoading(false);
      }
    },
    [diffPath],
  );

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
          <button onClick={() => void refresh()}>Refresh</button>
        </div>
      </header>

      {error && <div className="error">{error}</div>}

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
            onDiff={(path) => void fetchDiff(path)}
            activeDiffPath={diffPath}
          />
          <Section
            title="Changes"
            items={unstaged}
            action="stage"
            onAction={(paths) => void run(async () => { await api.stage(paths); await refresh(); })}
            onDiff={(path) => void fetchDiff(path)}
            activeDiffPath={diffPath}
          />

          {/* --- file diff panel --- */}
          {diffLoading && <p className="diff-loading">Loading diff...</p>}
          {diffPath && !diffLoading && (
            <div className="diff-panel">
              <h3>
                Diff: {diffPath}
                <button className="meta-close" onClick={() => { setDiffPath(null); setDiffEntries([]); }}>x</button>
              </h3>
              {diffEntries.length === 0 && <p className="empty">No differences found.</p>}
              {diffEntries.map((entry) => (
                <div key={entry.path} className="diff-entry">
                  <div className="diff-header">
                    <span className={`diff-action ${entry.action}`}>{entry.action}</span>
                    <span className="diff-path">{entry.path}</span>
                  </div>
                  <pre className="diff-patch">{entry.patch || "(binary or empty)"}</pre>
                </div>
              ))}
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
              </li>
            ))}
            {history.length === 0 && <li className="empty">no revisions</li>}
          </ul>
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
  onDiff,
  activeDiffPath,
}: {
  title: string;
  items: FileChange[];
  action: string;
  onAction: (paths: string[]) => void;
  onDiff: (path: string) => void;
  activeDiffPath: string | null;
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
            <button
              className={activeDiffPath === c.path ? "active" : ""}
              onClick={() => onDiff(c.path)}
            >
              diff
            </button>
            <button onClick={() => onAction([c.path])}>{action}</button>
          </li>
        ))}
        {items.length === 0 && <li className="empty">nothing</li>}
      </ul>
    </div>
  );
}
