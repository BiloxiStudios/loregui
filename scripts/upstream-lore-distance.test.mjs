#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  buildCommitDistanceReport,
  commitDistance,
  readPinnedRevisions,
} from "./upstream-lore-distance.mjs";

function git(repo, ...args) {
  return execFileSync("git", ["-C", repo, ...args], {
    encoding: "utf8",
  }).trim();
}

function commit(repo, message) {
  writeFileSync(join(repo, "value.txt"), message);
  git(repo, "add", "value.txt");
  git(repo, "commit", "-m", message);
  return git(repo, "rev-parse", "HEAD");
}

function makeRepo(root) {
  const repo = join(root, "history");
  mkdirSync(repo);
  git(repo, "init", "-b", "main");
  git(repo, "config", "user.name", "SBAI-5906 fixture");
  git(repo, "config", "user.email", "fixture@example.invalid");
  const commits = [];
  for (let index = 0; index < 28; index += 1) {
    commits.push(commit(repo, `commit-${index}`));
  }
  return { repo, commits };
}

function writePins(root, revisions) {
  const url = "https://github.com/EpicGames/lore.git";
  writeFileSync(
    join(root, "Cargo.toml"),
    `[workspace.dependencies]\nlore = { git = "${url}", rev = "${revisions.manifestLore}" }\n\n[patch.crates-io]\nquinn-proto = { git = "${url}", rev = "${revisions.manifestQuinnProto}" }\n`,
  );
  writeFileSync(
    join(root, "Cargo.lock"),
    `[[package]]\nname = "lore"\nversion = "0.1.0"\nsource = "git+${url}?rev=${revisions.lockLore}#${revisions.lockLore}"\n\n[[package]]\nname = "quinn-proto"\nversion = "0.11.0"\nsource = "git+${url}?rev=${revisions.lockQuinnProto}#${revisions.lockQuinnProto}"\n`,
  );
}

const root = mkdtempSync(join(tmpdir(), "lore-distance-"));
try {
  const { repo, commits } = makeRepo(root);
  const report = buildCommitDistanceReport(
    repo,
    commits[0],
    commits[27],
    commits[26],
  );
  assert.equal(report.productDrift.status, "BEHIND");
  assert.equal(report.productDrift.behind, 27);
  assert.equal(report.incrementalUpstreamMovement.status, "BEHIND");
  assert.equal(report.incrementalUpstreamMovement.behind, 1);

  assert.equal(commitDistance(repo, commits[27], commits[0]).status, "AHEAD");
  assert.equal(commitDistance(repo, "missing", commits[27]).status, "ERROR");

  git(repo, "checkout", "-b", "diverged", commits[10]);
  const divergent = commit(repo, "divergent");
  assert.equal(commitDistance(repo, commits[27], divergent).status, "DIVERGED");

  const pinsRoot = join(root, "pins");
  mkdirSync(pinsRoot);
  const matching = {
    manifestLore: commits[0],
    manifestQuinnProto: commits[0],
    lockLore: commits[0],
    lockQuinnProto: commits[0],
  };
  writePins(pinsRoot, matching);
  assert.equal(readPinnedRevisions(pinsRoot).revision, commits[0]);
  writePins(pinsRoot, { ...matching, manifestQuinnProto: commits[1] });
  assert.throws(() => readPinnedRevisions(pinsRoot), /revision mismatch/);
} finally {
  rmSync(root, { recursive: true, force: true });
}

console.error("upstream lore distance tests passed: product +27, incremental +1");

// SBAI-5910/5905 regression: the product pin may live on the BiloxiStudios
// maintenance fork (it carries credential hardening upstream has not taken).
// A host-hardcoded reader rejected that as "not pinned to an exact lore
// revision" and turned the parity watcher RED — see detect FAILURE on PR
// #450. Pin READING must accept either host; drift is still measured against
// EpicGames upstream.
{
  const { mkdtempSync, writeFileSync } = await import("node:fs");
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const { readPinnedRevisions } = await import("./upstream-lore-distance.mjs");
  const assert = (await import("node:assert/strict")).default;

  const REV = "2052749e36e1127c520a191b18141e23980b89d7";
  for (const host of [
    "https://github.com/EpicGames/lore.git",
    "https://github.com/BiloxiStudios/lore.git",
  ]) {
    const root = mkdtempSync(join(tmpdir(), "lore-pinhost-"));
    writeFileSync(
      join(root, "Cargo.toml"),
      `lore = { git = "${host}", rev = "${REV}" }\n` +
        `quinn-proto = { git = "${host}", rev = "${REV}" }\n`,
    );
    writeFileSync(
      join(root, "Cargo.lock"),
      `[[package]]\nname = "lore"\nsource = "git+${host}?rev=${REV}#${REV}"\n\n` +
        `[[package]]\nname = "quinn-proto"\nsource = "git+${host}?rev=${REV}#${REV}"\n`,
    );
    assert.equal(readPinnedRevisions(root).revision, REV, `pin on ${host} must read`);
  }
  console.log("pin-host regression passed: upstream and maintenance-fork pins both read");
}
