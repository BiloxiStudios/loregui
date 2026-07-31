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

test("(b)+(c) the fail-closed pin gate is the shared executable policy", () => {
  // The empty/unknown/unparseable STOP behaviour is proven by feeding real
  // fixtures through classifyPin in scripts/lore-pin-policy.test.mjs; what
  // must hold HERE is that the workflow actually invokes that same policy
  // (and does not reintroduce its own inline host check, which previously
  // treated any non-Epic host as an acceptable "maintenance fork").
  assert.ok(
    /node scripts\/lore-pin-policy\.mjs Cargo\.toml/.test(workflow),
    "the workflow must gate on the shared pin policy script",
  );
  assert.ok(
    !/fork_pinned/.test(workflow),
    "the old permissive fork_pinned branch must not return",
  );
  assert.ok(
    !/if \[ "\$HOST" != /.test(workflow),
    "no inline host comparison may bypass the shared policy",
  );
});

test("the token is least-privilege while mutation is disabled", () => {
  const perms = workflow.slice(
    workflow.indexOf("permissions:"),
    workflow.indexOf("jobs:"),
  );
  assert.ok(/contents:\s*read/.test(perms), "contents must be read-only");
  assert.ok(
    !/pull-requests:\s*write/.test(perms),
    "pull-requests: write must be dropped — no automated bump PR",
  );
  assert.ok(
    !/contents:\s*write/.test(perms),
    "contents: write must be dropped — no bump commit or branch push",
  );
});

test("un-gate documentation points at the repin/consumer-guard tickets", () => {
  assert.ok(
    /SBAI-5905/.test(workflow) && /SBAI-5916/.test(workflow),
    "the drift report must name the repin/consumer-guard gates (5905/5916)",
  );
});
