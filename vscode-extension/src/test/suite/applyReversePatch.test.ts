// applyReversePatch.test.ts — table-driven unit tests for the hand-rolled
// reverse-unified-diff used to reconstruct a file's baseline from its working
// text + the engine's "baseline → working" patch.
//
// This codepath runs for EVERY diff a user opens and previously had ZERO tests.
// It used to silently fall back to "show working text (no change)" on any
// mismatch, so a file with real changes could render as "no changes". The
// hardened version VALIDATES the patch against the working text and THROWS
// (ReversePatchError) on inconsistency. These tests cover: multi-hunk, CRLF,
// no-trailing-newline (both sides), add-only, delete-only, empty baseline, plus
// the binary-detection helpers.

import * as assert from 'assert';
import {
  applyReversePatch,
  ReversePatchError,
  hasBinaryExtension,
  isBinaryBuffer,
  binaryPlaceholder,
} from '../../extension';

// A unified-diff "patch" turns BASELINE into WORKING. Reverse-applying it to
// WORKING must reproduce BASELINE. Each case below provides baseline + working
// + the patch; the test reverse-applies and asserts it recovers baseline.
interface Case {
  name: string;
  working: string;
  patch: string;
  expected: string; // the baseline we expect to reconstruct
}

const cases: Case[] = [
  {
    name: 'single hunk, one line changed',
    working: 'alpha\nBETA\ngamma\n',
    expected: 'alpha\nbeta\ngamma\n',
    patch: ['@@ -1,3 +1,3 @@', ' alpha', '-beta', '+BETA', ' gamma', ''].join('\n'),
  },
  {
    name: 'multi-hunk patch',
    // baseline: l1..l8 ; working changes line2 and line7
    working: ['one', 'TWO', 'three', 'four', 'five', 'six', 'SEVEN', 'eight', ''].join('\n'),
    expected: ['one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', ''].join('\n'),
    patch: [
      '@@ -1,3 +1,3 @@',
      ' one',
      '-two',
      '+TWO',
      ' three',
      '@@ -6,3 +6,3 @@',
      ' six',
      '-seven',
      '+SEVEN',
      ' eight',
      '',
    ].join('\n'),
  },
  {
    name: 'CRLF line endings preserved',
    working: 'a\r\nB\r\nc\r\n',
    expected: 'a\r\nb\r\nc\r\n',
    patch: ['@@ -1,3 +1,3 @@', ' a\r', '-b\r', '+B\r', ' c\r', ''].join('\n'),
  },
  {
    name: 'no trailing newline on baseline (working added one)',
    // baseline has no final newline; working added a newline + a line.
    working: 'first\nsecond\n',
    expected: 'first',
    patch: [
      '@@ -1 +1,2 @@',
      '-first',
      '\\ No newline at end of file',
      '+first',
      '+second',
      '',
    ].join('\n'),
  },
  {
    name: 'no trailing newline on working (baseline had one)',
    working: 'only',
    expected: 'only\n',
    patch: [
      '@@ -1 +1 @@',
      '-only',
      '+only',
      '\\ No newline at end of file',
      '',
    ].join('\n'),
  },
  {
    name: 'add-only (baseline empty, working is a brand-new file)',
    working: 'new line 1\nnew line 2\n',
    expected: '',
    patch: ['@@ -0,0 +1,2 @@', '+new line 1', '+new line 2', ''].join('\n'),
  },
  {
    name: 'delete-only (working empty, baseline had content)',
    working: '',
    expected: 'gone 1\ngone 2\n',
    patch: ['@@ -1,2 +0,0 @@', '-gone 1', '-gone 2', ''].join('\n'),
  },
  {
    name: 'empty baseline AND empty working (no-op patch text)',
    working: '',
    expected: '',
    patch: '',
  },
  {
    name: 'patch with file headers before the hunk (--- / +++)',
    working: 'x\nY\n',
    expected: 'x\ny\n',
    patch: [
      'diff --git a/f b/f',
      '--- a/f',
      '+++ b/f',
      '@@ -1,2 +1,2 @@',
      ' x',
      '-y',
      '+Y',
      '',
    ].join('\n'),
  },
  {
    name: 'pure addition into the middle of an existing file',
    working: ['top', 'inserted', 'bottom', ''].join('\n'),
    expected: ['top', 'bottom', ''].join('\n'),
    patch: ['@@ -1,2 +1,3 @@', ' top', '+inserted', ' bottom', ''].join('\n'),
  },
  {
    name: 'pure deletion from the middle of an existing file',
    working: ['top', 'bottom', ''].join('\n'),
    expected: ['top', 'removed', 'bottom', ''].join('\n'),
    patch: ['@@ -1,3 +1,2 @@', ' top', '-removed', ' bottom', ''].join('\n'),
  },
];

suite('applyReversePatch (baseline reconstruction)', () => {
  for (const c of cases) {
    test(c.name, () => {
      const baseline = applyReversePatch(c.working, c.patch);
      assert.strictEqual(
        baseline,
        c.expected,
        `reverse-applying the patch to working text should reproduce baseline\n` +
          `  working : ${JSON.stringify(c.working)}\n` +
          `  patch   : ${JSON.stringify(c.patch)}\n` +
          `  got     : ${JSON.stringify(baseline)}\n` +
          `  expected: ${JSON.stringify(c.expected)}`,
      );
    });
  }

  // ---- Loud-failure cases: must THROW, not silently mask. ------------------

  test('THROWS when a context line does not match the working text', () => {
    // The context line " alpha" doesn't exist in the working text → the old
    // code would have silently returned the working text (a masked failure).
    const working = 'totally\ndifferent\n';
    const patch = ['@@ -1,2 +1,2 @@', ' alpha', '-beta', '+BETA', ''].join('\n');
    assert.throws(
      () => applyReversePatch(working, patch),
      ReversePatchError,
      'a context-line mismatch must throw ReversePatchError, not be masked',
    );
  });

  test('THROWS when a + line does not match the working text', () => {
    const working = 'a\nNOTBETA\nc\n';
    const patch = ['@@ -1,3 +1,3 @@', ' a', '-b', '+BETA', ' c', ''].join('\n');
    assert.throws(() => applyReversePatch(working, patch), ReversePatchError);
  });

  test('THROWS on a hunk count mismatch (header lies about line counts)', () => {
    // Header claims +1,3 but only two after-side lines follow.
    const working = 'a\nc\n';
    const patch = ['@@ -1,3 +1,3 @@', ' a', ' c', ''].join('\n');
    assert.throws(() => applyReversePatch(working, patch), ReversePatchError);
  });

  test('round-trips a realistic multi-line edit deterministically', () => {
    const baseline = 'function f() {\n  return 1;\n}\n';
    const working = 'function f() {\n  return 2;\n}\n';
    const patch = [
      '@@ -1,3 +1,3 @@',
      ' function f() {',
      '-  return 1;',
      '+  return 2;',
      ' }',
      '',
    ].join('\n');
    assert.strictEqual(applyReversePatch(working, patch), baseline);
  });
});

suite('binary-file detection', () => {
  test('hasBinaryExtension matches known binary extensions (case-insensitive)', () => {
    assert.ok(hasBinaryExtension('Content/Hero.uasset'));
    assert.ok(hasBinaryExtension('maps/Level.UMAP'));
    assert.ok(hasBinaryExtension('art/icon.PNG'));
    assert.ok(!hasBinaryExtension('docs/lore.md'));
    assert.ok(!hasBinaryExtension('src/main.rs'));
    assert.ok(!hasBinaryExtension('noextension'));
  });

  test('isBinaryBuffer detects a NUL byte in the sniff window', () => {
    assert.ok(isBinaryBuffer(Buffer.from([0x41, 0x00, 0x42])), 'NUL byte → binary');
    assert.ok(!isBinaryBuffer(Buffer.from('plain ascii text', 'utf8')));
    assert.ok(!isBinaryBuffer(Buffer.from('', 'utf8')), 'empty buffer is not binary');
  });

  test('isBinaryBuffer only sniffs the first 8 KiB (NUL past the window is ignored)', () => {
    const buf = Buffer.alloc(10000, 0x41); // 'A' * 10000
    buf[9000] = 0x00; // NUL beyond the 8 KiB sniff window
    assert.ok(!isBinaryBuffer(buf), 'NUL past 8 KiB is not sniffed');
    buf[100] = 0x00; // NUL inside the window
    assert.ok(isBinaryBuffer(buf), 'NUL inside 8 KiB is detected');
  });

  test('binaryPlaceholder reports the basename and byte count', () => {
    const msg = binaryPlaceholder('Content/Sub/Hero.uasset', 12345);
    assert.ok(msg.includes('Hero.uasset'), 'shows the file basename');
    assert.ok(msg.includes('12,345 bytes'), 'shows a humanised byte count');
    assert.ok(!msg.includes('Content/Sub/'), 'shows basename only, not full path');
  });
});
