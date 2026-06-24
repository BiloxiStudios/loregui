/**
 * Unit tests for the canonical entitlement model (SBAI-4089 / E0.7).
 *
 * Run with Node's built-in test runner (type-stripping handles the .ts import):
 *   node --test --experimental-strip-types frontend/src/commercial/entitlement.test.ts
 *
 * Covers the canonical `tier` ordinal + `features[]` model (ADR-0001 §2.5):
 *   - the LOCKED ordinal scheme,
 *   - `tier >= MIN` monotonic gating via featuresForTier,
 *   - legacy-string + numeric-string tier normalisation,
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
  featuresForTier,
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

test("tierOrdinal maps legacy plan strings", () => {
  assert.equal(tierOrdinal("enterprise"), TIER.enterprise);
  assert.equal(tierOrdinal(" Team "), TIER.team);
  assert.equal(tierOrdinal("INDIE"), TIER.indie);
});

test("tierOrdinal falls back to free for unknown/missing", () => {
  assert.equal(tierOrdinal(null), TIER.free);
  assert.equal(tierOrdinal(undefined), TIER.free);
  assert.equal(tierOrdinal(""), TIER.free);
  assert.equal(tierOrdinal("nonsense"), TIER.free);
});

// --- featuresForTier (DEPRECATED) --------------------------------------------

test("featuresForTier now returns empty array (gating moved to accounts)", () => {
  assert.deepEqual(featuresForTier(TIER.free), []);
  assert.deepEqual(featuresForTier(TIER.team), []);
  assert.deepEqual(featuresForTier(TIER.enterprise), []);
});

// --- bootstrapAccountsEntitlements (canonical claim → injection slot) ---------

test("bootstrap NO LONGER resolves monotonic features from the tier ordinal", () => {
  const out = bootstrapAccountsEntitlements({ tier: TIER.team, tier_id: "team" });
  assert.deepEqual(out, []);
  assert.ok(!isEntitled("reporting"));
});

test("bootstrap relies on features[] from the claim", () => {
  const out = bootstrapAccountsEntitlements({ 
    tier: TIER.enterprise, 
    features: ["reporting", "relay", "dam"] 
  });
  assert.ok(out.includes("reporting"));
  assert.ok(out.includes("relay"));
  assert.ok(out.includes("dam"));
});

test("non-monotonic add-ons from features[] pass through verbatim", () => {
  const out = bootstrapAccountsEntitlements({ tier: TIER.team, features: [FEATURE_BYOK] });
  assert.ok(out.includes(FEATURE_BYOK));
  assert.ok(!out.includes("reporting")); // no longer derived from tier
});

test("bootstrap UNIONS with already-injected entitlements (only adds)", () => {
  // Simulate an offline license already mirrored into the slot.
  (globalThis as unknown as { window: { __LOREGUI_ENTITLEMENTS__?: string[] } }).window
    .__LOREGUI_ENTITLEMENTS__ = ["reporting"];
  const out = bootstrapAccountsEntitlements({ 
    tier: TIER.enterprise,
    features: ["relay", "dam"]
  });
  // license-provided reporting is preserved, accounts adds relay+dam.
  assert.ok(out.includes("reporting"));
  assert.ok(out.includes("relay"));
  assert.ok(out.includes("dam"));
});

test("a null/garbage claim contributes nothing", () => {
  const out = bootstrapAccountsEntitlements(null);
  assert.deepEqual(out, []);
  assert.ok(!isEntitled("reporting"));
});
