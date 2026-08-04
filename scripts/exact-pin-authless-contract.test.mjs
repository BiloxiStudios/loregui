#!/usr/bin/env node
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  verifyManifestAndLock,
  verifyUpstreamAuthlessSource,
} from "./exact-pin-authless-contract.mjs";

const EXPECTED = "ba92f94305df15796283755040c0bdd9b351841e";
// SBAI-5910: the accepted source is the BiloxiStudios maintenance fork —
// fixtures must encode the CURRENT pin or they falsely pass forever.
const EXPECTED_HOST = "https://github.com/BiloxiStudios/lore.git";
// The pre-5910 upstream, used as the wrong-host fixture below.
const OLD_HOST = "https://github.com/EpicGames/lore.git";
const WRONG = "9179c6dc7cd14931af5b66beb3b2e186907f6360";

function fixture({ lore = EXPECTED, quinn = EXPECTED, lockLore = EXPECTED, lockQuinn = EXPECTED, host } = {}) {
  const root = mkdtempSync(join(tmpdir(), "loregui-authless-pin-"));
  writeFileSync(
    join(root, "Cargo.toml"),
    `[workspace.dependencies]\n` +
      `lore = { git = "${host ?? EXPECTED_HOST}", rev = "${lore}" }\n\n` +
      `[patch.crates-io]\n` +
      (quinn === null
        ? ""
        : `quinn-proto = { git = "${host ?? EXPECTED_HOST}", rev = "${quinn}" }\n`),
  );
  writeFileSync(
    join(root, "Cargo.lock"),
    `version = 4\n\n[[package]]\nname = "lore"\nversion = "0.8.6-nightly"\n` +
      `source = "git+${host ?? EXPECTED_HOST}?rev=${lore}#${lockLore}"\n\n` +
      `[[package]]\nname = "quinn-proto"\nversion = "0.11.13"\n` +
      `source = "git+${host ?? EXPECTED_HOST}?rev=${quinn ?? EXPECTED}#${lockQuinn}"\n`,
  );
  return root;
}

function upstreamFixture({ exchangeCount = 2, forwardCount = 3, legacyRemap = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), "loregui-authless-source-"));
  const exchangeDir = join(root, "lore-transport", "src", "auth");
  const userInfoDir = join(root, "lore-revision", "src", "auth");
  mkdirSync(exchangeDir, { recursive: true });
  mkdirSync(userInfoDir, { recursive: true });
  const exchange = Array.from(
    { length: exchangeCount },
    () =>
      `return Err(NotSupported { operation: "No authentication configured on server".to_string() }.into());`,
  ).join("\n");
  const forward = Array.from(
    { length: forwardCount },
    () => `.forward::<UserInfoError>("Failed authorization token exchange")?;`,
  ).join("\n");
  writeFileSync(join(exchangeDir, "exchange.rs"), exchange);
  writeFileSync(
    join(userInfoDir, "userinfo.rs"),
    `${forward}\n${legacyRemap ? ".debug_map_err(UserInfoError::from(NotAuthenticated))?;" : ""}`,
  );
  return root;
}

test("accepts only the exact dual manifest and lock pin", () => {
  assert.doesNotThrow(() => verifyManifestAndLock(fixture(), EXPECTED));
});

test("fails closed when both pins move together to the wrong host", () => {
  // SBAI-5910: consistency is not a guard — a lockstep move back to the
  // pre-fix upstream (or any other fork) must fail, because the SBAI-5909
  // credential fix exists only on the accepted maintenance fork.
  assert.throws(
    () => verifyManifestAndLock(fixture({ host: OLD_HOST }), EXPECTED),
    /manifest host .* does not equal required/,
  );
});

test("fails closed when the quinn-proto patch pin is missing", () => {
  assert.throws(
    () => verifyManifestAndLock(fixture({ quinn: null }), EXPECTED),
    // Table-aware reader (SBAI-5910) names the exact table + key.
    /(no quinn-proto entry|quinn-proto.*missing)/i,
  );
});

// Negative-case regexes derive the required-rev prefix from EXPECTED so a pin
// bump only has to move the constant (SBAI-5594).
const EXPECTED_SHORT = EXPECTED.slice(0, 7);

test("fails closed when the lore manifest pin is wrong", () => {
  assert.throws(
    () => verifyManifestAndLock(fixture({ lore: WRONG }), EXPECTED),
    new RegExp(`lore manifest pin.*9179c6d.*${EXPECTED_SHORT}`, "i"),
  );
});

test("fails closed when the resolved lore lock source is wrong", () => {
  assert.throws(
    () => verifyManifestAndLock(fixture({ lockLore: WRONG }), EXPECTED),
    new RegExp(`lore lock source.*9179c6d.*${EXPECTED_SHORT}`, "i"),
  );
});

test("fails closed when the resolved quinn-proto lock source is wrong", () => {
  assert.throws(
    () => verifyManifestAndLock(fixture({ lockQuinn: WRONG }), EXPECTED),
    new RegExp(`quinn-proto lock source.*9179c6d.*${EXPECTED_SHORT}`, "i"),
  );
});

test("accepts the exact exchange wire operation and three user-info forwarders", () => {
  assert.doesNotThrow(() => verifyUpstreamAuthlessSource(upstreamFixture()));
});

test("rejects a checkout missing either authless exchange branch", () => {
  assert.throws(
    () => verifyUpstreamAuthlessSource(upstreamFixture({ exchangeCount: 1 })),
    /two typed NotSupported authless exchange branches/i,
  );
});

test("rejects a checkout that still remaps user-info to NotAuthenticated", () => {
  assert.throws(
    () => verifyUpstreamAuthlessSource(upstreamFixture({ legacyRemap: true })),
    /legacy NotAuthenticated remap/i,
  );
});

test("rejects a checkout missing any user-info exchange forwarder", () => {
  assert.throws(
    () => verifyUpstreamAuthlessSource(upstreamFixture({ forwardCount: 2 })),
    /three forwarded user-info exchange errors/i,
  );
});
