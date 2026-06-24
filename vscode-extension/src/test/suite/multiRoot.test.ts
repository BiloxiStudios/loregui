// multiRoot.test.ts — UI-LEVEL coverage of multi-root: a workspace with TWO
// .lore folders must get a SEPARATE SourceControl per folder, and routing
// (repoForUri / pickRepository) must send each file to its owning repo.
//
// The window is launched with a multi-root `.code-workspace` listing both seeded
// repos (see seedWorkspace.ts + runTest.ts), so both LoreRepository instances are
// registered at activation — we do NOT add a folder at runtime (that would force a
// window reload and tear down mocha). We get the LIVE repos from the extension API
// and assert each owns its own SourceControl + resource groups, that getRepository
// routes by folder, and that an op in one repo doesn't touch the other.

import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import {
  activateExtension,
  getExtensionApi,
  waitFor,
  delay,
  LoreExtensionApi,
  LoreRepository,
} from './helpers';

function secondWorkspacePath(): string {
  const p = process.env.LORE_TEST_WORKSPACE2;
  if (!p || !fs.existsSync(path.join(p, '.lore'))) {
    throw new Error(
      `LORE_TEST_WORKSPACE2 not a seeded .lore repo: ${p} (run pretest/seedWorkspace)`,
    );
  }
  return p;
}

suite('Lore — multi-root (per-folder SourceControl)', function () {
  this.timeout(60_000);

  let api: LoreExtensionApi;
  let secondPath: string;

  suiteSetup(async function () {
    await activateExtension();
    api = await getExtensionApi();
    secondPath = secondWorkspacePath();

    // The multi-root workspace must have surfaced TWO .lore folders. If the host
    // somehow launched single-root (no workspace file), skip rather than fail —
    // the per-folder assertions are only meaningful with both folders open.
    const folders = vscode.workspace.workspaceFolders ?? [];
    const haveSecondFolder = folders.some((f) => f.uri.fsPath === secondPath);
    if (!haveSecondFolder) {
      // eslint-disable-next-line no-console
      console.warn(
        'multiRoot: second folder not open (launched single-root?) — skipping suite',
      );
      this.skip();
    }

    // Wait for both repos to be discovered + their initial refresh to settle.
    await waitFor(() => api.repositories.length >= 2, 15_000);
    await delay(1500);
  });

  test('two .lore folders → two registered repositories', () => {
    assert.ok(
      api.repositories.length >= 2,
      `expected >= 2 repositories for two .lore folders; got ${api.repositories.length} ` +
        `(${api.repositories.map((r) => r.folder.uri.fsPath).join(', ')})`,
    );
    const paths = api.repositories.map((r) => r.folder.uri.fsPath);
    assert.ok(paths.includes(secondPath), 'the second repo folder is among the registered repos');
  });

  test('each repository owns a DISTINCT SourceControl instance with its own resource groups', () => {
    const repos = api.repositories;
    const scms = repos.map((r) => r.scm);
    // Distinct SourceControl objects (not the same provider reused).
    for (let i = 0; i < scms.length; i++) {
      for (let j = i + 1; j < scms.length; j++) {
        assert.notStrictEqual(
          scms[i],
          scms[j],
          'each folder must have its own SourceControl instance',
        );
        assert.notStrictEqual(
          repos[i].stagedGroup,
          repos[j].stagedGroup,
          'each folder must have its own Staged Changes group',
        );
        assert.notStrictEqual(
          repos[i].changesGroup,
          repos[j].changesGroup,
          'each folder must have its own Changes group',
        );
      }
    }
    // The SourceControl rootUri must match each repo's folder.
    for (const r of repos) {
      assert.strictEqual(
        r.scm.rootUri?.fsPath,
        r.folder.uri.fsPath,
        `SourceControl.rootUri must be the folder it covers (${r.folder.uri.fsPath})`,
      );
    }
  });

  test('getRepository(uri) routes a file to the SourceControl of its owning folder', () => {
    const primary = api.repositories.find((r) => r.folder.uri.fsPath !== secondPath);
    const second = api.repositories.find((r) => r.folder.uri.fsPath === secondPath);
    assert.ok(primary && second, 'both repos resolved');

    const primaryFile = vscode.Uri.file(path.join(primary!.folder.uri.fsPath, 'tracked.txt'));
    const secondFile = vscode.Uri.file(
      path.join(second!.folder.uri.fsPath, 'second-tracked.txt'),
    );

    assert.strictEqual(
      api.getRepository(primaryFile)?.folder.uri.fsPath,
      primary!.folder.uri.fsPath,
      'a file under the primary folder must route to the primary repo',
    );
    assert.strictEqual(
      api.getRepository(secondFile)?.folder.uri.fsPath,
      second!.folder.uri.fsPath,
      'a file under the second folder must route to the second repo',
    );
    // A file in neither folder routes to nothing.
    assert.strictEqual(
      api.getRepository(vscode.Uri.file('/definitely/outside/any/repo.txt')),
      undefined,
      'a file outside every repo routes to no repository',
    );
  });

  test("the second repo's SCM state is independent (its own pending change, not the primary's)", async () => {
    const second = secondRepo();

    // Refresh until the second repo's Changes group shows its own pending file.
    // A single repository.status scan can transiently drop files on some engine
    // builds (the SBAI-4080 flush race), so retry the refresh a few times.
    let changeNames: string[] | undefined;
    for (let i = 0; i < 5 && !changeNames; i++) {
      await second.refresh();
      changeNames = await waitFor(() => {
        const names = second.changesGroup.resourceStates.map((s) =>
          path.basename(s.resourceUri.fsPath),
        );
        return names.some((n) => n === 'second-untracked.txt') ? names : undefined;
      }, 3000);
    }

    assert.ok(
      changeNames,
      `the second repo's Changes group must show ITS own pending file ` +
        `(second-untracked.txt), independent of the primary repo; got ` +
        `${JSON.stringify(second.changesGroup.resourceStates.map((s) => s.resourceUri.fsPath))}`,
    );
    // Sanity: it must NOT contain the primary repo's files.
    assert.ok(
      !changeNames!.includes('untracked.txt'),
      "the second repo must not leak the primary repo's changes",
    );
  });

  test("staged state is scoped per repository — staging in one does not touch the other's groups", () => {
    // Use the extension's own applyStatus() (the function that builds each repo's
    // SCM groups) to model "a file staged in the second repo". This pins the
    // per-folder isolation of the resource groups WITHOUT depending on the
    // engine's flaky cross-process staged-state read (BUGS.md #5 / SBAI-4080) —
    // the point under test is the routing/scoping, not the engine's flush.
    const second = secondRepo();
    const primary = primaryRepo();

    // Capture the primary's current groups so we can prove they are untouched.
    const primaryStagedBefore = primary.stagedGroup.resourceStates.map(
      (s) => s.resourceUri.fsPath,
    );
    const primaryChangesBefore = primary.changesGroup.resourceStates.map(
      (s) => s.resourceUri.fsPath,
    );

    // Model a staged change in the SECOND repo only.
    second.applyStatus({
      revision: {
        repository: 'r2',
        branch: 'b2',
        branch_name: 'main',
        revision: 'x',
        revision_number: 1,
        revision_staged: '',
      },
      files: [
        {
          path: 'second-untracked.txt',
          size: 1,
          action: 'add',
          node_type: 'file',
          staged: true,
          conflict: false,
          dirty: false,
          from_path: '',
        },
      ],
      count: null,
    });

    // The second repo's Staged group now shows the file...
    assert.deepStrictEqual(
      second.stagedGroup.resourceStates.map((s) => path.basename(s.resourceUri.fsPath)),
      ['second-untracked.txt'],
      "the second repo's Staged Changes group reflects ITS staged file",
    );
    // ...and the file's URI is under the SECOND folder, not the primary.
    assert.ok(
      second.stagedGroup.resourceStates[0].resourceUri.fsPath.startsWith(secondPath),
      'the staged row points at a file inside the second repo folder',
    );

    // The primary repo's groups are completely unchanged.
    assert.deepStrictEqual(
      primary.stagedGroup.resourceStates.map((s) => s.resourceUri.fsPath),
      primaryStagedBefore,
      "staging in the second repo must not alter the primary repo's Staged group",
    );
    assert.deepStrictEqual(
      primary.changesGroup.resourceStates.map((s) => s.resourceUri.fsPath),
      primaryChangesBefore,
      "staging in the second repo must not alter the primary repo's Changes group",
    );
  });

  suiteTeardown(async () => {
    // Re-sync the second repo's groups to its real engine state.
    const second = api?.repositories.find((r) => r.folder.uri.fsPath === secondPath);
    await second?.refresh().catch(() => undefined);
  });

  // ----- helpers -----
  function secondRepo(): LoreRepository {
    const r = api.repositories.find((x) => x.folder.uri.fsPath === secondPath);
    assert.ok(r, 'second repo resolved');
    return r!;
  }
  function primaryRepo(): LoreRepository {
    const r = api.repositories.find((x) => x.folder.uri.fsPath !== secondPath);
    assert.ok(r, 'primary repo resolved');
    return r!;
  }
});
