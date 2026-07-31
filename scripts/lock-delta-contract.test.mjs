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

/** Lines present in `a` but not `b`, preserving duplicates. */
function removed(a, b) {
  const counts = new Map();
  for (const line of b.split("\n")) counts.set(line, (counts.get(line) ?? 0) + 1);
  const out = [];
  for (const line of a.split("\n")) {
    const n = counts.get(line) ?? 0;
    if (n > 0) counts.set(line, n - 1);
    else out.push(line);
  }
  return out;
}

/** Parse `[[package]]` blocks into {name, deps[]}. */
function packages(lock) {
  const out = [];
  let current = null;
  let inDeps = false;
  for (const raw of lock.split("\n")) {
    const line = raw.trim();
    if (line === "[[package]]") {
      if (current) out.push(current);
      current = { name: "", deps: [] };
      inDeps = false;
    } else if (current && line.startsWith("name = \"")) {
      current.name = line.slice(8, -1);
    } else if (current && line.startsWith("dependencies = [")) {
      inDeps = true;
    } else if (inDeps && line === "]") {
      inDeps = false;
    } else if (inDeps && line.startsWith('"')) {
      current.deps.push(line.replace(/^"|",?$/g, "").split(" ")[0]);
    }
  }
  if (current) out.push(current);
  return out;
}

function packagesDependingOn(lock, dep) {
  return packages(lock)
    .filter((p) => p.deps.includes(dep))
    .map((p) => p.name);
}

function countEdgeIn(lock, pkgName, dep) {
  const pkg = packages(lock).find((p) => p.name === pkgName);
  if (!pkg) throw new Error(`package ${pkgName} not found in lock`);
  return pkg.deps.filter((d) => d === dep).length;
}

test("lock delta is exactly 13 source repins + one direct lore-credential edge", () => {
  const base = baseLock();
  const head = currentLock();

  const added = removed(head, base).map((l) => l.trim()).filter(Boolean);
  const dropped = removed(base, head).map((l) => l.trim()).filter(Boolean);

  const addedSources = added.filter((l) => l.startsWith("source = "));
  const droppedSources = dropped.filter((l) => l.startsWith("source = "));
  assert.equal(
    addedSources.length,
    13,
    `expected exactly 13 new source lines, got ${addedSources.length}: ${addedSources.join(" | ")}`,
  );
  assert.ok(
    addedSources.every((l) => l === `source = "${NEW_SOURCE}"`),
    "every new source line must be the accepted product source",
  );
  assert.equal(droppedSources.length, 13, "exactly 13 old source lines must be replaced");
  assert.ok(
    droppedSources.every((l) => l.includes(OLD_SOURCE_PREFIX)),
    "the replaced lines must all be the previous upstream pin",
  );

  // The ONLY other addition is the direct loregui -> lore-credential edge.
  const otherAdded = added.filter((l) => !l.startsWith("source = "));
  assert.deepEqual(
    otherAdded,
    ['"lore-credential",'],
    `only the direct lore-credential edge may be added; got: ${otherAdded.join(" | ")}`,
  );

  // ...and a global line count is not enough (review correction): prove the
  // edge sits INSIDE the loregui package's dependency block, exactly once,
  // and that no other package gained it.
  const owners = packagesDependingOn(head, "lore-credential");
  assert.ok(
    owners.includes("loregui"),
    `loregui must declare the direct lore-credential edge; owners: ${owners.join(", ")}`,
  );
  assert.equal(
    countEdgeIn(head, "loregui", "lore-credential"),
    1,
    "loregui must declare the lore-credential edge exactly once",
  );
  const baseOwners = packagesDependingOn(base, "lore-credential");
  assert.deepEqual(
    owners.filter((o) => !baseOwners.includes(o)),
    ["loregui"],
    `only loregui may gain the lore-credential edge; new owners: ${owners.join(", ")}`,
  );

  // Nothing else may be removed: no registry/resolver edge churn.
  const otherDropped = dropped.filter((l) => !l.startsWith("source = "));
  assert.deepEqual(
    otherDropped,
    [],
    `no registry or resolver edge churn is permitted; got: ${otherDropped.join(" | ")}`,
  );
});

test("no package resolves from a stale or split lore source", () => {
  const head = currentLock();
  const stale = head
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.startsWith("source = ") && l.includes("/lore.git?rev=") && l !== `source = "${NEW_SOURCE}"`);
  assert.deepEqual(stale, [], `stale or split lore sources: ${stale.join(" | ")}`);
});
