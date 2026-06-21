#!/usr/bin/env node
/**
 * Upstream lore API-parity detector.
 *
 * Keeps LoreGUI in parity with Epic's `lore` crate: it enumerates the op surface
 * of the *pinned* upstream `lore` source (every `pub async fn` in `lore/src/`)
 * and diffs it against our `crates/lore-vm/src/ops/<domain>/<op>.rs` bindings.
 *
 *   - NEW upstream ops we don't bind yet  → candidates to build (the pipeline
 *     should file a subtask + an agent binds it, exposes it in the palette).
 *   - Bindings with no matching upstream fn → possibly removed/renamed upstream
 *     (our binding may be stale after a rev bump).
 *
 * Run on a schedule (and after any `lore` rev bump). Output is JSON on stdout
 * plus a human summary on stderr; pass `--json` for machine consumption.
 *
 * Heuristic by nature (upstream has internal helper fns; we have `*_local`
 * naming variants) — it produces a review list, not a hard gate.
 */
import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";
import { homedir } from "node:os";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, ".."); // scripts -> repo root
const opsDir = join(repoRoot, "crates", "lore-vm", "src", "ops");

/** Internal upstream fns that are not user-facing ops (excluded from the diff). */
const UPSTREAM_IGNORE = new Set([
  "close_all_handles",
  "close_for_connection",
]);

/**
 * Known-internal upstream ops by full `<domain>.<fn>` id. The `layer/` subdir
 * holds low-level impl fns (`add`/`list`/`remove`) that the public `layer.rs`
 * facade (`layer_add`/`layer_list`/…, which we DO bind) wraps — so they are not
 * separate ops. Add new entries here only with a justification.
 */
const KNOWN_INTERNAL_IDS = new Set([
  "layer.add",
  "layer.list",
  "layer.remove",
]);

/**
 * Upstream modules that are internal plumbing, not part of the op API surface
 * we mirror (analytics, low-level call/remote RPC, logging, etc.). The diff is
 * scoped to the domains we actually bind.
 */
const OP_DOMAINS = new Set([
  "auth",
  "branch",
  "dependency",
  "file",
  "layer",
  "link",
  "lock",
  "notification",
  "repository",
  "revision",
  "service",
  "shared_store",
  "storage",
]);

/** Read the pinned lore git rev from Cargo.lock. */
function pinnedRev() {
  const lock = readFileSync(join(repoRoot, "Cargo.lock"), "utf8");
  const block = lock.split(/\n\[\[package\]\]/).find((b) =>
    /name = "lore"\n/.test(b),
  );
  if (!block) throw new Error('no [[package]] name = "lore" in Cargo.lock');
  const m = block.match(/source = ".*lore\.git\?rev=([0-9a-f]+)#/);
  if (!m) throw new Error("could not parse lore rev from Cargo.lock source");
  return m[1];
}

/** Locate the cargo git checkout for `rev` (cargo names the dir by 7-char rev). */
function loreSrcDir(rev) {
  const envOverride = process.env.LORE_SRC;
  if (envOverride && existsSync(join(envOverride, "lore", "src"))) {
    return join(envOverride, "lore", "src");
  }
  const base = join(homedir(), ".cargo", "git", "checkouts");
  const candidates = [];
  if (existsSync(base)) {
    for (const repo of readdirSync(base)) {
      if (!repo.startsWith("lore-")) continue;
      const repoDir = join(base, repo);
      for (const short of readdirSync(repoDir)) {
        if (rev.startsWith(short) || short.startsWith(rev.slice(0, 7))) {
          const src = join(repoDir, short, "lore", "src");
          if (existsSync(src)) candidates.push(src);
        }
      }
    }
  }
  return candidates[0] ?? null;
}

/** Map a path under lore/src to its domain (first segment, sans `.rs`). */
function domainOf(relPath) {
  const seg = relPath.split("/")[0];
  return seg.endsWith(".rs") ? seg.slice(0, -3) : seg;
}

/** Enumerate upstream ops as a set of "<domain>.<fn>". */
function upstreamOps(srcDir) {
  const ops = new Set();
  const walk = (dir, rel) => {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name);
      const r = rel ? `${rel}/${name}` : name;
      if (statSync(p).isDirectory()) walk(p, r);
      else if (name.endsWith(".rs") && !r.includes("test")) {
        const domain = domainOf(r);
        if (!OP_DOMAINS.has(domain)) continue;
        const src = readFileSync(p, "utf8");
        for (const m of src.matchAll(/^pub async fn ([a-z_][a-z0-9_]*)\s*[(<]/gm)) {
          const fn = m[1];
          if (UPSTREAM_IGNORE.has(fn)) continue;
          const id = `${domain}.${fn}`;
          if (KNOWN_INTERNAL_IDS.has(id)) continue;
          ops.add(id);
        }
      }
    }
  };
  walk(srcDir, "");
  return ops;
}

/** Enumerate our bindings as a set of "<domain>.<op>". */
function ourOps() {
  const ops = new Set();
  for (const domain of readdirSync(opsDir)) {
    const dpath = join(opsDir, domain);
    if (!statSync(dpath).isDirectory()) continue;
    for (const f of readdirSync(dpath)) {
      if (f.endsWith(".rs") && f !== "mod.rs") {
        ops.add(`${domain}.${f.slice(0, -3)}`);
      }
    }
  }
  return ops;
}

const rev = pinnedRev();
const srcDir = loreSrcDir(rev);
if (!srcDir) {
  console.error(
    `Could not locate upstream lore source for rev ${rev.slice(0, 12)}.\n` +
      `Run \`cargo fetch\` first, or set LORE_SRC=/path/to/lore-checkout.`,
  );
  process.exit(2);
}

const upstream = upstreamOps(srcDir);
const ours = ourOps();

// Compare by fn/op name within a domain. Upstream and our domain names line up
// (repository, branch, revision, file, storage, auth, …). `*_local` variants and
// deferred spike ops (cherry_pick/bisect) are reported but expected.
const newOps = [...upstream].filter((o) => !ours.has(o)).sort();
const orphanedBindings = [...ours].filter((o) => !upstream.has(o)).sort();

const report = {
  rev,
  upstreamOpCount: upstream.size,
  ourOpCount: ours.size,
  newOps, // upstream ops we don't bind yet → build these
  orphanedBindings, // our bindings with no matching upstream fn → review
};

if (process.argv.includes("--json")) {
  console.log(JSON.stringify(report, null, 2));
} else {
  console.error(`upstream lore parity @ ${rev.slice(0, 12)}`);
  console.error(
    `  upstream ops: ${upstream.size} · our bindings: ${ours.size}`,
  );
  console.error(`  NEW upstream ops not bound (${newOps.length}):`);
  for (const o of newOps) console.error(`    + ${o}`);
  console.error(`  bindings with no upstream match (${orphanedBindings.length}):`);
  for (const o of orphanedBindings) console.error(`    ? ${o}`);
  console.log(JSON.stringify(report));
}
