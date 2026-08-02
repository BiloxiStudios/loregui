#!/usr/bin/env node
/**
 * Trigger contract test for .github/workflows/upstream-parity.yml (SBAI-5978).
 *
 * The upstream-parity workflow exists to validate lore pin bumps. Its
 * pull_request paths filter MUST include every pin-bearing surface so the
 * canary job actually runs on the PR that changes the pin.
 *
 * Pin-bearing surfaces:
 *   - Cargo.toml    (lore git dependency rev)
 *   - Cargo.lock    (resolved lore + quinn-proto revs)
 *
 * This is the same discipline applied to release.yml in SBAI-5840, where
 * six successive fail-open designs were rejected before a byte-exact
 * canonical pin held.
 *
 * This test:
 *   1. Parses the workflow YAML (no external deps — minimal parser)
 *   2. Extracts the pull_request.paths list
 *   3. Asserts every pin-bearing surface is present
 *   4. Asserts no forbidden paths are present (e.g. wildcards that would
 *      trigger on every PR, defeating the filter's purpose)
 */
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const workflowPath = join(here, "..", ".github", "workflows", "upstream-parity.yml");

function assert(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
  console.error(`ok — ${msg}`);
}

// --- Minimal YAML paths extractor (no external deps) ---
// We only need the pull_request.paths list, which is a flat list of strings
// under `on: > pull_request: > paths:`. A full YAML parser is overkill.
function extractPullRequestPaths(yaml) {
  const lines = yaml.split("\n");
  const paths = [];
  let inPullRequestPaths = false;

  for (const line of lines) {
    // Detect `pull_request:` under the `on:` block (2-space indent)
    if (/^  pull_request:\s*$/.test(line)) {
      inPullRequestPaths = false;
      continue;
    }
    // Detect `paths:` immediately after pull_request (4-space indent)
    if (!inPullRequestPaths && /^    paths:\s*$/.test(line)) {
      inPullRequestPaths = true;
      continue;
    }
    // Collect path entries (6-space indent list items under paths)
    if (inPullRequestPaths) {
      const match = line.match(/^      -\s+(.+?)\s*$/);
      if (match) {
        paths.push(match[1].replace(/^["']|["']$/g, ""));
      } else if (!line.startsWith(" ") || line.trim() === "") {
        // Left-dedent or blank → end of paths block
        if (line.trim() !== "" && !line.startsWith(" ")) {
          break;
        }
        // If we hit another key at 4-space indent (same level as paths), done
        if (/^    \w/.test(line)) {
          inPullRequestPaths = false;
        }
      }
    }
  }

  return paths;
}

const yaml = readFileSync(workflowPath, "utf8");

// Must have on: block
assert(yaml.includes("on:"), "workflow declares on: trigger block");

// Must have pull_request
assert(yaml.includes("pull_request:"), "workflow declares pull_request trigger");

const paths = extractPullRequestPaths(yaml);
assert(paths.length > 0, `pull_request.paths is non-empty (${paths.length} entries)`);

// --- Pin-bearing surface assertions ---

const PIN_BEARING_SURFACES = ["Cargo.toml", "Cargo.lock"];

for (const surface of PIN_BEARING_SURFACES) {
  assert(
    paths.includes(surface),
    `pin-bearing surface "${surface}" is in pull_request.paths`,
  );
}

// --- Existing parity scripts must still be present ---

const PARITY_SCRIPTS = [
  "scripts/upstream-lore-distance.mjs",
  "scripts/upstream-lore-distance.test.mjs",
  "scripts/upstream-lore-parity.mjs",
  "scripts/upstream-lore-parity.test.mjs",
  ".github/workflows/upstream-parity.yml",
];

for (const script of PARITY_SCRIPTS) {
  assert(
    paths.includes(script),
    `parity script "${script}" is in pull_request.paths`,
  );
}

// --- No overly-broad wildcards ---

const hasWildcard = paths.some((p) => p.includes("*"));
assert(
  !hasWildcard,
  "pull_request.paths contains no wildcards (avoids triggering on every PR)",
);

// --- No src-tauri / frontend paths (those are outside parity scope) ---

const forbiddenPatterns = ["src-tauri/", "frontend/", "packages/"];
for (const pattern of forbiddenPatterns) {
  const found = paths.some((p) => p.includes(pattern));
  assert(
    !found,
    `pull_request.paths does not include broad pattern "${pattern}"`,
  );
}

console.error(
  `upstream-parity trigger contract passed: ${paths.length} paths, all pin-bearing surfaces covered`,
);
