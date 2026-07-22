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

const EXPECTED = "826ad5d20ff4f5814101c946df127cef8253ada3";
const WRONG = "9179c6dc7cd14931af5b66beb3b2e186907f6360";

function fixture({ lore = EXPECTED, quinn = EXPECTED, lockLore = EXPECTED, lockQuinn = EXPECTED } = {}) {
  const root = mkdtempSync(join(tmpdir(), "loregui-authless-pin-"));
  writeFileSync(
    join(root, "Cargo.toml"),
    `[workspace.dependencies]\n` +
      `lore = { git = "https://github.com/EpicGames/lore.git", rev = "${lore}" }\n\n` +
      `[patch.crates-io]\n` +
      (quinn === null
        ? ""
        : `quinn-proto = { git = "https://github.com/EpicGames/lore.git", rev = "${quinn}" }\n`),
  );
  writeFileSync(
    join(root, "Cargo.lock"),
    `version = 4\n\n[[package]]\nname = "lore"\nversion = "0.8.6-nightly"\n` +
      `source = "git+https://github.com/EpicGames/lore.git?rev=${lore}#${lockLore}"\n\n` +
      `[[package]]\nname = "quinn-proto"\nversion = "0.11.13"\n` +
      `source = "git+https://github.com/EpicGames/lore.git?rev=${quinn ?? EXPECTED}#${lockQuinn}"\n`,
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

test("fails closed when the quinn-proto patch pin is missing", () => {
  assert.throws(
    () => verifyManifestAndLock(fixture({ quinn: null }), EXPECTED),
    /quinn-proto.*missing/i,
  );
});

test("fails closed when the lore manifest pin is wrong", () => {
  assert.throws(
    () => verifyManifestAndLock(fixture({ lore: WRONG }), EXPECTED),
    /lore manifest pin.*9179c6d.*826ad5d/i,
  );
});

test("fails closed when the resolved lore lock source is wrong", () => {
  assert.throws(
    () => verifyManifestAndLock(fixture({ lockLore: WRONG }), EXPECTED),
    /lore lock source.*9179c6d.*826ad5d/i,
  );
});

test("fails closed when the resolved quinn-proto lock source is wrong", () => {
  assert.throws(
    () => verifyManifestAndLock(fixture({ lockQuinn: WRONG }), EXPECTED),
    /quinn-proto lock source.*9179c6d.*826ad5d/i,
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
