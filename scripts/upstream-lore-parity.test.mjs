#!/usr/bin/env node
/**
 * Policy tests for scripts/upstream-lore-parity.mjs intentional-orphan
 * classification (SBAI-5473).
 *
 * Ensures:
 *   - lock.file_message_send is classified compatibility-stub
 *   - revision.activity_report is classified derived-composite
 *   - real unbound upstream ops still appear in newOps
 *   - unknown orphans still appear in orphanedBindings (no blanket ignore)
 *   - shared LoreGlobalArgs schema drift is reported and enforcement-tested
 */
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  cpSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";

const here = dirname(fileURLToPath(import.meta.url));
const script = join(here, "upstream-lore-parity.mjs");
const source = readFileSync(script, "utf8");

function assert(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
  console.error(`ok — ${msg}`);
}

// Static policy: classifications must be documented in source.
assert(
  source.includes('KNOWN_INTENTIONAL_ORPHANS'),
  "scanner defines KNOWN_INTENTIONAL_ORPHANS",
);
assert(
  /"lock\.file_message_send"\s*:\s*"compatibility-stub"/.test(source),
  "lock.file_message_send classified compatibility-stub",
);
assert(
  /"revision\.activity_report"\s*:\s*"derived-composite"/.test(source),
  "revision.activity_report classified derived-composite",
);
assert(
  source.includes("Do NOT blanket-ignore orphan detection"),
  "scanner documents non-blanket orphan policy",
);

// Live scan against the pinned checkout (requires cargo fetch / LORE_SRC).
const run = spawnSync(process.execPath, [script, "--json"], {
  encoding: "utf8",
  env: process.env,
});
if (run.status !== 0) {
  console.error(run.stderr || run.stdout);
  console.error(
    "SKIP live scan assertions (could not locate pinned lore source — run cargo fetch)",
  );
  process.exit(0);
}

const report = JSON.parse(run.stdout);
const intentional = report.intentionalOrphans || [];
const byId = Object.fromEntries(intentional.map((o) => [o.id, o.classification]));

assert(
  byId["lock.file_message_send"] === "compatibility-stub",
  "live report classifies lock.file_message_send as compatibility-stub",
);
assert(
  byId["revision.activity_report"] === "derived-composite",
  "live report classifies revision.activity_report as derived-composite",
);
assert(
  !(report.orphanedBindings || []).includes("lock.file_message_send"),
  "lock.file_message_send is not a raw orphan",
);
assert(
  !(report.orphanedBindings || []).includes("revision.activity_report"),
  "revision.activity_report is not a raw orphan",
);
// After SBAI-5473 bindings land, newOps should not list the four mutable ops.
const newIds = (report.newOps || []).map((o) => o.id);
for (const id of [
  "storage.mutable_store",
  "storage.mutable_load",
  "storage.mutable_list",
  "storage.mutable_compare_and_swap",
]) {
  assert(!newIds.includes(id), `${id} is bound (not in newOps)`);
}

const globalSchema = report.sharedSchemas?.LoreGlobalArgs;
assert(globalSchema, "live report records shared LoreGlobalArgs schema");
assert(
  globalSchema.fields?.working_directory === "LoreString",
  "live report records LoreGlobalArgs.working_directory",
);
assert(
  (report.driftedSharedSchemas || []).length === 0,
  "pinned source has no shared-schema drift against itself",
);

// Demonstrate enforcement: remove only working_directory from a copy of the
// exact pinned shared interface. The scanner must report the shared-schema
// change even though no operation signature changed.
function pinnedLoreRoot(rev) {
  if (process.env.LORE_SRC) {
    return resolve(process.env.LORE_SRC);
  }
  const checkouts = join(homedir(), ".cargo", "git", "checkouts");
  for (const repository of readdirSync(checkouts)) {
    if (!repository.startsWith("lore-")) continue;
    for (const checkout of readdirSync(join(checkouts, repository))) {
      if (rev.startsWith(checkout)) {
        return join(checkouts, repository, checkout);
      }
    }
  }
  return null;
}

const pinnedRoot = pinnedLoreRoot(report.rev);
assert(pinnedRoot, "located exact pinned checkout for seeded schema drift");
const fixtureRoot = mkdtempSync(join(tmpdir(), "lore-parity-schema-"));
try {
  const fixtureLoreSrc = join(fixtureRoot, "lore", "src");
  const fixtureRevisionSrc = join(fixtureRoot, "lore-revision", "src");
  cpSync(join(pinnedRoot, "lore", "src"), fixtureLoreSrc, { recursive: true });
  cpSync(
    join(pinnedRoot, "lore-revision", "src"),
    fixtureRevisionSrc,
    { recursive: true },
  );
  const interfacePath = join(fixtureRevisionSrc, "interface.rs");
  const originalInterface = readFileSync(interfacePath, "utf8");
  const seededInterface = originalInterface.replace(
    /\n\s*\/\/\/ Directory that relative paths[\s\S]*?\n\s*pub working_directory:\s*LoreString,\n/,
    "\n",
  );
  assert(
    seededInterface !== originalInterface,
    "seed removed LoreGlobalArgs.working_directory from fixture",
  );
  writeFileSync(interfacePath, seededInterface);

  const seededRun = spawnSync(
    process.execPath,
    [script, "--head-src", fixtureLoreSrc, "--json"],
    { encoding: "utf8", env: process.env },
  );
  assert(seededRun.status === 0, "seeded shared-schema scan completes");
  const seededReport = JSON.parse(seededRun.stdout);
  const globalDrift = (seededReport.driftedSharedSchemas || []).find(
    (entry) => entry.id === "LoreGlobalArgs",
  );
  assert(globalDrift, "seeded LoreGlobalArgs drift is reported");
  assert(
    globalDrift.oldSchema.fields.working_directory === "LoreString" &&
      !("working_directory" in globalDrift.newSchema.fields),
    "seeded drift identifies removed working_directory field",
  );
} finally {
  rmSync(fixtureRoot, { recursive: true, force: true });
}

console.error("upstream-lore-parity policy tests passed");
