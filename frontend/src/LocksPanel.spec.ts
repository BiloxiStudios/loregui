/**
 * Unit tests for LocksPanel timestamp formatting (`fmtTime`, `toMs`).
 *
 * Regression cover for SBAI-5908: the original `fmtTime` double-scaled
 * millisecond timestamps by multiplying by 1000, turning a value like
 * 1718000000000 (June 2024) into 1718000000000000 (year ~56422).
 *
 * The fix introduces `toMs()` which heuristically detects the unit:
 * values > MS_THRESHOLD (year-2100 epoch in ms) are treated as milliseconds;
 * smaller values are assumed to be seconds and scaled up.
 */
import { describe, it, expect } from "vitest";
import { fmtTime, toMs, MS_THRESHOLD } from "./LocksPanel";

describe("toMs", () => {
  it("returns milliseconds unchanged when above threshold", () => {
    // June 2024 in ms
    const ms = 1_718_000_000_000;
    expect(toMs(ms)).toBe(ms);
  });

  it("scales seconds up when below threshold", () => {
    // June 2024 in seconds
    const secs = 1_718_000_000;
    expect(toMs(secs)).toBe(secs * 1000);
  });

  it("scales values at or below threshold", () => {
    // At exactly the threshold, value is still scaled (strict > comparison)
    expect(toMs(MS_THRESHOLD)).toBe(MS_THRESHOLD * 1000);
  });

  it("passes values above threshold unchanged", () => {
    expect(toMs(MS_THRESHOLD + 1)).toBe(MS_THRESHOLD + 1);
  });

  it("keeps zero as zero", () => {
    expect(toMs(0)).toBe(0);
  });

  it("handles a recent millisecond timestamp correctly", () => {
    // 2025-01-15 in ms
    const ms = 1_736_899_200_000;
    expect(toMs(ms)).toBe(ms);
  });

  it("scales a small seconds timestamp", () => {
    // A small value like 100 (clearly seconds, epoch + 100s)
    expect(toMs(100)).toBe(100_000);
  });
});

describe("fmtTime", () => {
  it("renders a millisecond timestamp as a valid date string containing the expected year", () => {
    // 1718000000000 = 2024-06-10T12:53:20Z
    const result = fmtTime(1_718_000_000_000);
    expect(result).not.toBe("\u2014");
    expect(result).toContain("2024");
  });

  it("renders a seconds timestamp correctly (legacy compat)", () => {
    // 1718000000 = 2024-06-10T12:53:20Z in seconds
    const result = fmtTime(1_718_000_000);
    expect(result).not.toBe("\u2014");
    expect(result).toContain("2024");
  });

  it("returns em-dash for zero", () => {
    expect(fmtTime(0)).toBe("\u2014");
  });

  it("returns em-dash for NaN", () => {
    expect(fmtTime(NaN)).toBe("\u2014");
  });

  it("produces the same year for equivalent ms and s inputs", () => {
    const msResult = fmtTime(1_718_000_000_000);
    const sResult = fmtTime(1_718_000_000);
    expect(msResult).toContain("2024");
    expect(sResult).toContain("2024");
  });
});

describe("no double-scaling regression", () => {
  it("ms value is NOT multiplied by 1000 again (year stays in 2020-2100)", () => {
    const ms = 1_718_000_000_000;
    const date = new Date(toMs(ms));
    const year = date.getFullYear();
    // If double-scaled, year would be ~56422; correct year is 2024
    expect(year).toBeGreaterThanOrEqual(2020);
    expect(year).toBeLessThanOrEqual(2100);
  });
});
