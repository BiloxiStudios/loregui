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

/**
 * Construct the ONLY permitted head lock from the base lock: apply the 13
 * exact source substitutions plus the one exact loregui -> lore-credential
 * insertion, then byte-compare the whole file.
 *
 * Review finding on f096255: a global line-multiset comparison is
 * context-blind — moving an existing edge between packages (e.g. lore-base
 * from lore-notification to loregui) preserves the multiset and passed the
 * advertised zero-churn proof. Byte-comparing a constructed expectation
 * cannot miss a relocation, reordering, or any other edit.
 */
function permittedHeadLock(base) {
  const oldSource = `source = "${OLD_SOURCE_PREFIX}${OLD_REV}#${OLD_REV}"`;
  const newSource = `source = "${NEW_SOURCE}"`;
  const occurrences = base.split(oldSource).length - 1;
  assert.equal(
    occurrences,
    13,
    `base lock must carry exactly 13 lore-tree sources, found ${occurrences}`,
  );
  let out = base.split(oldSource).join(newSource);

  // The single permitted structural change: loregui gains a direct
  // lore-credential edge, inserted in cargo's sorted position.
  const marker = '\nname = "loregui"\nversion = ';
  const at = out.indexOf(marker);
  assert.notEqual(at, -1, "base lock must contain the loregui package");
  const depsAt = out.indexOf("dependencies = [\n", at);
  assert.notEqual(depsAt, -1, "loregui package must have a dependencies list");
  const depsEnd = out.indexOf("\n]", depsAt);
  const depsBlock = out.slice(depsAt, depsEnd);
  assert.ok(
    !depsBlock.includes('"lore-credential"'),
    "base lock must not already carry the edge",
  );
  const lines = depsBlock.split("\n");
  const head = lines[0];
  const entries = lines.slice(1);
  entries.push(' "lore-credential",');
  // Cargo orders these by byte value, not locale (locale collation ignores
  // punctuation and would put "serde_json" before "serde").
  entries.sort((a, b) => (a.trim() < b.trim() ? -1 : a.trim() > b.trim() ? 1 : 0));
  out = out.slice(0, depsAt) + [head, ...entries].join("\n") + out.slice(depsEnd);
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
