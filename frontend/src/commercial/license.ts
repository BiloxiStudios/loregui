/**
 * Offline signed license-key verification (SBAI-4068).
 *
 * ## What this is (and is NOT)
 *
 * LoreGUI is open core (MIT) — the source, including this verifier and the
 * embedded public key, is public. So this is **signature-verified entitlement,
 * not anti-tamper DRM**. A determined user with the source can patch the gate;
 * that is fine and expected for honest open-core licensing. What this buys us:
 *
 * - A studio that pays gets a *real*, portable, **offline** license token they
 *   can drop on any machine — no account, no network, no phone-home.
 * - The token is **cryptographically signed** by a key only Biloxi holds, so it
 *   cannot be forged or self-minted, and it carries an **expiry** so a lapsed
 *   subscription naturally stops unlocking premium surfaces.
 * - The public verify key ships in the binary (safe — it can only *verify*).
 *   Only the matching Ed25519 **private** key (kept in Vaultwarden / Azure Key
 *   Vault, never in this repo) can issue a license.
 *
 * This is the authoritative entitlement source today. The StudioBrain accounts
 * JWT (RS256 tier claim) is the planned *second* source — see `entitlement.ts`.
 *
 * ## Token format — `payload.signature` (JWT-like, EdDSA / Ed25519)
 *
 *   <base64url(JSON payload)> "." <base64url(Ed25519 signature)>
 *
 * The signature is over the raw UTF-8 bytes of the **base64url payload segment**
 * (i.e. everything before the dot), exactly as in a JWS detached-style scheme.
 * The payload JSON is {@link LicensePayload}:
 *
 *   { licensee, tier, features: string[], issuedAt, expiresAt }
 *
 * `issuedAt` / `expiresAt` are unix epoch **seconds**. `features` is the literal
 * list of entitlement ids this license grants (we do not re-derive them from
 * `tier` here — the issuer decides; `tier` is informational / for display).
 */

/** The signed claims inside a license token. */
export interface LicensePayload {
  /** Who the license was issued to (studio / org name). Informational. */
  licensee: string;
  /** Commercial tier label (free / team / enterprise). Informational/display. */
  tier: string;
  /** The entitlement feature ids this license unlocks. Authoritative. */
  features: string[];
  /** Issue time, unix epoch seconds. */
  issuedAt: number;
  /** Expiry time, unix epoch seconds. Past → license is rejected. */
  expiresAt: number;
}

/**
 * Embedded Ed25519 **public** verify key, raw 32 bytes, base64url.
 *
 * SAFE TO BE PUBLIC: a verify key can only check signatures, never produce them.
 * The matching PRIVATE signing key is the licensing secret and MUST live only in
 * Vaultwarden / Azure Key Vault — never in this repo. See
 * `docs/COMMERCIAL-ADDONS.md` and `scripts/gen-license-keypair.mjs`.
 *
 * NOTE: the value below is a **throwaway placeholder** generated for development
 * (its private half was discarded and is NOT the real production signing key).
 * Before shipping a commercial build, replace it with the public key whose
 * private half is stored in Vaultwarden, via `scripts/gen-license-keypair.mjs`.
 */
export const LICENSE_PUBLIC_KEY_B64URL = "UgsjCegtHciM5_idRfCsnFG_AR4hJc2QhwnuH5PeBeg";

/** localStorage key holding a raw license token string. */
export const LICENSE_STORAGE_KEY = "loregui.license";

/**
 * base64url → ArrayBuffer (tolerant of missing padding; throws on bad chars).
 * Returns an `ArrayBuffer` so the result is unambiguously a WebCrypto
 * `BufferSource` regardless of TS lib's `Uint8Array` generic.
 */
function b64urlDecode(input: string): ArrayBuffer {
  const normalized = input.replace(/-/g, "+").replace(/_/g, "/");
  const pad = normalized.length % 4 === 0 ? "" : "=".repeat(4 - (normalized.length % 4));
  const bin = atob(normalized + pad);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out.buffer;
}

/** UTF-8 encode to an ArrayBuffer (a clean WebCrypto `BufferSource`). */
function utf8Bytes(text: string): ArrayBuffer {
  const view = new TextEncoder().encode(text);
  return view.buffer.slice(view.byteOffset, view.byteOffset + view.byteLength) as ArrayBuffer;
}

/** Get a SubtleCrypto, or null if unavailable (e.g. non-secure context). */
function subtle(): SubtleCrypto | null {
  try {
    return globalThis.crypto?.subtle ?? null;
  } catch {
    return null;
  }
}

let cachedKey: Promise<CryptoKey | null> | null = null;

/** Import the embedded Ed25519 public key once, memoized. */
function importVerifyKey(): Promise<CryptoKey | null> {
  if (cachedKey) return cachedKey;
  cachedKey = (async () => {
    const s = subtle();
    if (!s) return null;
    try {
      const raw = b64urlDecode(LICENSE_PUBLIC_KEY_B64URL);
      return await s.importKey("raw", raw, { name: "Ed25519" }, false, ["verify"]);
    } catch {
      return null;
    }
  })();
  return cachedKey;
}

/** Test seam: override the verify key (unit tests inject a test public key). */
let keyOverride: CryptoKey | null = null;
/** @internal — for tests only. Pass a key to override, or null to restore. */
export function __setVerifyKeyForTests(key: CryptoKey | null): void {
  keyOverride = key;
}

/** Current time in unix seconds. Overridable for deterministic tests. */
let nowSeconds = (): number => Math.floor(Date.now() / 1000);
/** @internal — for tests only. */
export function __setNowForTests(fn: (() => number) | null): void {
  nowSeconds = fn ?? (() => Math.floor(Date.now() / 1000));
}

function isLicensePayload(value: unknown): value is LicensePayload {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.licensee === "string" &&
    typeof v.tier === "string" &&
    Array.isArray(v.features) &&
    v.features.every((f) => typeof f === "string") &&
    typeof v.issuedAt === "number" &&
    typeof v.expiresAt === "number"
  );
}

/**
 * Verify a license token. Returns the granted feature ids on success, or `null`
 * on ANY failure (malformed, bad signature, wrong key, expired, missing crypto).
 *
 * Failure is always silent + null so the caller falls back to open-core: an
 * invalid or absent license must NEVER break the free app, only withhold premium
 * surfaces.
 */
export async function verifyLicense(token: string | null | undefined): Promise<string[] | null> {
  if (!token || typeof token !== "string") return null;
  const dot = token.indexOf(".");
  if (dot <= 0 || dot === token.length - 1) return null;
  const payloadSeg = token.slice(0, dot);
  const sigSeg = token.slice(dot + 1);

  const key = keyOverride ?? (await importVerifyKey());
  if (!key) return null;
  const s = subtle();
  if (!s) return null;

  let signature: ArrayBuffer;
  let signedBytes: ArrayBuffer;
  let payload: unknown;
  try {
    signature = b64urlDecode(sigSeg);
    signedBytes = utf8Bytes(payloadSeg);
    payload = JSON.parse(new TextDecoder().decode(b64urlDecode(payloadSeg)));
  } catch {
    return null;
  }

  let valid = false;
  try {
    valid = await s.verify({ name: "Ed25519" }, key, signature, signedBytes);
  } catch {
    return null;
  }
  if (!valid) return null;

  if (!isLicensePayload(payload)) return null;
  // Expiry check (and a sane issuedAt <= expiresAt guard).
  if (!(payload.expiresAt > payload.issuedAt)) return null;
  if (payload.expiresAt <= nowSeconds()) return null;

  return [...payload.features];
}

/**
 * Read the raw license token from the ambient sources, in priority order:
 *
 *   1. `LOREGUI_LICENSE` build-time env (`import.meta.env.VITE_LOREGUI_LICENSE`).
 *   2. `localStorage["loregui.license"]`.
 *   3. A `license.key` file on disk, read via the optional `read_license_file`
 *      Tauri command (only if a `loadFile` reader is supplied).
 *
 * Returns the first non-empty token found, else null. This only *locates* a
 * token; {@link verifyLicense} decides whether it actually grants anything.
 */
export async function readLicenseToken(loadFile?: () => Promise<string | null>): Promise<string | null> {
  // 1. build-time env
  try {
    const env = (import.meta.env?.VITE_LOREGUI_LICENSE as string | undefined)?.trim();
    if (env) return env;
  } catch {
    /* import.meta.env may be absent outside Vite */
  }
  // 2. localStorage
  try {
    const ls = globalThis.localStorage?.getItem(LICENSE_STORAGE_KEY)?.trim();
    if (ls) return ls;
  } catch {
    /* storage may be unavailable */
  }
  // 3. on-disk license.key via host-provided reader (Tauri command)
  if (loadFile) {
    try {
      const file = (await loadFile())?.trim();
      if (file) return file;
    } catch {
      /* reader failure → treat as no file */
    }
  }
  return null;
}

/**
 * Convenience: locate AND verify in one call. Returns granted features or null.
 * Used by the entitlement bootstrap to populate the runtime entitlement slot.
 */
export async function resolveLicensedFeatures(
  loadFile?: () => Promise<string | null>,
): Promise<string[] | null> {
  const token = await readLicenseToken(loadFile);
  return verifyLicense(token);
}
