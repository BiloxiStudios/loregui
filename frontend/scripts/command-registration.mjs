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
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const tauriSrc = join(repoRoot, "src-tauri", "src");
const libRs = join(tauriSrc, "lib.rs");

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

const reg = registered();
const cmds = commandFns();
const unregistered = [...cmds.keys()].filter((c) => !reg.has(c)).sort();

console.log(
  `command registration: ${cmds.size} #[tauri::command] fns, ${reg.size} registered.`,
);

if (unregistered.length) {
  console.error(
    "\nCOMMAND REGISTRATION FAILED:\n\n" +
      "These #[tauri::command] fns are NOT in lib.rs generate_handler! — they are\n" +
      "unreachable at runtime (any invoke of them fails). Register each in the\n" +
      "generate_handler![ ... ] list:\n" +
      unregistered.map((c) => `    - ${c}   (${cmds.get(c)})`).join("\n") +
      "\n",
  );
  process.exit(1);
}
console.log("command registration OK.");
