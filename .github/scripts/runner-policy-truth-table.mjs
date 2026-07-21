// RUNNERS_V1 static truth table (SBAI-5460).
//
// The live preflight job can only witness the event it actually runs under —
// a same-repo PR run never exercises the fork branch of the runner-selection
// expression. This script closes that gap: it extracts the REAL expression
// from every migrated workflow (failing on any drift), then evaluates it with
// GitHub's own expression engine (@actions/expressions, the engine behind
// actions/languageservices) across the full event-context matrix, including
// the fork rows CI cannot produce against itself.
//
// Exit non-zero on: expression drift between workflows, a fork context that
// resolves to anything but GitHub-hosted, or any row not matching policy.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import * as ex from "@actions/expressions";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

const CANON =
  "(github.event_name == 'pull_request' && (github.event.pull_request.head.repo.full_name != github.repository || github.actor == 'dependabot[bot]')) && 'ubuntu-latest' || fromJSON(vars.LOREGUI_LINUX_RUNNER || '[\"ubuntu-latest\"]')";

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
let totalSites = 0;
for (const [name, expected] of Object.entries(T1_WORKFLOWS)) {
  const src = readFileSync(join(ROOT, ".github", "workflows", name), "utf8");
  const sites = [...src.matchAll(/^\s*runs-on:\s*\$\{\{\s*(.*?)\s*\}\}\s*$/gm)];
  if (sites.length !== expected) {
    fail(`${name}: expected ${expected} expression runs-on site(s), found ${sites.length}`);
  }
  for (const m of sites) {
    totalSites++;
    if (m[1] !== CANON) fail(`${name}: runs-on expression drifted from canon:\n  ${m[1]}`);
  }
}
console.log(`drift gate: ${totalSites} runs-on sites checked against the canonical expression`);

// The policy doc's Mechanism code block must match the canon verbatim — the
// versioned policy text is not allowed to drift from the executable mechanism.
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
for (const m of preflight.matchAll(/^\s*runs-on:\s*(.*?)\s*$/gm)) {
  if (m[1] !== "ubuntu-latest") {
    fail(`runner-policy-preflight.yml: preflight jobs must be pinned to ubuntu-latest, found '${m[1]}'`);
  }
}

// ---- 2. Full-matrix evaluation with GitHub's own expression engine.
function toData(v) {
  if (v === null || v === undefined) return new ex.data.Null();
  if (typeof v === "string") return new ex.data.StringData(v);
  if (typeof v === "boolean") return new ex.data.BooleanData(v);
  if (typeof v === "object") {
    const d = new ex.data.Dictionary();
    for (const [k, val] of Object.entries(v)) d.add(k, toData(val));
    return d;
  }
  throw new Error(`unsupported context value: ${typeof v}`);
}

function toJs(result) {
  if (result instanceof ex.data.StringData) return result.coerceString();
  if (result instanceof ex.data.Array) return result.values().map(toJs);
  if (result instanceof ex.data.Null) return null;
  return result.coerceString();
}

function resolveRunsOn(github, vars) {
  const tokens = new ex.Lexer(CANON).lex().tokens;
  const tree = new ex.Parser(tokens, ["github", "vars"], []).parse();
  const ctx = new ex.data.Dictionary();
  ctx.add("github", toData(github));
  ctx.add("vars", toData(vars));
  return toJs(new ex.Evaluator(tree, ctx).evaluate());
}

const REPO = "BiloxiStudios/loregui";
const SELF_HOSTED = '["self-hosted","linux","proxmox"]';
const HOSTED = '["ubuntu-latest"]';

const prEvent = (headRepo, actor = "team-member") => ({
  event_name: "pull_request",
  repository: REPO,
  actor,
  event: { pull_request: { head: { repo: { full_name: headRepo } } } },
});
const bareEvent = (name) => ({
  event_name: name,
  repository: REPO,
  actor: "team-member",
  event: {},
});

// "untrusted" rows must resolve to GitHub-hosted regardless of the variable:
// fork PRs (attacker-controlled code), and Dependabot PRs — same head repo,
// but GitHub runs them with fork-like trust and they execute updated deps.
const EVENTS = [
  ["fork PR", prEvent("outside-user/loregui"), "untrusted"],
  ["dependabot fork PR", prEvent("outside-user/loregui", "dependabot[bot]"), "untrusted"],
  ["dependabot PR", prEvent(REPO, "dependabot[bot]"), "untrusted"],
  ["same-repo PR", prEvent(REPO), "trusted"],
  ["push", bareEvent("push"), "trusted"],
  ["schedule", bareEvent("schedule"), "trusted"],
  ["workflow_dispatch", bareEvent("workflow_dispatch"), "trusted"],
];
const VARS = [
  ["unset", {}, JSON.parse(HOSTED)],
  ["hosted", { LOREGUI_LINUX_RUNNER: HOSTED }, JSON.parse(HOSTED)],
  ["self-hosted", { LOREGUI_LINUX_RUNNER: SELF_HOSTED }, JSON.parse(SELF_HOSTED)],
];

console.log("\nevent              | LOREGUI_LINUX_RUNNER | resolved runs-on                      | verdict");
console.log("-------------------|----------------------|---------------------------------------|--------");
let rows = 0;
for (const [eventName, github, trust] of EVENTS) {
  for (const [varName, vars, trustedExpectation] of VARS) {
    rows++;
    const resolved = resolveRunsOn(github, vars);
    const expected = trust === "untrusted" ? "ubuntu-latest" : trustedExpectation;
    const ok = JSON.stringify(resolved) === JSON.stringify(expected);
    const selfHostedLeak =
      trust === "untrusted" && JSON.stringify(resolved).toLowerCase().includes("self-hosted");
    if (!ok) fail(`${eventName} / var=${varName}: resolved ${JSON.stringify(resolved)}, expected ${JSON.stringify(expected)}`);
    if (selfHostedLeak) fail(`${eventName} / var=${varName}: UNTRUSTED CONTEXT REACHED SELF-HOSTED`);
    console.log(
      `${eventName.padEnd(18)} | ${varName.padEnd(20)} | ${JSON.stringify(resolved).padEnd(37)} | ${ok && !selfHostedLeak ? "ok" : "VIOLATION"}`,
    );
  }
}

if (failures > 0) {
  console.error(`\n${failures} policy violation(s) — RUNNERS_V1 gate failed`);
  process.exit(1);
}
console.log(
  `\nRUNNERS_V1 truth table: all ${rows} rows match policy; untrusted contexts (fork + Dependabot PRs) never reach self-hosted`,
);
