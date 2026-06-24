// helpers.ts — shared test utilities for the lore SCM E2E suite.

import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { spawnSync } from 'child_process';
// Type-only import: the running extension is loaded from out/extension.js, but
// the test bundle would compile its OWN copy of extension.ts — so we must NOT
// import its classes as VALUES (cross-module `instanceof` would break). We take
// only the API shape as a compile-time type and read the LIVE objects through
// `ext.exports`, asserting on public, structurally-typed fields.
import type { LoreExtensionApi, LoreRepository } from '../../extension';

export type { LoreExtensionApi, LoreRepository };

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

/**
 * Activate the extension and return its LIVE public API (the real SourceControl
 * groups, tree providers, status bar, decoration provider). UI-level tests use
 * this to read the state the user actually sees.
 */
export async function getExtensionApi(): Promise<LoreExtensionApi> {
  const ext = await activateExtension();
  const api = ext.exports as LoreExtensionApi | undefined;
  if (!api || !Array.isArray(api.repositories)) {
    throw new Error(
      'extension did not export its LoreExtensionApi (activate() must return ' +
        'the repositories/providers handle for UI-state assertions)',
    );
  }
  return api;
}

/** The single seeded repository (first workspace folder). Fails if absent. */
export async function getRepo(): Promise<LoreRepository> {
  const api = await getExtensionApi();
  const repo = api.repositories[0];
  if (!repo) {
    throw new Error('no lore repository registered for the seeded workspace');
  }
  return repo;
}

/** Relative paths currently in a SourceControl resource group. */
export function groupPaths(
  group: vscode.SourceControlResourceGroup,
  root: string = workspaceRoot(),
): string[] {
  return group.resourceStates.map((s) => path.relative(root, s.resourceUri.fsPath));
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
