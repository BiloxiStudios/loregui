// knownBugs.test.ts — regression guards for the bug classes the owner hits
// manually. These assert the DESIRED behavior.
//
// BUG#1 and BUG#2 are now FIXED in the engine (SBAI-4080): file.stage resolves
// repo-relative paths against the repository root (--dir), so the exact paths
// the extension sends now stage correctly and commit persists. These guards are
// LIVE `test`s — a regression in the engine's relative-path staging turns the
// suite red. (They were `test.skip` pending guards until the fix landed.)
//
// BUG#3 is ACTIVE (it currently passes — sequential absolute-path commits work)
// and guards that the SBAI-4080 cross-process flush fix stays working.
//
// See BUGS.md in this directory for the full catalog + reproduction.

import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import { spawnSync } from 'child_process';
import { workspaceRoot, lorevmBin } from './helpers';

function op(repo: string, opId: string, args: Record<string, unknown>): unknown {
  const res = spawnSync(
    lorevmBin(),
    [opId, '--dir', repo, '--offline', '--identity', 'e2e-tester', '--args', JSON.stringify(args)],
    { encoding: 'utf8' },
  );
  const out = (res.stdout ?? '').trim();
  try {
    return JSON.parse(out);
  } catch {
    return { error: { kind: 'parse', message: out || res.stderr } };
  }
}

function freshRepo(): string {
  const os = require('os');
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'lore-bug-'));
  op(dir, 'repository.create', { repository_url: 'lore://localhost/bug' });
  return dir;
}

suite('Lore — known-bug regression guards (expected RED until fixed)', function () {
  this.timeout(60_000);

  // ---------------------------------------------------------------------
  // BUG #1 (P0): file.stage silently ignores REPO-RELATIVE paths.
  // The extension's resolveResourceTargets() passes path.relative(...) paths,
  // so every stage from the UI is a no-op (files: []). Absolute paths work.
  // Root: engine resolves stage paths against CWD/absolute, not --dir.
  // Effect: staging from VS Code does nothing → commit fails "Nothing staged".
  // ---------------------------------------------------------------------
  test('BUG#1: file.stage must honor repo-relative paths (as the extension sends them)', () => {
    const repo = freshRepo();
    fs.writeFileSync(path.join(repo, 'rel.txt'), 'content\n');
    const res = op(repo, 'file.stage', { paths: ['rel.txt'], scan: true }) as {
      files?: { path: string }[];
    };
    assert.ok(
      res.files && res.files.length > 0,
      `file.stage(['rel.txt']) staged nothing (files=${JSON.stringify(
        res.files,
      )}). The VS Code extension sends repo-relative paths, so UI staging is a ` +
        `silent no-op. file.stage must resolve relative paths against --dir.`,
    );
  });

  // ---------------------------------------------------------------------
  // BUG #2 (P0): full UI flow stage(relative) → commit must persist a revision
  // and leave the working tree clean across processes. With BUG#1 live, the
  // commit fails ("Nothing staged"). This mirrors the exact extension flow.
  // ---------------------------------------------------------------------
  test('BUG#2: stage(relative) → commit persists a new revision (cross-process)', () => {
    const repo = freshRepo();
    fs.writeFileSync(path.join(repo, 'a.txt'), 'alpha\n');

    // Exactly what the extension does: stage a RELATIVE path, then commit.
    op(repo, 'file.stage', { paths: ['a.txt'], scan: true });
    const commit = op(repo, 'revision.commit', { message: 'ui flow' }) as
      | { revision_number: number }
      | { error: { message: string } };

    assert.ok(
      'revision_number' in commit,
      `commit after stage(relative) failed: ${JSON.stringify(
        commit,
      )}. This is the "Nothing staged for commit" the owner hits from the UI.`,
    );

    // And the committed file must no longer appear as a pending change.
    const status = op(repo, 'repository.status', { scan: true }) as {
      files: { path: string; staged: boolean }[];
    };
    assert.strictEqual(
      status.files.length,
      0,
      `working tree must be clean after commit; still pending: ${JSON.stringify(
        status.files,
      )}`,
    );
  });

  // ---------------------------------------------------------------------
  // BUG #3 (P1): after a stage(absolute) + commit, a SECOND modify+commit cycle
  // must work. Documents that sequential separate-process commit cycles persist.
  // ---------------------------------------------------------------------
  test('BUG#3: second modify → stage(absolute) → commit cycle persists', () => {
    const repo = freshRepo();
    const file = path.join(repo, 'b.txt');
    fs.writeFileSync(file, 'one\n');
    op(repo, 'file.stage', { paths: [file], scan: true });
    const c1 = op(repo, 'revision.commit', { message: 'c1' }) as Record<string, unknown>;
    assert.ok('revision_number' in c1, `first commit failed: ${JSON.stringify(c1)}`);

    fs.appendFileSync(file, 'two\n');
    op(repo, 'file.stage', { paths: [file], scan: true });
    const c2 = op(repo, 'revision.commit', { message: 'c2' }) as Record<string, unknown>;
    assert.ok(
      'revision_number' in c2,
      `second commit cycle failed: ${JSON.stringify(c2)} — sequential ` +
        `separate-process commits must each persist.`,
    );

    const hist = op(repo, 'revision.history', { length: 10 }) as {
      entries: unknown[];
    };
    assert.strictEqual(
      hist.entries.length,
      2,
      `expected 2 revisions after two commits; got ${hist.entries.length}`,
    );
  });
});
