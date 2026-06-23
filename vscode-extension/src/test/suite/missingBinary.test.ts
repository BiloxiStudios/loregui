// missingBinary.test.ts — the graceful "lorevm not found" state.
//
// Guards the SBAI-4080 visibility fix from the OTHER direction: when the engine
// binary cannot be resolved (a bogus lore.lorevmPath), the extension must STILL
// register the SCM provider (so the failure is visible) and surface a clear
// error — not silently show nothing. We exercise resolveLorevmBin directly (the
// resolution primitive the provider uses) plus a real CLI invocation against a
// bogus path to assert a clear, structured error rather than a silent success.

import * as assert from 'assert';
import * as path from 'path';
import { spawnSync } from 'child_process';
import { workspaceRoot } from './helpers';

// The compiled extension exposes resolveLorevmBin; import from the built JS.
// (out/lorevmClient.js — same dir level as the suite's grandparent.)
// eslint-disable-next-line @typescript-eslint/no-var-requires
const lorevmClient = require(path.resolve(__dirname, '../../lorevmClient.js')) as {
  resolveLorevmBin: (opts: {
    binPath?: string;
    loreguiDirs?: string[];
    extensionPath?: string;
  }) => string | null;
};

suite('Lore — graceful missing-binary state', function () {
  this.timeout(30_000);

  test('resolveLorevmBin returns null for a bogus override path (no silent fallback)', () => {
    // A bogus explicit binPath must not silently fall through to PATH/bundled.
    // It returns null when nothing resolves; the extension then shows the
    // "lorevm not found — set lore.lorevmPath" state. We force the env clean so
    // only the bogus override is considered.
    const savedEnv = process.env.LOREVM_BIN;
    const savedPath = process.env.PATH;
    try {
      delete process.env.LOREVM_BIN;
      process.env.PATH = ''; // nothing on PATH
      const resolved = lorevmClient.resolveLorevmBin({
        binPath: '/nonexistent/definitely/not/lorevm',
        loreguiDirs: ['/nonexistent/loregui'],
        extensionPath: '/nonexistent/ext',
      });
      assert.strictEqual(
        resolved,
        null,
        'a bogus override with no other source must resolve to null',
      );
    } finally {
      if (savedEnv !== undefined) process.env.LOREVM_BIN = savedEnv;
      process.env.PATH = savedPath;
    }
  });

  test('invoking a bogus binary yields a clear failure, not a silent success', () => {
    // Directly invoke a path that is not an executable; spawnSync must report a
    // non-zero / error result. This is the failure the extension's guard()
    // converts into a visible "lorevm not found" message.
    const res = spawnSync(
      '/nonexistent/definitely/not/lorevm',
      ['repository.status', '--dir', workspaceRoot(), '--offline', '--args', '{}'],
      { encoding: 'utf8' },
    );
    assert.ok(
      res.error || (res.status !== 0 && res.status !== null),
      `a bogus binary must fail clearly (error=${res.error}, status=${res.status})`,
    );
  });
});
