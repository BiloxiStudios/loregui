//! `revision history` operation — binds `lore::revision::history`.
//!
//! Retrieves the revision history for the current branch or a specified
//! revision. Emits one `LoreEvent::RevisionHistoryEntry` per revision.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use crate::time_units::normalize_mixed;
use lore::interface::{LoreEvent, LoreString};

use lore::revision::{LoreRevisionHistoryArgs, LoreRevisionInfoArgs};
use serde::{Deserialize, Serialize};

/// Arguments for [`history`].
///
/// Mirrors `LoreRevisionHistoryArgs` from the upstream `lore` crate but uses
/// plain Rust types so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionHistoryArgs {
    /// Start from this revision; empty for current.
    #[serde(default)]
    pub revision: String,
    /// Restrict to this branch; empty for current.
    #[serde(default)]
    pub branch: String,
    /// Stop at revisions created before this instant; 0 disables.
    ///
    /// MIXED UNIT (SBAI-5905): seconds or milliseconds, normalized before any
    /// comparison. Deliberately NOT forwarded to upstream — see [`history`].
    #[serde(default)]
    pub date: u64,
    /// Maximum number of revisions to return; 0 for unlimited.
    #[serde(default)]
    pub length: u32,
    /// Stop when reaching a different branch.
    #[serde(default)]
    pub only_branch: bool,
}

impl RevisionHistoryArgs {
    fn into_lore(self) -> LoreRevisionHistoryArgs {
        LoreRevisionHistoryArgs {
            revision: LoreString::from_str(&self.revision),
            branch: LoreString::from_str(&self.branch),
            // SBAI-5905: upstream compares the stored metadata timestamp
            // against this value RAW (lore-revision history.rs:253-255). Stored
            // history spans Epic 6fd18e6, so it holds both seconds and
            // milliseconds, and a raw comparison against either unit truncates
            // mixed histories at the first record written in the other one.
            // The boundary is enforced in this adapter instead; length, branch
            // and only_branch are forwarded unchanged so the walk upstream
            // performs is otherwise identical.
            date: 0,
            length: self.length,
            only_branch: u8::from(self.only_branch),
        }
    }
}

/// A single entry in the revision history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionHistoryEntry {
    /// Revision hash signature.
    pub revision: String,
    /// Sequential revision number.
    pub revision_number: u64,
    /// Parent revision hashes (zero hashes are omitted).
    pub parents: Vec<String>,
}

/// Result returned on a successful history query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevisionHistoryResult {
    /// History entries, newest first.
    pub entries: Vec<RevisionHistoryEntry>,
}

/// Retrieve the revision history for the current branch or a specified revision.
///
/// Calls the upstream `lore::revision::history` in-process and collects
/// `RevisionHistoryEntry` events into a typed result.
pub async fn history(api: &LoreApi, args: RevisionHistoryArgs) -> Result<RevisionHistoryResult> {
    // Captured before `into_lore` consumes `args`.
    let requested_cutoff = args.date;
    let (callback, rx) = collect_events();

    let status = lore::revision::history(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision history failed with status {status}"),
        )));
    }

    let mut entries: Vec<RevisionHistoryEntry> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::RevisionHistoryEntry(data) = event {
                let parents: Vec<String> = data
                    .parent
                    .iter()
                    .filter(|h| !h.is_zero())
                    .map(|h| format!("{h}"))
                    .collect();
                Some(RevisionHistoryEntry {
                    revision: format!("{}", data.revision),
                    revision_number: data.revision_number,
                    parents,
                })
            } else {
                None
            }
        })
        .collect();

    // SBAI-5905: apply the date boundary here, because upstream cannot.
    //
    // Upstream BREAKS at the first entry older than the cutoff rather than
    // skipping it, so the emitted prefix is identical whether or not the cutoff
    // was forwarded — which is why the caller's `length` can be forwarded
    // untouched and this stops at the same place. Metadata is fetched only
    // until that first out-of-range entry, so the zero-cutoff case costs
    // nothing and a cutoff costs only the entries actually inspected.
    if requested_cutoff != 0 {
        let cutoff = normalize_mixed(requested_cutoff, "history date cutoff")?.unwrap_or(0);
        let mut inspected: Vec<u64> = Vec::new();
        for entry in entries.iter() {
            let ts = revision_timestamp_ms(api, &entry.revision).await?;
            inspected.push(ts);
            // Stop fetching the moment the boundary is crossed — upstream would
            // have stopped here too, so nothing later is eligible.
            if ts < cutoff {
                break;
            }
        }
        entries.truncate(keep_count(&inspected, cutoff));
    }

    Ok(RevisionHistoryResult { entries })
}

/// How many leading entries survive the cutoff.
///
/// Extracted pure so the boundary is provable without a `LoreApi`
/// (SBAI-5905). `timestamps` are canonical milliseconds in walk order, and
/// only as many as were inspected before the break; `cutoff` is canonical
/// milliseconds too, so this is ms-vs-ms.
///
/// This mirrors upstream's STOP semantics, not a filter: the walk ends at the
/// first entry older than the cutoff, and nothing after it is eligible even if
/// clocks were non-monotonic. Reproducing that exactly is what keeps the
/// adapter's answer identical to the one upstream would have produced.
fn keep_count(timestamps: &[u64], cutoff: u64) -> usize {
    timestamps
        .iter()
        .position(|ts| *ts < cutoff)
        .unwrap_or(timestamps.len())
}

/// Resolve one revision's commit timestamp, normalized to canonical
/// milliseconds (SBAI-5905).
///
/// Fails closed. Under an active cutoff a revision whose timestamp is missing,
/// unparseable or zero cannot be placed relative to the boundary, and guessing
/// would either drop a revision the caller asked for or include one they
/// excluded. Both are silent wrong answers, so this reports instead.
async fn revision_timestamp_ms(api: &LoreApi, revision: &str) -> Result<u64> {
    let (callback, rx) = collect_events();
    let info_args = LoreRevisionInfoArgs {
        revision: LoreString::from_str(revision),
        delta: 0,
        metadata: 1,
    };
    let _ = lore::revision::info(api.globals().build(), info_args, callback).await;
    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("info stream cancelled: {e}")))?;
    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(format!(
            "revision history date filter: could not read metadata for {revision}: {}",
            stream.error.as_deref().unwrap_or("unknown error")
        )));
    }
    let raw = stream
        .events
        .iter()
        .find_map(|event| match event {
            LoreEvent::Metadata(data) if data.key.as_str() == "timestamp" => match &data.value {
                lore::interface::LoreMetadata::String(v) => v.as_str().parse::<u64>().ok(),
                lore::interface::LoreMetadata::Numeric(n) => Some(*n),
                _ => None,
            },
            _ => None,
        })
        .ok_or_else(|| {
            LoreError::CommandFailed(format!(
                "revision history date filter: revision {revision} has no usable timestamp,                  so it cannot be placed against the requested cutoff"
            ))
        })?;
    normalize_mixed(raw, "revision metadata timestamp")?.ok_or_else(|| {
        LoreError::CommandFailed(format!(
            "revision history date filter: revision {revision} has a zero timestamp,              so it cannot be placed against the requested cutoff"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_args_defaults() {
        let json = r#"{}"#;
        let args: RevisionHistoryArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.revision, "");
        assert_eq!(args.branch, "");
        assert_eq!(args.length, 0);
        assert!(!args.only_branch);
    }

    #[test]
    fn history_args_into_lore_conversion() {
        let args = RevisionHistoryArgs {
            revision: "rev1".into(),
            branch: "main".into(),
            date: 0,
            length: 10,
            only_branch: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision.as_str(), "rev1");
        assert_eq!(lore_args.branch.as_str(), "main");
        assert_eq!(lore_args.length, 10);
        assert_eq!(lore_args.only_branch, 1);
    }

    #[test]
    fn history_result_serializes() {
        let result = RevisionHistoryResult {
            entries: vec![
                RevisionHistoryEntry {
                    revision: "r2".into(),
                    revision_number: 2,
                    parents: vec!["r1".into()],
                },
                RevisionHistoryEntry {
                    revision: "r1".into(),
                    revision_number: 1,
                    parents: vec![],
                },
            ],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("r1"));
        assert!(json.contains("r2"));
    }

    const S_2024: u64 = 1_718_000_000;
    const MS_2024: u64 = 1_718_000_000_000;
    const DAY_MS: u64 = 86_400_000;

    /// The cutoff must never be forwarded upstream, because upstream compares
    /// it raw against a mixed-unit stored value. length/branch/only_branch must
    /// still be forwarded untouched so the walk is otherwise identical.
    #[test]
    fn the_cutoff_is_withheld_but_the_rest_of_the_walk_is_forwarded() {
        let args = RevisionHistoryArgs {
            revision: "r".into(),
            branch: "main".into(),
            date: S_2024,
            length: 25,
            only_branch: true,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.date, 0, "the raw cutoff must NOT reach upstream");
        assert_eq!(lore_args.length, 25, "caller length forwarded unchanged");
        assert_eq!(lore_args.only_branch, 1, "branch boundary preserved");
        assert_eq!(lore_args.branch.as_str(), "main");
    }

    /// The same cutoff written in seconds or in milliseconds must produce the
    /// identical ordered stop set.
    #[test]
    fn seconds_and_ms_cutoffs_produce_the_same_stop_set() {
        // Newest first, one day apart, as the walk emits them.
        let chain = [MS_2024, MS_2024 - DAY_MS, MS_2024 - 2 * DAY_MS];
        let from_seconds = normalize_mixed(S_2024 - 86_400, "c")
            .expect("valid")
            .unwrap();
        let from_ms = normalize_mixed(MS_2024 - DAY_MS, "c")
            .expect("valid")
            .unwrap();
        assert_eq!(from_seconds, from_ms, "the two spellings are one instant");
        assert_eq!(keep_count(&chain, from_seconds), 2);
        assert_eq!(keep_count(&chain, from_ms), 2);
    }

    /// A legacy seconds record in the chain is the same instant as its ms twin,
    /// so it must survive a cutoff that its twin survives. Before normalization
    /// the seconds value was numerically tiny and always fell out.
    #[test]
    fn a_legacy_seconds_record_is_not_truncated_away() {
        let legacy = normalize_mixed(S_2024, "ts").expect("valid").unwrap();
        let twin = normalize_mixed(MS_2024, "ts").expect("valid").unwrap();
        assert_eq!(legacy, twin);
        let cutoff = normalize_mixed(S_2024 - 1, "c").expect("valid").unwrap();
        assert_eq!(keep_count(&[legacy], cutoff), 1);
        assert_eq!(keep_count(&[twin], cutoff), 1);
    }

    #[test]
    fn cutoff_at_the_first_entry_keeps_nothing() {
        let chain = [MS_2024 - 2 * DAY_MS, MS_2024 - 3 * DAY_MS];
        assert_eq!(keep_count(&chain, MS_2024), 0);
    }

    #[test]
    fn cutoff_after_n_keeps_exactly_n() {
        let chain = [
            MS_2024,
            MS_2024 - DAY_MS,
            MS_2024 - 2 * DAY_MS,
            MS_2024 - 3 * DAY_MS,
        ];
        assert_eq!(keep_count(&chain, MS_2024 - 2 * DAY_MS), 3);
        assert_eq!(
            keep_count(&chain, 1),
            4,
            "a cutoff below everything keeps all"
        );
    }

    /// Upstream BREAKS rather than skipping, so a later in-range entry after an
    /// out-of-range one is NOT resurrected. Reproducing that is the point.
    #[test]
    fn the_stop_is_a_break_not_a_filter() {
        let non_monotonic = [MS_2024, MS_2024 - 5 * DAY_MS, MS_2024 - DAY_MS];
        assert_eq!(keep_count(&non_monotonic, MS_2024 - 2 * DAY_MS), 1);
    }

    /// With no cutoff the boundary code never runs, so no metadata is fetched.
    #[test]
    fn a_zero_cutoff_inspects_nothing() {
        // The op guards the whole block on `requested_cutoff != 0`; with an
        // empty inspected list keep_count is a no-op over zero fetches.
        assert_eq!(keep_count(&[], 0), 0);
        let args = RevisionHistoryArgs::default();
        assert_eq!(args.date, 0);
        assert_eq!(args.into_lore().date, 0);
    }
}
