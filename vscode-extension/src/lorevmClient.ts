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
  /** Absolute path to the extension's installation root (context.extensionPath). */
  extensionPath?: string;
  /**
   * Per-invocation timeout in ms. When unset, defaults to 120s for local ops
   * and 600s for network ops (revision.sync / branch.push). An explicit value
   * overrides both.
   */
  timeoutMs?: number;
}

const BIN_NAME = process.platform === 'win32' ? 'lorevm.exe' : 'lorevm';

/** Default per-invocation timeout for local ops (ms). Matches lore-mcp. */
const DEFAULT_TIMEOUT_MS = 120_000;
/**
 * Larger default for network ops (sync / push). These contact a remote lore
 * server and can legitimately run well past the 120s local default; killing
 * them mid-transfer is worse than waiting.
 */
const NETWORK_TIMEOUT_MS = 600_000;
/** Grace period between SIGTERM and the hard SIGKILL on timeout (ms). */
const KILL_GRACE_MS = 2_000;

/** True for op ids that perform network I/O and warrant a larger timeout. */
function isNetworkOp(opId: string): boolean {
  return opId === 'revision.sync' || opId === 'branch.push';
}

/** Strip a leading UTF-8 BOM (U+FEFF) if present. */
function stripBom(s: string): string {
  return s.charCodeAt(0) === 0xfeff ? s.slice(1) : s;
}

/**
 * Recover the structured lorevm JSON envelope from possibly-noisy stdout.
 *
 * lorevm is supposed to print exactly one JSON object on stdout, but stray
 * lines (progress text, a leftover BOM, or multiple JSON objects) make a naive
 * `JSON.parse(stdout)` throw — masking the real engine error as a generic
 * `kind:'parse'`. Instead we scan candidate lines from LAST to FIRST and return
 * the first that parses to an object, preferring one that carries an `error`
 * key (the structured failure shape). Whole-string parse is tried first so a
 * normal single-object result is unaffected.
 *
 * Returns the parsed object, or `undefined` if nothing parses to an object.
 */
export function recoverEnvelope(stdout: string): Record<string, unknown> | undefined {
  const cleaned = stripBom(stdout);

  const tryParse = (candidate: string): Record<string, unknown> | undefined => {
    const t = stripBom(candidate).trim();
    if (!t) {
      return undefined;
    }
    try {
      const v = JSON.parse(t);
      // lorevm's contract is always a JSON object envelope — reject arrays and
      // scalars so a stray `[...]` / number line isn't mistaken for a result.
      if (v && typeof v === 'object' && !Array.isArray(v)) {
        return v as Record<string, unknown>;
      }
    } catch {
      // not JSON; caller keeps scanning
    }
    return undefined;
  };

  // Fast path: the whole (trimmed) output is a single JSON object.
  const whole = tryParse(cleaned);
  if (whole) {
    return whole;
  }

  // Scan lines from last to first, collecting every object that parses. Prefer
  // one that contains an `error` key so a structured engine error wins over a
  // stray progress object.
  let firstObject: Record<string, unknown> | undefined;
  const lines = cleaned.split(/\r?\n/);
  for (let i = lines.length - 1; i >= 0; i--) {
    const obj = tryParse(lines[i]);
    if (!obj) {
      continue;
    }
    if ('error' in obj) {
      return obj;
    }
    if (!firstObject) {
      firstObject = obj;
    }
  }
  return firstObject;
}

/**
 * Resolve the `lorevm` binary the same way lore-mcp does:
 *   1. explicit override (constructor / settings)
 *   2. LOREVM_BIN env var
 *   3. PATH
 *   4. <loreguiDir>/target/{release,debug}/lorevm for each candidate dir
 *   5. Bundled binary inside the extension's `bin/` directory (delivery)
 *
 * Returns the resolved absolute path, or null if not found.
 */
export function resolveLorevmBin(opts: {
  binPath?: string;
  loreguiDirs?: string[];
  extensionPath?: string;
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

  // Defensive copy: never mutate the caller-provided array, or repeated calls
  // would accumulate duplicate LOREGUI_DIR entries.
  const dirs = opts.loreguiDirs ? [...opts.loreguiDirs] : [];
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

  // Bundled binary: the primary delivery method for marketplace users.
  const bundled = opts.extensionPath
    ? path.join(opts.extensionPath, 'bin', BIN_NAME)
    : path.join(__dirname, '..', 'bin', BIN_NAME);
  if (fs.existsSync(bundled)) {
    return bundled;
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
      extensionPath: this.opts.extensionPath,
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

    const out = stripBom(stdout).trim();
    if (out) {
      // Tolerant recovery: scan stdout (last→first, BOM-stripped) for the JSON
      // envelope rather than parsing the whole buffer, so stray progress lines
      // or a leading BOM don't mask a real structured engine error.
      const parsed = recoverEnvelope(stdout);
      if (parsed) {
        if ('error' in parsed) {
          const body = (parsed as { error: LorevmErrorBody }).error;
          throw new LorevmError(
            body && typeof body === 'object'
              ? body
              : { kind: 'unknown', message: String(body) },
          );
        }
        return parsed as TResult;
      }

      // Nothing in stdout parsed to an object. If the process also failed,
      // surface stderr as an exec error (more useful than a generic parse
      // error); otherwise report the unparseable output, with raw streams
      // attached for diagnostics.
      const trimmedErr = stderr.trim();
      if (code !== 0 && code !== null) {
        throw new LorevmError({
          kind: 'exec',
          message: trimmedErr || `lorevm exited ${code} with non-JSON output`,
          stdout: out,
          stderr: trimmedErr,
          code,
        });
      }
      throw new LorevmError({
        kind: 'parse',
        message: 'lorevm produced non-JSON output',
        stdout: out,
        stderr: trimmedErr,
        code,
      });
    }

    // No stdout — surface stderr / exit code.
    throw new LorevmError({
      kind: 'exec',
      message: stderr.trim() || `lorevm exited ${code}`,
      stderr: stderr.trim(),
      code,
    });
  }

  /** Spawn the binary, collecting stdout/stderr and enforcing a timeout. */
  private spawn(
    bin: string,
    argv: string[],
  ): Promise<{ stdout: string; stderr: string; code: number | null }> {
    // Explicit override wins; otherwise pick a default by op kind. argv[0] is
    // the op id (e.g. "revision.sync").
    const timeoutMs =
      this.opts.timeoutMs ??
      (isNetworkOp(argv[0]) ? NETWORK_TIMEOUT_MS : DEFAULT_TIMEOUT_MS);
    return new Promise((resolve, reject) => {
      const child = spawn(bin, argv, {
        cwd: this.opts.repoDir,
        env: process.env,
      });

      let stdout = '';
      let stderr = '';
      let settled = false;
      let killTimer: NodeJS.Timeout | undefined;

      const timer = setTimeout(() => {
        if (settled) {
          return;
        }
        settled = true;
        // Graceful shutdown: SIGTERM first so the engine can finish/rollback an
        // in-flight write, then a hard SIGKILL after a short grace if it hangs.
        child.kill('SIGTERM');
        killTimer = setTimeout(() => {
          if (!child.killed) {
            child.kill('SIGKILL');
          }
        }, KILL_GRACE_MS);
        // Unref so the grace timer never keeps the host process alive.
        killTimer.unref?.();
        reject(
          new LorevmError({
            kind: 'timeout',
            message: `${argv[0]} timed out after ${timeoutMs}ms`,
            stdout: stripBom(stdout).trim(),
            stderr: stderr.trim(),
            timeoutMs,
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
        if (killTimer) {
          clearTimeout(killTimer);
        }
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

// --- branch domain ---------------------------------------------------------

export interface BranchPoint {
  branch: string;
  revision: string;
}

export interface BranchListEntry {
  location: string; // "local" | "remote"
  id: string;
  name: string;
  category: string;
  latest: string;
  stack: BranchPoint[];
  creator: string;
  created: number;
  is_current: boolean;
  archived: boolean;
}

export interface BranchListResult {
  entries: BranchListEntry[];
  count: number;
}

export interface BranchCreateResult {
  name: string;
  latest: string;
  is_commit: boolean;
}

export interface BranchSwitchResult {
  branch: string;
}

export interface BranchPushResult {
  branch_name: string;
  local_revision: string;
  remote_revision: string;
  local_history: number;
  already_pushed: boolean;
}

// --- revision history / info ------------------------------------------------

export interface RevisionHistoryEntry {
  revision: string;
  revision_number: number;
  parents: string[];
}

export interface RevisionHistoryResult {
  entries: RevisionHistoryEntry[];
}

export interface RevisionMetadataEntry {
  key: string;
  value: string;
}

export interface RevisionInfoData {
  repository: string;
  revision: string;
  revision_number: number;
  parents: string[];
}

export interface RevisionInfoResult {
  info: RevisionInfoData | null;
  deltas: unknown[];
  metadata: RevisionMetadataEntry[];
}

// --- lock domain (query / acquire / release) -------------------------------

export interface LockEntry {
  branch: string;
  path: string;
  owner: string;
  locked_at: number;
}

export interface FileQueryResult {
  count: number;
  locks: LockEntry[];
}

export interface FileAcquireResult {
  acquired: string[];
  ignored: string[];
}

export interface FileReleaseResult {
  released: string[];
  not_found: boolean;
}

// --- working-tree reset (discard) ------------------------------------------

export interface FileResetResult {
  files: { path: string; action: string; from_path: string }[];
  counts: {
    directory_reset_count: number;
    directory_delete_count: number;
    file_reset_count: number;
    file_delete_count: number;
  };
}

export interface RevertResult {
  has_conflicts: boolean;
  conflict_files: { path: string }[];
  committed_revision: string | null;
}
