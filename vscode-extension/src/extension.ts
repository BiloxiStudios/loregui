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
    const watcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(this.folder, '**/*'),
    );
    const onChange = (uri: vscode.Uri) => {
      // Ignore churn inside the .lore metadata dir to avoid refresh storms.
      if (uri.fsPath.includes(`${path.sep}.lore${path.sep}`)) {
        return;
      }
      this.scheduleRefresh();
    };
    watcher.onDidCreate(onChange);
    watcher.onDidChange(onChange);
    watcher.onDidDelete(onChange);
    this.disposables.push(watcher);
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
    // Use file.diff with source = revision (or baseline) and target = working
    // to obtain the patch, then derive the "before" text from the working file.
    const working = this.readWorking(rel);
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
    return applyReversePatch(working, entry.patch);
  }

  readWorking(rel: string): string {
    try {
      return fs.readFileSync(path.join(this.folder.uri.fsPath, rel), 'utf8');
    } catch {
      return '';
    }
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
    const confirm = await vscode.window.showWarningMessage(
      `Discard staged changes to ${paths.length} file(s)?`,
      {
        modal: true,
        detail:
          'This unstages the file(s) via the engine. To roll a file all the ' +
          'way back to a committed revision, use "Compare with Revision…" or ' +
          '"Sync (Pull)" with reset.',
      },
      'Discard',
    );
    if (confirm !== 'Discard') {
      return;
    }
    // The dispatchable engine surface has no per-file working-tree reset
    // (`file.reset`/`revision.revert` are not routed by lorevm). Unstaging is
    // the supported discard primitive; tree-wide reset is Sync-with-reset.
    await guard(() => repo.client.run('file.unstage', { paths }));
    await repo.refresh();
  });

  reg('lore.commit', async (arg?: unknown) => {
    const repo = repoFromArg(arg) ?? (await pickRepository());
    if (!repo) {
      return;
    }
    let message = repo.scm.inputBox.value.trim();
    if (!message) {
      message =
        (await vscode.window.showInputBox({
          prompt: 'Lore commit message',
          placeHolder: 'Describe this revision',
        })) ?? '';
      message = message.trim();
    }
    if (!message) {
      void vscode.window.showInformationMessage('Lore: commit aborted (empty message).');
      return;
    }
    const result = await guard(() =>
      repo.client.run<CommitResult>('revision.commit', { message }),
    );
    if (result) {
      repo.scm.inputBox.value = '';
      void vscode.window.showInformationMessage(
        `Lore: checked in r${result.revision_number} on ${result.branch}.`,
      );
      await repo.refresh();
    }
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
    const res = await guard(() =>
      repo.client.run<BranchCreateResult>('branch.create', { branch: name }),
    );
    if (res) {
      void vscode.window.showInformationMessage(`Lore: created branch ${res.name}.`);
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
      return [];
    }
    try {
      const res = await repo.client.run<FileQueryResult>('lock.file_query', {
        branch,
        owner: '',
        path: '',
      });
      return res.locks.map(
        (l) =>
          new LockNode(repo, l.path, l.owner, !!repo.myIdentity && l.owner === repo.myIdentity),
      );
    } catch (err) {
      log(`locks view: ${describe(err)}`);
      return [];
    }
  }
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
// Unified-patch reverse application (reconstruct the "before" text)
// ---------------------------------------------------------------------------

/**
 * Reverse-apply a unified diff to working text to recover the baseline.
 * Given the working ("after") content and the patch that turns baseline into
 * working, produce the baseline ("before") content. This lets us render a real
 * left/right diff without a second engine round-trip for the blob.
 *
 * Tolerant: if hunks don't line up cleanly it falls back to returning the
 * working text (so the diff shows "no change" rather than corrupt content).
 */
function applyReversePatch(working: string, patch: string): string {
  const lines = patch.split('\n');
  const beforeAll = working.split('\n');
  const result: string[] = [];
  let cursor = 0; // index into beforeAll (the working/after lines)
  let i = 0;

  const hunkRe = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;

  while (i < lines.length) {
    const m = hunkRe.exec(lines[i]);
    if (!m) {
      i++;
      continue;
    }
    const afterStart = parseInt(m[3], 10) - 1; // 0-based start in working
    // Copy unchanged working lines up to this hunk.
    while (cursor < afterStart && cursor < beforeAll.length) {
      result.push(beforeAll[cursor++]);
    }
    i++;
    // Process hunk body.
    while (i < lines.length && !lines[i].startsWith('@@')) {
      const ln = lines[i];
      if (ln.startsWith('+')) {
        // Added in working → drop from baseline; advance working cursor.
        cursor++;
      } else if (ln.startsWith('-')) {
        // Removed from working → present in baseline.
        result.push(ln.slice(1));
      } else if (ln.startsWith('\\')) {
        // "No newline at end of file" marker — ignore.
      } else {
        // Context line — present in both; emit and advance.
        result.push(ln.startsWith(' ') ? ln.slice(1) : ln);
        cursor++;
      }
      i++;
    }
  }
  // Trailing unchanged working lines.
  while (cursor < beforeAll.length) {
    result.push(beforeAll[cursor++]);
  }
  // Sanity: if we produced nothing, fall back to working.
  if (result.length === 0 && working.length > 0) {
    return working;
  }
  return result.join('\n');
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
