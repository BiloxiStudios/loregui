#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

function fail(message) {
  console.error(`release-supply-chain: ${message}`);
  process.exit(1);
}

function stagedFiles(stageDir, excludedName) {
  let entries;
  try {
    entries = readdirSync(stageDir);
  } catch (error) {
    fail(`cannot read stage directory ${stageDir}: ${error.message}`);
  }

  return entries
    .filter((name) => name !== excludedName)
    .filter((name) => statSync(join(stageDir, name)).isFile())
    .sort();
}

const [command, stageDir, triple] = process.argv.slice(2);
if (!command || !stageDir) {
  fail("usage: release-supply-chain.mjs <assert-nonempty|checksums> <stage-dir> [target-triple]");
}

if (command === "assert-nonempty") {
  if (stagedFiles(stageDir).length === 0) {
    fail(`no staged release subjects in ${stageDir}`);
  }
  process.exit(0);
}

if (command === "checksums") {
  if (!triple || !/^[A-Za-z0-9._-]+$/.test(triple)) {
    fail("checksums requires a safe target triple");
  }

  const manifestName = `SHA256SUMS-${triple}`;
  const files = stagedFiles(stageDir, manifestName);
  if (files.length === 0) {
    fail(`no staged release subjects in ${stageDir}`);
  }

  const lines = files.map((name) => {
    const digest = createHash("sha256").update(readFileSync(join(stageDir, name))).digest("hex");
    return `${digest}  ${name}`;
  });
  writeFileSync(join(stageDir, manifestName), `${lines.join("\n")}\n`, "utf8");
  console.log(`wrote ${basename(join(stageDir, manifestName))} for ${files.length} subject(s)`);
  process.exit(0);
}

fail(`unknown command: ${command}`);
