/**
 * Unit tests for the canonical entitlement model (SBAI-4089 / E0.7).
 *
 * Run with Node's built-in test runner (type-stripping handles the .ts import):
 *   node --test --experimental-strip-types frontend/src/commercial/entitlement.test.ts
 *
 * Covers the canonical `tier` ordinal + minted `features[]` model (ADR-0001
 * §2.5; SBAI-4170 / SBAI-4167 convergence):
 *   - the LOCKED ordinal scheme,
 *   - gating PURELY off the minted `features[]` claim (no local feature→tier),
 *   - legacy-string + numeric-string tier normalisation (PLAN_TO_TIER back-compat),
 *   - bootstrapAccountsEntitlements union into the runtime injection slot,
 *   - the gate (isEntitled) reading off the canonical resolution.
 *
 * These tests stub a minimal `window` so the injection-slot paths run under Node.
 */
import { test, beforeEach } from "node:test";
import assert from "node:assert/strict";

import {
  TIER,
  TIER_ID,
  FEATURE_BYOK,
  tierOrdinal,
  bootstrapAccountsEntitlements,
  isEntitled,
  __resetLicensedFeaturesForTests,
} from "./entitlement.ts";

// Minimal window shim so the injection-slot code paths run under node:test.
function resetWindow(): void {
  (globalThis as unknown as { window?: unknown }).window = {
    __LOREGUI_ENTITLEMENTS__: undefined,
  };
}

beforeEach(() => {
  resetWindow();
  __resetLicensedFeaturesForTests();
});

// --- LOCKED ordinal scheme (cross-repo contract) -----------------------------

test("tier ordinals match the LOCKED scheme", () => {
  assert.equal(TIER.free, 0);
  assert.equal(TIER.indie, 10);
  assert.equal(TIER.team, 20);
  assert.equal(TIER.enterprise, 30);
  assert.equal(TIER.staff, 90);
  assert.equal(TIER.superadmin, 99);
});

test("tier ids are the stable strings", () => {
  assert.equal(TIER_ID[TIER.free], "free");
  assert.equal(TIER_ID[TIER.indie], "indie");
  assert.equal(TIER_ID[TIER.team], "team");
  assert.equal(TIER_ID[TIER.enterprise], "enterprise");
  assert.equal(TIER_ID[TIER.staff], "staff");
  assert.equal(TIER_ID[TIER.superadmin], "superadmin");
});

// --- tierOrdinal normalisation ----------------------------------------------

test("tierOrdinal accepts a canonical integer", () => {
  assert.equal(tierOrdinal(30), 30);
  assert.equal(tierOrdinal(0), 0);
});

test("tierOrdinal accepts a numeric string", () => {
  assert.equal(tierOrdinal("20"), 20);
});

test("tierOrdinal maps legacy plan strings (PLAN_TO_TIER back-compat — removed under SBAI-4168)", () => {
  // PLAN_TO_TIER is intentionally still present until SBAI-4168 deprecates the
  // legacy `plan` string ecosystem-wide. These assert it still resolves.
  assert.equal(tierOrdinal("enterprise"), TIER.enterprise);
  assert.equal(tierOrdinal(" Team "), TIER.team);
  assert.equal(tierOrdinal("INDIE"), TIER.indie);
  assert.equal(tierOrdinal("free"), TIER.free);
  assert.equal(tierOrdinal("STAFF"), TIER.staff);
});

test("tierOrdinal falls back to free for unknown/missing", () => {
  assert.equal(tierOrdinal(null), TIER.free);
  assert.equal(tierOrdinal(undefined), TIER.free);
  assert.equal(tierOrdinal(""), TIER.free);
  assert.equal(tierOrdinal("nonsense"), TIER.free);
});

// --- gating off the minted features[] claim (SBAI-4170 / SBAI-4167) ----------
// LoreGUI no longer owns a feature→tier table; accounts mints the resolved
// feature ids onto features[]. These tests assert we gate purely off that array.

test("a JWT with features:['relay'] grants relay", () => {
  const out = bootstrapAccountsEntitlements({ tier: TIER.enterprise, features: ["relay"] });
  assert.ok(out.includes("relay"));
  assert.ok(isEntitled("relay"));
});

test("without relay in features[], relay is denied even at enterprise tier", () => {
  // The tier ordinal is NOT consulted for unlocks — only the minted array is.
  bootstrapAccountsEntitlements({ tier: TIER.enterprise, features: [] });
  assert.ok(!isEntitled("relay"));
  assert.ok(!isEntitled("dam"));
  assert.ok(!isEntitled("reporting"));
});

test("minted features[] are gated verbatim (reporting/relay/dam)", () => {
  const out = bootstrapAccountsEntitlements({
    tier_id: "enterprise",
    features: ["reporting", "relay", "dam"],
  });
  assert.deepEqual(out.sort(), ["dam", "relay", "reporting"]);
  assert.ok(isEntitled("reporting"));
  assert.ok(isEntitled("relay"));
  assert.ok(isEntitled("dam"));
});

test("dam is granted when minted, regardless of tier value", () => {
  bootstrapAccountsEntitlements({ tier: TIER.team, features: ["dam"] });
  assert.ok(isEntitled("dam"));
});

test("non-monotonic add-ons (byok) pass through verbatim", () => {
  const out = bootstrapAccountsEntitlements({ tier: TIER.team, features: [FEATURE_BYOK, "reporting"] });
  assert.ok(out.includes(FEATURE_BYOK));
  assert.ok(out.includes("reporting"));
});

test("bootstrap ignores the tier ordinal for unlocks (no local feature→tier)", () => {
  // A high tier with NO minted features unlocks nothing.
  const out = bootstrapAccountsEntitlements({ tier: TIER.superadmin });
  assert.deepEqual(out, []);
  assert.ok(!isEntitled("relay"));
});

test("bootstrap UNIONS with already-injected entitlements (only adds)", () => {
  // Simulate an offline license already mirrored into the slot.
  (globalThis as unknown as { window: { __LOREGUI_ENTITLEMENTS__?: string[] } }).window
    .__LOREGUI_ENTITLEMENTS__ = ["reporting"];
  const out = bootstrapAccountsEntitlements({ tier: TIER.enterprise, features: ["relay", "dam"] });
  // license-provided reporting is preserved, accounts adds the minted relay+dam.
  assert.ok(out.includes("reporting"));
  assert.ok(out.includes("relay"));
  assert.ok(out.includes("dam"));
});

test("a null/garbage claim contributes nothing", () => {
  const out = bootstrapAccountsEntitlements(null);
  assert.deepEqual(out, []);
  assert.ok(!isEntitled("reporting"));
});
