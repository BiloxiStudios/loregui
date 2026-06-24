/**
 * Unit tests for the canonical entitlement model (SBAI-4089 / E0.7).
 *
 * Run with Node's built-in test runner (type-stripping handles the .ts import):
 *   node --test --experimental-strip-types frontend/src/commercial/entitlement.test.ts
 *
 * Covers the canonical `tier` ordinal + `features[]` HYBRID model (ADR-0001
 * §2.5; SBAI-4170 correction):
 *   - the LOCKED ordinal scheme,
 *   - MONOTONIC features (reporting/relay/dam) gated by `tier >= MIN` from the
 *     local MONOTONIC_FEATURE_MIN_TIER map (mirrors accounts config/entitlement.py),
 *   - NON-MONOTONIC add-ons (byok) gated PURELY off the minted `features[]` claim,
 *   - legacy-string + numeric-string tier normalisation (PLAN_TO_TIER back-compat),
 *   - bootstrapAccountsEntitlements union (both axes) into the runtime slot,
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
  MONOTONIC_FEATURE_MIN_TIER,
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

// --- monotonic feature min-tiers (mirror accounts config/entitlement.py) ------

test("monotonic feature min-tiers match the canonical thresholds", () => {
  assert.equal(MONOTONIC_FEATURE_MIN_TIER.reporting, TIER.team);
  assert.equal(MONOTONIC_FEATURE_MIN_TIER.relay, TIER.enterprise);
  assert.equal(MONOTONIC_FEATURE_MIN_TIER.dam, TIER.enterprise);
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

// --- HYBRID gating: monotonic = tier-gated, non-monotonic = features[] --------
// accounts tier-gates reporting/relay/dam (never mints them into features[]) and
// mints only the non-monotonic add-ons (byok) onto features[]. LoreGUI mirrors.

test("team tier unlocks reporting but NOT relay/dam", () => {
  bootstrapAccountsEntitlements({ tier: TIER.team, features: [] });
  assert.ok(isEntitled("reporting"));
  assert.ok(!isEntitled("relay"));
  assert.ok(!isEntitled("dam"));
});

test("enterprise tier unlocks reporting + relay + dam (monotonic superset)", () => {
  const out = bootstrapAccountsEntitlements({ tier: TIER.enterprise, features: [] });
  assert.deepEqual([...out].sort(), ["dam", "relay", "reporting"]);
  assert.ok(isEntitled("reporting"));
  assert.ok(isEntitled("relay"));
  assert.ok(isEntitled("dam"));
});

test("indie tier unlocks none of the monotonic features", () => {
  bootstrapAccountsEntitlements({ tier: TIER.indie, features: [] });
  assert.ok(!isEntitled("reporting"));
  assert.ok(!isEntitled("relay"));
  assert.ok(!isEntitled("dam"));
});

test("free tier unlocks nothing", () => {
  const out = bootstrapAccountsEntitlements({ tier: TIER.free, features: [] });
  assert.deepEqual(out, []);
  assert.ok(!isEntitled("reporting"));
});

test("monotonic gating accepts a legacy/numeric-string tier", () => {
  bootstrapAccountsEntitlements({ tier: "enterprise", features: [] });
  assert.ok(isEntitled("relay"));
  bootstrapAccountsEntitlements({ tier: "20", features: [] });
  assert.ok(isEntitled("reporting"));
});

// --- non-monotonic add-on byok: from features[] ONLY, never tier-gated --------

test("byok requires features:['byok'] — granted when minted", () => {
  const out = bootstrapAccountsEntitlements({ tier: TIER.enterprise, features: [FEATURE_BYOK] });
  assert.ok(out.includes(FEATURE_BYOK));
  assert.ok(isEntitled("byok"));
});

test("byok is DENIED even at enterprise tier if not minted into features[]", () => {
  // byok is non-monotonic: it never comes from the tier ordinal, only the array.
  bootstrapAccountsEntitlements({ tier: TIER.enterprise, features: [] });
  assert.ok(!isEntitled("byok"));
});

test("byok is denied even at superadmin tier when not minted", () => {
  bootstrapAccountsEntitlements({ tier: TIER.superadmin, features: [] });
  assert.ok(!isEntitled("byok"));
});

// --- both axes resolve together ----------------------------------------------

test("enterprise + byok minted unlocks all monotonic features AND byok", () => {
  const out = bootstrapAccountsEntitlements({
    tier: TIER.enterprise,
    features: [FEATURE_BYOK],
  });
  assert.deepEqual([...out].sort(), ["byok", "dam", "relay", "reporting"]);
  assert.ok(isEntitled("reporting"));
  assert.ok(isEntitled("relay"));
  assert.ok(isEntitled("dam"));
  assert.ok(isEntitled("byok"));
});

test("team tier + byok minted unlocks reporting + byok, but NOT relay/dam", () => {
  bootstrapAccountsEntitlements({ tier: TIER.team, features: [FEATURE_BYOK] });
  assert.ok(isEntitled("reporting"));
  assert.ok(isEntitled("byok"));
  assert.ok(!isEntitled("relay"));
  assert.ok(!isEntitled("dam"));
});

// --- bootstrap edge cases ----------------------------------------------------

test("bootstrap UNIONS with already-injected entitlements (only adds)", () => {
  // Simulate an offline license already mirrored into the slot.
  (globalThis as unknown as { window: { __LOREGUI_ENTITLEMENTS__?: string[] } }).window
    .__LOREGUI_ENTITLEMENTS__ = ["dam"];
  const out = bootstrapAccountsEntitlements({ tier: TIER.team, features: [FEATURE_BYOK] });
  // license-provided dam is preserved; accounts adds tier-gated reporting + minted byok.
  assert.ok(out.includes("dam")); // from the pre-injected license
  assert.ok(out.includes("reporting")); // team tier
  assert.ok(out.includes("byok")); // minted add-on
});

test("a null/garbage claim contributes nothing", () => {
  const out = bootstrapAccountsEntitlements(null);
  assert.deepEqual(out, []);
  assert.ok(!isEntitled("reporting"));
});

test("a claim with no tier and no features contributes nothing", () => {
  const out = bootstrapAccountsEntitlements({});
  assert.deepEqual(out, []);
  assert.ok(!isEntitled("reporting"));
  assert.ok(!isEntitled("byok"));
});
