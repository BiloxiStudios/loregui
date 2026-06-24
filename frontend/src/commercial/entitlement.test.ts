/**
 * Unit tests for the canonical entitlement model (SBAI-4089 / E0.7).
 *
 * Run with Node's built-in test runner (type-stripping handles the .ts import):
 *   node --test --experimental-strip-types frontend/src/commercial/entitlement.test.ts
 *
 * Covers the canonical `tier` + `features[]` model (ADR-0001 §2.5; SBAI-4170
 * re-correction). The ONE gating rule (CLAUDE.md "Entitlement vs Plan",
 * SBAI-4165): gate on `features[]` ONLY — never re-derive a feature→tier map.
 * accounts' `config/entitlement.py` (SBAI-4167) mints the FULL resolved
 * `features[]` via `features_for_tier()`, so LoreGUI gates off that list verbatim:
 *   - the LOCKED ordinal scheme (for the `tier_id` DISPLAY label),
 *   - EVERY gateable feature (reporting/lore_relay/dam/byok) gated PURELY off the
 *     minted `features[]` claim — NO local min-tier map of any kind,
 *   - a high `tier` with empty `features[]` unlocks NOTHING (gating is
 *     features[]-only; tier is display-only),
 *   - legacy-string + numeric-string tier normalisation (PLAN_TO_TIER back-compat,
 *     for the display label only; SBAI-4168 DROPPED, plan retained as billing SKU),
 *   - bootstrapAccountsEntitlements unions features[] verbatim into the slot,
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

// --- LOCKED ordinal scheme (cross-repo contract; display label only) ---------

test("tier ordinals match the LOCKED scheme", () => {
  assert.equal(TIER.free, 0);
  assert.equal(TIER.indie, 10);
  assert.equal(TIER.team, 20);
  assert.equal(TIER.enterprise, 30);
  assert.equal(TIER.staff, 90);
  assert.equal(TIER.superadmin, 99);
});

test("tier ids are the stable strings (used for display, not gating)", () => {
  assert.equal(TIER_ID[TIER.free], "free");
  assert.equal(TIER_ID[TIER.indie], "indie");
  assert.equal(TIER_ID[TIER.team], "team");
  assert.equal(TIER_ID[TIER.enterprise], "enterprise");
  assert.equal(TIER_ID[TIER.staff], "staff");
  assert.equal(TIER_ID[TIER.superadmin], "superadmin");
});

// --- tierOrdinal normalisation (for the tier_id DISPLAY label only) ----------

test("tierOrdinal accepts a canonical integer", () => {
  assert.equal(tierOrdinal(30), 30);
  assert.equal(tierOrdinal(0), 0);
});

test("tierOrdinal accepts a numeric string", () => {
  assert.equal(tierOrdinal("20"), 20);
});

test("tierOrdinal maps legacy plan strings (PLAN_TO_TIER back-compat — SBAI-4168 DROPPED)", () => {
  // PLAN_TO_TIER is intentionally still present: `plan` is retained as the
  // billing SKU (SBAI-4168 was dropped). It normalises a legacy string tier for
  // the DISPLAY label only — it is NEVER gated on.
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

// --- features[]-ONLY gating (CLAUDE.md SBAI-4165) ----------------------------
// accounts (config/entitlement.py, SBAI-4167) mints the FULL resolved features[]
// via features_for_tier(); LoreGUI gates off that list verbatim and never
// re-derives feature→tier. There is NO local min-tier map.

test("features:['reporting'] entitles reporting, NOT lore_relay/dam", () => {
  bootstrapAccountsEntitlements({ tier: TIER.team, tier_id: "team", features: ["reporting"] });
  assert.ok(isEntitled("reporting"));
  assert.ok(!isEntitled("lore_relay"));
  assert.ok(!isEntitled("dam"));
});

test("the full minted superset entitles reporting + lore_relay + dam + byok", () => {
  const out = bootstrapAccountsEntitlements({
    tier: TIER.enterprise,
    tier_id: "enterprise",
    features: ["reporting", "lore_relay", "dam", FEATURE_BYOK],
  });
  assert.deepEqual([...out].sort(), ["byok", "dam", "lore_relay", "reporting"]);
  assert.ok(isEntitled("reporting"));
  assert.ok(isEntitled("lore_relay"));
  assert.ok(isEntitled("dam"));
  assert.ok(isEntitled("byok"));
});

test("empty features[] entitles NOTHING — even with a high tier claim (gating is features[]-only)", () => {
  // tier is DISPLAY-only; a superadmin tier with no minted features unlocks none.
  const out = bootstrapAccountsEntitlements({
    tier: TIER.superadmin,
    tier_id: "superadmin",
    features: [],
  });
  assert.deepEqual(out, []);
  assert.ok(!isEntitled("reporting"));
  assert.ok(!isEntitled("lore_relay"));
  assert.ok(!isEntitled("dam"));
  assert.ok(!isEntitled("byok"));
});

test("byok is entitled only when minted into features[], denied otherwise", () => {
  bootstrapAccountsEntitlements({ tier: TIER.enterprise, features: [FEATURE_BYOK] });
  assert.ok(isEntitled("byok"));
  // re-resolve with no minted features at the same tier -> denied.
  resetWindow();
  bootstrapAccountsEntitlements({ tier: TIER.enterprise, features: [] });
  assert.ok(!isEntitled("byok"));
});

test("lore_relay requires the minted id, not a tier threshold", () => {
  // a high tier with no lore_relay minted does NOT unlock it.
  bootstrapAccountsEntitlements({ tier: TIER.enterprise, features: ["reporting"] });
  assert.ok(!isEntitled("lore_relay"));
  // minting the id unlocks it.
  resetWindow();
  bootstrapAccountsEntitlements({ tier: TIER.free, features: ["lore_relay"] });
  assert.ok(isEntitled("lore_relay"));
});

// --- tier_id passes through for DISPLAY --------------------------------------

test("tier_id is available for the display label (not used for gating)", () => {
  const claim = { tier: TIER.enterprise, tier_id: "enterprise", features: [] as string[] };
  assert.equal(claim.tier_id, "enterprise");
  assert.equal(TIER_ID[tierOrdinal(claim.tier)], "enterprise");
});

// --- bootstrap edge cases ----------------------------------------------------

test("bootstrap UNIONS with already-injected entitlements (only adds)", () => {
  // Simulate an offline license already mirrored into the slot.
  (globalThis as unknown as { window: { __LOREGUI_ENTITLEMENTS__?: string[] } }).window
    .__LOREGUI_ENTITLEMENTS__ = ["dam"];
  const out = bootstrapAccountsEntitlements({
    tier: TIER.team,
    features: ["reporting", FEATURE_BYOK],
  });
  assert.ok(out.includes("dam")); // from the pre-injected license
  assert.ok(out.includes("reporting")); // minted
  assert.ok(out.includes("byok")); // minted
});

test("a null/garbage claim contributes nothing", () => {
  const out = bootstrapAccountsEntitlements(null);
  assert.deepEqual(out, []);
  assert.ok(!isEntitled("reporting"));
});

test("a claim with no features contributes nothing (tier alone unlocks nothing)", () => {
  const out = bootstrapAccountsEntitlements({ tier: TIER.enterprise });
  assert.deepEqual(out, []);
  assert.ok(!isEntitled("reporting"));
  assert.ok(!isEntitled("byok"));
});
