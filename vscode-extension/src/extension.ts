// extension.ts — Lore source control for VS Code.
//
// Registers a native SCM provider (`vscode.scm.createSourceControl('lore', ...)`)
// per workspace folder that contains a lore repo, populates "Staged Changes" and
// "Changes" resource groups from `repository.status`, drives the SCM commands
// (refresh / stage / unstage / discard / commit / diff / sync / push / revert),
// adds a dedicated Activity-Bar container with Branches / History / Locks tree
// views, a status-bar branch indicator, and lock-aware file decorations — all on
// OUR lorevm engine.
//
// SCM VISIBILITY FIX (SBAI-4080): the previous build only ever called
// createSourceControl AFTER a successful `repository.status` shell-out. If the
// `lorevm` binary was missing OR the op errored for any reason, NO provider was
// registered, so nothing appeared under Source Control and the failure was
// invisible. We now detect a lore repo by the cheap presence of a `.lore`
// directory (no binary needed), register the provider unconditionally for such a
// folder, and surface a clear "lorevm not found — set lore.lorevmPath" state in
// the SCM input box / status bar when the engine can't be resolved. Status is
// then layered in best-effort on top.
//
// Open-core seam: this is the FREE lore-SCM layer. The StudioBrain entity-aware
// premium layer (template-driven validation, cross-ref decorations, asset
// previews) is a later gated addon — see PREMIUM SEAM markers below. It would
// register additional decoration providers / resource group metadata against the
// same LorevmClient without forking this provider.

import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import {
  LorevmClient,
  LorevmError,
  RepositoryStatusResult,
  StatusFile,
  CommitResult,
  FileDiffEntry,
  FileHistoryResult,
  FileStatusResult,
  LockStatus,
  RevisionSyncResult,
  BranchListResult,
  BranchCreateResult,
  BranchSwitchResult,
  BranchPushResult,
  RevisionHistoryResult,
  RevisionInfoResult,
  FileQueryResult,
  FileAcquireResult,
  FileReleaseResult,
  resolveLorevmBin,
} from './lorevmClient';

const LORE_SCHEME = 'lore';
// Virtual-document scheme used to render diff blobs / historical contents.
const LORE_DOC_SCHEME = 'lore-doc';
const DOCS_DEFAULT = 'https://loregui.com/docs/vscode';

let repositories: LoreRepository[] = [];
let outputChannel: vscode.OutputChannel;
let missingBinaryWarned = false;
let docProvider: LoreDocumentProvider;
let lockDecorations: LockDecorationProvider;
let branchesView: BranchesTreeProvider;
let historyView: HistoryTreeProvider;
let locksView: LocksTreeProvider;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel('Lore');
  context.subscriptions.push(outputChannel);

  // Virtual document provider for diffs / historical blobs.
  docProvider = new LoreDocumentProvider();
  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(LORE_DOC_SCHEME, docProvider),
  );

  // File decoration provider for lock badges (locked-by-me / locked-by-other).
  lockDecorations = new LockDecorationProvider();
  context.subscriptions.push(
    vscode.window.registerFileDecorationProvider(lockDecorations),
  );

  // Activity-bar tree views.
  branchesView = new BranchesTreeProvider();
  historyView = new HistoryTreeProvider();
  locksView = new LocksTreeProvider();
  context.subscriptions.push(
    vscode.window.registerTreeDataProvider('lore.branches', branchesView),
    vscode.window.registerTreeDataProvider('lore.history', historyView),
    vscode.window.registerTreeDataProvider('lore.locks', locksView),
  );

  // Register commands once; they dispatch to the active/selected repository.
  registerCommands(context);

  // Discover lore repos in the open workspace folders.
  await discoverRepositories(context);

  // Re-discover when workspace folders change.
  context.subscriptions.push(
    vscode.workspace.onDidChangeWorkspaceFolders(async () => {
      disposeRepositories();
      await discoverRepositories(context);
    }),
  );

  // React to settings changes (path/identity/offline) by re-discovering.
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration(async (e) => {
      if (e.affectsConfiguration('lore')) {
        missingBinaryWarned = false;
        disposeRepositories();
        await discoverRepositories(context);
      }
    }),
  );

  context.subscriptions.push({ dispose: disposeRepositories });
}

export function deactivate(): void {
  disposeRepositories();
}

function disposeRepositories(): void {
  for (const repo of repositories) {
    repo.dispose();
  }
  repositories = [];
  refreshTreeViews();
}

// ---------------------------------------------------------------------------
// Repository discovery + activation gating
// ---------------------------------------------------------------------------

/**
 * A folder is a lore repo iff it contains a `.lore` directory (the engine's
 * on-disk metadata). This is a cheap filesystem check that does NOT depend on
 * the `lorevm` binary existing — so the SCM provider is registered and visible
 * even when the engine is missing (we then show a clear "not found" state).
 */
function isLoreRepo(fsPath: string): boolean {
  try {
    return fs.existsSync(path.join(fsPath, '.lore'));
  } catch {
    return false;
  }
}

async function discoverRepositories(context: vscode.ExtensionContext): Promise<void> {
  const folders = vscode.workspace.workspaceFolders ?? [];
  for (const folder of folders) {
    const fsPath = folder.uri.fsPath;
    if (!isLoreRepo(fsPath)) {
      log(`folder ${fsPath} is not a lore repo (no .lore) — skipping`);
      continue;
    }

    const client = makeClient(fsPath, context.extensionPath);
    const repo = new LoreRepository(context, folder, client, lockDecorations);
    repositories.push(repo);
    log(`registered lore SCM for ${fsPath}`);

    // Layer status in best-effort. The provider is already visible regardless.
    void repo.refresh();
  }
  refreshTreeViews();
}

function makeClient(repoDir: string, extensionPath: string): LorevmClient {
  const cfg = vscode.workspace.getConfiguration('lore');
  return new LorevmClient({
    repoDir,
    binPath: cfg.get<string>('lorevmPath') || undefined,
    offline: cfg.get<boolean>('offline', true),
    identity: cfg.get<string>('identity') || undefined,
    loreguiDirs: loreguiCandidateDirs(repoDir),
    extensionPath,
  });
}

/**
 * Candidate loregui checkouts to search for target/{debug,release}/lorevm:
 * the workspace folder itself (if it IS the loregui repo) and its ancestors.
 */
function loreguiCandidateDirs(repoDir: string): string[] {
  const dirs: string[] = [];
  let cur = repoDir;
  for (let i = 0; i < 6; i++) {
    dirs.push(cur);
    const parent = path.dirname(cur);
    if (parent === cur) {
      break;
    }
    cur = parent;
  }
  return dirs;
}

function warnMissingBinary(): void {
  if (missingBinaryWarned) {
    return;
  }
  missingBinaryWarned = true;
  void vscode.window
    .showWarningMessage(
      'Lore: the `lorevm` engine binary was not found. Build it with ' +
        '`cargo build -p lorevm-cli` in the loregui repo, or set the ' +
        '"lore.lorevmPath" / LOREVM_BIN path.',
      'Open Settings',
      'Open Docs',
    )
    .then((choice) => {
      if (choice === 'Open Settings') {
        void vscode.commands.executeCommand('lore.openSettings');
      } else if (choice === 'Open Docs') {
        void vscode.commands.executeCommand('lore.openDocs');
      }
    });
}

// ---------------------------------------------------------------------------
// A single lore repository: SCM source control + resource groups + watcher.
// ---------------------------------------------------------------------------

class LoreRepository implements vscode.Disposable {
  readonly scm: vscode.SourceControl;
  readonly stagedGroup: vscode.SourceControlResourceGroup;
  readonly changesGroup: vscode.SourceControlResourceGroup;
  readonly folder: vscode.WorkspaceFolder;
  readonly client: LorevmClient;

  /** Last good status (for tree views, diff baselines, branch context). */
  lastStatus: RepositoryStatusResult | undefined;

  private readonly disposables: vscode.Disposable[] = [];
  private readonly lockDecorations: LockDecorationProvider;
  private readonly statusBar: vscode.StatusBarItem;
  private refreshTimer: NodeJS.Timeout | undefined;
  /** Current identity (for locked-by-me vs locked-by-other). */
  private identity: string | undefined;

  constructor(
    _context: vscode.ExtensionContext,
    folder: vscode.WorkspaceFolder,
    client: LorevmClient,
    locks: LockDecorationProvider,
  ) {
    this.folder = folder;
    this.client = client;
    this.lockDecorations = locks;
    this.identity =
      vscode.workspace.getConfiguration('lore').get<string>('identity') || undefined;

    this.scm = vscode.scm.createSourceControl(LORE_SCHEME, 'Lore', folder.uri);
    this.scm.quickDiffProvider = {
      provideOriginalResource: (uri) => this.originalResource(uri),
    };
    this.scm.inputBox.placeholder = 'Message (Ctrl+Enter to check in)';
    this.scm.acceptInputCommand = {
      command: 'lore.commit',
      title: 'Commit',
      arguments: [this],
    };

    this.stagedGroup = this.scm.createResourceGroup('staged', 'Staged Changes');
    this.changesGroup = this.scm.createResourceGroup('changes', 'Changes');
    this.stagedGroup.hideWhenEmpty = true;
    this.changesGroup.hideWhenEmpty = true;

    this.statusBar = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      100,
    );
    this.statusBar.command = { command: 'lore.sync', title: 'Sync', arguments: [this] };

    this.disposables.push(
      this.scm,
      this.stagedGroup,
      this.changesGroup,
      this.statusBar,
    );

    // If the engine can't be resolved, say so plainly in the visible surfaces.
    if (!this.client.resolveBin()) {
      this.showMissingBinaryState();
    }

    this.setupWatcher();
  }

  /** Current branch name from the last good status, if any. */
  get branchName(): string | undefined {
    return this.lastStatus?.revision?.branch_name || undefined;
  }

  get myIdentity(): string | undefined {
    return this.identity;
  }

  /** True if `uri` lives under this repository. */
  owns(uri: vscode.Uri): boolean {
    return uri.fsPath === this.folder.uri.fsPath || uri.fsPath.startsWith(this.folder.uri.fsPath + path.sep);
  }

  private showMissingBinaryState(): void {
    this.scm.inputBox.placeholder =
      'lorevm not found — set "lore.lorevmPath" (click the status bar for help)';
    this.statusBar.text = '$(warning) Lore: lorevm not found';
    this.statusBar.tooltip =
      'The lorevm engine binary was not found. Click to configure lore.lorevmPath.';
    this.statusBar.command = { command: 'lore.openSettings', title: 'Open Settings' };
    this.statusBar.show();
  }

  private setupWatcher(): void {
    const autoRefresh = vscode.workspace
      .getConfiguration('lore')
      .get<boolean>('autoRefresh', true);
    if (!autoRefresh) {
      return;
    }
    // Belongs to this repo's working tree (and not the .lore metadata dir, whose
    // churn would cause refresh storms).
    const ownsPath = (fsPath: string): boolean => {
      const rel = path.relative(this.folder.uri.fsPath, fsPath);
      if (rel.startsWith('..') || path.isAbsolute(rel)) {
        return false; // outside this workspace folder
      }
      return !fsPath.includes(`${path.sep}.lore${path.sep}`);
    };

    const watcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(this.folder, '**/*'),
    );
    const onChange = (uri: vscode.Uri) => {
      if (!ownsPath(uri.fsPath)) {
        return;
      }
      this.scheduleRefresh();
    };
    watcher.onDidCreate(onChange);
    watcher.onDidChange(onChange);
    watcher.onDidDelete(onChange);
    this.disposables.push(watcher);

    // CRITICAL (SBAI-4080 — the "real flow" SCM-empty bug): the OS-level
    // FileSystemWatcher does NOT reliably fire when the file is saved from the VS
    // Code editor. With safe-save (atomic write-to-temp-then-rename, the default
    // on many setups), network/virtual filesystems, or simply event coalescing,
    // an in-editor save can produce no onDidChange — so a user who EDITS a tracked
    // file and saves it sees the SCM "Changes" group stay empty. Refreshing on the
    // editor's own save/create/delete document events closes that gap: these fire
    // for the exact buffers the user touched, independent of the fs watcher.
    this.disposables.push(
      vscode.workspace.onDidSaveTextDocument((doc) => {
        if (ownsPath(doc.uri.fsPath)) {
          this.scheduleRefresh();
        }
      }),
      vscode.workspace.onDidCreateFiles((e) => {
        if (e.files.some((u) => ownsPath(u.fsPath))) {
          this.scheduleRefresh();
        }
      }),
      vscode.workspace.onDidDeleteFiles((e) => {
        if (e.files.some((u) => ownsPath(u.fsPath))) {
          this.scheduleRefresh();
        }
      }),
      vscode.workspace.onDidRenameFiles((e) => {
        if (
          e.files.some(
            (f) => ownsPath(f.oldUri.fsPath) || ownsPath(f.newUri.fsPath),
          )
        ) {
          this.scheduleRefresh();
        }
      }),
    );
  }

  /** Debounced refresh so a burst of file events triggers one status call. */
  private scheduleRefresh(): void {
    if (this.refreshTimer) {
      clearTimeout(this.refreshTimer);
    }
    this.refreshTimer = setTimeout(() => {
      void this.refresh();
    }, 400);
  }

  async refresh(): Promise<void> {
    if (!this.client.resolveBin()) {
      this.showMissingBinaryState();
      warnMissingBinary();
      return;
    }
    try {
      const status = await this.client.run<RepositoryStatusResult>(
        'repository.status',
        { scan: true },
      );
      this.lastStatus = status;
      const locks = await this.queryLocks(status);
      this.applyStatus(status, locks);
    } catch (err) {
      if (err instanceof LorevmError && err.kind === 'config') {
        this.showMissingBinaryState();
        warnMissingBinary();
      }
      log(`refresh failed for ${this.folder.uri.fsPath}: ${describe(err)}`);
    }
    refreshTreeViews();
  }

  /** Best-effort lock lookup for the changed paths on the current branch. */
  private async queryLocks(
    status: RepositoryStatusResult,
  ): Promise<Map<string, LockStatus>> {
    const map = new Map<string, LockStatus>();
    const branch = status.revision?.branch_name;
    const paths = status.files.map((f) => f.path);
    if (!branch || paths.length === 0) {
      return map;
    }
    try {
      const res = await this.client.run<FileStatusResult>('lock.file_status', {
        paths,
        branch,
      });
      for (const lock of res.locks) {
        map.set(lock.path, lock);
      }
    } catch (err) {
      // Lock service may be unavailable (offline / no remote) — non-fatal.
      log(`lock.file_status unavailable: ${describe(err)}`);
    }
    return map;
  }

  /** Map a status result into the SCM resource groups + lock decorations. */
  applyStatus(
    status: RepositoryStatusResult,
    locks: Map<string, LockStatus> = new Map(),
  ): void {
    const staged: vscode.SourceControlResourceState[] = [];
    const changes: vscode.SourceControlResourceState[] = [];
    const lockEntries: { uri: vscode.Uri; lock: LockStatus; mine: boolean }[] = [];

    for (const file of status.files) {
      const uri = vscode.Uri.file(path.join(this.folder.uri.fsPath, file.path));
      const lock = locks.get(file.path);
      const state = this.toResourceState(uri, file, lock);
      if (file.staged) {
        staged.push(state);
      } else {
        changes.push(state);
      }
      if (lock) {
        lockEntries.push({
          uri,
          lock,
          mine: !!this.identity && lock.owner === this.identity,
        });
      }
    }

    this.stagedGroup.resourceStates = staged;
    this.changesGroup.resourceStates = changes;
    this.scm.count = staged.length + changes.length;
    this.lockDecorations.set(this.folder.uri.fsPath, lockEntries);

    // Surface branch/revision in the SCM title + status bar (like Git).
    const rev = status.revision;
    if (rev) {
      const dirty = status.files.length > 0;
      this.scm.statusBarCommands = [
        {
          command: 'lore.sync',
          title: `$(git-branch) ${rev.branch_name}`,
          tooltip: `Lore branch ${rev.branch_name} @ r${rev.revision_number} — Sync`,
          arguments: [this],
        },
      ];
      this.statusBar.text = `$(git-branch) ${rev.branch_name} @ r${rev.revision_number}${dirty ? ' $(pencil)' : ''}`;
      this.statusBar.tooltip = `Lore: ${rev.branch_name} @ r${rev.revision_number}${dirty ? ` · ${status.files.length} change(s)` : ' · clean'} — click to Sync`;
      this.statusBar.command = { command: 'lore.sync', title: 'Sync', arguments: [this] };
      this.statusBar.show();
    }
  }

  private toResourceState(
    uri: vscode.Uri,
    file: StatusFile,
    lock: LockStatus | undefined,
  ): vscode.SourceControlResourceState {
    const decoration = decorationFor(file.action, file.conflict);
    const lockTip = lock
      ? this.identity && lock.owner === this.identity
        ? ' — locked by you'
        : ` — locked by ${lock.owner}`
      : '';
    return {
      resourceUri: uri,
      command: {
        command: 'lore.openDiff',
        title: 'Open Changes',
        arguments: [uri],
      },
      decorations: {
        strikeThrough: file.action === 'delete',
        faded: false,
        tooltip: `${actionLabel(file.action)}${file.conflict ? ' (conflict)' : ''}${lockTip}`,
        light: { iconPath: decoration.icon },
        dark: { iconPath: decoration.icon },
      },
      contextValue: lock ? 'locked' : undefined,
    };
  }

  /** quickDiff original: the file at its current committed revision baseline. */
  private originalResource(uri: vscode.Uri): vscode.Uri | undefined {
    const rel = path.relative(this.folder.uri.fsPath, uri.fsPath);
    if (rel.startsWith('..')) {
      return undefined;
    }
    return buildBlobUri(this.folder.uri.fsPath, rel, '');
  }

  /**
   * Fetch a file's content at a given revision (empty = current committed
   * baseline) by diffing it against the empty target — the engine emits a full
   * "add" patch we reverse-apply. Simpler and robust: use file.diff which gives
   * us the working-vs-baseline patch; for the baseline blob we read the patch
   * and reconstruct. To keep this deterministic we instead ask file.diff for
   * source_revision vs working and rely on the unified patch for rendering.
   */
  async baselineContent(rel: string, revision: string): Promise<string> {
    // Binary files can't be rendered as a line diff — a utf8 read + reverse
    // patch would yield mojibake. Short-circuit to a stable placeholder so the
    // diff editor shows a graceful "binary file" notice on both sides instead.
    const raw = this.readWorkingRaw(rel);
    if (raw && isBinaryBuffer(raw)) {
      return binaryPlaceholder(rel, raw.length);
    }
    // Use file.diff with source = revision (or baseline) and target = working
    // to obtain the patch, then derive the "before" text from the working file.
    const working = raw ? raw.toString('utf8') : '';
    const diff = await this.client
      .run<FileDiffEntry[]>('file.diff', {
        paths: [rel],
        source_revision: revision,
      })
      .catch(() => [] as FileDiffEntry[]);
    const entry = diff.find((d) => d.path === rel) ?? diff[0];
    if (!entry || !entry.patch) {
      // No diff → baseline equals working.
      return working;
    }
    try {
      return applyReversePatch(working, entry.patch);
    } catch (err) {
      // LOUD failure: previously the patcher silently returned the working text,
      // so a real change could render as "no change". Log it and fall back to the
      // working text only as a last resort, but make the failure visible.
      log(`applyReversePatch failed for ${rel} @ ${revision || 'baseline'}: ${describe(err)}`);
      void vscode.window.showWarningMessage(
        `Lore: could not reconstruct the baseline for ${path.basename(rel)}; ` +
          'the diff may be incomplete (see the Lore output channel).',
      );
      return working;
    }
  }

  /** Raw bytes of the working file, or undefined if it can't be read. */
  readWorkingRaw(rel: string): Buffer | undefined {
    try {
      return fs.readFileSync(path.join(this.folder.uri.fsPath, rel));
    } catch {
      return undefined;
    }
  }

  readWorking(rel: string): string {
    const raw = this.readWorkingRaw(rel);
    return raw ? raw.toString('utf8') : '';
  }

  /** Whether the working file looks binary (NUL byte or a known binary ext). */
  isBinaryWorking(rel: string): boolean {
    if (hasBinaryExtension(rel)) {
      return true;
    }
    const raw = this.readWorkingRaw(rel);
    return raw ? isBinaryBuffer(raw) : false;
  }

  dispose(): void {
    if (this.refreshTimer) {
      clearTimeout(this.refreshTimer);
    }
    for (const d of this.disposables) {
      d.dispose();
    }
    this.lockDecorations.set(this.folder.uri.fsPath, []);
  }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

function registerCommands(context: vscode.ExtensionContext): void {
  const sub = context.subscriptions;
  const reg = (id: string, fn: (...a: unknown[]) => unknown) =>
    sub.push(vscode.commands.registerCommand(id, fn));

  reg('lore.refresh', async (arg?: unknown) => {
    const repo = repoFromArg(arg);
    if (repo) {
      await repo.refresh();
    } else {
      await Promise.all(repositories.map((r) => r.refresh()));
    }
  });

  reg('lore.stage', async (...args: unknown[]) => {
    const { repo, paths } = resolveResourceTargets(args);
    if (!repo || paths.length === 0) {
      return;
    }
    await guard(() => repo.client.run('file.stage', { paths, scan: true }));
    await repo.refresh();
  });

  reg('lore.stageAll', async (arg?: unknown) => {
    const repo = repoFromArg(arg) ?? repoFromGroup(arg) ?? (await pickRepository());
    if (!repo) {
      return;
    }
    const paths = repo.changesGroup.resourceStates.map((s) =>
      path.relative(repo.folder.uri.fsPath, s.resourceUri.fsPath),
    );
    if (paths.length === 0) {
      return;
    }
    await guard(() => repo.client.run('file.stage', { paths, scan: true }));
    await repo.refresh();
  });

  reg('lore.unstage', async (...args: unknown[]) => {
    const { repo, paths } = resolveResourceTargets(args);
    if (!repo || paths.length === 0) {
      return;
    }
    await guard(() => repo.client.run('file.unstage', { paths }));
    await repo.refresh();
  });

  reg('lore.unstageAll', async (arg?: unknown) => {
    const repo = repoFromArg(arg) ?? repoFromGroup(arg) ?? (await pickRepository());
    if (!repo) {
      return;
    }
    const paths = repo.stagedGroup.resourceStates.map((s) =>
      path.relative(repo.folder.uri.fsPath, s.resourceUri.fsPath),
    );
    if (paths.length === 0) {
      return;
    }
    await guard(() => repo.client.run('file.unstage', { paths }));
    await repo.refresh();
  });

  reg('lore.discard', async (...args: unknown[]) => {
    const { repo, paths } = resolveResourceTargets(args);
    if (!repo || paths.length === 0) {
      return;
    }
    // DISCARD SEMANTICS (documented choice):
    // The dispatchable engine surface exposes NO per-file working-tree reset —
    // `file.reset` and `revision.revert` are not in lorevm's dispatch table (see
    // crates/lore-vm/src/dispatch.rs::supported_ops). The only reset primitive is
    // `revision.sync { reset: true }`, which is TREE-WIDE (and its `root_files`
    // arg pulls a dependency closure, so it can't safely scope to just the
    // selected files). Rather than silently pretend to revert edits, we:
    //   (a) UNSTAGE the selected files (the supported, file-scoped operation),
    //   (b) make the confirmation say exactly that — it does NOT revert edits,
    //   (c) point users at the real revert path ("Revert to Revision…").
    // This keeps the label honest; a true scoped per-file revert is filed as a
    // follow-up for when the engine routes `file.reset`.
    const confirm = await vscode.window.showWarningMessage(
      `Unstage ${paths.length} file(s)? (Your edits are NOT reverted.)`,
      {
        modal: true,
        detail:
          'This removes the file(s) from the staged set; the working-tree edits ' +
          'are left untouched. Lore has no per-file working-tree reset, so to ' +
          'roll a file all the way back to a committed revision use ' +
          '"Revert to Revision…" (resets the whole tree) or "Compare with ' +
          'Revision…" to copy lines back manually.',
      },
      'Unstage',
    );
    if (confirm !== 'Unstage') {
      return;
    }
    await guard(() => repo.client.run('file.unstage', { paths }));
    await repo.refresh();
  });

  reg('lore.commit', async (arg?: unknown) => {
    const repo = repoFromArg(arg) ?? (await pickRepository());
    if (!repo) {
      return;
    }
    // If a message is passed directly (programmatic/tested path), use it;
    // otherwise fall back to the SCM input box, then to an input-box prompt.
    const direct = typeof arg === 'string' ? arg : undefined;
    await commit(repo, direct);
  });

  reg('lore.openDiff', async (arg?: unknown) => {
    const uri = uriFromArg(arg);
    const repo = uri ? repoForUri(uri) : await pickRepository();
    if (!repo || !uri) {
      return;
    }
    await openDiff(repo, uri, '');
  });

  reg('lore.openFile', async (arg?: unknown) => {
    const uri = uriFromArg(arg);
    if (uri) {
      await vscode.window.showTextDocument(uri, { preview: false });
    }
  });

  reg('lore.diffWithRevision', async (arg?: unknown) => {
    const uri = uriFromArg(arg);
    const repo = uri ? repoForUri(uri) : await pickRepository();
    if (!repo || !uri) {
      return;
    }
    const rel = path.relative(repo.folder.uri.fsPath, uri.fsPath);
    const rev = await pickRevisionForFile(repo, rel);
    if (rev === undefined) {
      return;
    }
    await openDiff(repo, uri, rev);
  });

  reg('lore.fileHistory', async (arg?: unknown) => {
    const uri = uriFromArg(arg);
    const repo = uri ? repoForUri(uri) : await pickRepository();
    if (!repo || !uri) {
      return;
    }
    await showFileHistory(repo, uri);
  });

  reg('lore.sync', async (arg?: unknown) => {
    const repo = repoFromArg(arg) ?? (await pickRepository());
    if (!repo) {
      return;
    }
    const result = await guard(() =>
      repo.client.run<RevisionSyncResult>('revision.sync', {}),
    );
    if (result) {
      void vscode.window.showInformationMessage(
        `Lore: synced (${result.files_updated} updated, ${result.files_deleted} deleted).`,
      );
      await repo.refresh();
    }
  });

  reg('lore.push', async (arg?: unknown) => {
    const repo = repoFromArg(arg) ?? (await pickRepository());
    if (!repo) {
      return;
    }
    const result = await guard(() =>
      repo.client.run<BranchPushResult>('branch.push', {}),
    );
    if (result) {
      void vscode.window.showInformationMessage(
        result.already_pushed
          ? `Lore: ${result.branch_name} already up to date with remote.`
          : `Lore: pushed ${result.local_history} revision(s) on ${result.branch_name}.`,
      );
      await repo.refresh();
    }
  });

  reg('lore.revert', async (arg?: unknown) => {
    const repo = repoFromArg(arg) ?? (await pickRepository());
    if (!repo) {
      return;
    }
    const rev = await pickRevision(repo, 'Revert (sync + reset) working tree to which revision?');
    if (!rev) {
      return;
    }
    const confirm = await vscode.window.showWarningMessage(
      `Reset the working tree to ${rev.slice(0, 12)}? Local modifications to ` +
        'tracked files will be overwritten.',
      { modal: true },
      'Revert',
    );
    if (confirm !== 'Revert') {
      return;
    }
    // `revision.revert` is not in the dispatchable engine surface; the supported
    // primitive is `revision.sync` targeting the revision with reset:true.
    const result = await guard(() =>
      repo.client.run<RevisionSyncResult>('revision.sync', {
        revision: rev,
        reset: true,
      }),
    );
    if (result) {
      void vscode.window.showInformationMessage(
        `Lore: reverted to ${rev.slice(0, 12)} (${result.files_updated} updated, ${result.files_deleted} deleted).`,
      );
      await repo.refresh();
    }
  });

  reg('lore.branchList', async (arg?: unknown) => {
    const repo = repoFromArg(arg) ?? (await pickRepository());
    if (!repo) {
      return;
    }
    const res = await guard(() =>
      repo.client.run<BranchListResult>('branch.list', {}),
    );
    if (!res) {
      return;
    }
    const items = res.entries.map((b) => ({
      label: `${b.is_current ? '$(check) ' : ''}${b.name}`,
      description: `${b.location}${b.category ? ` · ${b.category}` : ''}`,
      detail: b.latest ? `latest ${b.latest.slice(0, 12)}` : undefined,
    }));
    await vscode.window.showQuickPick(items, {
      title: `Lore branches (${res.count})`,
    });
  });

  reg('lore.branchCreate', async (arg?: unknown) => {
    const repo = repoFromArg(arg) ?? (await pickRepository());
    if (!repo) {
      return;
    }
    const name = await vscode.window.showInputBox({
      prompt: 'New lore branch name',
      placeHolder: 'feature/my-arc',
    });
    if (!name) {
      return;
    }
    // lore stacks branches: branch.create AUTO-SWITCHES the working tree onto the
    // new branch. Confirm so the user isn't silently moved off their current
    // branch (this surprised people — surface it explicitly).
    const current = repo.branchName;
    const confirm = await vscode.window.showWarningMessage(
      `Create branch "${name}" and switch to it now?`,
      {
        modal: true,
        detail: current
          ? `Lore stacks branches: creating "${name}" immediately switches your ` +
            `working tree off "${current}" and onto the new branch.`
          : 'Lore stacks branches: creating this branch immediately switches your ' +
            'working tree onto it.',
      },
      'Create & Switch',
    );
    if (confirm !== 'Create & Switch') {
      return;
    }
    const res = await guard(() =>
      repo.client.run<BranchCreateResult>('branch.create', { branch: name }),
    );
    if (res) {
      void vscode.window.showInformationMessage(
        `Lore: created branch ${res.name} and switched to it` +
          (current ? ` (was on ${current}).` : '.'),
      );
      await repo.refresh();
    }
  });

  reg('lore.branchSwitch', async (arg?: unknown) => {
    const repo = repoFromArg(arg) ?? branchRepoFromArg(arg) ?? (await pickRepository());
    if (!repo) {
      return;
    }
    let branch = branchNameFromArg(arg);
    if (!branch) {
      const res = await guard(() =>
        repo.client.run<BranchListResult>('branch.list', {}),
      );
      if (!res) {
        return;
      }
      const pick = await vscode.window.showQuickPick(
        res.entries
          .filter((b) => !b.is_current)
          .map((b) => ({ label: b.name, description: b.location })),
        { title: 'Switch to which branch?' },
      );
      branch = pick?.label;
    }
    if (!branch) {
      return;
    }
    const res = await guard(() =>
      repo.client.run<BranchSwitchResult>('branch.switch', { branch }),
    );
    if (res) {
      void vscode.window.showInformationMessage(`Lore: switched to ${res.branch}.`);
      await repo.refresh();
    }
  });

  reg('lore.lockAcquire', async (...args: unknown[]) => {
    const { repo, paths } = resolveResourceTargets(args);
    if (!repo || paths.length === 0) {
      return;
    }
    const branch = repo.branchName;
    if (!branch) {
      void vscode.window.showWarningMessage('Lore: no current branch for lock context.');
      return;
    }
    const res = await guard(() =>
      repo.client.run<FileAcquireResult>('lock.file_acquire', { paths, branch }),
    );
    if (res) {
      void vscode.window.showInformationMessage(
        `Lore: acquired ${res.acquired.length} lock(s)${res.ignored.length ? `, ${res.ignored.length} already held` : ''}.`,
      );
      await repo.refresh();
    }
  });

  reg('lore.lockRelease', async (...args: unknown[]) => {
    const lockArg = lockItemFromArg(args[0]);
    const { repo, paths } = lockArg
      ? { repo: lockArg.repo, paths: [lockArg.path] }
      : resolveResourceTargets(args);
    if (!repo || paths.length === 0) {
      return;
    }
    const branch = repo.branchName;
    const owner = repo.myIdentity;
    if (!branch || !owner) {
      void vscode.window.showWarningMessage(
        'Lore: releasing a lock needs a branch context and a configured identity (lore.identity).',
      );
      return;
    }
    const res = await guard(() =>
      repo.client.run<FileReleaseResult>('lock.file_release', {
        paths,
        branch,
        owner,
        owner_id: owner,
      }),
    );
    if (res) {
      void vscode.window.showInformationMessage(
        `Lore: released ${res.released.length} lock(s).`,
      );
      await repo.refresh();
    }
  });

  reg('lore.requestLock', async (arg?: unknown) => {
    const uri = uriFromArg(arg);
    // TODO(SBAI-4044): request-lock → tray-message flow. Acquiring a lock that
    // another user owns requires a cross-network "request from owner" round
    // trip (lock.file_message_send to the owner + a tray/notification reply).
    // That depends on SBAI-4044 (cross-network lock messaging) and is NOT wired
    // here. For now we only inform the user.
    void vscode.window.showInformationMessage(
      'Lore: requesting a lock from its current owner is not available yet ' +
        '(pending SBAI-4044 cross-network lock messaging). You can still ' +
        'acquire an unheld lock via "Lore: Acquire Lock".' +
        (uri ? ` Target: ${path.basename(uri.fsPath)}` : ''),
    );
  });

  reg('lore.openRevisionDiff', async (arg?: unknown) => {
    const item = revisionItemFromArg(arg);
    if (!item) {
      return;
    }
    await openRevisionOverview(item.repo, item.revision);
  });

  reg('lore.openDocs', async () => {
    const url =
      vscode.workspace.getConfiguration('lore').get<string>('docs') || DOCS_DEFAULT;
    await vscode.env.openExternal(vscode.Uri.parse(url));
  });

  reg('lore.openSettings', async () => {
    await vscode.commands.executeCommand(
      'workbench.action.openSettings',
      'lore.lorevmPath',
    );
  });

  reg('lore.openWalkthrough', async () => {
    await vscode.commands.executeCommand(
      'workbench.action.openWalkthrough',
      'BiloxiStudios.loregui-lore#lore.gettingStarted',
      false,
    );
  });
}

/**
 * Check in the staged changes for `repo`.
 *
 * Message resolution (so the check-in path is testable without VS Code UI):
 *   1. an explicit `message` argument, if provided and non-empty;
 *   2. the SCM input box value;
 *   3. an input-box prompt (interactive flow).
 * An empty final message aborts. Returns the commit result (or undefined on
 * abort/failure) so callers/tests can assert on it.
 */
export async function commit(
  repo: LoreRepository,
  message?: string,
): Promise<CommitResult | undefined> {
  let msg = (message ?? '').trim();
  if (!msg) {
    msg = repo.scm.inputBox.value.trim();
  }
  if (!msg) {
    msg =
      (
        (await vscode.window.showInputBox({
          prompt: 'Lore commit message',
          placeHolder: 'Describe this revision',
        })) ?? ''
      ).trim();
  }
  if (!msg) {
    void vscode.window.showInformationMessage('Lore: commit aborted (empty message).');
    return undefined;
  }
  const result = await guard(() =>
    repo.client.run<CommitResult>('revision.commit', { message: msg }),
  );
  if (result) {
    repo.scm.inputBox.value = '';
    void vscode.window.showInformationMessage(
      `Lore: checked in r${result.revision_number} on ${result.branch}.`,
    );
    await repo.refresh();
  }
  return result;
}

// ---------------------------------------------------------------------------
// Diff rendering (side-by-side + virtual baseline blob)
// ---------------------------------------------------------------------------

/**
 * Open a real side-by-side VS Code diff: the baseline (committed revision, or a
 * chosen revision) on the left, the working file on the right. The left side is
 * served from a `lore-doc:` virtual document reconstructed from the engine's
 * unified patch.
 */
async function openDiff(
  repo: LoreRepository,
  uri: vscode.Uri,
  revision: string,
): Promise<void> {
  const rel = path.relative(repo.folder.uri.fsPath, uri.fsPath);
  // Binary files: never feed them to the line-diff machinery (it produces
  // mojibake). Render a graceful "binary file — N bytes" notice on BOTH sides
  // via the virtual doc provider so the editor shows the message, not garbage.
  if (repo.isBinaryWorking(rel)) {
    const bytes = repo.readWorkingRaw(rel)?.length ?? 0;
    const notice = binaryPlaceholder(rel, bytes);
    const left = buildBlobUri(repo.folder.uri.fsPath, rel, revision);
    const right = buildBlobUri(repo.folder.uri.fsPath, rel, `${revision}~working`);
    docProvider.set(left, notice);
    docProvider.set(right, notice);
    await vscode.commands.executeCommand(
      'vscode.diff',
      left,
      right,
      `${path.basename(rel)} (binary)`,
      { preview: true },
    );
    return;
  }
  const baseline = await guard(() => repo.baselineContent(rel, revision));
  if (baseline === undefined) {
    return;
  }
  const left = buildBlobUri(repo.folder.uri.fsPath, rel, revision);
  docProvider.set(left, baseline);
  const title = revision
    ? `${path.basename(rel)} (${revision.slice(0, 8)} ↔ working)`
    : `${path.basename(rel)} (Lore ↔ working)`;
  await vscode.commands.executeCommand('vscode.diff', left, uri, title, {
    preview: true,
  });
}

/** Show a revision's overview (metadata + per-file delta) as a read-only doc. */
async function openRevisionOverview(
  repo: LoreRepository,
  revision: string,
): Promise<void> {
  const info = await guard(() =>
    repo.client.run<RevisionInfoResult>('revision.info', {
      revision,
      delta: true,
      metadata: true,
    }),
  );
  if (!info) {
    return;
  }
  const meta = Object.fromEntries(info.metadata.map((m) => [m.key, m.value]));
  const lines: string[] = [];
  lines.push(`# Revision ${info.info?.revision_number ?? '?'} — ${revision.slice(0, 16)}`);
  lines.push('');
  if (meta['message']) {
    lines.push(meta['message'], '');
  }
  if (meta['committed-by'] || meta['created-by']) {
    lines.push(`Author: ${meta['committed-by'] || meta['created-by']}`);
  }
  if (meta['timestamp']) {
    lines.push(`Date: ${meta['timestamp']}`);
  }
  if (info.info?.parents.length) {
    lines.push(`Parents: ${info.info.parents.map((p) => p.slice(0, 12)).join(', ')}`);
  }
  lines.push('', '## Files', '');
  for (const d of info.deltas as { path: string; action: string }[]) {
    lines.push(`- ${actionLabel(d.action)}: ${d.path}`);
  }
  const docUri = vscode.Uri.from({
    scheme: LORE_DOC_SCHEME,
    path: `/revision/${revision}.md`,
    query: `repo=${encodeURIComponent(repo.folder.uri.fsPath)}`,
  });
  docProvider.set(docUri, lines.join('\n'));
  const doc = await vscode.workspace.openTextDocument(docUri);
  await vscode.languages.setTextDocumentLanguage(doc, 'markdown');
  await vscode.window.showTextDocument(doc, { preview: true });
}

/** Quick-pick of a file's revision history; returns a revision hash or undefined. */
async function pickRevisionForFile(
  repo: LoreRepository,
  rel: string,
): Promise<string | undefined> {
  const result = await guard(() =>
    repo.client.run<FileHistoryResult>('file.history', { path: rel, length: 50 }),
  );
  if (!result || result.entries.length === 0) {
    void vscode.window.showInformationMessage(`Lore: no history for ${rel}.`);
    return undefined;
  }
  const pick = await vscode.window.showQuickPick(
    result.entries.map((e) => ({
      label: `r${e.revision_number} · ${actionLabel(e.action)}`,
      description: e.revision.slice(0, 12),
      detail: `${e.size} bytes`,
      revision: e.revision,
    })),
    { title: `Compare ${rel} with revision`, placeHolder: 'Pick a revision' },
  );
  return pick?.revision;
}

/** Quick-pick over the branch revision log; returns a revision hash. */
async function pickRevision(
  repo: LoreRepository,
  title: string,
): Promise<string | undefined> {
  const hist = await guard(() =>
    repo.client.run<RevisionHistoryResult>('revision.history', { length: 50 }),
  );
  if (!hist || hist.entries.length === 0) {
    void vscode.window.showInformationMessage('Lore: no revision history.');
    return undefined;
  }
  const pick = await vscode.window.showQuickPick(
    hist.entries.map((e) => ({
      label: `r${e.revision_number}`,
      description: e.revision.slice(0, 16),
      revision: e.revision,
    })),
    { title },
  );
  return pick?.revision;
}

/** Quick-pick of a file's revision history. */
async function showFileHistory(repo: LoreRepository, uri: vscode.Uri): Promise<void> {
  const rel = path.relative(repo.folder.uri.fsPath, uri.fsPath);
  const result = await guard(() =>
    repo.client.run<FileHistoryResult>('file.history', { path: rel, length: 50 }),
  );
  if (!result) {
    return;
  }
  if (result.entries.length === 0) {
    void vscode.window.showInformationMessage(`Lore: no history for ${rel}.`);
    return;
  }
  const pick = await vscode.window.showQuickPick(
    result.entries.map((e) => ({
      label: `r${e.revision_number} · ${actionLabel(e.action)}`,
      description: e.revision.slice(0, 12),
      detail: `${e.size} bytes · open diff`,
      revision: e.revision,
    })),
    {
      title: `Lore history — ${rel}`,
      placeHolder: `${result.entries.length} revisions — pick one to diff against working`,
    },
  );
  if (pick) {
    await openDiff(repo, uri, pick.revision);
  }
}

// ---------------------------------------------------------------------------
// Activity-bar tree views: Branches / History / Locks
// ---------------------------------------------------------------------------

function refreshTreeViews(): void {
  branchesView?.refresh();
  historyView?.refresh();
  locksView?.refresh();
}

/** A repository that has a resolvable engine, for tree views to query. */
function activeReadyRepo(): LoreRepository | undefined {
  return repositories.find((r) => r.client.resolveBin());
}

class BranchesTreeProvider implements vscode.TreeDataProvider<BranchNode> {
  private readonly emitter = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.emitter.event;
  refresh(): void {
    this.emitter.fire();
  }
  getTreeItem(node: BranchNode): vscode.TreeItem {
    return node;
  }
  async getChildren(): Promise<BranchNode[]> {
    const repo = activeReadyRepo();
    if (!repo) {
      return [];
    }
    try {
      const res = await repo.client.run<BranchListResult>('branch.list', {});
      return res.entries.map((b) => new BranchNode(repo, b.name, b.is_current, b.location, b.category));
    } catch (err) {
      log(`branches view: ${describe(err)}`);
      return [];
    }
  }
}

class BranchNode extends vscode.TreeItem {
  constructor(
    readonly repo: LoreRepository,
    readonly branchName: string,
    current: boolean,
    location: string,
    category: string,
  ) {
    super(branchName, vscode.TreeItemCollapsibleState.None);
    this.contextValue = 'loreBranch';
    this.description = `${location}${category ? ` · ${category}` : ''}${current ? ' · current' : ''}`;
    this.iconPath = new vscode.ThemeIcon(current ? 'check' : 'git-branch');
    if (!current) {
      this.command = {
        command: 'lore.branchSwitch',
        title: 'Switch',
        arguments: [this],
      };
    }
  }
}

class HistoryTreeProvider implements vscode.TreeDataProvider<RevisionNode> {
  private readonly emitter = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.emitter.event;
  refresh(): void {
    this.emitter.fire();
  }
  getTreeItem(node: RevisionNode): vscode.TreeItem {
    return node;
  }
  async getChildren(): Promise<RevisionNode[]> {
    const repo = activeReadyRepo();
    if (!repo) {
      return [];
    }
    try {
      const hist = await repo.client.run<RevisionHistoryResult>('revision.history', {
        length: 50,
      });
      // Enrich the first ~25 with message/author from revision.info (best-effort).
      const out: RevisionNode[] = [];
      for (const e of hist.entries) {
        let message = '';
        let author = '';
        if (out.length < 25) {
          try {
            const info = await repo.client.run<RevisionInfoResult>('revision.info', {
              revision: e.revision,
              metadata: true,
            });
            const meta = Object.fromEntries(info.metadata.map((m) => [m.key, m.value]));
            message = meta['message'] || '';
            author = meta['committed-by'] || meta['created-by'] || '';
          } catch {
            /* best-effort */
          }
        }
        out.push(new RevisionNode(repo, e.revision, e.revision_number, message, author));
      }
      return out;
    } catch (err) {
      log(`history view: ${describe(err)}`);
      return [];
    }
  }
}

class RevisionNode extends vscode.TreeItem {
  constructor(
    readonly repo: LoreRepository,
    readonly revision: string,
    revisionNumber: number,
    message: string,
    author: string,
  ) {
    const firstLine = message.split('\n')[0];
    super(`r${revisionNumber}${firstLine ? ` · ${firstLine}` : ''}`, vscode.TreeItemCollapsibleState.None);
    this.contextValue = 'loreRevision';
    this.description = [author, revision.slice(0, 12)].filter(Boolean).join(' · ');
    this.tooltip = `${revision}\n${message}`;
    this.iconPath = new vscode.ThemeIcon('git-commit');
    this.command = {
      command: 'lore.openRevisionDiff',
      title: 'Open Revision Changes',
      arguments: [this],
    };
  }
}

class LocksTreeProvider implements vscode.TreeDataProvider<LockNode> {
  private readonly emitter = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.emitter.event;
  refresh(): void {
    this.emitter.fire();
  }
  getTreeItem(node: LockNode): vscode.TreeItem {
    return node;
  }
  async getChildren(): Promise<LockNode[]> {
    const repo = activeReadyRepo();
    const branch = repo?.branchName;
    if (!repo || !branch) {
      setLocksNoRemote(false);
      return [];
    }
    try {
      const res = await repo.client.run<FileQueryResult>('lock.file_query', {
        branch,
        owner: '',
        path: '',
      });
      // Locks resolved — clear any "no remote" welcome hint.
      setLocksNoRemote(false);
      return res.locks.map(
        (l) =>
          new LockNode(repo, l.path, l.owner, !!repo.myIdentity && l.owner === repo.myIdentity),
      );
    } catch (err) {
      const msg = describe(err);
      log(`locks view: ${msg}`);
      // Local/offline repos have no lock server: the engine returns
      // "No remote configured". Surface a clear in-UI hint (viewsWelcome gated
      // on the `lore.locksNoRemote` context key) instead of a silent empty view.
      setLocksNoRemote(/no remote configured/i.test(msg));
      return [];
    }
  }
}

/** Toggle the `lore.locksNoRemote` context key that gates the Locks viewsWelcome hint. */
function setLocksNoRemote(value: boolean): void {
  void vscode.commands.executeCommand('setContext', 'lore.locksNoRemote', value);
}

class LockNode extends vscode.TreeItem {
  constructor(
    readonly repo: LoreRepository,
    readonly lockPath: string,
    owner: string,
    mine: boolean,
  ) {
    super(lockPath, vscode.TreeItemCollapsibleState.None);
    this.contextValue = mine ? 'loreLockMine' : 'loreLockOther';
    this.description = mine ? 'locked by you' : `locked by ${owner}`;
    this.iconPath = new vscode.ThemeIcon(mine ? 'lock' : 'lock-small');
    this.resourceUri = vscode.Uri.file(path.join(repo.folder.uri.fsPath, lockPath));
  }
}

// ---------------------------------------------------------------------------
// Virtual document provider (baseline blobs / revision overviews)
// ---------------------------------------------------------------------------

class LoreDocumentProvider implements vscode.TextDocumentContentProvider {
  private readonly contents = new Map<string, string>();
  private readonly emitter = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this.emitter.event;

  set(uri: vscode.Uri, content: string): void {
    this.contents.set(uri.toString(), content);
    this.emitter.fire(uri);
  }

  provideTextDocumentContent(uri: vscode.Uri): string {
    return this.contents.get(uri.toString()) ?? '';
  }
}

/** Build a stable `lore-doc:` URI for a file's baseline blob at a revision. */
function buildBlobUri(repoDir: string, rel: string, revision: string): vscode.Uri {
  const query = `repo=${encodeURIComponent(repoDir)}&rev=${encodeURIComponent(revision)}`;
  return vscode.Uri.from({
    scheme: LORE_DOC_SCHEME,
    path: '/' + rel,
    query,
  });
}

// ---------------------------------------------------------------------------
// Binary-file detection (so .uasset/image diffs don't render as mojibake)
// ---------------------------------------------------------------------------

/**
 * Extensions that are always treated as binary regardless of content sniffing.
 * Covers Unreal asset/map containers and common image/media/archive formats —
 * the files a lore game-dev repo most often holds that would diff as garbage.
 */
const BINARY_EXTENSIONS = new Set<string>([
  // Unreal Engine
  '.uasset', '.umap', '.ubulk', '.uexp', '.uptnl', '.upk', '.udk',
  // Images
  '.png', '.jpg', '.jpeg', '.gif', '.bmp', '.tga', '.tiff', '.tif',
  '.psd', '.ico', '.webp', '.exr', '.hdr', '.dds',
  // Audio / video
  '.wav', '.mp3', '.ogg', '.flac', '.aiff', '.mp4', '.mov', '.avi', '.webm',
  // 3D / fonts / archives / misc binaries
  '.fbx', '.obj', '.blend', '.gltf', '.glb', '.ttf', '.otf', '.woff', '.woff2',
  '.zip', '.gz', '.tar', '.7z', '.rar', '.pdf', '.bin', '.dll', '.so', '.dylib',
  '.exe', '.wasm',
]);

/** True if `rel`'s extension is in the known-binary set. */
export function hasBinaryExtension(rel: string): boolean {
  return BINARY_EXTENSIONS.has(path.extname(rel).toLowerCase());
}

/**
 * Heuristic binary sniff: a NUL byte in the first 8 KiB is the same signal git
 * uses. Cheap, no full read interpretation, and robust for the asset/image
 * files a lore repo holds. (UTF-16 text would trip this too — acceptable, since
 * VS Code can't line-diff it usefully anyway.)
 */
export function isBinaryBuffer(buf: Buffer): boolean {
  const n = Math.min(buf.length, 8192);
  for (let i = 0; i < n; i++) {
    if (buf[i] === 0) {
      return true;
    }
  }
  return false;
}

/** Human-readable placeholder shown on both sides of a binary diff. */
export function binaryPlaceholder(rel: string, bytes: number): string {
  return (
    `Binary file — ${path.basename(rel)} (${bytes.toLocaleString()} bytes)\n` +
    '\n' +
    'Lore does not render a line-by-line diff for binary files.\n' +
    'Open the file with its native tool to inspect changes.\n'
  );
}

// ---------------------------------------------------------------------------
// Unified-patch reverse application (reconstruct the "before" text)
// ---------------------------------------------------------------------------

/**
 * Thrown when a unified patch cannot be cleanly reverse-applied to the working
 * text (line counts disagree, a context/added line doesn't match the working
 * file, a hunk header is malformed, …). Callers MUST surface this rather than
 * silently swallow it — a masked failure renders a real change as "no change".
 */
export class ReversePatchError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ReversePatchError';
  }
}

/**
 * Reverse-apply a unified diff to working text to recover the baseline.
 *
 * Given the working ("after") content and the patch that turns baseline into
 * working, produce the baseline ("before") content. This lets us render a real
 * left/right diff without a second engine round-trip for the blob.
 *
 * Hardened (was: a tolerant best-effort that silently returned the working text
 * on any mismatch — so a file with real changes could render as "no change"):
 *
 *  - Every context (` `) and added (`+`) hunk line is VALIDATED against the
 *    working file at the expected position; a mismatch throws ReversePatchError
 *    instead of producing corrupt or misleadingly-empty baseline text.
 *  - Hunk `+`/`-`/context line counts are checked against the `@@` header.
 *  - CRLF is preserved: line content (including any trailing `\r`) is compared
 *    and reconstructed verbatim; only the `\n` record separator is split on.
 *  - "\ No newline at end of file" markers are honoured so an added or removed
 *    final newline round-trips exactly.
 *  - add-only (baseline empty), delete-only (working empty) and multi-hunk
 *    patches are all handled.
 *
 * On any inconsistency it THROWS; the caller (baselineContent) logs loudly and
 * decides the fallback. It never silently masks a mismatch here.
 */
export function applyReversePatch(working: string, patch: string): string {
  // Model text as (lines[], finalNewline): "a\nb\n" -> (["a","b"], true);
  // "a\nb" -> (["a","b"], false); "" -> ([], false). This lets us reconstruct
  // a missing trailing newline exactly rather than guessing via join('\n').
  const split = (s: string): { lines: string[]; finalNewline: boolean } => {
    if (s === '') {
      return { lines: [], finalNewline: false };
    }
    const finalNewline = s.endsWith('\n');
    const body = finalNewline ? s.slice(0, -1) : s;
    return { lines: body.split('\n'), finalNewline };
  };

  const work = split(working);
  const patchLines = patch.split('\n');
  // A patch string that ends with "\n" yields a trailing "" element — drop it;
  // it is a record separator, not a hunk line.
  if (patchLines.length > 0 && patchLines[patchLines.length - 1] === '') {
    patchLines.pop();
  }

  const result: string[] = [];
  // Final-newline tracking for the reconstructed BASELINE.
  //   - baselineTailIsWorking: the baseline's last line was copied from the
  //     working tail (no hunk touched EOF) → it inherits working's newline.
  //   - lastBaselineNoNewline: the last baseline-side line a hunk emitted was
  //     immediately followed by a "\ No newline at end of file" marker.
  // A "\" marker applies to the line right before it; only a marker after a
  // baseline-side ('-' or context) line tells us about the baseline's newline.
  let baselineTailIsWorking = work.finalNewline;
  let lastBaselineNoNewline = false;
  let lastEmittedWasBaselineLine = false;
  let cursor = 0; // 0-based index into work.lines (the "after" side)
  let i = 0;

  const hunkRe = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;

  while (i < patchLines.length) {
    const header = patchLines[i];
    // Skip file headers (---/+++/diff/index) and any preamble before a hunk.
    const m = hunkRe.exec(header);
    if (!m) {
      i++;
      continue;
    }
    const afterCount = m[4] === undefined ? 1 : parseInt(m[4], 10);
    const beforeCount = m[2] === undefined ? 1 : parseInt(m[2], 10);
    // Unified-diff uses a 1-based start; for an empty after side ("+N,0") the
    // header line number is the line BEFORE the change, so clamp to 0-based.
    const rawAfter = parseInt(m[3], 10);
    const afterStart = afterCount === 0 ? rawAfter : rawAfter - 1;

    if (afterStart < 0 || afterStart > work.lines.length) {
      throw new ReversePatchError(
        `hunk @@ +${afterStart + 1} starts past end of working text ` +
          `(${work.lines.length} line(s))`,
      );
    }
    // Emit unchanged working lines preceding this hunk into the baseline.
    while (cursor < afterStart) {
      if (cursor >= work.lines.length) {
        throw new ReversePatchError(
          `hunk @@ +${afterStart + 1} starts past working text while ` +
            `copying leading context`,
        );
      }
      result.push(work.lines[cursor++]);
    }
    i++;

    let seenAfter = 0; // working-side lines consumed by this hunk (+/context)
    let seenBefore = 0; // baseline-side lines emitted by this hunk (-/context)
    // Process the hunk body until the next header or EOF.
    while (i < patchLines.length && !patchLines[i].startsWith('@@')) {
      const ln = patchLines[i];
      const kind = ln.length === 0 ? ' ' : ln[0];
      const content = ln.length === 0 ? '' : ln.slice(1);
      if (kind === '+') {
        // Added in working → present on the after side, absent from baseline.
        // Validate it matches the working file at the cursor, then skip it.
        if (cursor >= work.lines.length || work.lines[cursor] !== content) {
          throw new ReversePatchError(
            `'+' line does not match working text at line ${cursor + 1}: ` +
              `patch=${JSON.stringify(content)} ` +
              `working=${JSON.stringify(work.lines[cursor])}`,
          );
        }
        cursor++;
        seenAfter++;
        lastEmittedWasBaselineLine = false; // '+' lines are working-only
      } else if (kind === '-') {
        // Removed from working → present in baseline only.
        result.push(content);
        seenBefore++;
        baselineTailIsWorking = false;
        lastEmittedWasBaselineLine = true;
        lastBaselineNoNewline = false; // reset; a following "\" re-sets it
      } else if (kind === '\\') {
        // "\ No newline at end of file" — applies to the immediately preceding
        // line. Only meaningful for the baseline if that line was baseline-side
        // ('-' or context); a marker after a '+' line is about the working side.
        if (lastEmittedWasBaselineLine) {
          lastBaselineNoNewline = true;
        }
      } else if (kind === ' ') {
        // Context line — present on both sides. Must match the working file.
        if (cursor >= work.lines.length || work.lines[cursor] !== content) {
          throw new ReversePatchError(
            `context line does not match working text at line ${cursor + 1}: ` +
              `patch=${JSON.stringify(content)} ` +
              `working=${JSON.stringify(work.lines[cursor])}`,
          );
        }
        result.push(content);
        cursor++;
        seenAfter++;
        seenBefore++;
        baselineTailIsWorking = false;
        lastEmittedWasBaselineLine = true;
        lastBaselineNoNewline = false;
      } else {
        throw new ReversePatchError(
          `unrecognised hunk line prefix ${JSON.stringify(kind)}: ${ln}`,
        );
      }
      i++;
    }
    // Validate the hunk consumed/produced the counts its header promised.
    if (seenAfter !== afterCount) {
      throw new ReversePatchError(
        `hunk +count mismatch: header said ${afterCount}, saw ${seenAfter}`,
      );
    }
    if (seenBefore !== beforeCount) {
      throw new ReversePatchError(
        `hunk -count mismatch: header said ${beforeCount}, saw ${seenBefore}`,
      );
    }
  }
  // Trailing unchanged working lines after the last hunk. These come from the
  // working tail, so the baseline ends exactly as the working file does.
  if (cursor < work.lines.length) {
    while (cursor < work.lines.length) {
      result.push(work.lines[cursor++]);
    }
    baselineTailIsWorking = true;
  }

  if (result.length === 0) {
    // A genuinely empty baseline (e.g. delete-only / a brand-new file's add)
    // reverses to "". That is a valid result, NOT a failure to mask.
    return '';
  }
  // Final newline: if the baseline's last line came from the working tail it
  // ends like working; otherwise it ends with a newline unless the last
  // baseline-side line carried a "\ No newline at end of file" marker.
  const baselineFinalNewline = baselineTailIsWorking
    ? work.finalNewline
    : !lastBaselineNoNewline;
  return result.join('\n') + (baselineFinalNewline ? '\n' : '');
}

// ---------------------------------------------------------------------------
// Lock decoration provider (badges for locked-by-me / locked-by-other)
// ---------------------------------------------------------------------------

class LockDecorationProvider implements vscode.FileDecorationProvider {
  // repoDir -> (fsPath -> {lock, mine})
  private readonly byRepo = new Map<
    string,
    Map<string, { lock: LockStatus; mine: boolean }>
  >();
  private readonly emitter = new vscode.EventEmitter<vscode.Uri[]>();
  readonly onDidChangeFileDecorations = this.emitter.event;

  set(
    repoDir: string,
    entries: { uri: vscode.Uri; lock: LockStatus; mine: boolean }[],
  ): void {
    const prev = this.byRepo.get(repoDir);
    const next = new Map<string, { lock: LockStatus; mine: boolean }>();
    for (const e of entries) {
      next.set(e.uri.fsPath, { lock: e.lock, mine: e.mine });
    }
    this.byRepo.set(repoDir, next);

    // Fire change for the union of old + new paths so cleared locks re-render.
    const changed = new Set<string>();
    prev?.forEach((_v, k) => changed.add(k));
    next.forEach((_v, k) => changed.add(k));
    this.emitter.fire([...changed].map((p) => vscode.Uri.file(p)));
  }

  provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    for (const map of this.byRepo.values()) {
      const entry = map.get(uri.fsPath);
      if (entry) {
        return entry.mine
          ? {
              badge: 'L',
              tooltip: 'Locked by you',
              color: new vscode.ThemeColor('gitDecoration.stageModifiedResourceForeground'),
            }
          : {
              badge: 'L',
              tooltip: `Locked by ${entry.lock.owner}`,
              color: new vscode.ThemeColor('gitDecoration.deletedResourceForeground'),
            };
      }
    }
    return undefined;
  }
}

// ---------------------------------------------------------------------------
// Decorations / labels
// ---------------------------------------------------------------------------

function decorationFor(
  action: string,
  conflict: boolean,
): { icon: vscode.ThemeIcon } {
  if (conflict) {
    return { icon: new vscode.ThemeIcon('warning') };
  }
  switch (action) {
    case 'add':
      return { icon: new vscode.ThemeIcon('diff-added') };
    case 'delete':
      return { icon: new vscode.ThemeIcon('diff-removed') };
    case 'move':
    case 'copy':
      return { icon: new vscode.ThemeIcon('diff-renamed') };
    default:
      return { icon: new vscode.ThemeIcon('diff-modified') };
  }
}

function actionLabel(action: string): string {
  switch (action.toLowerCase()) {
    case 'add':
      return 'Added';
    case 'delete':
      return 'Deleted';
    case 'move':
      return 'Moved';
    case 'copy':
      return 'Copied';
    case 'keep':
    case 'modify':
      return 'Modified';
    default:
      return action.charAt(0).toUpperCase() + action.slice(1);
  }
}

// ---------------------------------------------------------------------------
// Argument resolution helpers (SCM passes resource states / groups / uris;
// tree views pass our node classes)
// ---------------------------------------------------------------------------

function repoFromArg(arg: unknown): LoreRepository | undefined {
  return arg instanceof LoreRepository ? arg : undefined;
}

function repoFromGroup(arg: unknown): LoreRepository | undefined {
  if (arg && typeof arg === 'object' && 'resourceStates' in arg) {
    const group = arg as vscode.SourceControlResourceGroup;
    const first = group.resourceStates[0];
    return first ? repoForUri(first.resourceUri) : undefined;
  }
  return undefined;
}

function branchRepoFromArg(arg: unknown): LoreRepository | undefined {
  return arg instanceof BranchNode ? arg.repo : undefined;
}

function branchNameFromArg(arg: unknown): string | undefined {
  return arg instanceof BranchNode ? arg.branchName : undefined;
}

function revisionItemFromArg(
  arg: unknown,
): { repo: LoreRepository; revision: string } | undefined {
  return arg instanceof RevisionNode ? { repo: arg.repo, revision: arg.revision } : undefined;
}

function lockItemFromArg(
  arg: unknown,
): { repo: LoreRepository; path: string } | undefined {
  return arg instanceof LockNode ? { repo: arg.repo, path: arg.lockPath } : undefined;
}

function uriFromArg(arg: unknown): vscode.Uri | undefined {
  if (arg instanceof vscode.Uri) {
    return arg;
  }
  if (arg instanceof LockNode) {
    return arg.resourceUri;
  }
  if (arg && typeof arg === 'object' && 'resourceUri' in arg) {
    const r = (arg as vscode.SourceControlResourceState).resourceUri;
    if (r) {
      return r;
    }
  }
  return vscode.window.activeTextEditor?.document.uri;
}

/** Resolve the repo + repo-relative paths from SCM command arguments. */
function resolveResourceTargets(args: unknown[]): {
  repo: LoreRepository | undefined;
  paths: string[];
} {
  const uris: vscode.Uri[] = [];

  for (const a of args) {
    if (a instanceof vscode.Uri) {
      uris.push(a);
    } else if (a instanceof LockNode) {
      uris.push(a.resourceUri!);
    } else if (a && typeof a === 'object' && 'resourceUri' in a) {
      const r = (a as vscode.SourceControlResourceState).resourceUri;
      if (r) {
        uris.push(r);
      }
    } else if (a && typeof a === 'object' && 'resourceStates' in a) {
      // A whole resource group was passed (stage/unstage all).
      const group = a as vscode.SourceControlResourceGroup;
      for (const s of group.resourceStates) {
        uris.push(s.resourceUri);
      }
    }
  }

  if (uris.length === 0 && vscode.window.activeTextEditor) {
    uris.push(vscode.window.activeTextEditor.document.uri);
  }

  const repo = uris.length > 0 ? repoForUri(uris[0]) : undefined;
  if (!repo) {
    return { repo: undefined, paths: [] };
  }
  const paths = uris
    .filter((u) => repo.owns(u))
    .map((u) => path.relative(repo.folder.uri.fsPath, u.fsPath));
  return { repo, paths };
}

function repoForUri(uri: vscode.Uri): LoreRepository | undefined {
  return repositories.find((r) => r.owns(uri));
}

async function pickRepository(): Promise<LoreRepository | undefined> {
  if (repositories.length === 0) {
    void vscode.window.showInformationMessage('Lore: no lore repository in this workspace.');
    return undefined;
  }
  if (repositories.length === 1) {
    return repositories[0];
  }
  const pick = await vscode.window.showQuickPick(
    repositories.map((r) => ({ label: r.folder.name, repo: r })),
    { title: 'Select a lore repository' },
  );
  return pick?.repo;
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

/** Run an op, surfacing LorevmError as a VS Code error message; undefined on failure. */
async function guard<T>(fn: () => Promise<T>): Promise<T | undefined> {
  try {
    return await fn();
  } catch (err) {
    if (err instanceof LorevmError && err.kind === 'config') {
      warnMissingBinary();
    } else {
      void vscode.window.showErrorMessage(`Lore: ${describe(err)}`);
    }
    log(`op failed: ${describe(err)}`);
    return undefined;
  }
}

function describe(err: unknown): string {
  if (err instanceof LorevmError) {
    return `${err.kind}: ${err.message}`;
  }
  if (err instanceof Error) {
    return err.message;
  }
  return String(err);
}

function log(msg: string): void {
  outputChannel?.appendLine(`[${new Date().toISOString()}] ${msg}`);
}

// Re-export for potential premium-layer reuse without re-resolving the binary.
// PREMIUM SEAM: the StudioBrain entity-aware addon imports resolveLorevmBin and
// LorevmClient from this module to drive the same engine with extra ops.
export { resolveLorevmBin };
