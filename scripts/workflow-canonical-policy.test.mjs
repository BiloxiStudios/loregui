#!/usr/bin/env node
/**
 * SBAI-5910 — must-reject fixtures for the canonical workflow pin.
 *
 * Every case below is a valid-YAML false-green that defeated the two previous
 * hand-written parsers (reviewer reproductions on f096255 and 430d216). Under
 * a canonical byte pin they are rejected by construction — there is no parser
 * left to outwit — but they are retained as executable regressions so a future
 * refactor back toward parsing cannot silently reopen them.
 */
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import assert from "node:assert/strict";
import {
  CANONICAL_PATH,
  CANONICAL_SHA256,
  WORKFLOW_PATH,
  checkCanonical,
  checkRepository,
  sha256,
} from "./workflow-canonical-policy.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const workflow = readFileSync(join(repoRoot, WORKFLOW_PATH), "utf8");
const canonical = readFileSync(join(repoRoot, CANONICAL_PATH), "utf8");

test("the repository workflow matches its reviewed canonical form", () => {
  const verdict = checkRepository();
  assert.equal(verdict.ok, true, verdict.reason);
  assert.equal(verdict.digest, CANONICAL_SHA256);
});

test("BYPASS 1 (f096255): job-level write permissions + alternate mutation commands", () => {
  const tampered =
    workflow.replace("  canary:\n", "  canary:\n    permissions:\n      contents: write\n      pull-requests: write\n") +
    `
      - name: Reintroduced bump
        run: |
          node -e 'fs.writeFileSync("Cargo.toml","x")'
          git push "https://x-access-token:$GH_TOKEN@github.com/$GITHUB_REPOSITORY.git" HEAD:refs/heads/auto-bump
          gh api --method POST "repos/$GITHUB_REPOSITORY/pulls" -f title=auto-bump
`;
  const verdict = checkCanonical(tampered, canonical);
  assert.equal(verdict.ok, false);
  assert.match(verdict.reason, /workflow digest .* != canonical/);
});

test("BYPASS 2 (430d216): QUOTED permissions/run keys", () => {
  // Valid YAML: `"permissions":` and `"run": |` parse identically to the
  // unquoted forms, so a literal-string matcher walks straight past them.
  const tampered =
    workflow.replace("  canary:\n", '  canary:\n    "permissions":\n      contents: write\n      pull-requests: write\n') +
    `
      - name: Quoted-key mutation
        "run": |
          git push origin HEAD:refs/heads/auto-bump
`;
  const verdict = checkCanonical(tampered, canonical);
  assert.equal(verdict.ok, false);
  assert.match(verdict.reason, /workflow digest .* != canonical/);
});

test("BYPASS 3 (430d216): quoted/commented write values and a folded run: >- body", () => {
  const tampered =
    workflow.replace(
      "  canary:\n",
      '  canary:\n    permissions:\n      contents: "write" # keep\n      pull-requests: \'write\'\n',
    ) +
    `
      - name: Folded mutation body
        run: >-
          git push origin HEAD:refs/heads/auto-bump
`;
  const verdict = checkCanonical(tampered, canonical);
  assert.equal(verdict.ok, false);
  assert.match(verdict.reason, /workflow digest .* != canonical/);
});

test("the reviewer's exact evidence fixture is rejected when present", async () => {
  const evidence = "/srv/AI_Stuff/agent-scratch/tmp.X1SqItQKu9/workflow.yml";
  let text;
  try {
    text = readFileSync(evidence, "utf8");
  } catch {
    return; // Reviewer scratch space is not part of the repo; skip when absent.
  }
  const verdict = checkCanonical(text, canonical);
  assert.equal(verdict.ok, false, "the reviewer's bypass fixture must be rejected");
});

test("swapping the canonical copy alone does not help", () => {
  const attackerCanonical = canonical + "\n# appended\n";
  const verdict = checkCanonical(attackerCanonical, attackerCanonical);
  assert.equal(verdict.ok, false);
  assert.match(verdict.reason, /canonical copy digest .* != pinned/);
});

test("empty or unreadable inputs fail closed", () => {
  assert.equal(checkCanonical("", canonical).ok, false);
  assert.equal(checkCanonical(workflow, "").ok, false);
});

test("the pinned digest is the digest of the committed canonical bytes", () => {
  assert.equal(sha256(canonical), CANONICAL_SHA256);
});
