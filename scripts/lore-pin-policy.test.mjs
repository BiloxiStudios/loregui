#!/usr/bin/env node
/**
 * SBAI-5910 — EXECUTABLE fixtures for the lore pin policy (the review found
 * the previous test only asserted that regex strings existed in the workflow,
 * which is not proof the policy runs or bites). These feed real manifest text
 * through the same `classifyPin` the workflow invokes.
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  ACCEPTED_HOST,
  ACCEPTED_REV,
  DRIFT_TARGET_HOST,
  classifyPin,
  mutationAllowed,
} from "./lore-pin-policy.mjs";

const manifest = (host, rev, quinnHost = host, quinnRev = rev) =>
  `lore = { git = "${host}", rev = "${rev}" }\n` +
  `quinn-proto = { git = "${quinnHost}", rev = "${quinnRev}" }\n`;

test("accepts the exact product pin", () => {
  const verdict = classifyPin(manifest(ACCEPTED_HOST, ACCEPTED_REV));
  assert.equal(verdict.ok, true, verdict.reason);
  assert.equal(verdict.host, ACCEPTED_HOST);
  assert.equal(verdict.rev, ACCEPTED_REV);
});

test("STOPS on an unknown host, even with a valid 40-hex rev", () => {
  // The bug this closes: "not EpicGames" was treated as "a maintenance fork"
  // and allowed to continue, so any attacker host passed.
  const verdict = classifyPin(
    manifest("https://github.com/attacker.example/lore.git", ACCEPTED_REV),
  );
  assert.equal(verdict.ok, false);
  assert.match(verdict.reason, /unknown host/);
});

test("STOPS on the drift target (upstream is never a product pin)", () => {
  const verdict = classifyPin(manifest(DRIFT_TARGET_HOST, ACCEPTED_REV));
  assert.equal(verdict.ok, false);
  assert.match(verdict.reason, /drift target/);
});

test("STOPS on an unknown rev at the accepted host", () => {
  const verdict = classifyPin(
    manifest(ACCEPTED_HOST, "0123456789abcdef0123456789abcdef01234567"),
  );
  assert.equal(verdict.ok, false);
  assert.match(verdict.reason, /unknown rev/);
});

test("STOPS on an empty or unreadable manifest", () => {
  for (const empty of ["", "   ", "\n"]) {
    const verdict = classifyPin(empty);
    assert.equal(verdict.ok, false);
    assert.match(verdict.reason, /empty or unreadable/);
  }
});

test("STOPS on unparseable pins: missing dep, missing host, short rev, branch pin", () => {
  assert.match(
    classifyPin(`lore = { git = "${ACCEPTED_HOST}", rev = "${ACCEPTED_REV}" }\n`).reason,
    /quinn-proto pin is missing/,
  );
  assert.match(
    classifyPin(
      `lore = { path = "../lore" }\nquinn-proto = { git = "${ACCEPTED_HOST}", rev = "${ACCEPTED_REV}" }\n`,
    ).reason,
    /no git host/,
  );
  assert.match(
    classifyPin(manifest(ACCEPTED_HOST, "ba92f94")).reason,
    /no full 40-hex rev/,
  );
  assert.match(
    classifyPin(
      `lore = { git = "${ACCEPTED_HOST}", branch = "main" }\nquinn-proto = { git = "${ACCEPTED_HOST}", rev = "${ACCEPTED_REV}" }\n`,
    ).reason,
    /no full 40-hex rev/,
  );
});

test("STOPS on mixed host or mixed rev", () => {
  assert.match(
    classifyPin(
      manifest(ACCEPTED_HOST, ACCEPTED_REV, DRIFT_TARGET_HOST, ACCEPTED_REV),
    ).reason,
    /mixed host/,
  );
  assert.match(
    classifyPin(
      manifest(
        ACCEPTED_HOST,
        ACCEPTED_REV,
        ACCEPTED_HOST,
        "0123456789abcdef0123456789abcdef01234567",
      ),
    ).reason,
    /mixed rev/,
  );
});

test("mutation is never allowed while the fork is the product source", () => {
  assert.equal(mutationAllowed(), false);
});

test("the repository's own manifest satisfies the policy", async () => {
  const { readFileSync } = await import("node:fs");
  const { dirname, join, resolve } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const verdict = classifyPin(readFileSync(join(root, "Cargo.toml"), "utf8"));
  assert.equal(verdict.ok, true, verdict.reason);
});
