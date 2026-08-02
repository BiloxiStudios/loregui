#!/usr/bin/env node
/**
 * SBAI-5910 — EXECUTABLE proof of the claimed minimal lock delta.
 *
 * The review found the Rust-side check overclaimed: verifying that all lore
 * source lines are the accepted one does NOT prove the PR delta is exactly
 * "13 source replacements + one direct edge with zero registry churn". This
 * contract computes the delta against the exact base commit and enforces that
 * shape, so resolver churn cannot creep back in on a future repin.
 */
import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import assert from "node:assert/strict";
import { ACCEPTED_HOST, ACCEPTED_REV } from "./lore-pin-policy.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
/** The pre-5910 base: the merge that landed SBAI-5840's trigger control. */
const BASE = "0284b3e7";
const OLD_SOURCE_PREFIX = "git+https://github.com/EpicGames/lore.git?rev=";
const OLD_REV = "9664606f5a4708606642a6670a57d16bd3d37596";
const NEW_SOURCE = `git+${ACCEPTED_HOST}?rev=${ACCEPTED_REV}#${ACCEPTED_REV}`;

/**
 * Per-bump DECLARATIONS of the permitted delta.
 *
 * These constants are *supposed* to change on every parity bump, and changing
 * them is exactly the reviewable act: you cannot drop a crate or gain an edge
 * without saying so here. The construction below is generic, so the next bump
 * edits DATA and not logic — the distinction that keeps this from becoming the
 * kind of version-keyed landmine that stalls automatic parity.
 *
 * Counts are asserted rather than discovered: a base that stops carrying the
 * expected number of repins or bumps means the base moved under us, which must
 * fail loudly instead of silently constructing a different "permitted" lock.
 */
const SOURCE_REPINS = 13;
const OLD_LORE_VERSION = "0.8.6-nightly";
const NEW_LORE_VERSION = "0.8.7-nightly";
const LORE_VERSION_BUMPS = 9;
/** Edges gained. `[dependent, dependency]`, each to a crate already present. */
const ADDED_EDGES = [
  ["loregui", "lore-credential"], // SBAI-5910: direct credential edge
  ["lore", "uuid"], // SBAI-5905: upstream 0.8.7
  ["lore-macro", "proc-macro2"], // SBAI-5905: upstream 0.8.7
];
/** Edges dropped upstream when the compute pool and mmap reads were removed. */
const REMOVED_EDGES = [
  ["lore-base", "rayon"],
  ["lore-revision", "memmap2"],
  ["lore-storage", "memmap2"],
];
/** Crates that leave the graph entirely as a result of those edge removals. */
const REMOVED_PACKAGES = ["memmap2", "rayon", "rayon-core"];

function baseLock() {
  return execFileSync("git", ["show", `${BASE}:Cargo.lock`], {
    cwd: repoRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
}

function currentLock() {
  return execFileSync("git", ["show", "HEAD:Cargo.lock"], {
    cwd: repoRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
}

/** Locate a package's `dependencies = [...]` span, scoped to that package. */
function depsSpan(text, pkg) {
  const at = text.indexOf(`\nname = "${pkg}"\n`);
  assert.notEqual(at, -1, `lock must contain the ${pkg} package`);
  const nextPkg = text.indexOf("\n[[package]]", at);
  const limit = nextPkg === -1 ? text.length : nextPkg;
  const depsAt = text.indexOf("dependencies = [\n", at);
  assert.ok(
    depsAt !== -1 && depsAt < limit,
    `${pkg} must have a dependencies list`,
  );
  return { depsAt, depsEnd: text.indexOf("\n]", depsAt) };
}

function addEdge(text, pkg, dep) {
  const { depsAt, depsEnd } = depsSpan(text, pkg);
  const lines = text.slice(depsAt, depsEnd).split("\n");
  assert.ok(
    !lines.some((l) => l.trim() === `"${dep}",`),
    `${pkg} must not already depend on ${dep}`,
  );
  const head = lines[0];
  const entries = lines.slice(1);
  entries.push(` "${dep}",`);
  // Cargo orders these by byte value, not locale (locale collation ignores
  // punctuation and would put "serde_json" before "serde").
  entries.sort((a, b) => (a.trim() < b.trim() ? -1 : a.trim() > b.trim() ? 1 : 0));
  return text.slice(0, depsAt) + [head, ...entries].join("\n") + text.slice(depsEnd);
}

function removeEdge(text, pkg, dep) {
  const { depsAt, depsEnd } = depsSpan(text, pkg);
  const lines = text.slice(depsAt, depsEnd).split("\n");
  const idx = lines.findIndex((l) => l.trim() === `"${dep}",`);
  assert.notEqual(idx, -1, `${pkg} must depend on ${dep} before it is removed`);
  lines.splice(idx, 1);
  return text.slice(0, depsAt) + lines.join("\n") + text.slice(depsEnd);
}

function removePackage(text, pkg) {
  const needle = `\n[[package]]\nname = "${pkg}"\n`;
  const start = text.indexOf(needle);
  assert.notEqual(start, -1, `lock must contain ${pkg} before it is removed`);
  const next = text.indexOf("\n[[package]]", start + needle.length);
  return text.slice(0, start) + (next === -1 ? "" : text.slice(next));
}

/**
 * Construct the ONLY permitted head lock from the base lock by applying every
 * declared change above — source repins, lore version bumps, edge additions,
 * edge removals, package removals — then byte-compare the whole file.
 *
 * Review finding on f096255: a global line-multiset comparison is
 * context-blind — moving an existing edge between packages (e.g. lore-base
 * from lore-notification to loregui) preserves the multiset and passed the
 * advertised zero-churn proof. Byte-comparing a constructed expectation
 * cannot miss a relocation, reordering, or any other edit.
 *
 * SBAI-5905: this originally modelled only "13 repins + one edge", so the
 * 0.8.7 parity bump failed it even though the bump was exactly what the PR
 * claimed. The fix was to model the delta the parity mandate actually
 * produces, not to relax the byte comparison — every change is still declared
 * and still enforced to the byte.
 */
function permittedHeadLock(base) {
  const oldSource = `source = "${OLD_SOURCE_PREFIX}${OLD_REV}#${OLD_REV}"`;
  const newSource = `source = "${NEW_SOURCE}"`;
  const sources = base.split(oldSource).length - 1;
  assert.equal(
    sources,
    SOURCE_REPINS,
    `base lock must carry exactly ${SOURCE_REPINS} lore-tree sources, found ${sources}`,
  );
  let out = base.split(oldSource).join(newSource);

  const oldVersion = `version = "${OLD_LORE_VERSION}"`;
  const bumps = out.split(oldVersion).length - 1;
  assert.equal(
    bumps,
    LORE_VERSION_BUMPS,
    `base lock must carry exactly ${LORE_VERSION_BUMPS} lore crates at ${OLD_LORE_VERSION}, found ${bumps}`,
  );
  out = out.split(oldVersion).join(`version = "${NEW_LORE_VERSION}"`);

  for (const [pkg, dep] of ADDED_EDGES) out = addEdge(out, pkg, dep);
  for (const [pkg, dep] of REMOVED_EDGES) out = removeEdge(out, pkg, dep);
  for (const pkg of REMOVED_PACKAGES) out = removePackage(out, pkg);
  return out;
}

/**
 * SHARED enforcement: the same checker guards the repository head AND the
 * adversarial fixtures (review finding on 430d216 — the context-swap case
 * only asserted `swapped !== permitted`, so a refactor could weaken real
 * enforcement while the advertised must-reject fixture stayed green).
 *
 * @returns {{ok: true} | {ok: false, reason: string}}
 */
export function checkLockCandidate(base, candidate) {
  let permitted;
  try {
    permitted = permittedHeadLock(base);
  } catch (error) {
    return { ok: false, reason: `cannot construct the permitted lock: ${error.message}` };
  }
  if (candidate === permitted) return { ok: true };
  const c = candidate.split("\n");
  const p = permitted.split("\n");
  let i = 0;
  while (i < Math.min(c.length, p.length) && c[i] === p[i]) i += 1;
  return {
    ok: false,
    reason:
      `lock diverges from the only permitted construction at line ${i + 1}: ` +
      `permitted ${JSON.stringify(p[i] ?? null)}, actual ${JSON.stringify(c[i] ?? null)}`,
  };
}

test("head lock is byte-identical to the only permitted construction from base", () => {
  const base = baseLock();
  const head = currentLock();
  const verdict = checkLockCandidate(base, head);
  assert.equal(verdict.ok, true, verdict.reason);
  const permitted = permittedHeadLock(base);
  if (head !== permitted) {
    // Show the first divergence rather than dumping the whole lock.
    const h = head.split("\n");
    const p = permitted.split("\n");
    let i = 0;
    while (i < Math.min(h.length, p.length) && h[i] === p[i]) i += 1;
    assert.fail(
      `lock diverges from the only permitted construction at line ${i + 1}:\n` +
        `  permitted: ${p[i] ?? "<eof>"}\n  actual:    ${h[i] ?? "<eof>"}`,
    );
  }
});

test("an adversarial context swap is rejected", () => {
  // The reviewer's reproduction: relocate an existing edge between packages.
  // The multiset is unchanged; the bytes are not.
  const base = baseLock();
  const permitted = permittedHeadLock(base);
  const swapped = (() => {
    const start = permitted.indexOf('[[package]]\nname = "lore-notification"');
    const end = permitted.indexOf("\n[[package]]", start + 1);
    let block = permitted.slice(start, end);
    if (!block.includes(' "lore-base",\n')) return null;
    block = block.replace(' "lore-base",\n', "");
    let out = permitted.slice(0, start) + block + permitted.slice(end);
    const gStart = out.indexOf('[[package]]\nname = "loregui"');
    const gEnd = out.indexOf("\n[[package]]", gStart + 1);
    let gBlock = out.slice(gStart, gEnd);
    gBlock = gBlock.replace("dependencies = [\n", 'dependencies = [\n "lore-base",\n');
    return out.slice(0, gStart) + gBlock + out.slice(gEnd);
  })();
  assert.ok(swapped, "fixture precondition: lore-notification depends on lore-base");
  // Feed the adversarial candidate through the SAME checker that guards the
  // repository head, and require a NAMED rejection — not merely inequality.
  const verdict = checkLockCandidate(base, swapped);
  assert.equal(verdict.ok, false, "a relocated edge must be rejected");
  assert.match(
    verdict.reason,
    /diverges from the only permitted construction at line \d+/,
    `rejection must name the divergence; got: ${verdict.reason}`,
  );
});

test("churn beyond the declared delta is still rejected", () => {
  // SBAI-5905 widened this contract to model version bumps, edge changes and
  // package removals. Widening what a guard permits is the moment it can
  // quietly become a blanket allowance, so prove the permitted set is exactly
  // the DECLARED one: each mutation below is the same *kind* of change the
  // contract now sanctions, differing only in not having been declared.
  const base = baseLock();
  const permitted = permittedHeadLock(base);

  const cases = [
    [
      "an undeclared package removal",
      () => removePackage(permitted, "vergen"),
    ],
    [
      "an undeclared edge addition",
      () => addEdge(permitted, "loregui", "libc"),
    ],
    [
      "an undeclared edge removal",
      () => removeEdge(permitted, "lore", "uuid"),
    ],
    [
      "an undeclared version bump",
      () =>
        permitted.replace(
          `version = "${NEW_LORE_VERSION}"`,
          'version = "0.8.8-nightly"',
        ),
    ],
  ];

  for (const [label, mutate] of cases) {
    const candidate = mutate();
    assert.notEqual(candidate, permitted, `fixture precondition: ${label} must alter the lock`);
    const verdict = checkLockCandidate(base, candidate);
    assert.equal(verdict.ok, false, `${label} must be rejected`);
    assert.match(
      verdict.reason,
      /diverges from the only permitted construction at line \d+/,
      `${label} must be named, not merely refused; got: ${verdict.reason}`,
    );
  }
});

test("no package resolves from a stale or split lore source", () => {
  const head = currentLock();
  const stale = head
    .split("\n")
    .map((l) => l.trim())
    .filter(
      (l) =>
        l.startsWith("source = ") &&
        l.includes("/lore.git?rev=") &&
        l !== `source = "${NEW_SOURCE}"`,
    );
  assert.deepEqual(stale, [], `stale or split lore sources: ${stale.join(" | ")}`);
});
