/**
 * Unit tests for the offline signed license verifier (SBAI-4068).
 *
 * Run with Node's built-in test runner (type-stripping handles the .ts import):
 *   node --test --experimental-strip-types frontend/src/commercial/license.test.ts
 *
 * A dedicated TEST keypair is generated/embedded HERE ONLY — it is unrelated to
 * the public key shipped in `license.ts` and never leaves this file. We inject
 * its public half via the `__setVerifyKeyForTests` seam so we exercise the real
 * WebCrypto verification path with a key whose private half we control.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { createPrivateKey, sign as edSign } from "node:crypto";

import {
  verifyLicense,
  __setVerifyKeyForTests,
  __setNowForTests,
} from "./license.ts";

// --- Test keypair (THIS FILE ONLY — not the shipped key) ---------------------
// Raw 32-byte Ed25519 seed + matching raw 32-byte public key, base64url.
const TEST_PRIVATE_SEED_B64URL = "G13yonnUXKISUWd9fz0mR9HPEseP62s62xiB2D4sAZA";
const TEST_PUBLIC_B64URL = "O4XX8oJhN9lelj1x6RZm2DO_EWXsofN3u4CJApnhEww";

// A different, unrelated public key used for the "wrong key" case.
const OTHER_PUBLIC_B64URL = "UgsjCegtHciM5_idRfCsnFG_AR4hJc2QhwnuH5PeBeg";

function b64urlToBytes(s: string): Uint8Array {
  const n = s.replace(/-/g, "+").replace(/_/g, "/");
  const pad = n.length % 4 === 0 ? "" : "=".repeat(4 - (n.length % 4));
  return new Uint8Array(Buffer.from(n + pad, "base64"));
}

function b64url(bytes: Uint8Array | Buffer): string {
  return Buffer.from(bytes)
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

function testPrivateKey() {
  const seed = Buffer.from(b64urlToBytes(TEST_PRIVATE_SEED_B64URL));
  const pkcs8Prefix = Buffer.from("302e020100300506032b657004220420", "hex");
  return createPrivateKey({
    key: Buffer.concat([pkcs8Prefix, seed]),
    format: "der",
    type: "pkcs8",
  });
}

/** Sign a payload object with the TEST private key → compact token. */
function mintWithTestKey(payload: Record<string, unknown>): string {
  const payloadSeg = b64url(Buffer.from(JSON.stringify(payload), "utf8"));
  const sig = edSign(null, Buffer.from(payloadSeg, "utf8"), testPrivateKey());
  return `${payloadSeg}.${b64url(sig)}`;
}

async function importPublic(b64: string): Promise<CryptoKey> {
  return crypto.subtle.importKey("raw", b64urlToBytes(b64), { name: "Ed25519" }, false, ["verify"]);
}

const NOW = 1_800_000_000; // fixed "current" time (seconds) for determinism
const validPayload = {
  licensee: "Test Studio",
  tier: "team",
  features: ["reporting"],
  issuedAt: NOW - 86_400,
  expiresAt: NOW + 86_400,
};

test.beforeEach(async () => {
  __setNowForTests(() => NOW);
  __setVerifyKeyForTests(await importPublic(TEST_PUBLIC_B64URL));
});

test.afterEach(() => {
  __setNowForTests(null);
  __setVerifyKeyForTests(null);
});

test("valid license → returns its features", async () => {
  const token = mintWithTestKey(validPayload);
  const features = await verifyLicense(token);
  assert.deepEqual(features, ["reporting"]);
});

test("multiple features round-trip", async () => {
  const token = mintWithTestKey({ ...validPayload, features: ["reporting", "future-thing"] });
  assert.deepEqual(await verifyLicense(token), ["reporting", "future-thing"]);
});

test("tampered payload → null (signature no longer matches)", async () => {
  const token = mintWithTestKey(validPayload);
  const [payloadSeg, sigSeg] = token.split(".");
  // Flip the payload to grant an extra feature, keep the original signature.
  const tampered = { ...validPayload, features: ["reporting", "stolen"] };
  void payloadSeg;
  const forgedSeg = b64url(Buffer.from(JSON.stringify(tampered), "utf8"));
  const forged = `${forgedSeg}.${sigSeg}`;
  assert.equal(await verifyLicense(forged), null);
});

test("tampered signature bytes → null", async () => {
  const token = mintWithTestKey(validPayload);
  const [payloadSeg, sigSeg] = token.split(".");
  const sigBytes = b64urlToBytes(sigSeg);
  sigBytes[0] ^= 0xff; // corrupt one byte
  assert.equal(await verifyLicense(`${payloadSeg}.${b64url(sigBytes)}`), null);
});

test("expired license → null", async () => {
  const token = mintWithTestKey({
    ...validPayload,
    issuedAt: NOW - 2 * 86_400,
    expiresAt: NOW - 86_400, // already past
  });
  assert.equal(await verifyLicense(token), null);
});

test("expiresAt not after issuedAt → null", async () => {
  const token = mintWithTestKey({ ...validPayload, issuedAt: NOW, expiresAt: NOW });
  assert.equal(await verifyLicense(token), null);
});

test("wrong verify key → null (signed by test key, verified by other key)", async () => {
  const token = mintWithTestKey(validPayload);
  __setVerifyKeyForTests(await importPublic(OTHER_PUBLIC_B64URL));
  assert.equal(await verifyLicense(token), null);
});

test("malformed tokens → null", async () => {
  for (const bad of ["", "no-dot", ".onlysig", "onlypayload.", "a.b.c", null, undefined]) {
    assert.equal(await verifyLicense(bad as string), null, `expected null for ${JSON.stringify(bad)}`);
  }
});

test("payload missing required fields → null", async () => {
  const token = mintWithTestKey({ licensee: "x", tier: "team" }); // no features/dates
  assert.equal(await verifyLicense(token), null);
});

test("non-base64url signature segment → null", async () => {
  const token = mintWithTestKey(validPayload);
  const payloadSeg = token.split(".")[0];
  assert.equal(await verifyLicense(`${payloadSeg}.!!!not-base64!!!`), null);
});
