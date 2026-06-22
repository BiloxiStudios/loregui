#!/usr/bin/env node
/**
 * Issue (mint) a LoreGUI commercial license token (SBAI-4068).
 *
 * Signs a `{ licensee, tier, features, issuedAt, expiresAt }` payload with the
 * Ed25519 PRIVATE signing key and prints a compact `payload.signature` token
 * (base64url(JSON) "." base64url(Ed25519 sig) — JWT-like, EdDSA). That exact
 * format is what `frontend/src/commercial/license.ts#verifyLicense` checks
 * against the embedded public key.
 *
 * The PRIVATE key is the licensing secret. It is read from the environment or a
 * file — it is NEVER read from, or written to, the repo:
 *   - `LOREGUI_LICENSE_PRIVATE_KEY`        raw 32-byte seed, base64url, OR
 *   - `LOREGUI_LICENSE_PRIVATE_KEY_FILE`   path to a file holding the same.
 * Source it from Vaultwarden / Azure Key Vault at issuing time; do not commit it.
 *
 * Usage:
 *   LOREGUI_LICENSE_PRIVATE_KEY=<b64url-seed> \
 *     node frontend/scripts/issue-license.mjs \
 *       --licensee "Acme Studios" \
 *       --tier team \
 *       --features reporting \
 *       --days 365
 *
 * Flags:
 *   --licensee <str>     who it's for (required)
 *   --tier <str>         tier label for display (default: "team")
 *   --features <csv>     entitlement ids granted (default: "reporting")
 *   --days <n>           validity window in days from now (default: 365)
 *   --expires <iso>      explicit expiry (ISO 8601), overrides --days
 *
 * This is signature-verified open-core entitlement, not anti-tamper DRM.
 */
import { createPrivateKey, sign as edSign } from "node:crypto";
import { readFileSync } from "node:fs";

function b64url(buf) {
  return Buffer.from(buf).toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith("--")) {
      const key = a.slice(2);
      const val = argv[i + 1] && !argv[i + 1].startsWith("--") ? argv[++i] : "true";
      out[key] = val;
    }
  }
  return out;
}

function loadPrivateSeed() {
  let seedB64 = process.env.LOREGUI_LICENSE_PRIVATE_KEY?.trim();
  const file = process.env.LOREGUI_LICENSE_PRIVATE_KEY_FILE?.trim();
  if (!seedB64 && file) seedB64 = readFileSync(file, "utf8").trim();
  if (!seedB64) {
    console.error(
      "ERROR: no private signing key. Set LOREGUI_LICENSE_PRIVATE_KEY (raw 32-byte\n" +
        "seed, base64url) or LOREGUI_LICENSE_PRIVATE_KEY_FILE. Get it from Vaultwarden.",
    );
    process.exit(1);
  }
  const seed = Buffer.from(seedB64.replace(/-/g, "+").replace(/_/g, "/"), "base64");
  if (seed.length !== 32) {
    console.error(`ERROR: private seed must be 32 bytes, got ${seed.length}.`);
    process.exit(1);
  }
  // Wrap the raw 32-byte seed in a PKCS8 DER so Node can import it as an Ed25519 key.
  const pkcs8Prefix = Buffer.from("302e020100300506032b657004220420", "hex");
  return createPrivateKey({
    key: Buffer.concat([pkcs8Prefix, seed]),
    format: "der",
    type: "pkcs8",
  });
}

const args = parseArgs(process.argv.slice(2));

if (!args.licensee) {
  console.error('ERROR: --licensee is required (e.g. --licensee "Acme Studios").');
  process.exit(1);
}

const licensee = args.licensee;
const tier = args.tier || "team";
const features = (args.features || "reporting")
  .split(/[,\s]+/)
  .map((s) => s.trim())
  .filter(Boolean);

const issuedAt = Math.floor(Date.now() / 1000);
let expiresAt;
if (args.expires) {
  const ms = Date.parse(args.expires);
  if (Number.isNaN(ms)) {
    console.error(`ERROR: --expires is not a valid ISO date: ${args.expires}`);
    process.exit(1);
  }
  expiresAt = Math.floor(ms / 1000);
} else {
  const days = Number(args.days ?? 365);
  if (!Number.isFinite(days) || days <= 0) {
    console.error(`ERROR: --days must be a positive number, got ${args.days}`);
    process.exit(1);
  }
  expiresAt = issuedAt + Math.round(days * 86400);
}

const payload = { licensee, tier, features, issuedAt, expiresAt };
const payloadSeg = b64url(Buffer.from(JSON.stringify(payload), "utf8"));

const privateKey = loadPrivateSeed();
// Ed25519 signs the raw bytes (no pre-hash, no algorithm arg).
const signature = edSign(null, Buffer.from(payloadSeg, "utf8"), privateKey);
const token = `${payloadSeg}.${b64url(signature)}`;

console.error("Minted license:");
console.error("  licensee:  " + licensee);
console.error("  tier:      " + tier);
console.error("  features:  " + features.join(", "));
console.error("  issuedAt:  " + new Date(issuedAt * 1000).toISOString());
console.error("  expiresAt: " + new Date(expiresAt * 1000).toISOString());
console.error("");
console.error("Token (give this to the studio; they set LOREGUI_LICENSE / license.key /");
console.error('localStorage["loregui.license"]):');
// The token itself goes to stdout so it can be piped/captured cleanly.
console.log(token);
