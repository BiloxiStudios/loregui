// statusBarDecorations.test.ts — UI-LEVEL coverage of the status-bar indicator
// and the lock file-decoration badge (the "L").
//
// scm.test.ts asserts the engine's branch/revision JSON but never reads the
// StatusBarItem text the user actually sees, nor the FileDecorationProvider badge.
// Here we read the LIVE status-bar item (via the repo handle) and the LIVE
// FileDecorationProvider (via the extension API) and assert their rendered output.

import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import {
  activateExtension,
  getExtensionApi,
  getRepo,
  workspaceRoot,
  runOp,
  waitFor,
  delay,
  LoreExtensionApi,
  LoreRepository,
} from './helpers';

interface RepoStatus {
  revision: { branch_name: string; revision_number: number } | null;
  files: { path: string }[];
}
function status(): RepoStatus {
  return runOp('repository.status', { scan: true }) as RepoStatus;
}

/** Minimal structural view of the concrete LockDecorationProvider for the test. */
interface LockDecorationSetter {
  set(
    repoDir: string,
    entries: { uri: vscode.Uri; lock: { owner: string }; mine: boolean }[],
  ): void;
}

suite('Lore — status bar + file decorations (UI state)', function () {
  this.timeout(60_000);

  let api: LoreExtensionApi;
  let repo: LoreRepository;

  suiteSetup(async () => {
    await activateExtension();
    api = await getExtensionApi();
    repo = await getRepo();
    await api.refreshAll();
    await delay(500);
  });

  // -----------------------------------------------------------------------
  // Status bar text: "$(git-branch) <branch> @ r<n> [ $(pencil)]".
  // -----------------------------------------------------------------------
  test('status-bar item shows the branch and revision number the user sees', async () => {
    // Ensure the item has been populated by a real refresh.
    await waitFor(() => repo.statusBarItem.text.includes('@ r'), 8000);
    const text = repo.statusBarItem.text;
    const s = status();
    assert.ok(s.revision, 'engine reports a revision context');

    assert.ok(
      text.includes(`$(git-branch) ${s.revision!.branch_name}`),
      `status bar must render the branch with a git-branch icon; got "${text}"`,
    );
    assert.ok(
      text.includes(`@ r${s.revision!.revision_number}`),
      `status bar must render "@ r<revision_number>"; got "${text}" vs r${
        s.revision!.revision_number
      }`,
    );
  });

  test('status-bar tooltip + sync command back the indicator (clickable Sync affordance)', () => {
    assert.ok(
      typeof repo.statusBarItem.tooltip === 'string' &&
        (repo.statusBarItem.tooltip as string).includes(
          repo.branchName ?? '',
        ),
      'tooltip names the current branch',
    );
    const cmd = repo.statusBarItem.command;
    const cmdId = typeof cmd === 'string' ? cmd : cmd?.command;
    assert.strictEqual(cmdId, 'lore.sync', 'clicking the status bar runs lore.sync');
  });

  test('status-bar shows the dirty pencil when the working tree has changes, and drops it when clean is reported', async () => {
    // The seeded workspace has working-tree changes → pencil present.
    const dirtyText = await waitFor(
      () => (repo.statusBarItem.text.includes('$(pencil)') ? repo.statusBarItem.text : undefined),
      8000,
    );
    const s = status();
    if (s.files.length > 0) {
      assert.ok(
        dirtyText,
        `with ${s.files.length} pending change(s) the status bar must show the ` +
          `dirty $(pencil) marker; got "${repo.statusBarItem.text}"`,
      );
    } else {
      assert.ok(
        !repo.statusBarItem.text.includes('$(pencil)'),
        'a clean tree must not show the dirty pencil',
      );
    }
  });

  // -----------------------------------------------------------------------
  // File decorations: the "L" lock badge.
  // -----------------------------------------------------------------------
  test('no lock badge on a file when there are no locks (local repo, real state)', () => {
    const uri = vscode.Uri.file(path.join(workspaceRoot(), 'tracked.txt'));
    const deco = api.lockDecorations.provideFileDecoration(
      uri,
      new vscode.CancellationTokenSource().token,
    );
    // provideFileDecoration may return a Thenable; our impl is synchronous.
    assert.ok(!(deco instanceof Promise), 'decoration is computed synchronously');
    assert.strictEqual(
      deco,
      undefined,
      'an unlocked file must have no decoration on a no-remote/local repo',
    );
  });

  test('FileDecorationProvider renders the "L" badge for a locked-by-me file (mine = staged color)', () => {
    const lockedPath = path.join(workspaceRoot(), 'tracked.txt');
    const uri = vscode.Uri.file(lockedPath);
    const setter = api.lockDecorations as unknown as LockDecorationSetter;

    // Feed the LIVE provider a lock the same way the extension's refresh path does
    // (repoDir + entries), then read back the badge it renders.
    setter.set(repo.folder.uri.fsPath, [{ uri, lock: { owner: 'me' }, mine: true }]);
    try {
      const deco = api.lockDecorations.provideFileDecoration(
        uri,
        new vscode.CancellationTokenSource().token,
      ) as vscode.FileDecoration | undefined;
      assert.ok(deco, 'a locked file must get a decoration');
      assert.strictEqual(deco!.badge, 'L', 'the lock badge is the letter L');
      assert.ok(
        typeof deco!.tooltip === 'string' && /locked by you/i.test(deco!.tooltip as string),
        `locked-by-me tooltip should say "Locked by you"; got ${deco!.tooltip}`,
      );
      assert.ok(deco!.color instanceof vscode.ThemeColor, 'badge uses a theme color');
    } finally {
      setter.set(repo.folder.uri.fsPath, []); // clear
    }
  });

  test('FileDecorationProvider renders the "L" badge with the owner name for a locked-by-other file', () => {
    const lockedPath = path.join(workspaceRoot(), 'tracked.txt');
    const uri = vscode.Uri.file(lockedPath);
    const setter = api.lockDecorations as unknown as LockDecorationSetter;

    setter.set(repo.folder.uri.fsPath, [{ uri, lock: { owner: 'alice' }, mine: false }]);
    try {
      const deco = api.lockDecorations.provideFileDecoration(
        uri,
        new vscode.CancellationTokenSource().token,
      ) as vscode.FileDecoration | undefined;
      assert.ok(deco, 'a file locked by another user must get a decoration');
      assert.strictEqual(deco!.badge, 'L', 'badge is still the letter L');
      assert.ok(
        typeof deco!.tooltip === 'string' && /alice/.test(deco!.tooltip as string),
        `locked-by-other tooltip must name the owner; got ${deco!.tooltip}`,
      );
    } finally {
      setter.set(repo.folder.uri.fsPath, []);
    }
  });

  test('clearing the lock removes the badge (decoration goes back to undefined)', () => {
    const uri = vscode.Uri.file(path.join(workspaceRoot(), 'tracked.txt'));
    const setter = api.lockDecorations as unknown as LockDecorationSetter;
    setter.set(repo.folder.uri.fsPath, [{ uri, lock: { owner: 'me' }, mine: true }]);
    setter.set(repo.folder.uri.fsPath, []); // unlock
    const deco = api.lockDecorations.provideFileDecoration(
      uri,
      new vscode.CancellationTokenSource().token,
    );
    assert.strictEqual(deco, undefined, 'a released lock must clear the L badge');
  });

  suiteTeardown(() => {
    // Belt-and-suspenders: leave no synthetic decorations behind for other suites.
    const setter = api?.lockDecorations as unknown as LockDecorationSetter | undefined;
    if (setter && repo) {
      try {
        // Touch a throwaway file path to avoid affecting real entries.
        const tmp = path.join(workspaceRoot(), '.lore-deco-clear');
        fs.writeFileSync(tmp, '');
        setter.set(repo.folder.uri.fsPath, []);
        fs.rmSync(tmp, { force: true });
      } catch {
        /* best-effort */
      }
    }
  });
});
