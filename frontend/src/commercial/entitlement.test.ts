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
  FEATURE_MIN_TIER,
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

// --- featuresForTier (tier >= MIN monotonic gating) --------------------------

test("free unlocks no premium features", () => {
  assert.deepEqual(featuresForTier(TIER.free), []);
});

test("team unlocks reporting only", () => {
  assert.deepEqual(featuresForTier(TIER.team).sort(), ["reporting"]);
});

test("enterprise unlocks reporting, relay and dam", () => {
  assert.deepEqual(featuresForTier(TIER.enterprise).sort(), ["dam", "relay", "reporting"]);
});

test("staff/superadmin are strict supersets of enterprise", () => {
  for (const t of [TIER.staff, TIER.superadmin]) {
    const f = featuresForTier(t).sort();
    assert.deepEqual(f, ["dam", "relay", "reporting"]);
  }
});

test("a reserved-band tier (40) gates exactly like its ordinal", () => {
  // 40 >= team(20) but < enterprise(30)? No — 40 >= 30, so it unlocks all three.
  assert.deepEqual(featuresForTier(40).sort(), ["dam", "relay", "reporting"]);
  // 25 is between team and enterprise: reporting only.
  assert.deepEqual(featuresForTier(25).sort(), ["reporting"]);
});

test("FEATURE_MIN_TIER thresholds are the documented ones", () => {
  assert.equal(FEATURE_MIN_TIER.reporting, TIER.team);
  assert.equal(FEATURE_MIN_TIER.relay, TIER.enterprise);
  assert.equal(FEATURE_MIN_TIER.dam, TIER.enterprise);
});

// --- bootstrapAccountsEntitlements (canonical claim → injection slot) ---------

test("bootstrap resolves monotonic features from the tier ordinal", () => {
  const out = bootstrapAccountsEntitlements({ tier: TIER.team, tier_id: "team" });
  assert.deepEqual(out.sort(), ["reporting"]);
  assert.ok(isEntitled("reporting"));
  assert.ok(!isEntitled("relay"));
});

test("bootstrap reads canonical integer tier directly (no plan translation)", () => {
  bootstrapAccountsEntitlements({ tier: 30 });
  assert.ok(isEntitled("reporting"));
  assert.ok(isEntitled("relay"));
  assert.ok(isEntitled("dam"));
});

test("bootstrap accepts a legacy string tier", () => {
  bootstrapAccountsEntitlements({ tier: "enterprise" });
  assert.ok(isEntitled("dam"));
});

test("non-monotonic add-ons from features[] pass through verbatim", () => {
  const out = bootstrapAccountsEntitlements({ tier: TIER.team, features: [FEATURE_BYOK] });
  assert.ok(out.includes(FEATURE_BYOK));
  assert.ok(out.includes("reporting"));
});

test("bootstrap UNIONS with already-injected entitlements (only adds)", () => {
  // Simulate an offline license already mirrored into the slot.
  (globalThis as unknown as { window: { __LOREGUI_ENTITLEMENTS__?: string[] } }).window
    .__LOREGUI_ENTITLEMENTS__ = ["reporting"];
  const out = bootstrapAccountsEntitlements({ tier: TIER.enterprise });
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
