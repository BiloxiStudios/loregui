#!/usr/bin/env node
/**
 * Upstream lore API-parity detector (Enhanced).
 *
 * Keeps LoreGUI in parity with Epic's `lore` crate: it enumerates the op surface
 * of the upstream `lore` source (every `pub async fn` in `lore/src/`) and diffs
 * it against our `crates/lore-vm/src/ops/<domain>/<op>.rs` bindings. It also
 * records shared interfaces consumed by every operation (currently
 * `LoreGlobalArgs`) so schema-only changes cannot hide outside op signatures.
 *
 * It also supports comparing a "head" source (e.g. latest lore HEAD) against
 * the "pinned" source (the version we currently use) to detect signature drift.
 *
 * SBAI-5906: Reports two commit-distance metrics:
 *   - `productDrift`: pinned rev → upstream HEAD (the true parity distance).
 *   - `incrementalUpstreamMovement`: previous checkpoint → upstream HEAD
 *     (watcher-to-watcher movement, NOT the parity distance).
 * These are reported separately; the watcher MUST NOT present incremental
 * movement as parity distance.
 *
 * Run on a schedule (and after any `lore` rev bump). Output is JSON on stdout
 * plus a human summary on stderr; pass `--json` for machine consumption.
 */
import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";
import { homedir } from "node:os";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, ".."); // scripts -> repo root
const opsDir = join(repoRoot, "crates", "lore-vm", "src", "ops");

/**
 * Shared interfaces consumed by every bound operation. These do not appear in
 * op argument signatures, so a field added here used to evade parity checks.
 */
const SHARED_SCHEMA_SOURCES = {
  LoreGlobalArgs: ["lore-revision", "src", "interface.rs"],
};

/** Internal upstream fns that are not user-facing ops (excluded from the diff). */
const UPSTREAM_IGNORE = new Set([
  "close_all_handles",
  "close_for_connection",
]);

/**
 * Known-internal upstream ops by full `<domain>.<fn>` id.
 */
const KNOWN_INTERNAL_IDS = new Set([
  "layer.add",
  "layer.list",
  "layer.remove",
]);

/**
 * LoreGUI-owned bindings that intentionally have no upstream primitive.
 * These are NOT drift — they are documented compatibility stubs or derived
 * composites. Real orphans (deleted upstream ops still bound, or accidental
 * leftovers) must still surface in `orphanedBindings`.
 *
 * Do NOT blanket-ignore orphan detection: only the ids listed here with an
 * explicit classification are filtered. Adding a new binding without an
 * upstream match still fails the scan.
 *
 * Classifications:
 *   - `compatibility-stub` — typed LoreGUI surface kept while upstream lacks
 *     the primitive/event; returns a clear error until unblocked.
 *   - `derived-composite` — LoreGUI convenience op composed from real upstream
 *     primitives (not a 1:1 binding).
 *
 * SBAI-5473.
 */
const KNOWN_INTENTIONAL_ORPHANS = {
  "lock.file_message_send": "compatibility-stub",
  "revision.activity_report": "derived-composite",
};

/**
 * Upstream modules that are internal plumbing, not part of the op API surface.
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
  const lockPath = join(repoRoot, "Cargo.lock");
  if (!existsSync(lockPath)) return null;
  const lock = readFileSync(lockPath, "utf8");
  const block = lock.split(/\n\[\[package\]\]/).find((b) =>
    /name = "lore"\n/.test(b),
  );
  if (!block) return null;
  const m = block.match(/source = ".*lore\.git\?rev=([0-9a-f]+)#/);
  if (!m) return null;
  return m[1];
}

/** Locate the cargo git checkout for `rev`. */
function loreSrcDir(rev) {
  const envOverride = process.env.LORE_SRC;
  if (envOverride && existsSync(join(envOverride, "lore", "src"))) {
    return join(envOverride, "lore", "src");
  }
  const base = join(homedir(), ".cargo", "git", "checkouts");
  if (existsSync(base)) {
    for (const repo of readdirSync(base)) {
      if (!repo.startsWith("lore-")) continue;
      const repoDir = join(base, repo);
      for (const short of readdirSync(repoDir)) {
        if (rev.startsWith(short) || short.startsWith(rev.slice(0, 7))) {
          const src = join(repoDir, short, "lore", "src");
          if (existsSync(src)) return src;
        }
      }
    }
  }
  return null;
}

/** Map a path under lore/src to its domain. */
function domainOf(relPath) {
  const seg = relPath.split("/")[0];
  return seg.endsWith(".rs") ? seg.slice(0, -3) : seg;
}

/**
 * Enumerate upstream ops and their signatures.
 * Returns Map<id, { argsType, resultType, fields: { [name]: type } }>
 */
function collectSignatures(srcDir) {
  const signatures = new Map();
  const structs = new Map();

  const walk = (dir, rel) => {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name);
      const r = rel ? `${rel}/${name}` : name;
      if (statSync(p).isDirectory()) walk(p, r);
      else if (name.endsWith(".rs") && !r.includes("test")) {
        const domain = domainOf(r);
        if (!OP_DOMAINS.has(domain)) continue;
        const src = readFileSync(p, "utf8");

        // Extract structs
        const structRegex = /pub struct ([A-Za-z0-9_]+)\s*\{([\s\S]*?)\}/g;
        const fieldRegex = /pub ([a-z_][a-z0-9_]*):\s*([A-Za-z0-9_<>, ]+)/g;
        let sMatch;
        while ((sMatch = structRegex.exec(src)) !== null) {
          const sName = sMatch[1];
          const sBody = sMatch[2];
          const fields = new Map();
          let fMatch;
          while ((fMatch = fieldRegex.exec(sBody)) !== null) {
            fields.set(fMatch[1], fMatch[2].trim());
          }
          structs.set(sName, fields);
        }

        // Extract fns
        const fnRegex = /pub async fn ([a-z_][a-z0-9_]*)\s*\(([\s\S]*?)\)\s*(?:->\s*([^{]+))?\s*\{/g;
        const argRegex = /args:\s*([A-Za-z0-9_]+)/;
        let fMatch;
        while ((fMatch = fnRegex.exec(src)) !== null) {
          const fName = fMatch[1];
          if (UPSTREAM_IGNORE.has(fName)) continue;
          const fArgs = fMatch[2];
          const fRet = (fMatch[3] || '()').trim();
          const argMatch = argRegex.exec(fArgs);
          const argsType = argMatch ? argMatch[1] : null;
          const id = `${domain}.${fName}`;
          if (KNOWN_INTERNAL_IDS.has(id)) continue;
          signatures.set(id, { argsType, resultType: fRet });
        }
      }
    }
  };

  walk(srcDir, "");

  // Link structs to fns
  for (const [id, sig] of signatures) {
    if (sig.argsType && structs.has(sig.argsType)) {
      sig.fields = Object.fromEntries(structs.get(sig.argsType));
    } else {
      sig.fields = {};
    }
  }

  return signatures;
}

/** Extract selected shared structs from the upstream checkout containing lore/src. */
function collectSharedSchemas(srcDir) {
  const upstreamRoot = resolve(srcDir, "..", "..");
  const schemas = new Map();

  for (const [id, segments] of Object.entries(SHARED_SCHEMA_SOURCES)) {
    const sourcePath = join(upstreamRoot, ...segments);
    if (!existsSync(sourcePath)) {
      schemas.set(id, { source: segments.join("/"), fields: null });
      continue;
    }
    const source = readFileSync(sourcePath, "utf8");
    const struct = source.match(
      new RegExp(`pub\\s+struct\\s+${id}\\s*\\{([\\s\\S]*?)^\\}`, "m"),
    );
    if (!struct) {
      schemas.set(id, { source: segments.join("/"), fields: null });
      continue;
    }
    const fields = {};
    const fieldRegex = /^\s*pub\s+([a-z_][a-z0-9_]*)\s*:\s*([^,\n]+),/gm;
    let field;
    while ((field = fieldRegex.exec(struct[1])) !== null) {
      fields[field[1]] = field[2].trim();
    }
    schemas.set(id, { source: segments.join("/"), fields });
  }

  return schemas;
}

/** Enumerate our bindings as a set of "<domain>.<op>". */
function ourOps() {
  const ops = new Set();
  if (!existsSync(opsDir)) return ops;
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

// ── SBAI-5906: git commit-distance calculations ─────────────────────────

/** Resolve a short SHA to full 40-char hex in a git repo. */
function resolveSha(repoDir, sha) {
  try {
    return execSync(`git -C "${repoDir}" rev-parse "${sha}"`, {
      encoding: "utf8",
    }).trim();
  } catch {
    return null;
  }
}

/** Return how many commits `a` is behind `b` (a..b range count). */
function commitsAhead(repoDir, a, b) {
  try {
    const out = execSync(
      `git -C "${repoDir}" rev-list --count "${a}..${b}"`,
      { encoding: "utf8" },
    ).trim();
    return parseInt(out, 10);
  } catch {
    return null;
  }
}

/** Determine the ancestry relationship between two SHAs in a repo. */
function ancestryStatus(repoDir, a, b) {
  // Is `a` an ancestor of `b`?
  try {
    execSync(`git -C "${repoDir}" merge-base --is-ancestor "${a}" "${b}"`, {
      stdio: "pipe",
    });
    return "a_is_ancestor_of_b"; // a is ancestor of b (b is ahead)
  } catch {
    // Not an ancestor — check the reverse
  }
  try {
    execSync(`git -C "${repoDir}" merge-base --is-ancestor "${b}" "${a}"`, {
      stdio: "pipe",
    });
    return "b_is_ancestor_of_b"; // b is ancestor of a (a is ahead)
  } catch {
    // Neither is ancestor of the other
  }
  // Check if they share any history at all
  try {
    const mergeBase = execSync(
      `git -C "${repoDir}" merge-base "${a}" "${b}"`,
      { encoding: "utf8" },
    ).trim();
    if (mergeBase === a || mergeBase === b) {
      // one is ancestor — already handled above
    }
    return "diverged"; // both have commits the other doesn't
  } catch {
    return "no_common_ancestor";
  }
}

/**
 * Calculate the commit distance between two SHAs.
 * Returns { ahead, behind, ancestry, status } or { error } on failure.
 */
function commitDistance(repoDir, pinnedSha, headSha) {
  if (!pinnedSha || !headSha) {
    return { error: "missing_sha", detail: "one or both SHAs are null" };
  }
  if (pinnedSha === headSha) {
    return {
      ahead: 0,
      behind: 0,
      ancestry: "identical",
      status: "LOCKSTEP",
    };
  }

  const ancestry = ancestryStatus(repoDir, pinnedSha, headSha);

  if (ancestry === "a_is_ancestor_of_b") {
    // pinned is ancestor of head → we are behind
    const behind = commitsAhead(repoDir, pinnedSha, headSha);
    return {
      ahead: 0,
      behind,
      ancestry: "pinned_is_ancestor",
      status: behind === null ? "UNKNOWN" : "BEHIND",
      securityRangeUrl: `https://github.com/EpicGames/lore/compare/${pinnedSha}...${headSha}`,
    };
  }
  if (ancestry === "b_is_ancestor_of_b") {
    // head is ancestor of pinned → we are ahead
    const ahead = commitsAhead(repoDir, headSha, pinnedSha);
    return {
      ahead,
      behind: 0,
      ancestry: "head_is_ancestor",
      status: ahead === null ? "UNKNOWN" : "AHEAD",
      securityRangeUrl: `https://github.com/EpicGames/lore/compare/${headSha}...${pinnedSha}`,
    };
  }
  if (ancestry === "diverged") {
    const pinnedAhead = commitsAhead(repoDir, headSha, pinnedSha);
    const headAhead = commitsAhead(repoDir, pinnedSha, headSha);
    return {
      ahead: pinnedAhead,
      behind: headAhead,
      ancestry: "diverged",
      status: "DIVERGED",
      securityRangeUrl: `https://github.com/EpicGames/lore/compare/${pinnedSha}...${headSha}`,
    };
  }

  return {
    error: "no_common_ancestor",
    status: "UNKNOWN",
    detail: "pinned and head share no commit history",
  };
}

/**
 * Build a commit-distance report comparing the product pin against
 * the upstream HEAD, plus the incremental movement since the last checkpoint.
 *
 * Acceptance (SBAI-5906): two-distance payload — product drift AND
 * incremental upstream movement, NEVER conflated.
 */
function buildCommitDistanceReport(repoDir, pinnedSha, headSha, checkpointSha) {
  const report = {};

  // ── Product drift: pinned rev → upstream HEAD ──
  if (!pinnedSha) {
    report.productDrift = {
      status: "ERROR",
      detail: "no pinned rev found in Cargo.toml/Cargo.lock",
      actionable: true,
    };
  } else if (!headSha) {
    report.productDrift = {
      status: "ERROR",
      detail: "no upstream HEAD SHA provided (--head-git or --head-sha required)",
      actionable: true,
    };
  } else {
    const fullPinned = resolveSha(repoDir, pinnedSha);
    const fullHead = resolveSha(repoDir, headSha);
    if (!fullPinned) {
      report.productDrift = {
        status: "ERROR",
        detail: `pinned SHA ${pinnedSha} not found in repo`,
        actionable: true,
      };
    } else if (!fullHead) {
      report.productDrift = {
        status: "ERROR",
        detail: `upstream HEAD SHA ${headSha} not found in repo`,
        actionable: true,
      };
    } else {
      report.productDrift = {
        pinnedSha: fullPinned,
        headSha: fullHead,
        ...commitDistance(repoDir, fullPinned, fullHead),
      };
    }
  }

  // ── Incremental upstream movement: checkpoint → HEAD ──
  // SBAI-5906: This is watcher bookkeeping, NOT parity distance.
  if (checkpointSha) {
    const fullCheckpoint = resolveSha(repoDir, checkpointSha);
    const fullHead = headSha ? resolveSha(repoDir, headSha) : null;
    if (!fullCheckpoint) {
      report.incrementalUpstreamMovement = {
        status: "ERROR",
        detail: `checkpoint SHA ${checkpointSha} not found in repo`,
      };
    } else if (!fullHead) {
      report.incrementalUpstreamMovement = {
        status: "UNKNOWN",
        detail: "upstream HEAD not available",
      };
    } else {
      const movement = commitDistance(repoDir, fullCheckpoint, fullHead);
      report.incrementalUpstreamMovement = {
        checkpointSha: fullCheckpoint,
        headSha: fullHead,
        ...movement,
      };
    }
  } else {
    report.incrementalUpstreamMovement = null;
  }

  return report;
}

const args = process.argv.slice(2);
const headSrcPath = args.find((a) => a === "--head-src")
  ? args[args.indexOf("--head-src") + 1]
  : null;
const headGitPath = args.find((a) => a === "--head-git")
  ? args[args.indexOf("--head-git") + 1]
  : null;
const headSha = args.find((a) => a === "--head-sha")
  ? args[args.indexOf("--head-sha") + 1]
  : null;
const checkpointSha = args.find((a) => a === "--checkpoint")
  ? args[args.indexOf("--checkpoint") + 1]
  : null;

const rev = pinnedRev();
const pinnedDir = loreSrcDir(rev);

if (!pinnedDir) {
  console.error(
    `Could not locate pinned upstream lore source.\n` +
    `Run \`cargo fetch\` first, or set LORE_SRC.`
  );
  process.exit(2);
}

// ── SBAI-5906: resolve upstream HEAD SHA for commit-distance ──
// Priority: --head-sha > --head-git (git rev-parse HEAD) > null
let upstreamHeadSha = headSha || null;
if (!upstreamHeadSha && headGitPath && existsSync(join(headGitPath, ".git"))) {
  upstreamHeadSha = resolveSha(headGitPath, "HEAD");
}

const pinnedSigs = collectSignatures(pinnedDir);
const headSigs = headSrcPath ? collectSignatures(headSrcPath) : pinnedSigs;
const pinnedSharedSchemas = collectSharedSchemas(pinnedDir);
const headSharedSchemas = headSrcPath
  ? collectSharedSchemas(headSrcPath)
  : pinnedSharedSchemas;
const ours = ourOps();

const newOps = [];
const driftedOps = [];
const orphanedBindings = [];
const driftedSharedSchemas = [];

// Compare head vs ours
for (const [id, sig] of headSigs) {
  if (!ours.has(id)) {
    newOps.push({ id, sig });
  } else if (headSrcPath) {
    // Check for drift against pinned
    const pinnedSig = pinnedSigs.get(id);
    if (pinnedSig && JSON.stringify(pinnedSig) !== JSON.stringify(sig)) {
      driftedOps.push({ id, oldSig: pinnedSig, newSig: sig });
    }
  }
}

const intentionalOrphans = [];

for (const id of ours) {
  if (!headSigs.has(id)) {
    if (KNOWN_INTENTIONAL_ORPHANS[id]) {
      intentionalOrphans.push({
        id,
        classification: KNOWN_INTENTIONAL_ORPHANS[id],
      });
    } else {
      orphanedBindings.push(id);
    }
  }
}

for (const [id, newSchema] of headSharedSchemas) {
  const oldSchema = pinnedSharedSchemas.get(id);
  if (oldSchema && JSON.stringify(oldSchema) !== JSON.stringify(newSchema)) {
    driftedSharedSchemas.push({ id, oldSchema, newSchema });
  }
}

// SBAI-5906: Build commit-distance report (two-distance payload)
const commitDist = headGitPath
  ? buildCommitDistanceReport(headGitPath, rev, upstreamHeadSha, checkpointSha)
  : { productDrift: { status: "NO_UPSTREAM_GIT", detail: "--head-git not provided; commit distance unavailable" }, incrementalUpstreamMovement: null };

const report = {
  rev,
  pinnedOpCount: pinnedSigs.size,
  headOpCount: headSigs.size,
  ourOpCount: ours.size,
  newOps: newOps.sort((a, b) => a.id.localeCompare(b.id)),
  driftedOps: driftedOps.sort((a, b) => a.id.localeCompare(b.id)),
  orphanedBindings: orphanedBindings.sort(),
  intentionalOrphans: intentionalOrphans.sort((a, b) => a.id.localeCompare(b.id)),
  sharedSchemas: Object.fromEntries(headSharedSchemas),
  driftedSharedSchemas: driftedSharedSchemas.sort((a, b) =>
    a.id.localeCompare(b.id),
  ),
  // SBAI-5906: Two-distance commit payload
  productDrift: commitDist.productDrift,
  incrementalUpstreamMovement: commitDist.incrementalUpstreamMovement,
};

if (process.argv.includes("--json")) {
  console.log(JSON.stringify(report, null, 2));
} else {
  console.error(`upstream lore parity @ ${rev?.slice(0, 12) || "unknown"}`);
  console.error(`  pinned ops: ${pinnedSigs.size} · our bindings: ${ours.size}`);
  if (headSrcPath) console.error(`  head ops: ${headSigs.size}`);

  // SBAI-5906: Two-distance commit report
  const pd = report.productDrift;
  if (pd?.status === "BEHIND") {
    console.error(
      `  PRODUCT DRIFT: ${pd.behind} commits behind ` +
        `(${pd.pinnedSha?.slice(0, 7)} → ${pd.headSha?.slice(0, 7)}) ` +
        `[${pd.securityRangeUrl}]`,
    );
  } else if (pd?.status === "AHEAD") {
    console.error(
      `  PRODUCT DRIFT: ${pd.ahead} commits ahead ` +
        `(${pd.pinnedSha?.slice(0, 7)} → ${pd.headSha?.slice(0, 7)})`,
    );
  } else if (pd?.status === "LOCKSTEP") {
    console.error(`  PRODUCT DRIFT: lockstep (pin == upstream HEAD)`);
  } else if (pd?.status === "DIVERGED") {
    console.error(
      `  PRODUCT DRIFT: DIVERGED — ${pd.ahead ?? "?"} ahead / ${pd.behind ?? "?"} behind ` +
        `[${pd.securityRangeUrl}]`,
    );
  } else if (pd?.status?.startsWith("ERROR") || pd?.status === "UNKNOWN") {
    console.error(
      `  PRODUCT DRIFT: ${pd.status} — ${pd.detail || pd.error || "see JSON"}`,
    );
  }

  const ium = report.incrementalUpstreamMovement;
  if (ium) {
    if (ium.status === "BEHIND") {
      console.error(
        `  INCREMENTAL MOVEMENT: +${ium.behind} commits ` +
          `(${ium.checkpointSha?.slice(0, 7)} → ${ium.headSha?.slice(0, 7)})`,
      );
    } else if (ium.status === "LOCKSTEP") {
      console.error(`  INCREMENTAL MOVEMENT: none (checkpoint == HEAD)`);
    } else {
      console.error(
        `  INCREMENTAL MOVEMENT: ${ium.status} — ${ium.detail || ium.error || "see JSON"}`,
      );
    }
  }

  console.error(`  NEW upstream ops not bound (${newOps.length}):`);
  for (const o of newOps) console.error(`    + ${o.id} (${o.sig.argsType})`);

  console.error(`  DRIFTED ops signatures (${driftedOps.length}):`);
  for (const o of driftedOps) console.error(`    ! ${o.id} (signature changed)`);

  console.error(`  DRIFTED shared schemas (${driftedSharedSchemas.length}):`);
  for (const schema of driftedSharedSchemas) {
    console.error(`    ! ${schema.id} (shared schema changed)`);
  }

  console.error(`  bindings with no upstream match (${orphanedBindings.length}):`);
  for (const o of orphanedBindings) console.error(`    ? ${o}`);

  console.error(
    `  intentional orphans (classified, not drift) (${intentionalOrphans.length}):`,
  );
  for (const o of intentionalOrphans) {
    console.error(`    ~ ${o.id} [${o.classification}]`);
  }

  console.log(JSON.stringify(report));
}
