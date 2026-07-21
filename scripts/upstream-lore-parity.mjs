#!/usr/bin/env node
/**
 * Upstream lore API-parity detector (Enhanced).
 *
 * Keeps LoreGUI in parity with Epic's `lore` crate: it enumerates the op surface
 * of the upstream `lore` source (every `pub async fn` in `lore/src/`) and diffs
 * it against our `crates/lore-vm/src/ops/<domain>/<op>.rs` bindings.
 *
 * It also supports comparing a "head" source (e.g. latest lore HEAD) against
 * the "pinned" source (the version we currently use) to detect signature drift.
 *
 * Run on a schedule (and after any `lore` rev bump). Output is JSON on stdout
 * plus a human summary on stderr; pass `--json` for machine consumption.
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

const args = process.argv.slice(2);
const headSrcPath = args.find((a, i) => a === "--head-src") ? args[args.indexOf("--head-src") + 1] : null;

const rev = pinnedRev();
const pinnedDir = loreSrcDir(rev);

if (!pinnedDir) {
  console.error(
    `Could not locate pinned upstream lore source.\n` +
    `Run \`cargo fetch\` first, or set LORE_SRC.`
  );
  process.exit(2);
}

const pinnedSigs = collectSignatures(pinnedDir);
const headSigs = headSrcPath ? collectSignatures(headSrcPath) : pinnedSigs;
const ours = ourOps();

const newOps = [];
const driftedOps = [];
const orphanedBindings = [];

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

const report = {
  rev,
  pinnedOpCount: pinnedSigs.size,
  headOpCount: headSigs.size,
  ourOpCount: ours.size,
  newOps: newOps.sort((a, b) => a.id.localeCompare(b.id)),
  driftedOps: driftedOps.sort((a, b) => a.id.localeCompare(b.id)),
  orphanedBindings: orphanedBindings.sort(),
  intentionalOrphans: intentionalOrphans.sort((a, b) => a.id.localeCompare(b.id)),
};

if (process.argv.includes("--json")) {
  console.log(JSON.stringify(report, null, 2));
} else {
  console.error(`upstream lore parity @ ${rev?.slice(0, 12) || "unknown"}`);
  console.error(`  pinned ops: ${pinnedSigs.size} · our bindings: ${ours.size}`);
  if (headSrcPath) console.error(`  head ops: ${headSigs.size}`);

  console.error(`  NEW upstream ops not bound (${newOps.length}):`);
  for (const o of newOps) console.error(`    + ${o.id} (${o.sig.argsType})`);

  console.error(`  DRIFTED ops signatures (${driftedOps.length}):`);
  for (const o of driftedOps) console.error(`    ! ${o.id} (signature changed)`);

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
