// helpers.ts — shared test utilities for the lore SCM E2E suite.

import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { spawnSync } from 'child_process';

export const EXTENSION_ID = 'BiloxiStudios.loregui-lore';

/** The seeded workspace folder (a real .lore repo). */
export function workspaceRoot(): string {
  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    throw new Error('no workspace folder open — runTest.ts must launch with one');
  }
  return folders[0].uri.fsPath;
}

/** Resolved lorevm binary (forwarded via extensionTestsEnv). */
export function lorevmBin(): string {
  const bin = process.env.LOREVM_BIN;
  if (!bin || !fs.existsSync(bin)) {
    throw new Error(`LOREVM_BIN not resolvable: ${bin}`);
  }
  return bin;
}

/** Drive a lorevm op directly (out-of-band assertions on engine state). */
export function runOp(
  opId: string,
  args: Record<string, unknown>,
  repo = workspaceRoot(),
): unknown {
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
  const res = spawnSync(lorevmBin(), argv, { encoding: 'utf8' });
  const out = (res.stdout ?? '').trim();
  if (!out) {
    throw new Error(`lorevm ${opId} produced no stdout (stderr: ${res.stderr})`);
  }
  const parsed = JSON.parse(out);
  if (parsed && typeof parsed === 'object' && 'error' in parsed) {
    throw new Error(`lorevm ${opId} error: ${JSON.stringify(parsed.error)}`);
  }
  return parsed;
}

/** Activate the extension and return its exports. */
export async function activateExtension(): Promise<vscode.Extension<unknown>> {
  const ext = vscode.extensions.getExtension(EXTENSION_ID);
  if (!ext) {
    throw new Error(`extension ${EXTENSION_ID} not found`);
  }
  if (!ext.isActive) {
    await ext.activate();
  }
  return ext;
}

export function delay(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

/**
 * Poll `fn` until it returns truthy or `timeoutMs` elapses. Returns the value or
 * undefined on timeout. Used to wait out the extension's 400ms debounced
 * refresh + lorevm shell-out latency.
 */
export async function waitFor<T>(
  fn: () => T | Promise<T>,
  timeoutMs = 15_000,
  stepMs = 250,
): Promise<T | undefined> {
  const deadline = Date.now() + timeoutMs;
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const v = await fn();
    if (v) {
      return v;
    }
    if (Date.now() > deadline) {
      return undefined;
    }
    await delay(stepMs);
  }
}

export function rel(p: string): string {
  return path.relative(workspaceRoot(), p);
}
