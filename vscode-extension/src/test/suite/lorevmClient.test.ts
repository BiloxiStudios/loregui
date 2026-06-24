// lorevmClient.test.ts — pure unit tests for the lorevmClient hardening
// (fix/vscode-lorevmclient-hardening). These exercise the engine-agnostic
// primitives directly (no VS Code APIs, no real lorevm spawn):
//
//   1. recoverEnvelope() — tolerant JSON-envelope recovery from noisy stdout
//      (stray progress lines, leading BOM, multiple JSON objects), preferring a
//      structured `error` envelope so a real engine error is never masked.
//   2. resolveLorevmBin() — must NOT mutate the caller-provided loreguiDirs
//      array (no accumulating duplicate LOREGUI_DIR entries across calls).
//
// We import from the compiled JS, the same way missingBinary.test.ts does.

import * as assert from 'assert';
import * as path from 'path';

// eslint-disable-next-line @typescript-eslint/no-var-requires
const lorevmClient = require(path.resolve(__dirname, '../../lorevmClient.js')) as {
  recoverEnvelope: (stdout: string) => Record<string, unknown> | undefined;
  resolveLorevmBin: (opts: {
    binPath?: string;
    loreguiDirs?: string[];
    extensionPath?: string;
  }) => string | null;
};

suite('lorevmClient — envelope-parse recovery', function () {
  const { recoverEnvelope } = lorevmClient;

  test('parses a clean single-object result', () => {
    const env = recoverEnvelope('{"ok": true, "value": 42}');
    assert.deepStrictEqual(env, { ok: true, value: 42 });
  });

  test('recovers the JSON object when stray progress text precedes it', () => {
    const stdout = 'syncing...\nfetching index\n{"files_updated": 3}';
    const env = recoverEnvelope(stdout);
    assert.deepStrictEqual(env, { files_updated: 3 });
  });

  test('strips a leading BOM before parsing', () => {
    const env = recoverEnvelope('﻿{"ok": true}');
    assert.deepStrictEqual(env, { ok: true });
  });

  test('strips a BOM that appears on the JSON line amid noise', () => {
    const env = recoverEnvelope('progress\n﻿{"ok": true}');
    assert.deepStrictEqual(env, { ok: true });
  });

  test('prefers an envelope containing an `error` key over a stray object', () => {
    // A stray progress object appears AFTER the structured error. last→first
    // scanning would hit the stray object first; the `error` preference must
    // still win so the real engine error is surfaced.
    const stdout =
      '{"error": {"kind": "lock", "message": "file is locked"}}\n' +
      '{"progress": "done"}';
    const env = recoverEnvelope(stdout);
    assert.ok(env && 'error' in env, 'must recover the error envelope');
    assert.deepStrictEqual(env.error, { kind: 'lock', message: 'file is locked' });
  });

  test('returns the last object when multiple non-error objects are present', () => {
    const stdout = '{"step": 1}\n{"step": 2}\n{"final": true}';
    const env = recoverEnvelope(stdout);
    assert.deepStrictEqual(env, { final: true });
  });

  test('returns undefined when nothing parses to an object', () => {
    assert.strictEqual(recoverEnvelope('not json at all'), undefined);
    assert.strictEqual(recoverEnvelope(''), undefined);
    // A bare JSON array/number/string is not an object envelope.
    assert.strictEqual(recoverEnvelope('[1,2,3]'), undefined);
    assert.strictEqual(recoverEnvelope('42'), undefined);
  });

  test('tolerates CRLF line endings', () => {
    const env = recoverEnvelope('progress\r\n{"ok": true}\r\n');
    assert.deepStrictEqual(env, { ok: true });
  });
});

suite('lorevmClient — resolveLorevmBin does not mutate input', function () {
  const { resolveLorevmBin } = lorevmClient;

  test('does not push LOREGUI_DIR onto the caller-provided loreguiDirs array', () => {
    const savedEnv = process.env.LOREVM_BIN;
    const savedPath = process.env.PATH;
    const savedLoregui = process.env.LOREGUI_DIR;
    try {
      // Force every other resolution source to miss so the LOREGUI_DIR branch
      // (the one that used to mutate `dirs`) is exercised.
      delete process.env.LOREVM_BIN;
      process.env.PATH = '';
      process.env.LOREGUI_DIR = '/nonexistent/loregui-env';

      const dirs = ['/nonexistent/a', '/nonexistent/b'];
      const before = [...dirs];

      // Call twice: a mutating impl would grow the array on each call.
      resolveLorevmBin({ loreguiDirs: dirs });
      resolveLorevmBin({ loreguiDirs: dirs });

      assert.deepStrictEqual(
        dirs,
        before,
        'resolveLorevmBin must not mutate the caller-provided loreguiDirs array',
      );
    } finally {
      if (savedEnv !== undefined) process.env.LOREVM_BIN = savedEnv;
      else delete process.env.LOREVM_BIN;
      process.env.PATH = savedPath;
      if (savedLoregui !== undefined) process.env.LOREGUI_DIR = savedLoregui;
      else delete process.env.LOREGUI_DIR;
    }
  });
});
