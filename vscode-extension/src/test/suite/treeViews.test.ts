// treeViews.test.ts — UI-LEVEL coverage of the activity-bar tree views.
//
// scm.test.ts asserts that `branch.list` / `revision.history` (the CLI) return
// the right data — but it NEVER calls the extension's TreeDataProviders, so the
// Branches / History / Locks panels could render nothing and those tests would
// still pass. Here we get the LIVE providers from the extension API and call
// `getChildren()` — the exact tree VS Code asks the provider to render — and
// assert on the resulting TreeItems (label / description / contextValue / icon /
// command), cross-checking against the engine as a sanity anchor.

import * as assert from 'assert';
import * as vscode from 'vscode';
import {
  activateExtension,
  getExtensionApi,
  runOp,
  LoreExtensionApi,
} from './helpers';

interface BranchListResult {
  entries: { name: string; is_current: boolean }[];
}
interface RevisionHistoryResult {
  entries: { revision_number: number; revision: string }[];
}

/** TreeItem label can be string | TreeItemLabel; normalise to string. */
function labelText(item: vscode.TreeItem): string {
  const l = item.label;
  return typeof l === 'string' ? l : (l?.label ?? '');
}

function themeIconId(item: vscode.TreeItem): string | undefined {
  return item.iconPath instanceof vscode.ThemeIcon ? item.iconPath.id : undefined;
}

/** Await a provider's getChildren() and assert it yielded a concrete array. */
async function treeChildren(
  provider: vscode.TreeDataProvider<vscode.TreeItem>,
): Promise<vscode.TreeItem[]> {
  const res = await provider.getChildren();
  assert.ok(Array.isArray(res), 'getChildren() returned an array (the rendered tree)');
  return res;
}

suite('Lore — activity-bar tree views (getChildren UI state)', function () {
  this.timeout(60_000);

  let api: LoreExtensionApi;

  suiteSetup(async () => {
    await activateExtension();
    api = await getExtensionApi();
    await api.refreshAll();
  });

  // -----------------------------------------------------------------------
  // Branches view.
  // -----------------------------------------------------------------------
  test('Branches getChildren() renders one TreeItem per branch, with main present', async () => {
    const children = await treeChildren(api.branchesView);
    assert.ok(Array.isArray(children) && children.length >= 1, 'at least one branch node');

    const engine = runOp('branch.list', {}) as BranchListResult;
    const treeLabels = children.map(labelText);

    // Every engine branch name must be substring-present in some tree node label
    // (the node label is the branch name; current branch may carry a marker).
    for (const b of engine.entries) {
      assert.ok(
        treeLabels.some((l) => l.includes(b.name)),
        `branch "${b.name}" from branch.list must be rendered as a tree node; ` +
          `tree labels=${JSON.stringify(treeLabels)}`,
      );
    }
    assert.ok(
      treeLabels.some((l) => l.includes('main')),
      'the Branches tree must show the main branch',
    );
  });

  test('Branches nodes carry loreBranch contextValue + a branch/check icon, and non-current ones a switch command', async () => {
    const children = await treeChildren(api.branchesView);
    const engine = runOp('branch.list', {}) as BranchListResult;

    for (const node of children) {
      assert.strictEqual(
        node.contextValue,
        'loreBranch',
        `branch node "${labelText(node)}" must use the loreBranch contextValue ` +
          '(drives the right-click menu)',
      );
      const icon = themeIconId(node);
      assert.ok(
        icon === 'git-branch' || icon === 'check',
        `branch node icon should be git-branch or check (current); got ${icon}`,
      );
    }

    // The current branch node must NOT have a switch command; non-current ones must.
    const current = engine.entries.find((b) => b.is_current);
    if (current) {
      const currentNode = children.find((n) => labelText(n).includes(current.name));
      assert.ok(currentNode, 'current branch has a node');
      assert.ok(
        !currentNode!.command || currentNode!.command.command !== 'lore.branchSwitch',
        'the current branch row must not be a switch action',
      );
    }
    const other = engine.entries.find((b) => !b.is_current);
    if (other) {
      const otherNode = children.find(
        (n) => labelText(n).includes(other.name) && !labelText(n).includes('current'),
      );
      if (otherNode) {
        assert.strictEqual(
          otherNode.command?.command,
          'lore.branchSwitch',
          'a non-current branch row must run lore.branchSwitch on click',
        );
      }
    }
  });

  // NOTE: we intentionally do NOT create a branch here. branch.create
  // AUTO-SWITCHES the current branch on the shared primary repo, which would
  // perturb the other suites' assumptions about the seeded working tree / current
  // branch. The "a new branch appears" behaviour is covered (engine-side) by
  // scm.test.ts; here we only assert the TreeDataProvider faithfully RENDERS
  // whatever branches the engine reports, which the two tests above already do.

  test('Branches tree node count matches branch.list exactly (no phantom / dropped rows)', async () => {
    const children = await treeChildren(api.branchesView);
    const engine = runOp('branch.list', {}) as BranchListResult;
    assert.strictEqual(
      children.length,
      engine.entries.length,
      `Branches tree must render exactly one node per branch.list entry; ` +
        `tree=${children.length} engine=${engine.entries.length}`,
    );
  });

  // -----------------------------------------------------------------------
  // History view.
  // -----------------------------------------------------------------------
  test('History getChildren() renders a node per revision (r-number labels), seed r1 present', async () => {
    const children = await treeChildren(api.historyView);
    assert.ok(children.length >= 1, 'at least the seed revision node');

    const engine = runOp('revision.history', { length: 50 }) as RevisionHistoryResult;
    assert.strictEqual(
      children.length,
      engine.entries.length,
      `History tree node count (${children.length}) must match revision.history ` +
        `(${engine.entries.length})`,
    );

    const labels = children.map(labelText);
    assert.ok(
      labels.every((l) => /^r\d+/.test(l)),
      `every History node label must start with the revision number (rN); got ${JSON.stringify(
        labels,
      )}`,
    );
    assert.ok(
      labels.some((l) => /^r1\b/.test(l) || l.startsWith('r1 ')),
      'the seed revision r1 must be a node in the History tree',
    );
  });

  test('History nodes carry loreRevision contextValue, a git-commit icon, and an openRevisionDiff command', async () => {
    const children = await treeChildren(api.historyView);
    assert.ok(children.length >= 1);
    for (const node of children) {
      assert.strictEqual(node.contextValue, 'loreRevision', 'History node contextValue');
      assert.strictEqual(themeIconId(node), 'git-commit', 'History node icon');
      assert.strictEqual(
        node.command?.command,
        'lore.openRevisionDiff',
        'clicking a revision opens its diff overview',
      );
    }
  });

  // -----------------------------------------------------------------------
  // Locks view — on a local/offline repo there is no lock server, so the tree is
  // empty AND the provider sets the lore.locksNoRemote context (which gates the
  // viewsWelcome hint the user sees). Assert that exact UI behavior.
  // -----------------------------------------------------------------------
  test('Locks getChildren() is empty on a no-remote repo and does not throw (graceful degrade)', async () => {
    const children = await treeChildren(api.locksView);
    assert.ok(Array.isArray(children), 'Locks getChildren returns an array');
    assert.strictEqual(
      children.length,
      0,
      `local/offline repo has no lock server, so the Locks tree must be empty; ` +
        `got ${JSON.stringify(children.map(labelText))}`,
    );
  });

  test('Locks view repeated getChildren() stays stable (no crash) after refreshes', async () => {
    await api.refreshAll();
    const a = await treeChildren(api.locksView);
    const b = await treeChildren(api.locksView);
    assert.strictEqual(a.length, 0);
    assert.strictEqual(b.length, 0);
  });
});
