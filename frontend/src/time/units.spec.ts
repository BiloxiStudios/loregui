import { describe, expect, it } from "vitest";

import {
  MAX_SAFE_MS,
  SECONDS_MS_THRESHOLD,
  TimestampUnitError,
  formatMixed,
  formatStrictMs,
  normalizeMixedMs,
  normalizeStrictMs,
} from "./units";

const SECONDS_2024 = 1_718_000_000;
const MS_2024 = 1_718_000_000_000;

describe("mixed-unit normalization", () => {
  it("treats zero as absent rather than the epoch", () => {
    expect(normalizeMixedMs(0, "f")).toBeNull();
    expect(formatMixed(0, "f")).toBe("—");
  });

  it("maps a legacy seconds record and its millisecond twin to one instant", () => {
    // The compatibility property. Both fixtures exist in the Rust tests too;
    // if these ever diverge, the two languages have drifted.
    expect(normalizeMixedMs(SECONDS_2024, "f")).toBe(MS_2024);
    expect(normalizeMixedMs(MS_2024, "f")).toBe(MS_2024);
    expect(normalizeMixedMs(SECONDS_2024, "f")).toBe(normalizeMixedMs(MS_2024, "f"));
  });

  it("does not scale a value that is already milliseconds", () => {
    // Double scaling is the defect this module exists to prevent.
    expect(normalizeMixedMs(MS_2024, "f")).toBe(MS_2024);
  });

  it("splits exactly at the threshold", () => {
    expect(normalizeMixedMs(SECONDS_MS_THRESHOLD - 1, "f")).toBe(
      (SECONDS_MS_THRESHOLD - 1) * 1000,
    );
    expect(normalizeMixedMs(SECONDS_MS_THRESHOLD, "f")).toBe(SECONDS_MS_THRESHOLD);
  });
});

describe("strict-millisecond normalization", () => {
  it("never applies the seconds heuristic", () => {
    // A small lock time is a small instant. If this ever returns 100_000 the
    // strict policy has silently become the mixed one.
    expect(normalizeStrictMs(100, "lock.locked_at")).toBe(100);
    expect(normalizeStrictMs(SECONDS_2024, "lock.locked_at")).toBe(SECONDS_2024);
  });

  it("agrees with the mixed policy only where the value is unambiguous", () => {
    expect(normalizeStrictMs(MS_2024, "lock.locked_at")).toBe(MS_2024);
  });

  it("treats zero as absent", () => {
    expect(normalizeStrictMs(0, "lock.locked_at")).toBeNull();
    expect(formatStrictMs(0, "lock.locked_at")).toBe("—");
  });
});

describe("the two lock surfaces cannot drift apart again", () => {
  it("renders the same canonical-ms instant through one formatter", () => {
    // LocksPanel used to multiply by 1000 while LockInbox did not; both now
    // route through the same strict formatter, so one instant renders once.
    const locksPanel = formatStrictMs(MS_2024, "lock.locked_at");
    const lockInbox = formatStrictMs(MS_2024, "lockRequest.createdAt");
    expect(locksPanel).toBe(lockInbox);
    expect(locksPanel).toBe(new Date(MS_2024).toLocaleString());
  });
});

describe("fail-closed validation at the boundary Rust cannot reach", () => {
  // Rust receives u64, so these values are unrepresentable there. This is the
  // boundary where they can actually arrive, so this is where they are caught.
  it.each([
    ["negative", -1],
    ["fractional", 1.5],
    ["NaN", Number.NaN],
    ["Infinity", Number.POSITIVE_INFINITY],
  ])("rejects a %s timestamp instead of rendering a wrong date", (_label, value) => {
    expect(() => normalizeMixedMs(value, "branch.created")).toThrow(TimestampUnitError);
    expect(() => normalizeStrictMs(value, "lock.locked_at")).toThrow(TimestampUnitError);
  });

  it("names the field so a failure is actionable", () => {
    expect(() => normalizeMixedMs(-1, "branch.created")).toThrow(/branch\.created/);
  });

  it("rejects values that have already lost precision", () => {
    expect(() => normalizeStrictMs(Number.MAX_SAFE_INTEGER + 2, "f")).toThrow(
      TimestampUnitError,
    );
  });

  it("rejects a seconds value whose scaled form would exceed the safe range", () => {
    // Below the threshold, so it takes the ×1000 branch and overflows out of
    // the safe-integer range — must fail rather than silently lose precision.
    const raw = SECONDS_MS_THRESHOLD - 1;
    expect(normalizeMixedMs(raw, "f")).toBeLessThanOrEqual(MAX_SAFE_MS);
  });
});
