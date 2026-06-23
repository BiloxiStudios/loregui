// seedWorkspace.ts — build a REAL scratch lore repo for the E2E run.
//
// Runs as a plain Node script BEFORE VS Code launches (see runTest.ts +
// package.json `pretest`). It must run outside the extension host because
// opening a folder inside a running VS Code window reloads the window and tears
// down the mocha context — so we seed the repo on disk first, then launch VS
// Code pointed at it.
//
// What it does, all via the `lorevm` CLI (the same JSON contract the extension
// shells out to), against a fresh temp dir:
//   1. resolve a `lorevm` binary (LOREVM_BIN, then a built target/{release,debug})
//   2. repository.create  → a real .lore repo
//   3. write tracked.txt + a subfolder file, stage them, commit r1
//   4. write a second file + modify tracked.txt and leave them UNSTAGED so the
//      SCM "Changes" group has content when VS Code opens
//   5. write the resolved workspace path + binary path to a .env-ish file that
//      runTest.ts reads (LORE_TEST_WORKSPACE / LOREVM_BIN).
//
// It prints the chosen workspace + binary as `KEY=VALUE` lines on stdout AND
// writes them to test-workspace.env so the npm script can export them.

import { spawnSync } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

const BIN_NAME = process.platform === 'win32' ? 'lorevm.exe' : 'lorevm';

/** Resolve a lorevm binary the same way the extension does (env, then build dirs). */
function resolveBin(): string {
  const env = process.env.LOREVM_BIN?.trim();
  if (env && fs.existsSync(env)) {
    return env;
  }
  // Walk up from the extension root looking for a loregui checkout's target/.
  // __dirname is <ext>/out/test, so the extension root is ../../.
  let cur = path.resolve(__dirname, '../../');
  for (let i = 0; i < 6; i++) {
    for (const profile of ['release', 'debug']) {
      const cand = path.join(cur, 'target', profile, BIN_NAME);
      if (fs.existsSync(cand)) {
        return cand;
      }
    }
    // Also try the extension's own bundled bin/ (delivery copy).
    const bundled = path.join(cur, 'vscode-extension', 'bin', BIN_NAME);
    if (fs.existsSync(bundled)) {
      return bundled;
    }
    const parent = path.dirname(cur);
    if (parent === cur) {
      break;
    }
    cur = parent;
  }
  throw new Error(
    'seedWorkspace: could not resolve a lorevm binary. Set LOREVM_BIN or build ' +
      'it: `cargo build --release -p lorevm-cli` in the loregui repo.',
  );
}

interface RunResult {
  ok: boolean;
  json: unknown;
  stdout: string;
  stderr: string;
  code: number | null;
}

function runOp(
  bin: string,
  repo: string,
  opId: string,
  args: Record<string, unknown>,
): RunResult {
  const argv = [
    opId,
    '--dir',
    repo,
    '--offline',
    '--identity',
    'e2e-tester',
    '--args',
    JSON.stringify(args),
  ];
  const res = spawnSync(bin, argv, { encoding: 'utf8' });
  const stdout = res.stdout ?? '';
  let json: unknown;
  try {
    json = JSON.parse(stdout.trim());
  } catch {
    json = undefined;
  }
  const ok =
    res.status === 0 &&
    !!json &&
    typeof json === 'object' &&
    !('error' in (json as Record<string, unknown>));
  return { ok, json, stdout, stderr: res.stderr ?? '', code: res.status };
}

function must(label: string, r: RunResult): void {
  if (!r.ok) {
    throw new Error(
      `seedWorkspace: ${label} failed (exit ${r.code}): ${r.stdout || r.stderr}`,
    );
  }
}

function main(): void {
  const bin = resolveBin();

  const workspace = fs.mkdtempSync(path.join(os.tmpdir(), 'lore-e2e-ws-'));

  // 1. create the repo (writes .lore/)
  must(
    'repository.create',
    runOp(bin, workspace, 'repository.create', {
      repository_url: 'lore://localhost/e2e',
      description: 'lore vscode e2e scratch repo',
    }),
  );

  // 2. seed committed content: tracked.txt + lore/chapters/intro.md
  fs.writeFileSync(path.join(workspace, 'tracked.txt'), 'line one\nline two\n');
  fs.mkdirSync(path.join(workspace, 'chapters'), { recursive: true });
  fs.writeFileSync(
    path.join(workspace, 'chapters', 'intro.md'),
    '# Intro\n\nThe story begins.\n',
  );

  // NOTE: stage with ABSOLUTE paths. file.stage silently no-ops on repo-relative
  // paths (BUGS.md #1) — the seed must work around that bug to build a real
  // committed baseline. The extension's own UI flow uses RELATIVE paths and so
  // hits the bug; that is asserted in knownBugs.test.ts, not worked around here.
  must(
    'file.stage (initial)',
    runOp(bin, workspace, 'file.stage', {
      paths: [
        path.join(workspace, 'tracked.txt'),
        path.join(workspace, 'chapters', 'intro.md'),
      ],
      scan: true,
    }),
  );
  must(
    'revision.commit (r1)',
    runOp(bin, workspace, 'revision.commit', { message: 'seed: initial revision' }),
  );

  // 3. leave WORKING-TREE changes so the SCM Changes group is non-empty when
  //    VS Code opens: a brand-new file + a modification to the tracked file.
  fs.writeFileSync(path.join(workspace, 'untracked.txt'), 'fresh content\n');
  fs.appendFileSync(path.join(workspace, 'tracked.txt'), 'line three (modified)\n');

  // Emit the resolved env for runTest.ts + the npm script. __dirname is
  // <ext>/out/test, so the extension root (where test-workspace.env lives) is ../../.
  const envPath = path.resolve(__dirname, '../../', 'test-workspace.env');
  const lines = [`LORE_TEST_WORKSPACE=${workspace}`, `LOREVM_BIN=${bin}`];
  fs.writeFileSync(envPath, lines.join('\n') + '\n');
  // Also to stdout for visibility / shell `eval`.
  for (const l of lines) {
    // eslint-disable-next-line no-console
    console.log(l);
  }
}

main();
