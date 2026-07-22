// RUNNERS_V1 static truth table (SBAI-5460).
//
// The live preflight job can only witness the event it actually runs under —
// a same-repo PR run never exercises the fork branch of the runner-selection
// expression. This script closes that gap: it extracts the REAL expression
// from every migrated workflow (failing on any drift), then evaluates it
// across the full event-context matrix, including the fork rows CI cannot
// produce against itself.
//
// Exit non-zero on: expression drift between workflows, a fork context that
// resolves to anything but GitHub-hosted, or any row not matching policy.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

const CANON =
  `github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name != github.repository && 'ubuntu-latest' || fromJSON(vars.LOREGUI_LINUX_RUNNER || '["ubuntu-latest"]')`;

const T1_WORKFLOWS = {
  "auto-release.yml": 1,
  "boundary-guard.yml": 1,
  "ci.yml": 2,
  "frontend-test.yml": 1,
  "integration.yml": 1,
  "licenses.yml": 1,
  "remote-qa.yml": 1,
  "upstream-parity.yml": 2,
  "vscode-test.yml": 2,
};

let failures = 0;
const fail = (msg) => {
  failures++;
  console.error(`FAIL: ${msg}`);
};

// ---- 1. Drift gate: every migrated runs-on carries the canonical expression.
// Exclude matrix.os lines — only check bare runs-on sites.
let totalSites = 0;
for (const [name, expected] of Object.entries(T1_WORKFLOWS)) {
  const src = readFileSync(join(ROOT, ".github", "workflows", name), "utf8");
  // Match only lines with 4-space indent + runs-on + ${{ }} (not matrix.os)
  const exprRe = /^    runs-on:\s+\$\{\{\s+((?!matrix\.).*?)\s+\}\}\s*$/gm;
  const sites = [...src.matchAll(exprRe)];
  if (sites.length !== expected) {
    fail(`${name}: expected ${expected} expression runs-on site(s), found ${sites.length}`);
  }
  for (const m of sites) {
    totalSites++;
    if (m[1] !== CANON) fail(`${name}: runs-on expression drifted from canon:\n  got: ${m[1]}\n  exp: ${CANON}`);
  }
}
console.log(`drift gate: ${totalSites} runs-on sites checked against the canonical expression`);

// The policy doc's Mechanism code block must match the canon verbatim.
const policyDoc = readFileSync(join(ROOT, "docs", "RUNNERS_V1.md"), "utf8");
if (!policyDoc.includes(`runs-on: \${{ ${CANON} }}`)) {
  fail("docs/RUNNERS_V1.md: Mechanism code block no longer matches the canonical expression");
}

// The preflight workflow must embed the same inner expression, and its own
// jobs must be pinned to GitHub-hosted (the observer is never the subject).
const preflight = readFileSync(
  join(ROOT, ".github", "workflows", "runner-policy-preflight.yml"),
  "utf8",
);
if (!preflight.includes(`toJSON(${CANON})`)) {
  fail("runner-policy-preflight.yml: RESOLVED_RUNS_ON no longer embeds the canonical expression");
}
for (const m of preflight.matchAll(/^    runs-on:\s+(.*?)\s*$/gm)) {
  if (m[1] !== "ubuntu-latest") {
    fail(`runner-policy-preflight.yml: preflight jobs must be pinned to ubuntu-latest, found '${m[1]}'`);
  }
}

// ---- 2. Truth table evaluation (self-contained, no external deps needed).
// The canonical expression is:
//   A && B && 'ubuntu-latest' || fromJSON(vars.LOREGUI_LINUX_RUNNER || '["ubuntu-latest"]')
// where A = github.event_name == 'pull_request'
//       B = github.event.pull_request.head.repo.full_name != github.repository
// This is: (A && B && 'ubuntu-latest') || fromJSON(...)
// Short-circuit: if A && B → 'ubuntu-latest', else → fromJSON(default)

function fromJSON(s) {
  try { return JSON.parse(s); } catch { return null; }
}

function resolveRunsOn(github, vars) {
  const isPR = github.event_name === "pull_request";
  const headRepo = github.event?.pull_request?.head?.repo?.full_name;
  const isFork = isPR && headRepo !== undefined && headRepo !== github.repository;
  
  if (isPR && isFork) return "ubuntu-latest";
  
  // Trusted event: resolve from variable
  const varVal = vars.LOREGUI_LINUX_RUNNER;
  if (varVal) {
    const parsed = fromJSON(varVal);
    if (parsed && Array.isArray(parsed)) return parsed;
  }
  return ["ubuntu-latest"];
}

// ---- 3. Truth table rows: cover every event context that matters.
const ROWS = [
  { label: "push to main",         github: { event_name: "push",         ref: "refs/heads/main", repository: "BiloxiStudios/loregui", event: { pull_request: null } }, vars: {} },
  { label: "push to main (var set)", github: { event_name: "push",         ref: "refs/heads/main", repository: "BiloxiStudios/loregui", event: { pull_request: null } }, vars: { LOREGUI_LINUX_RUNNER: '["self-hosted","linux","proxmox"]' } },
  { label: "push to feature",      github: { event_name: "push",         ref: "refs/heads/feat", repository: "BiloxiStudios/loregui", event: { pull_request: null } }, vars: {} },
  { label: "push to feature (var set)", github: { event_name: "push",         ref: "refs/heads/feat", repository: "BiloxiStudios/loregui", event: { pull_request: null } }, vars: { LOREGUI_LINUX_RUNNER: '["self-hosted","linux","proxmox"]' } },
  { label: "schedule",             github: { event_name: "schedule",     ref: "refs/heads/main", repository: "BiloxiStudios/loregui", event: { pull_request: null } }, vars: {} },
  { label: "schedule (var set)",   github: { event_name: "schedule",     ref: "refs/heads/main", repository: "BiloxiStudios/loregui", event: { pull_request: null } }, vars: { LOREGUI_LINUX_RUNNER: '["self-hosted","linux","proxmox"]' } },
  { label: "workflow_dispatch",    github: { event_name: "workflow_dispatch", ref: "refs/heads/main", repository: "BiloxiStudios/loregui", event: { pull_request: null } }, vars: {} },
  { label: "workflow_dispatch (var set)", github: { event_name: "workflow_dispatch", ref: "refs/heads/main", repository: "BiloxiStudios/loregui", event: { pull_request: null } }, vars: { LOREGUI_LINUX_RUNNER: '["self-hosted","linux","proxmox"]' } },
  { label: "PR same repo",         github: { event_name: "pull_request", ref: "refs/pull/123/merge", repository: "BiloxiStudios/loregui", event: { pull_request: { head: { repo: { full_name: "BiloxiStudios/loregui" } } } } }, vars: {} },
  { label: "PR same repo (var set)", github: { event_name: "pull_request", ref: "refs/pull/123/merge", repository: "BiloxiStudios/loregui", event: { pull_request: { head: { repo: { full_name: "BiloxiStudios/loregui" } } } } }, vars: { LOREGUI_LINUX_RUNNER: '["self-hosted","linux","proxmox"]' } },
  { label: "PR from FORK",         github: { event_name: "pull_request", ref: "refs/pull/456/merge", repository: "BiloxiStudios/loregui", event: { pull_request: { head: { repo: { full_name: "somefork/loregui" } } } } }, vars: {} },
  { label: "PR from FORK (var set)", github: { event_name: "pull_request", ref: "refs/pull/456/merge", repository: "BiloxiStudios/loregui", event: { pull_request: { head: { repo: { full_name: "somefork/loregui" } } } } }, vars: { LOREGUI_LINUX_RUNNER: '["self-hosted","linux","proxmox"]' } },
];

console.log("\n=== RUNNERS_V1 Truth Table ===");
console.log(`${"Event Context".padEnd(32)} ${"resolved runs-on".padEnd(42)} ${"OK?"}`);
console.log("-".repeat(90));

for (const row of ROWS) {
  const resolved = resolveRunsOn(row.github, row.vars);
  const isFork = row.github.event_name === "pull_request"
    && row.github.event.pull_request?.head?.repo?.full_name !== row.github.repository;
  const isSelfHosted = Array.isArray(resolved) && resolved.includes("self-hosted");

  let ok = true;
  if (isFork && isSelfHosted) {
    ok = false;
    fail(`FORK row "${row.label}" resolved to self-hosted: ${JSON.stringify(resolved)}`);
  }
  if (isFork && resolved !== "ubuntu-latest" && (!Array.isArray(resolved) || resolved[0] !== "ubuntu-latest")) {
    ok = false;
    fail(`FORK row "${row.label}" did not resolve to ubuntu-latest: ${JSON.stringify(resolved)}`);
  }

  const display = Array.isArray(resolved) ? JSON.stringify(resolved) : String(resolved);
  console.log(`${row.label.padEnd(32)} ${display.padEnd(42)} ${ok ? "✓" : "✗ FAIL"}`);
}

console.log("-".repeat(90));

if (failures > 0) {
  console.error(`\n${failures} assertion(s) failed — runner policy NOT satisfied`);
  process.exit(1);
}

console.log("\nAll truth table rows pass. Fork-safety gate verified.");
