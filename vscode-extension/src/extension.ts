// extension.ts — Lore source control for VS Code.
//
// Registers a native SCM provider (`vscode.scm.createSourceControl('lore', ...)`)
// per workspace folder that contains a lore repo, populates "Staged Changes" and
// "Changes" resource groups from `repository.status`, and wires the SCM
// commands (refresh / stage / unstage / commit / diff / history / sync) plus
// lock-awareness decorations onto OUR lorevm engine.
//
// Open-core seam: this is the FREE lore-SCM layer (MIT). The StudioBrain
// entity-aware premium layer (template-driven validation, cross-ref decorations,
// asset previews) is a later gated addon — see PREMIUM SEAM markers below. It
// would register additional decoration providers / resource group metadata
// against the same LorevmClient without forking this provider.

import * as vscode from 'vscode';
import * as path from 'path';
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
  resolveLorevmBin,
} from './lorevmClient';

const LORE_SCHEME = 'lore';
// Virtual-document scheme used to render diff patches / file contents.
const LORE_DOC_SCHEME = 'lore-doc';

let repositories: LoreRepository[] = [];
let outputChannel: vscode.OutputChannel;
let missingBinaryWarned = false;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel('Lore');
  context.subscriptions.push(outputChannel);

  // Virtual document provider for diffs / historical contents.
  const docProvider = new LoreDocumentProvider();
  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(LORE_DOC_SCHEME, docProvider),
  );

  // File decoration provider for lock badges (locked-by-me / locked-by-other).
  const lockDecorations = new LockDecorationProvider();
  context.subscriptions.push(
    vscode.window.registerFileDecorationProvider(lockDecorations),
  );

  // Register commands once; they dispatch to the active/selected repository.
  registerCommands(context, docProvider);

  // Discover lore repos in the open workspace folders.
  await discoverRepositories(context, lockDecorations);

  // Re-discover when workspace folders change.
  context.subscriptions.push(
    vscode.workspace.onDidChangeWorkspaceFolders(async () => {
      disposeRepositories();
      await discoverRepositories(context, lockDecorations);
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
}

// ---------------------------------------------------------------------------
// Repository discovery + activation gating
// ---------------------------------------------------------------------------

async function discoverRepositories(
  context: vscode.ExtensionContext,
  lockDecorations: LockDecorationProvider,
): Promise<void> {
  const folders = vscode.workspace.workspaceFolders ?? [];
  for (const folder of folders) {
    const client = makeClient(folder.uri.fsPath, context.extensionPath);

    // Activation gate: a folder is a lore repo iff `repository.status` succeeds.
    // (lorevm errors with a non-repo kind otherwise.) This doubles as the
    // graceful-messaging path when lorevm itself isn't found.
    let status: RepositoryStatusResult | undefined;
    try {
      status = await client.run<RepositoryStatusResult>('repository.status', {
        scan: true,
      });
    } catch (err) {
      if (err instanceof LorevmError && err.kind === 'config') {
        warnMissingBinary();
      } else {
        // Not a lore repo (or some other op error) — skip this folder silently.
        log(`folder ${folder.uri.fsPath} is not a lore repo: ${describe(err)}`);
      }
      continue;
    }

    const repo = new LoreRepository(context, folder, client, lockDecorations);
    repo.applyStatus(status);
    repositories.push(repo);
    log(`activated lore SCM for ${folder.uri.fsPath}`);
  }
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
    )
    .then((choice) => {
      if (choice === 'Open Settings') {
        void vscode.commands.executeCommand(
          'workbench.action.openSettings',
          'lore.lorevmPath',
        );
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

  private readonly disposables: vscode.Disposable[] = [];
  private readonly lockDecorations: LockDecorationProvider;
  private refreshTimer: NodeJS.Timeout | undefined;
  /** Current identity (for locked-by-me vs locked-by-other). */
  private identity: string | undefined;

  constructor(
    _context: vscode.ExtensionContext,
    folder: vscode.WorkspaceFolder,
    client: LorevmClient,
    lockDecorations: LockDecorationProvider,
  ) {
    this.folder = folder;
    this.client = client;
    this.lockDecorations = lockDecorations;
    this.identity = vscode.workspace.getConfiguration('lore').get<string>('identity') || undefined;

    this.scm = vscode.scm.createSourceControl(LORE_SCHEME, 'Lore', folder.uri);
    this.scm.quickDiffProvider = {
      provideOriginalResource: (uri) => this.originalResource(uri),
    };
    this.scm.inputBox.placeholder = 'Message (commit with the checkmark)';
    this.scm.acceptInputCommand = {
      command: 'lore.commit',
      title: 'Commit',
      arguments: [this],
    };

    this.stagedGroup = this.scm.createResourceGroup('staged', 'Staged Changes');
    this.changesGroup = this.scm.createResourceGroup('changes', 'Changes');
    this.stagedGroup.hideWhenEmpty = true;
    this.changesGroup.hideWhenEmpty = true;

    this.disposables.push(this.scm, this.stagedGroup, this.changesGroup);

    this.setupWatcher();
  }

  /** True if `repo` is this repository (used by command dispatch). */
  owns(uri: vscode.Uri): boolean {
    return uri.fsPath.startsWith(this.folder.uri.fsPath);
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
    const onChange = () => this.scheduleRefresh();
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
    try {
      const status = await this.client.run<RepositoryStatusResult>(
        'repository.status',
        { scan: true },
      );
      // Fold in lock status (best-effort; doesn't block the status refresh).
      const locks = await this.queryLocks(status);
      this.applyStatus(status, locks);
    } catch (err) {
      if (err instanceof LorevmError && err.kind === 'config') {
        warnMissingBinary();
      }
      log(`refresh failed for ${this.folder.uri.fsPath}: ${describe(err)}`);
    }
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
    this.lockDecorations.set(this.folder.uri.fsPath, lockEntries);

    // Surface branch/revision in the SCM title (like Git's branch indicator).
    const rev = status.revision;
    if (rev) {
      this.scm.statusBarCommands = [
        {
          command: 'lore.sync',
          title: `$(git-branch) ${rev.branch_name} ($(sync))`,
          tooltip: `Lore branch ${rev.branch_name} @ r${rev.revision_number} — sync`,
          arguments: [this],
        },
      ];
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

  /** quickDiff original: the file at its current revision (pre-edit baseline). */
  private originalResource(uri: vscode.Uri): vscode.Uri | undefined {
    const rel = path.relative(this.folder.uri.fsPath, uri.fsPath);
    if (rel.startsWith('..')) {
      return undefined;
    }
    return buildDocUri(this.folder.uri.fsPath, rel, '', 'original');
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

function registerCommands(
  context: vscode.ExtensionContext,
  docProvider: LoreDocumentProvider,
): void {
  const sub = context.subscriptions;

  sub.push(
    vscode.commands.registerCommand('lore.refresh', async (arg?: unknown) => {
      const repo = repoFromArg(arg);
      if (repo) {
        await repo.refresh();
      } else {
        await Promise.all(repositories.map((r) => r.refresh()));
      }
    }),
  );

  sub.push(
    vscode.commands.registerCommand('lore.stage', async (...args: unknown[]) => {
      const { repo, paths } = resolveResourceTargets(args);
      if (!repo || paths.length === 0) {
        return;
      }
      await guard(() => repo.client.run('file.stage', { paths, scan: true }));
      await repo.refresh();
    }),
  );

  sub.push(
    vscode.commands.registerCommand('lore.unstage', async (...args: unknown[]) => {
      const { repo, paths } = resolveResourceTargets(args);
      if (!repo || paths.length === 0) {
        return;
      }
      await guard(() => repo.client.run('file.unstage', { paths }));
      await repo.refresh();
    }),
  );

  sub.push(
    vscode.commands.registerCommand('lore.commit', async (arg?: unknown) => {
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
          `Lore: committed r${result.revision_number} on ${result.branch}.`,
        );
        await repo.refresh();
      }
    }),
  );

  sub.push(
    vscode.commands.registerCommand('lore.openDiff', async (arg?: unknown) => {
      const uri = uriFromArg(arg);
      const repo = uri ? repoForUri(uri) : await pickRepository();
      if (!repo || !uri) {
        return;
      }
      await openDiff(repo, uri, docProvider);
    }),
  );

  sub.push(
    vscode.commands.registerCommand('lore.fileHistory', async (arg?: unknown) => {
      const uri = uriFromArg(arg);
      const repo = uri ? repoForUri(uri) : await pickRepository();
      if (!repo || !uri) {
        return;
      }
      await showFileHistory(repo, uri);
    }),
  );

  sub.push(
    vscode.commands.registerCommand('lore.sync', async (arg?: unknown) => {
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
    }),
  );

  sub.push(
    vscode.commands.registerCommand('lore.requestLock', async (arg?: unknown) => {
      const uri = uriFromArg(arg);
      // TODO(SBAI-4044): request-lock → tray-message flow. Acquiring a lock that
      // another user owns requires a cross-network "request from owner" round
      // trip (lock.file_message_send to the owner + a tray/notification reply).
      // That depends on SBAI-4044 (cross-network lock messaging) and is NOT
      // wired here. For now we only inform the user.
      void vscode.window.showInformationMessage(
        'Lore: requesting a lock from its current owner is not available yet ' +
          '(pending SBAI-4044 cross-network lock messaging). You can still ' +
          'acquire an unheld lock via the lorevm `lock.file_acquire` op.' +
          (uri ? ` Target: ${path.basename(uri.fsPath)}` : ''),
      );
    }),
  );
}

/** Open a VS Code diff: current-revision baseline ⟷ working tree. */
async function openDiff(
  repo: LoreRepository,
  uri: vscode.Uri,
  docProvider: LoreDocumentProvider,
): Promise<void> {
  const rel = path.relative(repo.folder.uri.fsPath, uri.fsPath);
  const diff = await guard(() =>
    repo.client.run<FileDiffEntry[]>('file.diff', { paths: [rel] }),
  );
  if (!diff) {
    return;
  }
  const entry = diff.find((d) => d.path === rel) ?? diff[0];
  if (!entry || !entry.patch) {
    void vscode.window.showInformationMessage(`Lore: no changes for ${rel}.`);
    return;
  }
  // Render the unified patch in a read-only virtual document. (A full
  // left/right text diff would require both revision blobs; the patch is the
  // engine's native diff output and is the most faithful MVP rendering.)
  const docUri = buildDocUri(repo.folder.uri.fsPath, rel, entry.patch, 'patch');
  docProvider.set(docUri, entry.patch);
  const doc = await vscode.workspace.openTextDocument(docUri);
  await vscode.languages.setTextDocumentLanguage(doc, 'diff');
  await vscode.window.showTextDocument(doc, { preview: true });
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
  const items: vscode.QuickPickItem[] = result.entries.map((e) => ({
    label: `r${e.revision_number} · ${actionLabel(e.action)}`,
    description: e.revision.slice(0, 12),
    detail: `${e.size} bytes · address ${e.address.slice(0, 12)}`,
  }));
  await vscode.window.showQuickPick(items, {
    title: `Lore history — ${rel}`,
    placeHolder: `${result.entries.length} revisions`,
  });
}

// ---------------------------------------------------------------------------
// Virtual document provider (diffs / historical blobs)
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

/** Build a stable `lore-doc:` URI keyed by repo + path + kind. */
function buildDocUri(
  repoDir: string,
  rel: string,
  _payload: string,
  kind: string,
): vscode.Uri {
  const query = `repo=${encodeURIComponent(repoDir)}&kind=${kind}`;
  return vscode.Uri.from({
    scheme: LORE_DOC_SCHEME,
    path: '/' + rel,
    query,
  });
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
  switch (action) {
    case 'add':
      return 'Added';
    case 'delete':
      return 'Deleted';
    case 'move':
      return 'Moved';
    case 'copy':
      return 'Copied';
    case 'keep':
      return 'Modified';
    default:
      return action.charAt(0).toUpperCase() + action.slice(1);
  }
}

// ---------------------------------------------------------------------------
// Argument resolution helpers (SCM passes resource states / groups / uris)
// ---------------------------------------------------------------------------

function repoFromArg(arg: unknown): LoreRepository | undefined {
  if (arg instanceof LoreRepository) {
    return arg;
  }
  return undefined;
}

function uriFromArg(arg: unknown): vscode.Uri | undefined {
  if (arg instanceof vscode.Uri) {
    return arg;
  }
  if (arg && typeof arg === 'object' && 'resourceUri' in arg) {
    const r = (arg as vscode.SourceControlResourceState).resourceUri;
    if (r) {
      return r;
    }
  }
  // Fall back to the active editor.
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

/** Run an op, surfacing LorevmError as a VS Code error message; returns undefined on failure. */
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
