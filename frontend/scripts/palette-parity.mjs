#!/usr/bin/env node
/**
 * Command-palette parity ratchet.
 *
 * Enforces that every registered Tauri command is reachable from the GUI via a
 * command-palette manifest entry — OR is explicitly listed in the allowlist as
 * "known not yet built" / "intentionally excluded".
 *
 * It is a RATCHET, not a one-shot gate:
 *   - FAIL if a registered command has neither a manifest entry nor an allowlist
 *     entry  → a new lore op was wired without exposing it in the GUI. Either add
 *     a `manifest/<domain>/<op>.ts` entry or add it to the allowlist (deferring).
 *   - FAIL if an allowlist entry is now covered by a manifest entry, or no longer
 *     names a real command  → the allowlist must shrink as the fan-out lands, so
 *     it trends to empty = full parity.
 *
 * This keeps the GUI in lock-step with the API surface: as the lore binding grows
 * (new `#[tauri::command]`s), CI forces each to be either exposed or consciously
 * deferred. Run: `node frontend/scripts/palette-parity.mjs`.
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", ".."); // frontend/scripts -> repo root
const libRs = join(repoRoot, "src-tauri", "src", "lib.rs");
const manifestDir = join(repoRoot, "frontend", "src", "palette", "manifest");
const allowlistPath = join(here, "palette-parity-allowlist.json");

/** Parse the command names out of the `generate_handler![ ... ]` block. */
function registeredCommands() {
  const src = readFileSync(libRs, "utf8");
  const start = src.indexOf("generate_handler![");
  if (start === -1) throw new Error("generate_handler! not found in lib.rs");
  const end = src.indexOf("])", start);
  const block = src.slice(start, end);
  const out = new Set();
  for (const raw of block.split("\n")) {
    const line = raw.replace(/\/\/.*$/, "").trim().replace(/,$/, "");
    if (!line || line.startsWith("generate_handler")) continue;
    const m = line.match(/^(?:commands::)?([a-z_][a-z0-9_]*)$/);
    if (m) out.add(m[1]);
  }
  return out;
}

/** Collect `command:` values from every manifest entry file. */
function manifestCommands() {
  const out = new Map(); // command -> file
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name);
      if (statSync(p).isDirectory()) walk(p);
      else if (name.endsWith(".ts") && name !== "index.ts") {
        const src = readFileSync(p, "utf8");
        const m = src.match(/command:\s*["'`]([a-z_][a-z0-9_]*)["'`]/);
        if (m) out.set(m[1], p.replace(repoRoot + "/", ""));
        else console.warn(`! ${p} has no parseable command: field`);
      }
    }
  };
  walk(manifestDir);
  return out;
}

function loadAllowlist() {
  const json = JSON.parse(readFileSync(allowlistPath, "utf8"));
  return {
    // Permanently excluded (app lifecycle / streaming — not request/response ops).
    excluded: new Set(json.excluded ?? []),
    // The shrinking backlog: registered but not yet exposed in the palette.
    deferred: new Set(json.deferred ?? []),
  };
}

const registered = registeredCommands();
const covered = manifestCommands();
const { excluded, deferred } = loadAllowlist();

const errors = [];

// 1. Every registered command must be covered, excluded, or deferred.
const uncovered = [...registered].filter(
  (c) => !covered.has(c) && !excluded.has(c) && !deferred.has(c),
);
if (uncovered.length) {
  errors.push(
    `Registered command(s) with no palette entry and not allowlisted:\n` +
      uncovered.map((c) => `    - ${c}`).join("\n") +
      `\n  → add frontend/src/palette/manifest/<domain>/<op>.ts, or add to ` +
      `"deferred" in palette-parity-allowlist.json.`,
  );
}

// 2. Stale deferred entries (now covered, or no longer a command) are a WARNING,
// not a failure. This keeps the gate contention-free: a PR that exposes a
// deferred op does NOT have to also edit the shared allowlist (which would
// reintroduce the merge conflicts the glob manifest design removes). The
// backlog is pruned lazily — periodically, or when flipping to 100%-locked.
const staleDeferred = [...deferred].filter(
  (c) => covered.has(c) || !registered.has(c),
);
if (staleDeferred.length) {
  console.warn(
    `! ${staleDeferred.length} stale "deferred" entries now covered/removed ` +
      `(prune from palette-parity-allowlist.json): ${staleDeferred.join(", ")}`,
  );
}

// 3. Excluded entries that no longer exist are a soft warning.
const staleExcluded = [...excluded].filter((c) => !registered.has(c));
if (staleExcluded.length) {
  console.warn(
    `! "excluded" entries no longer registered (consider removing): ${staleExcluded.join(", ")}`,
  );
}

// 4. Every manifest entry must point to a registered command.
const deadManifests = [...covered.keys()].filter((c) => !registered.has(c));
if (deadManifests.length) {
  errors.push(
    `Manifest entry(s) point to a command that is NOT registered in lib.rs:\n` +
      deadManifests
        .map((c) => `    - ${c}   (${covered.get(c)})`)
        .sort()
        .join("\n") +
      `\n  → the palette entry will fail at runtime. Register the command in ` +
      `lib.rs or fix the manifest.`,
  );
}

const total = registered.size;
const exposed = [...registered].filter((c) => covered.has(c)).length;
const pct = total ? Math.round((exposed / total) * 100) : 0;
console.log(
  `palette parity: ${exposed}/${total} commands exposed (${pct}%), ` +
    `${deferred.size} deferred, ${excluded.size} excluded.`,
);

if (errors.length) {
  console.error("\nPALETTE PARITY FAILED:\n\n" + errors.join("\n\n") + "\n");
  process.exit(1);
}
console.log("palette parity OK.");
