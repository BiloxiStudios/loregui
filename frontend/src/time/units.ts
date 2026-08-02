/**
 * SBAI-5905 — the declared unit policy for every timestamp the GUI renders.
 *
 * Epic `6fd18e6` moved `Repository.created`, `Branch.created` and the
 * thin-client `Revision.timestamp` from Unix epoch **seconds** to
 * **milliseconds** without changing their field types. Nothing failed to
 * parse; values simply render about a thousandfold into the future. Stored
 * history therefore holds both units, so those fields are *mixed* and must be
 * classified per value.
 *
 * This mirrors `crates/lore-vm/src/time_units.rs`. The threshold and the
 * policy live in exactly two places — one per language — and must not be
 * re-derived at a call site. The bug this replaces was born precisely that
 * way: three local `fmtTime` helpers, each assuming a unit, two of which
 * disagreed about the same field.
 *
 * Validation lives HERE rather than in Rust, deliberately. Rust receives
 * `u64`, so negative, fractional and non-finite values are unrepresentable
 * there and tests for them would be unreachable. This is the boundary where
 * such values can actually arrive — from JSON, from a hand-edited config, or
 * from a backend that changed shape — so this is where they are rejected.
 */

/**
 * At or above this a value is already canonical milliseconds; below it, it is
 * Unix seconds.
 *
 * `100_000_000_000` ms is 1973-03-03; read as *seconds* it would be the year
 * 5138. Every timestamp this product handles falls unambiguously on one side,
 * so the split classifies rather than guesses.
 */
export const SECONDS_MS_THRESHOLD = 100_000_000_000;

/** Largest millisecond value that survives a JSON round-trip intact. */
export const MAX_SAFE_MS = Number.MAX_SAFE_INTEGER;

/** Raised instead of returning a plausible-but-wrong instant. */
export class TimestampUnitError extends Error {
  constructor(field: string, detail: string) {
    super(`${field}: ${detail}`);
    this.name = "TimestampUnitError";
  }
}

function assertRepresentable(field: string, raw: number): void {
  if (typeof raw !== "number" || !Number.isFinite(raw)) {
    throw new TimestampUnitError(field, `timestamp is not a finite number (${raw})`);
  }
  if (!Number.isInteger(raw)) {
    throw new TimestampUnitError(field, `timestamp is not an integer (${raw})`);
  }
  if (raw < 0) {
    throw new TimestampUnitError(field, `timestamp is negative (${raw})`);
  }
  if (!Number.isSafeInteger(raw)) {
    throw new TimestampUnitError(
      field,
      `timestamp ${raw} exceeds the safe-integer range and has already lost precision`,
    );
  }
}

function bounded(field: string, canonical: number): number {
  if (canonical > MAX_SAFE_MS) {
    throw new TimestampUnitError(
      field,
      `normalizes to ${canonical}ms, beyond the safe-integer limit ${MAX_SAFE_MS}`,
    );
  }
  return canonical;
}

/**
 * Normalize a **mixed-unit** field to canonical milliseconds.
 *
 * Returns `null` for `0`, which every caller here means as absent or
 * unbounded — never as the epoch.
 */
export function normalizeMixedMs(raw: number, field: string): number | null {
  assertRepresentable(field, raw);
  if (raw === 0) return null;
  if (raw < SECONDS_MS_THRESHOLD) {
    return bounded(field, raw * 1000);
  }
  return bounded(field, raw);
}

/**
 * Normalize a field that was **always** milliseconds.
 *
 * There is deliberately no seconds branch. `lock.locked_at` and lock-request
 * `createdAt` are written by upstream's `util::time::timestamp()` and read
 * back with `from_timestamp_millis`; a small value is a small instant, not a
 * seconds value, and scaling it would invent a date.
 */
export function normalizeStrictMs(raw: number, field: string): number | null {
  assertRepresentable(field, raw);
  if (raw === 0) return null;
  return bounded(field, raw);
}

/** Render a mixed-unit timestamp, or an em dash when it is absent. */
export function formatMixed(raw: number, field: string): string {
  const ms = normalizeMixedMs(raw, field);
  return ms === null ? "—" : new Date(ms).toLocaleString();
}

/** Render an always-milliseconds timestamp, or an em dash when absent. */
export function formatStrictMs(raw: number, field: string): string {
  const ms = normalizeStrictMs(raw, field);
  return ms === null ? "—" : new Date(ms).toLocaleString();
}
