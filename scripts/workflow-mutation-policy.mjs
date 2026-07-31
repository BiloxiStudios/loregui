#!/usr/bin/env node
/**
 * SBAI-5910 — structural no-mutation policy for `.github/workflows/upstream-parity.yml`.
 *
 * Review finding on f096255: the previous contract was a BLACKLIST of literal
 * spellings (`sed -i`, `gh pr create`, `git push origin`) and inspected only
 * the text above `jobs:`. A valid workflow restored job-level write
 * permissions and mutated via `writeFileSync`, an authenticated push URL, and
 * `gh api .../pulls` — passing 5/5.
 *
 * This policy is structural instead: it parses the workflow's indentation
 * tree, checks EVERY permission block (top level and per job), and scans every
 * `run:` body for write-capable operations by capability rather than by one
 * spelling. The workflow invokes this same module in CI, so the file cannot
 * drift from the policy that guards it.
 */

/** Indentation-aware line records: {indent, text}. */
function rows(yaml) {
  return yaml
    .split("\n")
    .map((raw) => ({ indent: raw.length - raw.trimStart().length, text: raw.trim(), raw }))
    .filter((r) => r.text !== "" && !r.text.startsWith("#"));
}

/**
 * Every `permissions:` block with a label describing where it lives.
 * @returns {{scope: string, scopes: Record<string,string>}[]}
 */
export function permissionBlocks(yaml) {
  const all = rows(yaml);
  const out = [];
  let currentJob = null;
  for (let i = 0; i < all.length; i += 1) {
    const row = all[i];
    // A job header is a key at indent 2 directly under `jobs:`.
    if (row.indent === 2 && row.text.endsWith(":") && !row.text.includes(" ")) {
      const priorTop = all
        .slice(0, i)
        .reverse()
        .find((r) => r.indent === 0);
      if (priorTop && priorTop.text === "jobs:") currentJob = row.text.slice(0, -1);
    }
    if (row.text === "permissions:") {
      const scope = row.indent === 0 ? "workflow" : `job:${currentJob ?? "unknown"}`;
      const scopes = {};
      for (let j = i + 1; j < all.length && all[j].indent > row.indent; j += 1) {
        const [key, value] = all[j].text.split(":").map((s) => s.trim());
        if (key) scopes[key] = value ?? "";
      }
      out.push({ scope, scopes });
    }
    // Inline mapping form: `permissions: write-all`.
    if (row.text.startsWith("permissions:") && row.text !== "permissions:") {
      out.push({
        scope: row.indent === 0 ? "workflow" : `job:${currentJob ?? "unknown"}`,
        scopes: { _inline: row.text.slice("permissions:".length).trim() },
      });
    }
  }
  return out;
}

/** Every `run:` body in the workflow, joined per step. */
export function runBodies(yaml) {
  const all = rows(yaml);
  const bodies = [];
  for (let i = 0; i < all.length; i += 1) {
    const row = all[i];
    if (row.text === "run: |" || row.text === "run: >" || row.text.startsWith("run: |")) {
      const body = [];
      for (let j = i + 1; j < all.length && all[j].indent > row.indent; j += 1) {
        body.push(all[j].text);
      }
      bodies.push(body.join("\n"));
    } else if (row.text.startsWith("run:")) {
      bodies.push(row.text.slice(4).trim());
    }
  }
  return bodies;
}

/**
 * Write-capable operations, matched by CAPABILITY. Each entry is
 * [label, RegExp]; the regexes deliberately cover alternate spellings the
 * reviewer demonstrated.
 */
export const MUTATION_PATTERNS = [
  ["in-place file rewrite (sed)", /\bsed\b[^\n|]*-[a-zA-Z]*i\b/],
  ["in-place file rewrite (perl/awk)", /\bperl\b[^\n]*-[a-zA-Z]*i\b|\bawk\b[^\n]*>\s*Cargo\./],
  ["node/script file write", /writeFileSync|fs\.write|open\([^)]*['"]w['"]/],
  ["shell redirect into a tracked manifest", />\s*Cargo\.(toml|lock)|tee\s+Cargo\./],
  ["git commit", /\bgit\s+commit\b/],
  ["git push (any remote or URL)", /\bgit\s+push\b/],
  ["git tag", /\bgit\s+tag\b/],
  ["gh pr creation", /\bgh\s+pr\s+(create|merge|ready)\b/],
  ["gh api write", /\bgh\s+api\b[^\n]*(--method\s+(POST|PATCH|PUT|DELETE)|-X\s+(POST|PATCH|PUT|DELETE))/],
  ["gh api pulls/releases endpoint", /\bgh\s+api\b[^\n]*\/(pulls|releases|git\/refs|tags)\b/],
  ["gh release mutation", /\bgh\s+release\s+(create|edit|upload|delete)\b/],
  ["lock re-resolution", /\bcargo\s+update\b/],
  ["workflow dispatch of a mutating job", /\bgh\s+workflow\s+run\b/],
];

/**
 * @returns {{ok: true} | {ok: false, violations: string[]}}
 */
export function classifyWorkflow(yaml) {
  const violations = [];
  if (typeof yaml !== "string" || yaml.trim() === "") {
    return { ok: false, violations: ["workflow is empty or unreadable"] };
  }

  const blocks = permissionBlocks(yaml);
  if (blocks.length === 0) {
    violations.push("no permissions block: the token would inherit repository defaults");
  }
  for (const { scope, scopes } of blocks) {
    if (scopes._inline !== undefined) {
      violations.push(`${scope}: inline permissions "${scopes._inline}" — declare scopes explicitly`);
      continue;
    }
    for (const [key, value] of Object.entries(scopes)) {
      if (value !== "write") continue;
      // `issues: write` is reporting (ticket dispatch), permitted ONLY on the
      // detect job; everything else must be read.
      if (key === "issues" && scope === "job:detect") continue;
      violations.push(`${scope}: ${key}: write is not permitted while mutation is disabled`);
    }
  }

  for (const body of runBodies(yaml)) {
    for (const [label, pattern] of MUTATION_PATTERNS) {
      if (pattern.test(body)) {
        const line = body.split("\n").find((l) => pattern.test(l)) ?? body.slice(0, 80);
        violations.push(`mutation capability "${label}" present: ${line.trim()}`);
      }
    }
  }

  return violations.length === 0 ? { ok: true } : { ok: false, violations };
}

// CLI: `node scripts/workflow-mutation-policy.mjs <workflow-path>`
if (process.argv[1] && import.meta.url.endsWith(process.argv[1].split("/").pop())) {
  const { readFileSync } = await import("node:fs");
  const path = process.argv[2];
  let text = "";
  try {
    text = readFileSync(path, "utf8");
  } catch (error) {
    console.error(`workflow mutation policy: cannot read ${path}: ${error.message}`);
    process.exit(1);
  }
  const verdict = classifyWorkflow(text);
  if (!verdict.ok) {
    console.error("workflow mutation policy: STOP");
    for (const v of verdict.violations) console.error(`  - ${v}`);
    process.exit(1);
  }
  console.log(`workflow mutation policy: ${path} is mutation-free and least-privilege`);
}
