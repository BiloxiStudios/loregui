#!/usr/bin/env node
/**
 * SBAI-5910 — Rule-40 fixtures for the upstream-parity mutation contract.
 *
 * Binding ruling (sb-lore + sb-fable): the PRODUCT SOURCE is the
 * BiloxiStudios maintenance fork at the reviewed pin; EpicGames upstream is a
 * DRIFT TARGET ONLY. Observation and reporting continue; **mutation fails
 * closed**. Bumping to upstream HEAD would silently drop the fork-only
 * credential hardening (SBAI-5909), which has no upstream equivalent yet.
 *
 * These fixtures assert on the workflow source itself, because the failure
 * they guard against is a *reintroduced* mutation step, not a runtime bug:
 *
 *   (a) a fork pin never auto-rewrites the manifest or opens a PR;
 *   (b) an empty pin STOPS;
 *   (c) an unknown/unparseable pin STOPS.
 */
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import assert from "node:assert/strict";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const workflow = readFileSync(
  join(repoRoot, ".github/workflows/upstream-parity.yml"),
  "utf8",
);

/** The `Get pinned lore rev` step body — where the fail-closed gate lives. */
function pinStep() {
  const start = workflow.indexOf("- name: Get pinned lore rev");
  assert.notEqual(start, -1, "workflow must still detect the pinned rev");
  const rest = workflow.slice(start + 1);
  const end = rest.indexOf("\n      - name:");
  return end === -1 ? rest : rest.slice(0, end);
}

test("(a) no mutation path can rewrite the pin or open a PR", () => {
  assert.ok(
    !/^\s*sed -i/m.test(workflow),
    "upstream-parity must not rewrite the manifest — the pin moves only through a reviewed ticket",
  );
  assert.ok(
    !/gh pr create/.test(workflow),
    "upstream-parity must not open an automated bump PR while fork-only hardening is absent upstream",
  );
  assert.ok(
    !/git push origin/.test(workflow),
    "upstream-parity must not push a bump branch",
  );
  assert.ok(
    !/cargo update -p lore\b/.test(workflow),
    "upstream-parity must not re-resolve the lock onto a new rev",
  );
});

test("(a2) drift is still observed and reported", () => {
  assert.ok(
    /Report rev drift \(mutation disabled\)/.test(workflow),
    "drift must still be reported — observation continues, only mutation stops",
  );
  assert.ok(
    /upstream-lore-parity\.mjs/.test(workflow),
    "the parity check itself must still run",
  );
});

test("(b) an empty pin fails closed", () => {
  const step = pinStep();
  assert.ok(
    /if \[ -z "\$REV" \]/.test(step) && /exit 1/.test(step),
    "an empty rev must stop the workflow, never fall through to a bump",
  );
});

test("(c) an unknown or unparseable pin fails closed", () => {
  const step = pinStep();
  assert.ok(
    /-z "\$HOST"/.test(step),
    "an unparseable host must stop the workflow",
  );
  assert.ok(
    /\[0-9a-f\]\{40\}/.test(step),
    "rev extraction must require a full 40-hex rev, so a branch or tag pin cannot parse",
  );
  assert.ok(
    /rev = "\[\^"\]\+"|git = "\[\^"\]\+"/.test(step),
    "extraction must be host-agnostic — a host-specific pattern yields an EMPTY rev after a fork switch",
  );
});

test("the maintenance-fork pin is recognised and never auto-followed to upstream", () => {
  const step = pinStep();
  assert.ok(
    /EpicGames\/lore\.git/.test(step) && /fork_pinned/.test(step),
    "the workflow must detect that the product pin sits on the fork and mark it",
  );
});
