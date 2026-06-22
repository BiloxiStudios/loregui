#!/usr/bin/env node
/**
 * Generate an Ed25519 license keypair for LoreGUI's commercial entitlement gate
 * (SBAI-4068).
 *
 * Prints:
 *   - PUBLIC  key (raw 32-byte, base64url) — embed in `frontend/src/commercial/
 *     license.ts` as `LICENSE_PUBLIC_KEY_B64URL`. Safe to be public; it can only
 *     *verify* signatures.
 *   - PRIVATE key (raw 32-byte seed, base64url) — THE LICENSING SECRET. Store it
 *     ONLY in Vaultwarden / Azure Key Vault. NEVER commit it. Anyone holding it
 *     can mint licenses that unlock every premium surface.
 *
 * Usage:
 *   node frontend/scripts/gen-license-keypair.mjs
 *
 * This is signature-verified open-core entitlement, not anti-tamper DRM: the app
 * and the public key are public; only the private key is secret.
 */
import { generateKeyPairSync } from "node:crypto";

function b64url(buf) {
  return Buffer.from(buf).toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

const { publicKey, privateKey } = generateKeyPairSync("ed25519");

// Raw 32-byte key material lives in the trailing 32 bytes of the DER encodings.
const spki = publicKey.export({ type: "spki", format: "der" });
const pkcs8 = privateKey.export({ type: "pkcs8", format: "der" });
const rawPublic = spki.subarray(spki.length - 32);
const rawPrivate = pkcs8.subarray(pkcs8.length - 32);

console.log("LoreGUI license keypair (Ed25519)\n");
console.log("PUBLIC  (embed in license.ts as LICENSE_PUBLIC_KEY_B64URL):");
console.log("  " + b64url(rawPublic) + "\n");
console.log("PRIVATE (THE SECRET — store in Vaultwarden / Azure KV, NEVER commit):");
console.log("  " + b64url(rawPrivate) + "\n");
console.log("To mint a license with this key:");
console.log("  LOREGUI_LICENSE_PRIVATE_KEY=<private-above> \\");
console.log('    node frontend/scripts/issue-license.mjs --licensee "Acme" --tier team \\');
console.log("      --features reporting --days 365");
