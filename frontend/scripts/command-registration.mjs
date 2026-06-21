#!/usr/bin/env node
/**
 * Tauri command-registration guard.
 *
 * Catches the recurring bug where a `#[tauri::command]` is written in
 * `src-tauri/src/**` but never added to `lib.rs`'s `generate_handler![ ... ]`.
 * Such a command compiles fine (it's just dead code — a warning at most), but it
 * is UNREACHABLE at runtime: any `invoke("that_command")` from the frontend
 * fails, silently breaking panels and palette entries that target it.
 *
 * Fails CI if any `#[tauri::command]` fn is not registered. Run:
 *   node frontend/scripts/command-registration.mjs
 *
 * REVERSE GUARD (manifest -> registered): also fails CI if any palette manifest
 * entry's `command:` value names a command that is NOT registered in lib.rs's
 * `generate_handler![ ... ]` (and is not in the parity allowlist's `excluded`).
 * `CommandPalette.tsx` invokes that string verbatim, so an unregistered name is
 * a DEAD ROW: clicking it throws at runtime. The forward parity gate only checks
 * registered -> manifest, so it cannot catch a manifest entry pointing at a
 * misnamed or never-registered command. This guard closes that gap.
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const tauriSrc = join(repoRoot, "src-tauri", "src");
const libRs = join(tauriSrc, "lib.rs");
const manifestDir = join(repoRoot, "frontend", "src", "palette", "manifest");
const allowlistPath = join(here, "palette-parity-allowlist.json");

/** Commands registered in the `generate_handler![ ... ]` block. */
function registered() {
  const src = readFileSync(libRs, "utf8");
  const start = src.indexOf("generate_handler![");
  if (start === -1) throw new Error("generate_handler! not found in lib.rs");
  const end = src.indexOf("])", start);
  const out = new Set();
  for (const raw of src.slice(start, end).split("\n")) {
    const line = raw.replace(/\/\/.*$/, "").trim().replace(/,$/, "");
    if (!line || line.startsWith("generate_handler")) continue;
    const m = line.match(/^(?:commands::)?([a-z_][a-z0-9_]*)$/);
    if (m) out.add(m[1]);
  }
  return out;
}

/** Every `#[tauri::command]` fn under src-tauri/src, with its file. */
function commandFns() {
  const found = new Map(); // name -> "file:line"
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name);
      if (statSync(p).isDirectory()) walk(p);
      else if (name.endsWith(".rs")) {
        const src = readFileSync(p, "utf8");
        // #[tauri::command] (+ any further attrs) then `pub [async] fn <name>`
        const re =
          /#\[tauri::command\][^\n]*\n(?:\s*#\[[^\n]*\]\n)*\s*pub (?:async )?fn ([a-z_][a-z0-9_]*)/g;
        let m;
        while ((m = re.exec(src))) {
          const before = src.slice(0, m.index).split("\n").length;
          found.set(m[1], `${p.replace(repoRoot + "/", "")}:${before}`);
        }
      }
    }
  };
  walk(tauriSrc);
  return found;
}

/**
 * Every `command:` value declared by a palette manifest entry, mapped to the
 * file(s) that declare it. A single file may legitimately declare only one
 * command, but we scan ALL occurrences (matchAll) so a stray second `command:`
 * cannot hide.
 */
function manifestCommands() {
  const found = new Map(); // command -> Set("file" ...)
  const re = /command:\s*["'`]([a-z_][a-z0-9_]*)["'`]/g;
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name);
      if (statSync(p).isDirectory()) walk(p);
      else if (name.endsWith(".ts") && name !== "index.ts") {
        const src = readFileSync(p, "utf8");
        for (const m of src.matchAll(re)) {
          const rel = p.replace(repoRoot + "/", "");
          if (!found.has(m[1])) found.set(m[1], new Set());
          found.get(m[1]).add(rel);
        }
      }
    }
  };
  walk(manifestDir);
  return found;
}

/** Parity allowlist `excluded` set (commands intentionally out of the palette). */
function excludedCommands() {
  const json = JSON.parse(readFileSync(allowlistPath, "utf8"));
  return new Set(json.excluded ?? []);
}

const reg = registered();
const cmds = commandFns();
const unregistered = [...cmds.keys()].filter((c) => !reg.has(c)).sort();

console.log(
  `command registration: ${cmds.size} #[tauri::command] fns, ${reg.size} registered.`,
);

// Reverse guard: manifest command: -> must be a registered command or excluded.
const manifestCmds = manifestCommands();
const excluded = excludedCommands();
const deadRows = [...manifestCmds.entries()]
  .filter(([cmd]) => !reg.has(cmd) && !excluded.has(cmd))
  .sort(([a], [b]) => a.localeCompare(b));

console.log(
  `palette manifest: ${manifestCmds.size} distinct command: values referenced.`,
);

let failed = false;

if (unregistered.length) {
  console.error(
    "\nCOMMAND REGISTRATION FAILED:\n\n" +
      "These #[tauri::command] fns are NOT in lib.rs generate_handler! — they are\n" +
      "unreachable at runtime (any invoke of them fails). Register each in the\n" +
      "generate_handler![ ... ] list:\n" +
      unregistered.map((c) => `    - ${c}   (${cmds.get(c)})`).join("\n") +
      "\n",
  );
  failed = true;
}

if (deadRows.length) {
  console.error(
    "\nPALETTE MANIFEST -> COMMAND CHECK FAILED (dead palette rows):\n\n" +
      "These palette manifest entries point a `command:` at a name that is NOT\n" +
      "registered in lib.rs generate_handler! and is NOT in the parity allowlist's\n" +
      "`excluded`. CommandPalette.tsx invokes the string verbatim, so each row\n" +
      "throws at runtime. Fix the manifest `command:` to the real registered name,\n" +
      "add+register the missing #[tauri::command], delete the manifest entry, or\n" +
      "add the command to `excluded` in palette-parity-allowlist.json:\n" +
      deadRows
        .map(([cmd, files]) => `    - ${cmd}   (${[...files].join(", ")})`)
        .join("\n") +
      "\n",
  );
  failed = true;
}

if (failed) process.exit(1);
console.log("command registration OK.");
console.log("palette manifest -> command OK (no dead rows).");
