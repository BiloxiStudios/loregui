//! SBAI-5905 — the single source of truth for timestamp units.
//!
//! Epic `6fd18e6` moved `Repository.created`, `Branch.created` and the
//! thin-client `Revision.timestamp` from Unix epoch **seconds** to
//! **milliseconds**, without changing their field types. Nothing fails to
//! deserialize; values simply render about a thousandfold into the future.
//! Stored history therefore contains both units, so a field that predates the
//! change is *mixed* and must be classified per value.
//!
//! Two policies, and they are deliberately not interchangeable:
//!
//! - **Mixed** (`normalize_mixed`) — for fields whose stored history spans the
//!   change: `Repository.created`, `Branch.created`, revision metadata
//!   timestamps. A value below [`SECONDS_MS_THRESHOLD`] is read as seconds and
//!   scaled; at or above it, the value is already canonical milliseconds.
//! - **Strict milliseconds** (`normalize_strict_ms`) — for fields that were
//!   *always* milliseconds: `lock.locked_at` and lock-request `createdAt`
//!   (upstream writes `util::time::timestamp()` and its CLI parses them with
//!   `from_timestamp_millis`). These must **never** take the seconds
//!   heuristic: a genuinely small lock timestamp is a small timestamp, not a
//!   seconds value, and silently scaling it would invent a date.
//!
//! Both return `Ok(None)` for `0`, which every caller in this codebase means
//! as "absent" or "unbounded", never as the epoch.
//!
//! Range checking is not decoration. These values cross into `i64`/`chrono`
//! and into JavaScript, where integers above `Number.MAX_SAFE_INTEGER` lose
//! precision silently. Scaling therefore uses `checked_mul` and every result
//! is bounded before it can escape.
//!
//! Note on validation that is deliberately absent here: this module takes
//! `u64`, so "negative", "non-finite" and "non-integer" are unrepresentable
//! and cannot be tested. Those belong at the deserialization and TypeScript
//! boundaries, where such values can actually arrive.

use crate::error::LoreError;

/// Values at or above this are already canonical milliseconds; below it they
/// are Unix seconds.
///
/// `100_000_000_000` ms is 1973-03-03, and as *seconds* it would be the year
/// 5138. Every real timestamp this product handles falls unambiguously on one
/// side, so the split is not a guess about which one a value is.
pub const SECONDS_MS_THRESHOLD: u64 = 100_000_000_000;

/// The largest millisecond value permitted to leave this module.
///
/// `2^53 - 1` — JavaScript's `Number.MAX_SAFE_INTEGER`. Beyond it a JSON
/// number silently loses precision, so a value that cannot survive the trip is
/// rejected here rather than quietly corrupted at the boundary.
pub const MAX_SAFE_MS: u64 = 9_007_199_254_740_991;

fn out_of_range(field: &str, raw: u64, canonical: Option<u64>) -> LoreError {
    let detail = match canonical {
        Some(ms) => format!("{raw} normalizes to {ms}ms"),
        None => format!("{raw} seconds overflows when scaled to milliseconds"),
    };
    LoreError::CommandFailed(format!(
        "{field}: timestamp out of representable range ({detail}, limit {MAX_SAFE_MS}ms). \
         Refusing to emit a value that loses precision as a JSON number."
    ))
}

fn bounded(field: &str, raw: u64, canonical: u64) -> Result<Option<u64>, LoreError> {
    if canonical > MAX_SAFE_MS {
        return Err(out_of_range(field, raw, Some(canonical)));
    }
    Ok(Some(canonical))
}

/// Normalize a **mixed-unit** timestamp to canonical milliseconds.
///
/// `field` names the source in any error, so a failure is actionable rather
/// than an anonymous range complaint.
pub fn normalize_mixed(raw: u64, field: &str) -> Result<Option<u64>, LoreError> {
    if raw == 0 {
        return Ok(None);
    }
    if raw < SECONDS_MS_THRESHOLD {
        let scaled = raw
            .checked_mul(1000)
            .ok_or_else(|| out_of_range(field, raw, None))?;
        return bounded(field, raw, scaled);
    }
    bounded(field, raw, raw)
}

/// Normalize a **strict-milliseconds** timestamp.
///
/// Deliberately has no seconds branch: applying the heuristic to a field that
/// was always milliseconds is how a small-but-valid lock time would be turned
/// into a fabricated date.
pub fn normalize_strict_ms(raw: u64, field: &str) -> Result<Option<u64>, LoreError> {
    if raw == 0 {
        return Ok(None);
    }
    bounded(field, raw, raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECONDS_2024: u64 = 1_718_000_000;
    const MS_2024: u64 = 1_718_000_000_000;

    #[test]
    fn zero_is_absent_not_the_epoch() {
        assert_eq!(normalize_mixed(0, "f").expect("zero is valid"), None);
        assert_eq!(normalize_strict_ms(0, "f").expect("zero is valid"), None);
    }

    #[test]
    fn seconds_and_milliseconds_agree_on_the_same_instant() {
        // The compatibility property: a legacy seconds record and the same
        // instant stored in milliseconds must normalize identically.
        assert_eq!(
            normalize_mixed(SECONDS_2024, "f").expect("seconds normalize"),
            normalize_mixed(MS_2024, "f").expect("ms normalize"),
        );
        assert_eq!(
            normalize_mixed(SECONDS_2024, "f").expect("seconds normalize"),
            Some(MS_2024)
        );
    }

    #[test]
    fn canonical_milliseconds_are_not_scaled_again() {
        // Double scaling is the failure this whole module exists to prevent.
        assert_eq!(
            normalize_mixed(MS_2024, "f").expect("already ms"),
            Some(MS_2024)
        );
    }

    #[test]
    fn threshold_sides_are_exact() {
        assert_eq!(
            normalize_mixed(SECONDS_MS_THRESHOLD - 1, "f").expect("below is seconds"),
            Some((SECONDS_MS_THRESHOLD - 1) * 1000)
        );
        assert_eq!(
            normalize_mixed(SECONDS_MS_THRESHOLD, "f").expect("at threshold is ms"),
            Some(SECONDS_MS_THRESHOLD)
        );
    }

    #[test]
    fn strict_ms_never_applies_the_seconds_heuristic() {
        // A small lock value is a small instant, NOT seconds. If this ever
        // returns 100_000ms the strict policy has silently become mixed.
        assert_eq!(normalize_strict_ms(100, "lock").expect("small ms"), Some(100));
        assert_eq!(
            normalize_strict_ms(SECONDS_2024, "lock").expect("no heuristic"),
            Some(SECONDS_2024)
        );
    }

    #[test]
    fn scaling_overflow_fails_closed_with_an_actionable_error() {
        let err = normalize_mixed(u64::MAX / 100, "branch.created")
            .expect_err("must not silently wrap");
        let msg = err.to_string();
        assert!(msg.contains("branch.created"), "names the field: {msg}");
        assert!(msg.contains("out of representable range"), "{msg}");
    }

    #[test]
    fn values_beyond_js_safe_integer_are_rejected() {
        assert_eq!(
            normalize_strict_ms(MAX_SAFE_MS, "f").expect("at the limit is allowed"),
            Some(MAX_SAFE_MS)
        );
        let err = normalize_strict_ms(MAX_SAFE_MS + 1, "lock.locked_at")
            .expect_err("beyond the limit must fail");
        assert!(err.to_string().contains("lock.locked_at"));
        // Mixed must enforce the same ceiling on its already-ms branch.
        assert!(normalize_mixed(MAX_SAFE_MS + 1, "f").is_err());
    }
}
