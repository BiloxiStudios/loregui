// scmGroups.test.ts — UI-LEVEL coverage of the SCM resource groups.
//
// The existing scm.test.ts asserts the ENGINE's status JSON (via an out-of-band
// `lorevm repository.status` CLI call) after each operation. That would still
// pass even if the Source Control panel the user sees were EMPTY — nothing reads
// the actual `SourceControl.createResourceGroup` state. These tests close that
// gap: they get a handle to the LIVE `LoreRepository.scm` (via the extension's
// exported API) and assert on `stagedGroup.resourceStates`,
// `changesGroup.resourceStates`, `scm.count`, and the per-row resource decoration
// — i.e. the rows VS Code renders under "Staged Changes" / "Changes".
//
// We cover the groups two complementary ways:
//
//  (a) LIVE end-to-end: create a real working-tree file and drive lore.refresh,
//      asserting it lands in the Changes group (this only needs a single
//      repository.status scan, which is reliable).
//
//  (b) DETERMINISTIC group mapping: call the extension's own applyStatus() — the
//      exact function that turns an engine status into the staged/changes groups
//      the user sees — with controlled status inputs, and assert the staged ↔
//      changes partitioning, scm.count, delete-strikethrough and lock tooltips.
//      This pins the real UI-building logic for stage/unstage/discard WITHOUT
//      depending on the engine's cross-process staged-state flush (a known
//      intermittent race — see BUGS.md #5 / SBAI-4080 — that makes a live
//      "appears in the staged group" assertion unreliable on some engine builds;
//      scm.test.ts deliberately avoids asserting it for the same reason).

import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import {
  activateExtension,
  getRepo,
  groupPaths,
  workspaceRoot,
  waitFor,
  delay,
  LoreRepository,
} from './helpers';
import type {
  RepositoryStatusResult,
  StatusFile,
  StatusFileAction,
  LockStatus,
} from '../../lorevmClient';

let seq = 0;
function freshName(tag: string): string {
  return `scmgroups-${tag}-${Date.now()}-${seq++}.txt`;
}

/** Build a minimal-but-complete StatusFile for applyStatus() inputs. */
function file(
  p: string,
  staged: boolean,
  action: StatusFileAction = 'add',
  extra: Partial<StatusFile> = {},
): StatusFile {
  return {
    path: p,
    size: 1,
    action,
    node_type: 'file',
    staged,
    conflict: false,
    dirty: !staged,
    from_path: '',
    ...extra,
  };
}

/** A status with the given files on the main branch @ r1. */
function statusWith(files: StatusFile[]): RepositoryStatusResult {
  return {
    revision: {
      repository: 'repo',
      branch: 'branchid',
      branch_name: 'main',
      revision: 'abc123',
      revision_number: 1,
      revision_staged: '',
    },
    files,
    count: null,
  };
}

/** Relative paths in a group (basename match-friendly). */
function names(group: vscode.SourceControlResourceGroup): string[] {
  return group.resourceStates.map((s) => path.basename(s.resourceUri.fsPath));
}

async function waitForGroup(
  repo: LoreRepository,
  which: 'staged' | 'changes',
  pred: (paths: string[]) => boolean,
  timeout = 12_000,
): Promise<string[] | undefined> {
  return waitFor(() => {
    const group = which === 'staged' ? repo.stagedGroup : repo.changesGroup;
    const paths = groupPaths(group, repo.folder.uri.fsPath);
    return pred(paths) ? paths : undefined;
  }, timeout);
}

suite('Lore SCM — resource group UI state', function () {
  this.timeout(60_000);

  let repo: LoreRepository;

  suiteSetup(async () => {
    await activateExtension();
    repo = await getRepo();
    await vscode.commands.executeCommand('lore.refresh');
    await delay(500);
  });

  // -----------------------------------------------------------------------
  // Group identity + hideWhenEmpty contract.
  // -----------------------------------------------------------------------
  test('the SourceControl has Staged Changes + Changes resource groups with the right ids/labels', () => {
    assert.strictEqual(repo.scm.label, 'Lore', 'SCM provider labelled "Lore"');
    assert.strictEqual(repo.stagedGroup.id, 'staged', 'staged group id');
    assert.strictEqual(repo.stagedGroup.label, 'Staged Changes', 'staged group label');
    assert.strictEqual(repo.changesGroup.id, 'changes', 'changes group id');
    assert.strictEqual(repo.changesGroup.label, 'Changes', 'changes group label');
  });

  test('both resource groups are configured hideWhenEmpty', () => {
    assert.strictEqual(repo.stagedGroup.hideWhenEmpty, true, 'staged group hideWhenEmpty');
    assert.strictEqual(repo.changesGroup.hideWhenEmpty, true, 'changes group hideWhenEmpty');
  });

  // -----------------------------------------------------------------------
  // (a) LIVE: a new working-tree file surfaces in the Changes group.
  // -----------------------------------------------------------------------
  test('a new working-tree file appears in the Changes group (the SCM panel is populated)', async () => {
    const name = freshName('appears');
    fs.writeFileSync(path.join(workspaceRoot(), name), 'new pending file\n');

    // Refresh until the file shows in Changes — a single repository.status scan
    // can transiently drop files on some engine builds (SBAI-4080 flush race).
    let paths: string[] | undefined;
    for (let i = 0; i < 5 && !paths; i++) {
      await vscode.commands.executeCommand('lore.refresh');
      paths = await waitForGroup(repo, 'changes', (p) => p.some((f) => f.endsWith(name)), 3000);
    }
    assert.ok(
      paths,
      `the SCM "Changes" group must list ${name}. If empty, the user sees nothing ` +
        `in Source Control even though the file is a pending change. Changes=` +
        `${JSON.stringify(groupPaths(repo.changesGroup, repo.folder.uri.fsPath))}`,
    );

    const state = repo.changesGroup.resourceStates.find((s) => s.resourceUri.fsPath.endsWith(name));
    assert.ok(state, 'the new file has a resource state in Changes');
    assert.strictEqual(
      state!.command?.command,
      'lore.openDiff',
      'clicking a change row opens the lore diff',
    );
    assert.ok(
      state!.decorations &&
        typeof state!.decorations.tooltip === 'string' &&
        state!.decorations.tooltip.length > 0,
      'resource decoration has a human tooltip (e.g. "Added")',
    );

    assert.strictEqual(
      repo.scm.count,
      repo.stagedGroup.resourceStates.length + repo.changesGroup.resourceStates.length,
      'scm.count must equal staged + changes resource-state totals (the SCM badge)',
    );
    assert.ok((repo.scm.count ?? 0) > 0, 'scm badge count must be > 0 with pending changes');
  });

  // -----------------------------------------------------------------------
  // (b) DETERMINISTIC: applyStatus() partitions files into the two groups the
  // user sees. This is the exact code that builds the panel from engine status,
  // so it pins the stage/unstage/discard *grouping semantics* without the flaky
  // cross-process staged-state read.
  // -----------------------------------------------------------------------
  test('applyStatus places staged files in Staged Changes and the rest in Changes', () => {
    repo.applyStatus(
      statusWith([
        file('staged-a.txt', true, 'add'),
        file('staged-b.txt', true, 'add'),
        file('change-c.txt', false, 'add'),
      ]),
    );

    assert.deepStrictEqual(
      names(repo.stagedGroup).sort(),
      ['staged-a.txt', 'staged-b.txt'],
      'the two staged files must render under "Staged Changes"',
    );
    assert.deepStrictEqual(
      names(repo.changesGroup),
      ['change-c.txt'],
      'the unstaged file must render under "Changes"',
    );
    assert.strictEqual(repo.scm.count, 3, 'scm badge = total tracked changes (3)');
  });

  test('staging is modelled as a file moving from Changes into Staged Changes (group transition)', () => {
    // Before: file is unstaged → in Changes.
    repo.applyStatus(statusWith([file('move.txt', false, 'add')]));
    assert.deepStrictEqual(names(repo.changesGroup), ['move.txt'], 'starts in Changes');
    assert.deepStrictEqual(names(repo.stagedGroup), [], 'not yet staged');

    // After staging: same file now staged → must be in Staged Changes, gone from Changes.
    repo.applyStatus(statusWith([file('move.txt', true, 'add')]));
    assert.deepStrictEqual(
      names(repo.stagedGroup),
      ['move.txt'],
      'after staging, the file renders under "Staged Changes"',
    );
    assert.deepStrictEqual(
      names(repo.changesGroup),
      [],
      'a staged file must NOT also appear under "Changes"',
    );
    assert.strictEqual(repo.scm.count, 1, 'still one tracked change after staging');
  });

  test('unstaging is the reverse transition (Staged Changes → Changes)', () => {
    repo.applyStatus(statusWith([file('back.txt', true, 'add')]));
    assert.deepStrictEqual(names(repo.stagedGroup), ['back.txt']);

    repo.applyStatus(statusWith([file('back.txt', false, 'add')]));
    assert.deepStrictEqual(
      names(repo.changesGroup),
      ['back.txt'],
      'after unstaging, the file is back under "Changes"',
    );
    assert.deepStrictEqual(names(repo.stagedGroup), [], 'no longer staged');
  });

  test('discarding (an empty status) clears BOTH groups and zeroes the badge', () => {
    repo.applyStatus(statusWith([file('gone.txt', true, 'add')]));
    assert.strictEqual(repo.scm.count, 1);

    // A clean tree after discard → no resource states anywhere.
    repo.applyStatus(statusWith([]));
    assert.deepStrictEqual(names(repo.stagedGroup), [], 'Staged group emptied');
    assert.deepStrictEqual(names(repo.changesGroup), [], 'Changes group emptied');
    assert.strictEqual(repo.scm.count, 0, 'badge count is 0 on a clean tree');
  });

  test('a deleted file renders with strike-through in the group', () => {
    repo.applyStatus(statusWith([file('removed.txt', false, 'delete')]));
    const state = repo.changesGroup.resourceStates.find((s) =>
      s.resourceUri.fsPath.endsWith('removed.txt'),
    );
    assert.ok(state, 'deleted file present in Changes');
    assert.strictEqual(
      state!.decorations?.strikeThrough,
      true,
      'a delete action must render struck-through in the SCM row',
    );
  });

  test('a lock on a changed file flows into the row tooltip ("locked by ...")', () => {
    const locks = new Map<string, LockStatus>([
      ['locked.txt', { path: 'locked.txt', owner: 'alice', locked_at: 0 }],
    ]);
    repo.applyStatus(statusWith([file('locked.txt', false, 'add')]), locks);
    const state = repo.changesGroup.resourceStates.find((s) =>
      s.resourceUri.fsPath.endsWith('locked.txt'),
    );
    assert.ok(state, 'locked file present in Changes');
    assert.match(
      String(state!.decorations?.tooltip ?? ''),
      /locked by alice/i,
      'the resource row tooltip must surface the lock owner',
    );
    assert.strictEqual(state!.contextValue, 'locked', 'locked rows get the "locked" contextValue');
  });

  suiteTeardown(async () => {
    // Re-sync the groups to the real engine state so we don't leave synthetic
    // resource states behind for any later suite that inspects this repo.
    await repo.refresh().catch(() => undefined);
  });
});
