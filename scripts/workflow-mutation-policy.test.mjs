#!/usr/bin/env node
/**
 * SBAI-5910 — EXECUTABLE fixtures for the structural workflow mutation policy.
 *
 * Every case below is a bypass that the previous blacklist contract PASSED
 * (reviewer reproduction on f096255). They are permanent must-reject
 * regressions: the policy is the same module the workflow invokes in CI.
 */
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import assert from "node:assert/strict";
import { classifyWorkflow, permissionBlocks } from "./workflow-mutation-policy.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const realWorkflow = readFileSync(
  join(repoRoot, ".github/workflows/upstream-parity.yml"),
  "utf8",
);

test("the committed workflow is mutation-free and least-privilege", () => {
  const verdict = classifyWorkflow(realWorkflow);
  assert.equal(verdict.ok, true, JSON.stringify(verdict.violations));
});

test("REVIEWER BYPASS: job-level write permissions are rejected", () => {
  // The blacklist only sliced text above `jobs:`, so a job could restore write.
  const swapped = realWorkflow.replace(
    "  canary:\n",
    "  canary:\n    permissions:\n      contents: write\n      pull-requests: write\n",
  );
  const verdict = classifyWorkflow(swapped);
  assert.equal(verdict.ok, false);
  assert.ok(
    verdict.violations.some((v) => v.includes("job:canary") && v.includes("contents: write")),
    `must name the job-level escalation; got: ${verdict.violations.join(" | ")}`,
  );
  assert.ok(
    verdict.violations.some((v) => v.includes("pull-requests: write")),
    "must reject pull-requests: write at job level",
  );
});

test("REVIEWER BYPASS: alternate mutation commands are rejected by capability", () => {
  const step = `
      - name: Reintroduced automated bump through alternate commands
        run: |
          node -e 'const fs=require("fs"); fs.writeFileSync("Cargo.toml", "x")'
          git push "https://x-access-token:\${GH_TOKEN}@github.com/\${GITHUB_REPOSITORY}.git" HEAD:refs/heads/auto-bump
          gh api --method POST "repos/\${GITHUB_REPOSITORY}/pulls" -f title=auto-bump
`;
  const verdict = classifyWorkflow(realWorkflow + step);
  assert.equal(verdict.ok, false);
  for (const capability of [
    "node/script file write",
    "git push (any remote or URL)",
    "gh api write",
  ]) {
    assert.ok(
      verdict.violations.some((v) => v.includes(capability)),
      `must catch "${capability}"; got: ${verdict.violations.join(" | ")}`,
    );
  }
});

test("classic spellings stay rejected", () => {
  for (const [label, snippet] of [
    ["sed", "      - name: x\n        run: |\n          sed -i 's/a/b/' Cargo.toml\n"],
    ["gh pr create", "      - name: x\n        run: |\n          gh pr create --title t\n"],
    ["cargo update", "      - name: x\n        run: |\n          cargo update -p lore\n"],
    ["git commit", "      - name: x\n        run: |\n          git commit -m bump\n"],
    ["redirect", "      - name: x\n        run: |\n          echo hi > Cargo.toml\n"],
  ]) {
    const verdict = classifyWorkflow(realWorkflow + snippet);
    assert.equal(verdict.ok, false, `${label} must be rejected`);
  }
});

test("inline permissions and a missing permissions block are rejected", () => {
  assert.equal(classifyWorkflow(realWorkflow.replace("permissions:\n  contents: read", "permissions: write-all")).ok, false);
  const stripped = realWorkflow
    .replace("permissions:\n  contents: read\n", "")
    .replace("    permissions:\n      contents: read\n      issues: write\n", "");
  assert.equal(classifyWorkflow(stripped).ok, false, "no permissions block must fail closed");
});

test("permission blocks are discovered at both workflow and job scope", () => {
  const blocks = permissionBlocks(realWorkflow);
  const scopes = blocks.map((b) => b.scope);
  assert.ok(scopes.includes("workflow"), `workflow scope missing: ${scopes.join(", ")}`);
  assert.ok(scopes.includes("job:detect"), `detect scope missing: ${scopes.join(", ")}`);
});

test("issues: write is permitted ONLY on the reporting job", () => {
  const moved = realWorkflow.replace(
    "  canary:\n",
    "  canary:\n    permissions:\n      issues: write\n",
  );
  const verdict = classifyWorkflow(moved);
  assert.equal(verdict.ok, false);
  assert.ok(verdict.violations.some((v) => v.includes("job:canary") && v.includes("issues: write")));
});
