// lorevmClient.ts — a thin wrapper over the `lorevm` JSON CLI.
//
// This is the SAME contract the lore-mcp server speaks (see lore-mcp/server.py
// and lore-mcp/README.md): it spawns
//
//   lorevm <domain>.<op> --dir <repo> [--offline] [--identity <id>] --args '<json>'
//
// and parses the op's typed JSON result from stdout. On any failure lorevm itself
// prints `{"error": {kind, message}}` to stdout and exits 1; we surface that as a
// structured LorevmError so callers always get the same shape.
//
// We deliberately drive OUR `lorevm` (crates/lorevm-cli) which binds the lore
// engine in-process — we do NOT shell out to Epic's public `lore` CLI.

import { spawn } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

/** The structured error shape lorevm emits: `{"error": {kind, message, ...}}`. */
export interface LorevmErrorBody {
  kind: string;
  message: string;
  [k: string]: unknown;
}

/** Thrown when an op fails (config, exec, parse, or an op-level lore error). */
export class LorevmError extends Error {
  readonly kind: string;
  readonly body: LorevmErrorBody;
  constructor(body: LorevmErrorBody) {
    super(body.message || body.kind || 'lorevm error');
    this.name = 'LorevmError';
    this.kind = body.kind || 'unknown';
    this.body = body;
  }
}

/** Options controlling how the binary is resolved and invoked. */
export interface LorevmClientOptions {
  /** Repository working directory (passed as --dir). */
  repoDir: string;
  /** Explicit binary path override (highest priority after constructor arg). */
  binPath?: string;
  /** Pass --offline for purely local repos. */
  offline?: boolean;
  /** Optional --identity value. */
  identity?: string;
  /**
   * Extra directories to search for the loregui `target/{debug,release}/lorevm`
   * build (the workspace root and the extension's own repo root). Mirrors
   * lore-mcp's LOREGUI_DIR fallback.
   */
  loreguiDirs?: string[];
  /** Per-invocation timeout in ms (default 120s, matching lore-mcp). */
  timeoutMs?: number;
}

const BIN_NAME = process.platform === 'win32' ? 'lorevm.exe' : 'lorevm';

/**
 * Resolve the `lorevm` binary the same way lore-mcp does:
 *   1. explicit override (constructor / settings)
 *   2. LOREVM_BIN env var
 *   3. PATH
 *   4. <loreguiDir>/target/{release,debug}/lorevm for each candidate dir
 *
 * Returns the resolved absolute path, or null if not found.
 */
export function resolveLorevmBin(opts: {
  binPath?: string;
  loreguiDirs?: string[];
}): string | null {
  const override = opts.binPath?.trim();
  if (override && fs.existsSync(override)) {
    return override;
  }

  const env = process.env.LOREVM_BIN?.trim();
  if (env && fs.existsSync(env)) {
    return env;
  }

  const onPath = whichOnPath(BIN_NAME);
  if (onPath) {
    return onPath;
  }

  const dirs = opts.loreguiDirs ?? [];
  if (process.env.LOREGUI_DIR) {
    dirs.push(process.env.LOREGUI_DIR);
  }
  for (const base of dirs) {
    if (!base) {
      continue;
    }
    for (const profile of ['release', 'debug']) {
      const cand = path.join(base, 'target', profile, BIN_NAME);
      if (fs.existsSync(cand)) {
        return cand;
      }
    }
  }

  return null;
}

/** Minimal `which`: scan PATH for an executable of the given name. */
function whichOnPath(name: string): string | null {
  const pathEnv = process.env.PATH || '';
  const sep = process.platform === 'win32' ? ';' : ':';
  for (const dir of pathEnv.split(sep)) {
    if (!dir) {
      continue;
    }
    const cand = path.join(dir, name);
    try {
      fs.accessSync(cand, fs.constants.X_OK);
      return cand;
    } catch {
      // not here; keep looking
    }
  }
  return null;
}

/**
 * LorevmClient wraps spawn + JSON parse + error handling for one repo dir.
 *
 * Use `run<TResult, TArgs>(opId, args)` to invoke any op id from the lorevm
 * dispatch table (e.g. `repository.status`, `revision.commit`, `file.stage`).
 */
export class LorevmClient {
  private readonly opts: LorevmClientOptions;

  constructor(opts: LorevmClientOptions) {
    this.opts = opts;
  }

  /** The resolved binary path, or null if lorevm can't be found. */
  resolveBin(): string | null {
    return resolveLorevmBin({
      binPath: this.opts.binPath,
      loreguiDirs: this.opts.loreguiDirs ? [...this.opts.loreguiDirs] : [],
    });
  }

  /**
   * Invoke `lorevm <opId> --dir <repo> --args '<json>'` and return the parsed
   * JSON result. Throws LorevmError on any failure (missing binary, exec error,
   * non-JSON output, or an op-level error).
   */
  async run<TResult = unknown>(
    opId: string,
    args: Record<string, unknown> = {},
  ): Promise<TResult> {
    const bin = this.resolveBin();
    if (!bin) {
      throw new LorevmError({
        kind: 'config',
        message:
          'lorevm binary not found. Set "lore.lorevmPath" or LOREVM_BIN, put ' +
          'lorevm on PATH, or build it: `cargo build -p lorevm-cli` in the ' +
          'loregui repo (binary lands in target/debug/lorevm).',
      });
    }
    if (!this.opts.repoDir) {
      throw new LorevmError({
        kind: 'config',
        message: 'no repository directory configured.',
      });
    }

    const argv = [opId, '--dir', this.opts.repoDir, '--args', JSON.stringify(args)];
    if (this.opts.offline) {
      argv.push('--offline');
    }
    if (this.opts.identity) {
      argv.push('--identity', this.opts.identity);
    }

    const { stdout, stderr, code } = await this.spawn(bin, argv);

    const out = stdout.trim();
    if (out) {
      let parsed: unknown;
      try {
        parsed = JSON.parse(out);
      } catch {
        throw new LorevmError({
          kind: 'parse',
          message: 'lorevm produced non-JSON output',
          stdout: out,
          stderr: stderr.trim(),
        });
      }
      if (
        parsed &&
        typeof parsed === 'object' &&
        'error' in (parsed as Record<string, unknown>)
      ) {
        const body = (parsed as { error: LorevmErrorBody }).error;
        throw new LorevmError(
          body && typeof body === 'object'
            ? body
            : { kind: 'unknown', message: String(body) },
        );
      }
      return parsed as TResult;
    }

    // No stdout — surface stderr / exit code.
    throw new LorevmError({
      kind: 'exec',
      message: stderr.trim() || `lorevm exited ${code}`,
    });
  }

  /** Spawn the binary, collecting stdout/stderr and enforcing a timeout. */
  private spawn(
    bin: string,
    argv: string[],
  ): Promise<{ stdout: string; stderr: string; code: number | null }> {
    const timeoutMs = this.opts.timeoutMs ?? 120_000;
    return new Promise((resolve, reject) => {
      const child = spawn(bin, argv, {
        cwd: this.opts.repoDir,
        env: process.env,
      });

      let stdout = '';
      let stderr = '';
      let settled = false;

      const timer = setTimeout(() => {
        if (settled) {
          return;
        }
        settled = true;
        child.kill('SIGKILL');
        reject(
          new LorevmError({
            kind: 'timeout',
            message: `${argv[0]} timed out after ${timeoutMs}ms`,
          }),
        );
      }, timeoutMs);

      child.stdout.on('data', (d) => (stdout += d.toString()));
      child.stderr.on('data', (d) => (stderr += d.toString()));

      child.on('error', (err) => {
        if (settled) {
          return;
        }
        settled = true;
        clearTimeout(timer);
        reject(
          new LorevmError({
            kind: 'exec',
            message: `failed to launch lorevm: ${err.message}`,
          }),
        );
      });

      child.on('close', (code) => {
        if (settled) {
          return;
        }
        settled = true;
        clearTimeout(timer);
        resolve({ stdout, stderr, code });
      });
    });
  }
}

// ---------------------------------------------------------------------------
// Typed shapes for the ops the extension drives. These mirror the serde structs
// in crates/lore-vm/src/ops/<domain>/<op>.rs — kept narrow to what the SCM UI
// needs. Unknown fields are tolerated (JSON.parse keeps them).
// ---------------------------------------------------------------------------

export type StatusFileAction = 'keep' | 'add' | 'delete' | 'move' | 'copy';
export type StatusNodeType = 'directory' | 'file' | 'link';

export interface StatusFile {
  path: string;
  size: number;
  action: StatusFileAction;
  node_type: StatusNodeType;
  staged: boolean;
  conflict: boolean;
  dirty: boolean;
  from_path: string;
}

export interface StatusRevision {
  repository: string;
  branch: string;
  branch_name: string;
  revision: string;
  revision_number: number;
  revision_staged: string;
}

export interface RepositoryStatusResult {
  revision: StatusRevision | null;
  files: StatusFile[];
  count: { directories: number; files: number } | null;
}

export interface CommitResult {
  revision: string;
  revision_number: number;
  branch: string;
}

export interface FileDiffEntry {
  path: string;
  patch: string;
  action: 'keep' | 'add' | 'delete' | 'move' | 'copy';
}

export interface FileHistoryEntry {
  path: string;
  repository: string;
  revision: string;
  revision_number: number;
  parents: string[];
  address: string;
  size: number;
  action: string;
}

export interface FileHistoryResult {
  entries: FileHistoryEntry[];
}

export interface LockStatus {
  path: string;
  owner: string;
  locked_at: number;
}

export interface FileStatusResult {
  locks: LockStatus[];
}

export interface RevisionSyncResult {
  files: { path: string; size: number; action: string; is_file: boolean }[];
  revisions: unknown[];
  files_updated: number;
  files_deleted: number;
}
