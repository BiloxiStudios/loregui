import { useCallback, useEffect, useState } from "react";
import {
  api,
  branchListApi,
  branchInfoApi,
  branchProtectApi,
  branchUnprotectApi,
  branchArchiveApi,
  branchResetApi,
  branchLatestListApi,
  branchMergeStartApi,
  branchMergeIntoApi,
  branchMergeRestartApi,
  branchMergeResolveApi,
  branchMergeResolveMineApi,
  branchMergeResolveTheirsApi,
  branchMergeUnresolveApi,
  branchMergeAbortApi,
  type Branch,
  type BranchListEntry,
  type BranchInfoResult,
  type BranchLatestListEntry,
  type BranchMergeStartResult,
} from "./api";

/**
 * Branches panel (top-bar nav, daily domain) — the rich home for the branch
 * domain, per `docs/INFORMATION-ARCHITECTURE.md` (branch row: "Branches panel +
 * row menus", a daily core-loop domain). The branch list is the centerpiece;
 * each row offers the common per-branch actions (switch / info / protect /
 * archive).
 *
 * Surfaces the common branch ops that have a registered `#[tauri::command]`
 * (verified in src-tauri/src/commands.rs + lib.rs generate_handler!):
 *  - branches          → the centerpiece list: name, current marker, latest
 *                        revision (the core `branches` command).
 *  - create_branch     → create a branch at the current revision.
 *  - switch_branch     → switch the working copy to a branch.
 *  - branch_info       → a branch's details (id, category, parent, creator…).
 *  - branch_protect /  → mark / unmark a branch protected. Unprotect lifts a
 *    branch_unprotect     safeguard, so it confirms first.
 *  - branch_archive    → archive a branch (hides it from the default list);
 *                        confirms first.
 *  - branch_reset      → move a branch pointer to a revision (destructive —
 *                        discards revisions ahead of the target); confirms.
 *  - branch_latest_list→ the latest revision pointer(s) for branches.
 *  - merge flow        → branch_merge_start (begin a merge from a source
 *                        branch), and if it conflicts the resolve helpers
 *                        (resolve_mine / resolve_theirs / resolve / restart /
 *                        unresolve) plus branch_merge_into (finish into a
 *                        branch) and branch_merge_abort (cancel). A stateful
 *                        flow, surfaced clearly in its own section.
 *
 * The richer per-entity branch ops (branch_create with explicit category/id,
 * branch_metadata_get) live in the command palette; this panel covers the
 * common workflow. Each section handles empty / loading / error / success and
 * is themed entirely via `--surface-*` tokens, reusing the shared overlay-panel
 * classes from StoragePanel/RepositoryPanel/LocksPanel/DependenciesPanel/
 * HistoryPanel (no new styles needed). Esc closes; one primary action per
 * section; destructive reset/unprotect/archive confirm.
 */

import { formatMixed } from "./time/units";

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

export default function BranchesPanel({ onClose }: { onClose: () => void }) {
  // --- branches (the centerpiece list) ---
  const [branches, setBranches] = useState<Branch[] | null>(null);
  const [listEntries, setListEntries] = useState<BranchListEntry[] | null>(
    null,
  );
  const [showArchived, setShowArchived] = useState(false);
  const [listLoading, setListLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);

  // Per-row pending action (switch/protect/archive on a named branch).
  const [rowBusy, setRowBusy] = useState<string | null>(null);
  const [rowError, setRowError] = useState<string | null>(null);
  const [rowOk, setRowOk] = useState<string | null>(null);

  // --- create ---
  const [createName, setCreateName] = useState("");
  const [creating, setCreating] = useState(false);
  const [createDone, setCreateDone] = useState<string | null>(null);
  const [createError, setCreateError] = useState<string | null>(null);

  // --- switch ---
  const [switchName, setSwitchName] = useState("");
  const [switching, setSwitching] = useState(false);
  const [switchDone, setSwitchDone] = useState<string | null>(null);
  const [switchError, setSwitchError] = useState<string | null>(null);

  // --- info (selected branch detail) ---
  const [infoName, setInfoName] = useState("");
  const [info, setInfo] = useState<BranchInfoResult | null>(null);
  const [infoLoading, setInfoLoading] = useState(false);
  const [infoError, setInfoError] = useState<string | null>(null);

  // --- protect / unprotect ---
  const [protName, setProtName] = useState("");
  const [protBusy, setProtBusy] = useState(false);
  const [protDone, setProtDone] = useState<string | null>(null);
  const [protError, setProtError] = useState<string | null>(null);
  const [confirmUnprotect, setConfirmUnprotect] = useState(false);

  // --- archive (destructive — confirms) ---
  const [archName, setArchName] = useState("");
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [archiving, setArchiving] = useState(false);
  const [archDone, setArchDone] = useState<string | null>(null);
  const [archError, setArchError] = useState<string | null>(null);

  // --- reset (destructive — confirms) ---
  const [resetBranch, setResetBranch] = useState("");
  const [resetRevision, setResetRevision] = useState("");
  const [confirmReset, setConfirmReset] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [resetDone, setResetDone] = useState<BranchLatestListEntry | null>(
    null,
  );
  const [resetError, setResetError] = useState<string | null>(null);

  // --- latest_list (latest revision pointers) ---
  const [latestBranch, setLatestBranch] = useState("");
  const [latestLimit, setLatestLimit] = useState("");
  const [latest, setLatest] = useState<BranchLatestListEntry[] | null>(null);
  const [latestLoading, setLatestLoading] = useState(false);
  const [latestError, setLatestError] = useState<string | null>(null);

  // --- merge flow (stateful) ---
  const [mergeSource, setMergeSource] = useState("");
  const [mergeMessage, setMergeMessage] = useState("");
  const [mergeNoCommit, setMergeNoCommit] = useState(false);
  const [merging, setMerging] = useState(false);
  const [mergeResult, setMergeResult] = useState<BranchMergeStartResult | null>(
    null,
  );
  const [mergeError, setMergeError] = useState<string | null>(null);

  // merge conflict resolution (paths + chosen side)
  const [resolvePaths, setResolvePaths] = useState("");
  const [mergeStepBusy, setMergeStepBusy] = useState(false);
  const [mergeStepOk, setMergeStepOk] = useState<string | null>(null);
  const [mergeStepError, setMergeStepError] = useState<string | null>(null);

  // finish merge into a target branch
  const [mergeIntoTarget, setMergeIntoTarget] = useState("");
  const [mergeIntoBusy, setMergeIntoBusy] = useState(false);
  const [mergeIntoOk, setMergeIntoOk] = useState<string | null>(null);
  const [mergeIntoError, setMergeIntoError] = useState<string | null>(null);

  // abort merge
  const [confirmAbort, setConfirmAbort] = useState(false);
  const [aborting, setAborting] = useState(false);
  const [abortOk, setAbortOk] = useState(false);
  const [abortError, setAbortError] = useState<string | null>(null);

  // The centerpiece uses the core `branches` command for the current marker and
  // latest revision; `branch_list` is queried only when "show archived" is on
  // (the core command doesn't expose archived/category). Merge them by name so
  // the list shows archived branches too when requested.
  const loadBranches = useCallback(async () => {
    setListLoading(true);
    setListError(null);
    try {
      const [core, listed] = await Promise.all([
        api.branches(),
        branchListApi.list(showArchived),
      ]);
      setBranches(core);
      setListEntries(listed.entries);
    } catch (e) {
      setBranches(null);
      setListEntries(null);
      setListError(errMsg(e));
    } finally {
      setListLoading(false);
    }
  }, [showArchived]);

  // Load the list on first open + whenever the archived toggle changes.
  useEffect(() => {
    void loadBranches();
  }, [loadBranches]);

  // Esc closes the panel (DESIGN-SYSTEM: overlays dismiss on Esc).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const runCreate = useCallback(async () => {
    if (!createName.trim()) return;
    setCreating(true);
    setCreateError(null);
    setCreateDone(null);
    try {
      await api.createBranch(createName.trim());
      setCreateDone(createName.trim());
      setCreateName("");
      void loadBranches();
    } catch (e) {
      setCreateError(errMsg(e));
    } finally {
      setCreating(false);
    }
  }, [createName, loadBranches]);

  const runSwitch = useCallback(
    async (name: string) => {
      if (!name.trim()) return;
      setSwitching(true);
      setSwitchError(null);
      setSwitchDone(null);
      try {
        await api.switchBranch(name.trim());
        setSwitchDone(name.trim());
        void loadBranches();
      } catch (e) {
        setSwitchError(errMsg(e));
      } finally {
        setSwitching(false);
      }
    },
    [loadBranches],
  );

  const runInfo = useCallback(async (name: string) => {
    if (!name.trim()) return;
    setInfoLoading(true);
    setInfoError(null);
    setInfo(null);
    try {
      setInfo(await branchInfoApi.info(name.trim()));
    } catch (e) {
      setInfoError(errMsg(e));
    } finally {
      setInfoLoading(false);
    }
  }, []);

  const runProtect = useCallback(
    async (name: string) => {
      if (!name.trim()) return;
      setProtBusy(true);
      setProtError(null);
      setProtDone(null);
      try {
        await branchProtectApi.protect(name.trim());
        setProtDone(`Protected ${name.trim()}`);
        void loadBranches();
      } catch (e) {
        setProtError(errMsg(e));
      } finally {
        setProtBusy(false);
      }
    },
    [loadBranches],
  );

  const runUnprotect = useCallback(async () => {
    if (!protName.trim()) return;
    setProtBusy(true);
    setProtError(null);
    setProtDone(null);
    try {
      await branchUnprotectApi.unprotect(protName.trim());
      setProtDone(`Unprotected ${protName.trim()}`);
      void loadBranches();
    } catch (e) {
      setProtError(errMsg(e));
    } finally {
      setProtBusy(false);
      setConfirmUnprotect(false);
    }
  }, [protName, loadBranches]);

  const runArchive = useCallback(async () => {
    if (!archName.trim()) return;
    setArchiving(true);
    setArchError(null);
    setArchDone(null);
    try {
      await branchArchiveApi.archive(archName.trim());
      setArchDone(archName.trim());
      void loadBranches();
    } catch (e) {
      setArchError(errMsg(e));
    } finally {
      setArchiving(false);
      setConfirmArchive(false);
    }
  }, [archName, loadBranches]);

  const runReset = useCallback(async () => {
    if (!resetRevision.trim()) return;
    setResetting(true);
    setResetError(null);
    setResetDone(null);
    try {
      const res = await branchResetApi.reset(
        resetRevision.trim(),
        resetBranch.trim(),
      );
      setResetDone(res);
      void loadBranches();
    } catch (e) {
      setResetError(errMsg(e));
    } finally {
      setResetting(false);
      setConfirmReset(false);
    }
  }, [resetRevision, resetBranch, loadBranches]);

  const runLatest = useCallback(async () => {
    setLatestLoading(true);
    setLatestError(null);
    setLatest(null);
    try {
      const lim = parseInt(latestLimit, 10);
      const res = await branchLatestListApi.latestList(
        latestBranch.trim(),
        Number.isFinite(lim) && lim > 0 ? lim : 0,
      );
      setLatest(res.entries);
    } catch (e) {
      setLatestError(errMsg(e));
    } finally {
      setLatestLoading(false);
    }
  }, [latestBranch, latestLimit]);

  const runMergeStart = useCallback(async () => {
    if (!mergeSource.trim()) return;
    setMerging(true);
    setMergeError(null);
    setMergeResult(null);
    setMergeStepOk(null);
    setMergeStepError(null);
    setMergeIntoOk(null);
    setAbortOk(false);
    try {
      const res = await branchMergeStartApi.mergeStart(
        mergeSource.trim(),
        mergeMessage.trim(),
        mergeNoCommit,
      );
      setMergeResult(res);
      if (res.has_conflicts && res.conflict_files.length > 0) {
        setResolvePaths(res.conflict_files.join("\n"));
      }
      void loadBranches();
    } catch (e) {
      setMergeError(errMsg(e));
    } finally {
      setMerging(false);
    }
  }, [mergeSource, mergeMessage, mergeNoCommit, loadBranches]);

  // A conflict-resolution step: pick mine/theirs/resolve(both)/restart/
  // unresolve over the listed paths.
  const runMergeResolve = useCallback(
    async (
      kind: "mine" | "theirs" | "resolve" | "restart" | "unresolve",
    ) => {
      const paths = splitPaths(resolvePaths);
      if (paths.length === 0) return;
      setMergeStepBusy(true);
      setMergeStepError(null);
      setMergeStepOk(null);
      try {
        if (kind === "mine") {
          const r = await branchMergeResolveMineApi.mergeResolveMine(paths);
          setMergeStepOk(`Resolved ${r.resolved_paths.length} with “mine”.`);
        } else if (kind === "theirs") {
          const r =
            await branchMergeResolveTheirsApi.mergeResolveTheirs(paths);
          setMergeStepOk(
            `Resolved ${r.resolved_paths.length} with “theirs”.`,
          );
        } else if (kind === "resolve") {
          const r = await branchMergeResolveApi.mergeResolve(paths);
          setMergeStepOk(`Marked ${r.resolved_paths.length} resolved.`);
        } else if (kind === "restart") {
          const r = await branchMergeRestartApi.mergeRestart(paths);
          setMergeStepOk(
            `Restarted merge — ${r.conflict_files.length} conflict${
              r.conflict_files.length === 1 ? "" : "s"
            } remaining.`,
          );
        } else {
          const r = await branchMergeUnresolveApi.mergeUnresolve(paths);
          setMergeStepOk(
            `Reopened ${r.unresolved_paths.length} as unresolved.`,
          );
        }
      } catch (e) {
        setMergeStepError(errMsg(e));
      } finally {
        setMergeStepBusy(false);
      }
    },
    [resolvePaths],
  );

  const runMergeInto = useCallback(async () => {
    if (!mergeIntoTarget.trim()) return;
    setMergeIntoBusy(true);
    setMergeIntoError(null);
    setMergeIntoOk(null);
    try {
      const r = await branchMergeIntoApi.mergeInto(
        mergeIntoTarget.trim(),
        mergeMessage.trim(),
      );
      setMergeIntoOk(`Merged into ${mergeIntoTarget.trim()} · #${r.revision_number}`);
      void loadBranches();
    } catch (e) {
      setMergeIntoError(errMsg(e));
    } finally {
      setMergeIntoBusy(false);
    }
  }, [mergeIntoTarget, mergeMessage, loadBranches]);

  const runMergeAbort = useCallback(async () => {
    setAborting(true);
    setAbortError(null);
    setAbortOk(false);
    try {
      await branchMergeAbortApi.mergeAbort();
      setAbortOk(true);
      setMergeResult(null);
      void loadBranches();
    } catch (e) {
      setAbortError(errMsg(e));
    } finally {
      setAborting(false);
      setConfirmAbort(false);
    }
  }, [loadBranches]);

  // Prefill the per-branch forms from a list row (one-click row actions).
  const prefillInfo = useCallback(
    (name: string) => {
      setInfoName(name);
      void runInfo(name);
    },
    [runInfo],
  );

  const prefillArchive = useCallback((name: string) => {
    setArchName(name);
    setConfirmArchive(false);
    setArchDone(null);
    setArchError(null);
  }, []);

  // Category/archived flags come from branch_list; index them by name so the
  // centerpiece rows (from the core `branches` command) can show them.
  const metaByName = new Map(
    (listEntries ?? []).map((e) => [e.name, e]),
  );

  // When "show archived" is on, the core `branches` command omits archived
  // branches, so fall back to branch_list entries for the row set.
  const rows: Array<{
    name: string;
    latest: string;
    isCurrent: boolean;
    archived: boolean;
    category: string;
  }> = showArchived
    ? (listEntries ?? []).map((e) => ({
        name: e.name,
        latest: e.latest,
        isCurrent: e.is_current,
        archived: e.archived,
        category: e.category,
      }))
    : (branches ?? []).map((b) => ({
        name: b.name,
        latest: b.latest_revision,
        isCurrent: b.is_current,
        archived: metaByName.get(b.name)?.archived ?? false,
        category: metaByName.get(b.name)?.category ?? "",
      }));

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Branches"
      className="storage-scrim"
      onClick={onClose}
    >
      <div className="storage-panel" onClick={(e) => e.stopPropagation()}>
        <header className="storage-panel-header">
          <h2>Branches</h2>
          <button onClick={onClose} title="Close (Esc)">
            Close
          </button>
        </header>

        {/* --- Branches (the centerpiece list) --- */}
        <section className="storage-section">
          <h3>Branches</h3>
          <p className="storage-help">
            The branches in this repository. The current branch is marked; each
            row's latest revision is shown. Use a row to switch to a branch, see
            its details, protect it, or archive it.
          </p>
          <label
            htmlFor="branch-show-archived"
            style={{ display: "block", marginBottom: 6 }}
          >
            <input
              id="branch-show-archived"
              type="checkbox"
              checked={showArchived}
              onChange={(e) => setShowArchived(e.target.checked)}
            />{" "}
            Show archived branches
          </label>
          {rowError && (
            <div className="error storage-inline-error">{rowError}</div>
          )}
          {rowOk && <div className="storage-ok">{rowOk}</div>}
          {listError && (
            <>
              <div className="error storage-inline-error">{listError}</div>
              <button onClick={() => void loadBranches()}>Retry</button>
            </>
          )}
          {!listError && rows.length > 0 && !listLoading && (
            <ul className="storage-list">
              {rows.map((b) => (
                <li key={b.name}>
                  <code>{b.name}</code>
                  <span
                    className={`storage-status ${
                      b.isCurrent ? "ok" : "unknown"
                    }`}
                  >
                    {b.isCurrent ? "● current" : "○"}
                    {b.archived ? " · archived" : ""} ·{" "}
                    {b.latest ? b.latest.slice(0, 12) : "—"}
                  </span>
                  {!b.isCurrent && (
                    <button
                      disabled={rowBusy === b.name || switching}
                      onClick={() => {
                        setRowBusy(b.name);
                        setRowError(null);
                        setRowOk(null);
                        void (async () => {
                          try {
                            await api.switchBranch(b.name);
                            setRowOk(`Switched to ${b.name}.`);
                            void loadBranches();
                          } catch (e) {
                            setRowError(errMsg(e));
                          } finally {
                            setRowBusy(null);
                          }
                        })();
                      }}
                      title="Switch the working copy to this branch"
                    >
                      {rowBusy === b.name ? "Switching…" : "Switch"}
                    </button>
                  )}
                  <button
                    onClick={() => prefillInfo(b.name)}
                    title="Show this branch's details below"
                  >
                    Info
                  </button>
                  <button
                    disabled={protBusy}
                    onClick={() => {
                      setProtName(b.name);
                      void runProtect(b.name);
                    }}
                    title="Mark this branch protected"
                  >
                    Protect
                  </button>
                  <button
                    onClick={() => prefillArchive(b.name)}
                    title="Fill the archive form below with this branch"
                  >
                    Archive…
                  </button>
                </li>
              ))}
            </ul>
          )}
          {!listError && rows.length === 0 && !listLoading && (
            <p className="empty">
              No {showArchived ? "archived " : ""}branches
              {showArchived ? "." : " — create one below to get started."}
            </p>
          )}
          <button
            className="storage-primary"
            disabled={listLoading}
            onClick={() => void loadBranches()}
          >
            {listLoading ? "Loading…" : "Refresh branches"}
          </button>
        </section>

        {/* --- Selected branch detail (info) --- */}
        {infoName && (
          <section className="storage-section">
            <h3>
              Branch <code>{infoName}</code>
            </h3>
            {infoLoading && <p className="storage-help">Loading details…</p>}
            {infoError && (
              <div className="error storage-inline-error">{infoError}</div>
            )}
            {info && !infoLoading && (
              <dl className="metadata-dl">
                <span>
                  <dt>Name</dt>
                  <dd>{info.name || "—"}</dd>
                </span>
                <span>
                  <dt>ID</dt>
                  <dd>
                    <code>{info.id.slice(0, 16) || "—"}</code>
                  </dd>
                </span>
                <span>
                  <dt>Category</dt>
                  <dd>{info.category || "—"}</dd>
                </span>
                <span>
                  <dt>Latest revision</dt>
                  <dd>
                    <code>{info.latest.slice(0, 16) || "—"}</code>
                  </dd>
                </span>
                <span>
                  <dt>Parent</dt>
                  <dd>{info.parent || "—"}</dd>
                </span>
                <span>
                  <dt>Branch point</dt>
                  <dd>
                    <code>{info.branch_point.slice(0, 16) || "—"}</code>
                  </dd>
                </span>
                <span>
                  <dt>Creator</dt>
                  <dd>{info.creator || "—"}</dd>
                </span>
                <span>
                  <dt>Created</dt>
                  <dd>{formatMixed(info.created, "branch.created")}</dd>
                </span>
                <span>
                  <dt>Archived</dt>
                  <dd>{info.archived ? "yes" : "no"}</dd>
                </span>
              </dl>
            )}
          </section>
        )}

        {/* --- Create --- */}
        <section className="storage-section">
          <h3>Create branch</h3>
          <p className="storage-help">
            Create a new branch at the current revision.
          </p>
          <div className="onboarding-field">
            <label htmlFor="branch-create-name">Branch name *</label>
            <input
              id="branch-create-name"
              type="text"
              value={createName}
              onChange={(e) => setCreateName(e.target.value)}
              placeholder="e.g. feature/new-level"
            />
          </div>
          {createError && (
            <div className="error storage-inline-error">{createError}</div>
          )}
          {createDone && !creating && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Created{" "}
              <code>{createDone}</code>.
            </div>
          )}
          <button
            className="storage-primary"
            disabled={!createName.trim() || creating}
            onClick={() => void runCreate()}
          >
            {creating ? "Creating…" : "Create branch"}
          </button>
        </section>

        {/* --- Switch --- */}
        <section className="storage-section">
          <h3>Switch branch</h3>
          <p className="storage-help">
            Switch the working copy to another branch by name.
          </p>
          <div className="onboarding-field">
            <label htmlFor="branch-switch-name">Branch name *</label>
            <input
              id="branch-switch-name"
              type="text"
              value={switchName}
              onChange={(e) => setSwitchName(e.target.value)}
              placeholder="e.g. main"
            />
          </div>
          {switchError && (
            <div className="error storage-inline-error">{switchError}</div>
          )}
          {switchDone && !switching && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Switched to{" "}
              <code>{switchDone}</code>.
            </div>
          )}
          <button
            disabled={!switchName.trim() || switching}
            onClick={() => void runSwitch(switchName)}
          >
            {switching ? "Switching…" : "Switch"}
          </button>
        </section>

        {/* --- Info (by name) --- */}
        <section className="storage-section">
          <h3>Branch info</h3>
          <p className="storage-help">
            Look up a branch's details by name (also reachable from a row's
            Info button).
          </p>
          <div className="onboarding-field">
            <label htmlFor="branch-info-name">Branch name *</label>
            <input
              id="branch-info-name"
              type="text"
              value={infoName}
              onChange={(e) => setInfoName(e.target.value)}
              placeholder="e.g. main"
            />
          </div>
          <button
            disabled={!infoName.trim() || infoLoading}
            onClick={() => void runInfo(infoName)}
          >
            {infoLoading ? "Loading…" : "Get info"}
          </button>
        </section>

        {/* --- Protect / unprotect --- */}
        <section className="storage-section storage-danger">
          <h3>Protect &amp; unprotect</h3>
          <p className="storage-help">
            Protect a branch to guard it against accidental changes. Unprotect
            lifts that safeguard, so it's confirmed first.
          </p>
          <div className="onboarding-field">
            <label htmlFor="branch-prot-name">Branch name *</label>
            <input
              id="branch-prot-name"
              type="text"
              value={protName}
              onChange={(e) => {
                setProtName(e.target.value);
                setConfirmUnprotect(false);
              }}
              placeholder="e.g. main"
            />
          </div>
          {protError && (
            <div className="error storage-inline-error">{protError}</div>
          )}
          {protDone && !protBusy && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> {protDone}.
            </div>
          )}
          <button
            disabled={!protName.trim() || protBusy}
            onClick={() => void runProtect(protName)}
          >
            {protBusy ? "Working…" : "Protect"}
          </button>{" "}
          {!confirmUnprotect ? (
            <button
              className="storage-danger-btn"
              disabled={!protName.trim() || protBusy}
              onClick={() => setConfirmUnprotect(true)}
            >
              Unprotect
            </button>
          ) : (
            <div className="storage-confirm">
              <span>
                Unprotect <code>{protName.trim()}</code>? This removes the
                safeguard against changes to the branch.
              </span>
              <button
                className="storage-danger-btn"
                disabled={protBusy}
                onClick={() => void runUnprotect()}
              >
                {protBusy ? "Unprotecting…" : "Yes, unprotect"}
              </button>
              <button
                disabled={protBusy}
                onClick={() => setConfirmUnprotect(false)}
              >
                Cancel
              </button>
            </div>
          )}
        </section>

        {/* --- Archive (destructive — confirms) --- */}
        <section className="storage-section storage-danger">
          <h3>Archive branch</h3>
          <p className="storage-help">
            Archiving hides a branch from the default list (its history is
            preserved). Confirmed first. Toggle "Show archived" above to see
            archived branches.
          </p>
          <div className="onboarding-field">
            <label htmlFor="branch-arch-name">Branch name *</label>
            <input
              id="branch-arch-name"
              type="text"
              value={archName}
              onChange={(e) => {
                setArchName(e.target.value);
                setConfirmArchive(false);
              }}
              placeholder="e.g. feature/old-prototype"
            />
          </div>
          {archError && (
            <div className="error storage-inline-error">{archError}</div>
          )}
          {archDone && !archiving && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Archived{" "}
              <code>{archDone}</code>.
            </div>
          )}
          {!confirmArchive ? (
            <button
              className="storage-danger-btn"
              disabled={!archName.trim() || archiving}
              onClick={() => setConfirmArchive(true)}
            >
              Archive branch
            </button>
          ) : (
            <div className="storage-confirm">
              <span>
                Archive <code>{archName.trim()}</code>? It will be hidden from
                the default branch list.
              </span>
              <button
                className="storage-danger-btn"
                disabled={archiving}
                onClick={() => void runArchive()}
              >
                {archiving ? "Archiving…" : "Yes, archive"}
              </button>
              <button
                disabled={archiving}
                onClick={() => setConfirmArchive(false)}
              >
                Cancel
              </button>
            </div>
          )}
        </section>

        {/* --- Reset (destructive — confirms) --- */}
        <section className="storage-section storage-danger">
          <h3>Reset branch</h3>
          <p className="storage-help">
            Move a branch's pointer to a specific revision. This discards any
            revisions ahead of the target on that branch and cannot be undone —
            so it's confirmed first.
          </p>
          <div className="onboarding-field">
            <label htmlFor="branch-reset-branch">
              Branch (empty = current)
            </label>
            <input
              id="branch-reset-branch"
              type="text"
              value={resetBranch}
              onChange={(e) => {
                setResetBranch(e.target.value);
                setConfirmReset(false);
              }}
              placeholder="current branch"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="branch-reset-rev">Target revision *</label>
            <input
              id="branch-reset-rev"
              type="text"
              value={resetRevision}
              onChange={(e) => {
                setResetRevision(e.target.value);
                setConfirmReset(false);
              }}
              placeholder="revision hash to reset to"
            />
          </div>
          {resetError && (
            <div className="error storage-inline-error">{resetError}</div>
          )}
          {resetDone && !resetting && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Reset{" "}
              <code>{resetDone.branch}</code> to{" "}
              <code>{resetDone.revision.slice(0, 12)}</code>.
            </div>
          )}
          {!confirmReset ? (
            <button
              className="storage-danger-btn"
              disabled={!resetRevision.trim() || resetting}
              onClick={() => setConfirmReset(true)}
            >
              Reset branch
            </button>
          ) : (
            <div className="storage-confirm">
              <span>
                Reset{" "}
                <code>{resetBranch.trim() || "the current branch"}</code> to{" "}
                <code>{resetRevision.trim().slice(0, 12)}</code>? Revisions ahead
                of the target will be discarded. This cannot be undone.
              </span>
              <button
                className="storage-danger-btn"
                disabled={resetting}
                onClick={() => void runReset()}
              >
                {resetting ? "Resetting…" : "Yes, reset"}
              </button>
              <button disabled={resetting} onClick={() => setConfirmReset(false)}>
                Cancel
              </button>
            </div>
          )}
        </section>

        {/* --- Latest revisions (latest_list) --- */}
        <section className="storage-section">
          <h3>Latest revisions</h3>
          <p className="storage-help">
            The latest revision pointer for one branch, or for all branches if
            left empty.
          </p>
          <div className="onboarding-field">
            <label htmlFor="branch-latest-branch">
              Branch (empty = all)
            </label>
            <input
              id="branch-latest-branch"
              type="text"
              value={latestBranch}
              onChange={(e) => setLatestBranch(e.target.value)}
              placeholder="e.g. main"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="branch-latest-limit">Limit (0 = all)</label>
            <input
              id="branch-latest-limit"
              type="number"
              min="0"
              value={latestLimit}
              onChange={(e) => setLatestLimit(e.target.value)}
              placeholder="0"
            />
          </div>
          {latestError && (
            <div className="error storage-inline-error">{latestError}</div>
          )}
          {latest && !latestLoading && (
            <>
              {latest.length === 0 ? (
                <p className="empty">No latest-revision pointers found.</p>
              ) : (
                <ul className="storage-list">
                  {latest.map((e, i) => (
                    <li key={`${e.branch}:${i}`}>
                      <code>{e.branch}</code>
                      <span className="storage-status unknown">
                        ● {e.revision ? e.revision.slice(0, 12) : "—"}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
          <button disabled={latestLoading} onClick={() => void runLatest()}>
            {latestLoading ? "Loading…" : "List latest"}
          </button>
        </section>

        {/* --- Merge flow (stateful) --- */}
        <section className="storage-section">
          <h3>Merge</h3>
          <p className="storage-help">
            Merge another branch into the current one. Start the merge from a
            source branch; if it conflicts, resolve the listed files (take mine
            or theirs, mark resolved, restart, or reopen), then finish the merge
            into a target branch — or abort to cancel it entirely.
          </p>
          <div className="onboarding-field">
            <label htmlFor="branch-merge-source">Source branch *</label>
            <input
              id="branch-merge-source"
              type="text"
              value={mergeSource}
              onChange={(e) => setMergeSource(e.target.value)}
              placeholder="branch to merge from"
            />
          </div>
          <div className="onboarding-field">
            <label htmlFor="branch-merge-msg">
              Merge message (empty = default)
            </label>
            <input
              id="branch-merge-msg"
              type="text"
              value={mergeMessage}
              onChange={(e) => setMergeMessage(e.target.value)}
              placeholder="Merge <source> into <current>"
            />
          </div>
          <label
            htmlFor="branch-merge-no-commit"
            style={{ display: "block", marginBottom: 6 }}
          >
            <input
              id="branch-merge-no-commit"
              type="checkbox"
              checked={mergeNoCommit}
              onChange={(e) => setMergeNoCommit(e.target.checked)}
            />{" "}
            No-commit — stage the merge without committing
          </label>
          {mergeError && (
            <div className="error storage-inline-error">{mergeError}</div>
          )}
          {mergeResult && !merging && !mergeResult.has_conflicts && (
            <div className="storage-ok">
              <span className="success-icon">&#10003;</span> Merge started from{" "}
              <code>{mergeResult.source_branch}</code> · #
              {mergeResult.source_revision_number} — no conflicts.
            </div>
          )}
          {mergeResult && !merging && mergeResult.has_conflicts && (
            <div className="error storage-inline-error">
              Merge produced {mergeResult.conflict_files.length} conflict
              {mergeResult.conflict_files.length === 1 ? "" : "s"}. Resolve the
              files below, then finish or abort.
            </div>
          )}
          <button
            className="storage-primary"
            disabled={!mergeSource.trim() || merging}
            onClick={() => void runMergeStart()}
          >
            {merging ? "Starting merge…" : "Start merge"}
          </button>

          {/* conflict resolution (second step of the stateful flow) */}
          <div style={{ marginTop: 16 }}>
            <p className="storage-help">
              Resolve merge conflicts: list the conflicting files, then choose
              how to resolve them.
            </p>
            <div className="onboarding-field">
              <label htmlFor="branch-merge-resolve-paths">
                Conflicting paths (one per line)
              </label>
              <textarea
                id="branch-merge-resolve-paths"
                value={resolvePaths}
                onChange={(e) => setResolvePaths(e.target.value)}
                placeholder={"Content/Maps/main.umap"}
              />
            </div>
            {mergeStepError && (
              <div className="error storage-inline-error">{mergeStepError}</div>
            )}
            {mergeStepOk && (
              <div className="storage-ok">
                <span className="success-icon">&#10003;</span> {mergeStepOk}
              </div>
            )}
            <button
              disabled={splitPaths(resolvePaths).length === 0 || mergeStepBusy}
              onClick={() => void runMergeResolve("mine")}
              title="Keep the current branch's version for these files"
            >
              Take mine
            </button>{" "}
            <button
              disabled={splitPaths(resolvePaths).length === 0 || mergeStepBusy}
              onClick={() => void runMergeResolve("theirs")}
              title="Take the source branch's version for these files"
            >
              Take theirs
            </button>{" "}
            <button
              disabled={splitPaths(resolvePaths).length === 0 || mergeStepBusy}
              onClick={() => void runMergeResolve("resolve")}
              title="Mark these files resolved as-is"
            >
              Mark resolved
            </button>{" "}
            <button
              disabled={splitPaths(resolvePaths).length === 0 || mergeStepBusy}
              onClick={() => void runMergeResolve("restart")}
              title="Re-sync these files from the merge source"
            >
              Restart
            </button>{" "}
            <button
              disabled={splitPaths(resolvePaths).length === 0 || mergeStepBusy}
              onClick={() => void runMergeResolve("unresolve")}
              title="Reopen these files as unresolved"
            >
              Reopen
            </button>
          </div>

          {/* finish into a target branch */}
          <div style={{ marginTop: 16 }}>
            <p className="storage-help">
              Finish the merge into a target branch.
            </p>
            <div className="onboarding-field">
              <label htmlFor="branch-merge-into">Target branch *</label>
              <input
                id="branch-merge-into"
                type="text"
                value={mergeIntoTarget}
                onChange={(e) => setMergeIntoTarget(e.target.value)}
                placeholder="branch to merge into"
              />
            </div>
            {mergeIntoError && (
              <div className="error storage-inline-error">{mergeIntoError}</div>
            )}
            {mergeIntoOk && (
              <div className="storage-ok">
                <span className="success-icon">&#10003;</span> {mergeIntoOk}.
              </div>
            )}
            <button
              disabled={!mergeIntoTarget.trim() || mergeIntoBusy}
              onClick={() => void runMergeInto()}
            >
              {mergeIntoBusy ? "Merging…" : "Merge into branch"}
            </button>
          </div>

          {/* abort (cancels the in-progress merge) */}
          <div style={{ marginTop: 16 }}>
            {abortError && (
              <div className="error storage-inline-error">{abortError}</div>
            )}
            {abortOk && (
              <div className="storage-ok">
                <span className="success-icon">&#10003;</span> Merge aborted.
              </div>
            )}
            {!confirmAbort ? (
              <button
                className="storage-danger-btn"
                disabled={aborting}
                onClick={() => setConfirmAbort(true)}
              >
                Abort merge
              </button>
            ) : (
              <div className="storage-confirm">
                <span>
                  Abort the in-progress merge? Any merge progress is discarded
                  and the working copy returns to its pre-merge state.
                </span>
                <button
                  className="storage-danger-btn"
                  disabled={aborting}
                  onClick={() => void runMergeAbort()}
                >
                  {aborting ? "Aborting…" : "Yes, abort"}
                </button>
                <button
                  disabled={aborting}
                  onClick={() => setConfirmAbort(false)}
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
