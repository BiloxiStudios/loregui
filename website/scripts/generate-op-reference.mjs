#!/usr/bin/env node
/**
 * Build step: generate the docs op-reference data from the LoreGUI command-palette
 * manifests.
 *
 * The palette manifests (`frontend/src/palette/manifest/<domain>/<op>.ts`) are the
 * single source of truth for every op the GUI can invoke — its id, label,
 * description, and argument FieldSpecs. The `/docs/op-reference` page is generated
 * from them so the documentation tracks the real API automatically rather than
 * drifting from a hand-maintained list.
 *
 * This mirrors the pattern `lore-mcp/generate_catalog.py` uses to build the MCP
 * tool catalog: parse the plain-data TS object literal in each manifest file,
 * normalise it to JSON, and emit a combined, domain-grouped catalog. Here the
 * output is a typed TS data module (`website/src/lib/op-reference.generated.ts`)
 * that the docs page imports and renders.
 *
 * Run directly (`node scripts/generate-op-reference.mjs`) or via the `predev` /
 * `prebuild` npm hooks, which run it automatically before Next builds.
 */

import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEBSITE_DIR = path.resolve(HERE, "..");
const REPO_DIR = path.resolve(WEBSITE_DIR, "..");
const MANIFEST_ROOT = path.join(
  REPO_DIR,
  "frontend",
  "src",
  "palette",
  "manifest",
);
const OUT_PATH = path.join(
  WEBSITE_DIR,
  "src",
  "lib",
  "op-reference.generated.ts",
);

/**
 * Human-friendly blurbs per domain, shown above each op group. Kept short and
 * factual; sourced from the lore mental model (see .claude/skills/lore/SKILL.md).
 * A domain with no blurb here still renders — it just shows ops without a lede.
 */
const DOMAIN_BLURBS = {
  auth: "Identity and sign-in: log in to a server, inspect the current user, and log out.",
  branch:
    "Create, switch, protect, reset, and archive branches, and drive the guided three-way merge state machine.",
  dependency:
    "Track and edit the links between a file and the assets it references.",
  file: "Stage, unstage, diff, hash, and inspect files, plus per-file history and dependency edits.",
  layer: "Compose content in layers — add, remove, and list them.",
  link: "Add, remove, update, and list the links that compose content.",
  lock: "Advisory per-file locks for unmergeable binary assets — acquire, release, and see who holds what.",
  repository:
    "Open, create, clone, and administer a repository: status, info, flush, garbage-collect, verify, and metadata.",
  revision:
    "The commit surface: commit, amend, diff, inspect, find, revert, restore, and sync revisions.",
  service: "Start and stop the lore background service.",
  shared_store:
    "Create and configure a shared store that multiple repositories read from.",
  storage:
    "Content-addressed storage backends — open/close/flush a backend and put, get, copy, or obliterate fragments.",
};

// --- TS object-literal -> JSON, mirroring generate_catalog.py -----------------

function stripBlockComments(src) {
  return src.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Return the brace-balanced text of the `const manifest ... = { ... }` object. */
function extractManifestObject(src) {
  src = stripBlockComments(src);
  let m = src.match(/manifest\s*:\s*OpManifest\s*=\s*\{/);
  if (!m) m = src.match(/const\s+manifest\s*=\s*\{/);
  if (!m) throw new Error("no `manifest` object literal found");
  const start = src.indexOf("{", m.index);
  let depth = 0;
  for (let i = start; i < src.length; i++) {
    const c = src[i];
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) return src.slice(start, i + 1);
    }
  }
  throw new Error("unbalanced braces in manifest object");
}

/**
 * Best-effort convert a plain-data TS object literal into JSON. Scans left to
 * right, emitting string literals as opaque JSON-encoded tokens and rewriting
 * structure (bare keys, single quotes, `+` concatenation, trailing commas) only
 * on the *non-string* segments — so braces, colons, and quotes that appear
 * inside descriptions/placeholders are never mistaken for structure. (A
 * stricter take on generate_catalog.py, which rewrote the reassembled string
 * and mis-quoted values containing a `, word:` pattern.)
 */
function tsObjectToJson(objSrc) {
  // Token stream: { lit: "..." } for string literals, { code: "..." } for the
  // structural text between them.
  const tokens = [];
  let i = 0;
  let codeBuf = "";
  const n = objSrc.length;
  const flushCode = () => {
    if (codeBuf) {
      tokens.push({ code: codeBuf });
      codeBuf = "";
    }
  };
  while (i < n) {
    const c = objSrc[i];
    if (c === '"' || c === "'") {
      flushCode();
      const quote = c;
      let j = i + 1;
      const buf = [];
      while (j < n) {
        const cj = objSrc[j];
        if (cj === "\\" && j + 1 < n) {
          buf.push(objSrc.slice(j, j + 2));
          j += 2;
          continue;
        }
        if (cj === quote) break;
        buf.push(cj);
        j += 1;
      }
      const content = buf.join("");
      tokens.push({ lit: JSON.stringify(unescapeForJson(content, quote)) });
      i = j + 1;
    } else {
      codeBuf += c;
      i += 1;
    }
  }
  flushCode();

  // Reassemble. Drop `+` that sits between two string literals (concatenation),
  // and on code segments quote bare keys and strip trailing commas.
  const parts = [];
  for (let k = 0; k < tokens.length; k++) {
    const t = tokens[k];
    if (t.lit !== undefined) {
      parts.push(t.lit);
      continue;
    }
    let seg = t.code;
    // String concatenation: literal <ws> + <ws> literal — collapse the `+`.
    const between =
      k > 0 &&
      k < tokens.length - 1 &&
      tokens[k - 1].lit !== undefined &&
      tokens[k + 1].lit !== undefined;
    if (between && /^\s*\+\s*$/.test(seg)) {
      // Collapse: emit nothing so the two literals are adjacent — but JSON has
      // no string concatenation, so merge them by stripping the closing/opening
      // quotes of the neighbours.
      parts[parts.length - 1] = parts[parts.length - 1].replace(/"$/, "");
      tokens[k + 1].lit = tokens[k + 1].lit.replace(/^"/, "");
      continue;
    }
    // Quote bare object keys:  key:  ->  "key":
    seg = seg.replace(/([{,]\s*)([A-Za-z_][A-Za-z0-9_]*)\s*:/g, '$1"$2":');
    // Drop trailing commas before } or ].
    seg = seg.replace(/,(\s*[}\]])/g, "$1");
    parts.push(seg);
  }
  return JSON.parse(parts.join(""));
}

/**
 * Turn the raw (already quote-stripped) literal content into a plain JS string,
 * resolving escape sequences from the original literal so JSON.stringify can
 * re-encode them cleanly.
 */
function unescapeForJson(content, quote) {
  let res = "";
  for (let k = 0; k < content.length; k++) {
    if (content[k] === "\\" && k + 1 < content.length) {
      const next = content[k + 1];
      if (next === quote) {
        res += quote;
      } else if (next === "\\") {
        res += "\\";
      } else if (next === "n") {
        res += "\n";
      } else if (next === "t") {
        res += "\t";
      } else {
        res += next;
      }
      k++;
    } else {
      res += content[k];
    }
  }
  return res;
}

async function parseManifestFile(file) {
  const src = await fs.readFile(file, "utf-8");
  return tsObjectToJson(extractManifestObject(src));
}

// --- Catalog assembly ---------------------------------------------------------

function fieldSummary(field) {
  const kind = field.kind || "text";
  let type = "string";
  if (kind === "number") type = "number";
  else if (kind === "boolean") type = "boolean";
  else if (kind === "string-list") type = "string[]";
  else if (kind === "enum") {
    const opts = (field.options || [])
      .map((o) => o && o.value)
      .filter(Boolean);
    type = opts.length ? `enum(${opts.join(" | ")})` : "enum";
  }
  return {
    name: field.name,
    label: field.label || field.name,
    type,
    description: field.description || "",
    required: Boolean(field.required),
  };
}

async function build() {
  const domains = (await fs.readdir(MANIFEST_ROOT, { withFileTypes: true }))
    .filter((d) => d.isDirectory())
    .map((d) => d.name)
    .sort();

  const groups = [];
  let opCount = 0;

  for (const domain of domains) {
    const dir = path.join(MANIFEST_ROOT, domain);
    const files = (await fs.readdir(dir))
      .filter((f) => f.endsWith(".ts"))
      .sort();
    const ops = [];
    for (const f of files) {
      const data = await parseManifestFile(path.join(dir, f));
      if (!data || !data.id) continue;
      ops.push({
        id: data.id,
        op: data.op || data.id.split(".").slice(1).join("."),
        label: data.label || data.id,
        description: data.description || "",
        command: data.command || "",
        surface: data.surface || "palette",
        resultKind: data.resultKind || "json",
        args: (data.args || []).map(fieldSummary),
      });
    }
    if (!ops.length) continue;
    ops.sort((a, b) => a.id.localeCompare(b.id));
    opCount += ops.length;
    groups.push({
      domain,
      blurb: DOMAIN_BLURBS[domain] || "",
      ops,
    });
  }

  return { groups, opCount, domainCount: groups.length };
}

function header() {
  return `// AUTO-GENERATED — DO NOT EDIT BY HAND.
//
// Generated by website/scripts/generate-op-reference.mjs from the LoreGUI
// command-palette manifests (frontend/src/palette/manifest/<domain>/<op>.ts),
// the single source of truth for the op surface. Regenerate with:
//   npm --prefix website run generate:ops
// It also runs automatically via the predev / prebuild npm hooks.
`;
}

async function main() {
  const catalog = await build();
  const body = `${header()}
export interface OpArg {
  name: string;
  label: string;
  /** Human-readable type, e.g. "string", "number", "enum(local | s3)". */
  type: string;
  description: string;
  required: boolean;
}

export interface OpReferenceEntry {
  /** Stable id "<domain>.<op>". */
  id: string;
  op: string;
  label: string;
  description: string;
  /** The Tauri command the palette invokes. */
  command: string;
  /** Where the op lives in the app: "panel" | "menu" | "palette". */
  surface: string;
  resultKind: string;
  args: OpArg[];
}

export interface OpReferenceGroup {
  domain: string;
  blurb: string;
  ops: OpReferenceEntry[];
}

/** Total ops documented across all domains. */
export const OP_COUNT = ${catalog.opCount};

/** Number of domains documented. */
export const DOMAIN_COUNT = ${catalog.domainCount};

export const OP_REFERENCE: OpReferenceGroup[] = ${JSON.stringify(
    catalog.groups,
    null,
    2,
  )};
`;

  await fs.mkdir(path.dirname(OUT_PATH), { recursive: true });
  await fs.writeFile(OUT_PATH, body, "utf-8");
  console.log(
    `Wrote ${catalog.opCount} ops across ${catalog.domainCount} domains to ${path.relative(
      WEBSITE_DIR,
      OUT_PATH,
    )}`,
  );
}

main().catch((err) => {
  console.error("generate-op-reference failed:", err);
  process.exit(1);
});
