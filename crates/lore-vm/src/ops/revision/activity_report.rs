//! `revision activity_report` — aggregated "who did what when" over a revision chain.
//!
//! Walks the revision chain via `lore::revision::history`, then enriches each
//! entry with commit message, author, and timestamp via `lore::revision::info`
//! (with `metadata=true`).  Returns a typed report suitable for the
//! Activity & History UI panel.
//!
//! This is a **LoreGUI-derived composite**, not an upstream primitive — there is
//! no `lore::revision::activity_report`. Scanner classification:
//! **derived-composite** intentional orphan
//! (`scripts/upstream-lore-parity.mjs` `KNOWN_INTENTIONAL_ORPHANS`).
//!
//! Optional filters:
//! - `author` — only include revisions whose author contains this substring.
//! - `date_from` / `date_to` — timestamp window (0 = unbounded). MIXED UNIT:
//!   normalized to canonical milliseconds before any comparison (SBAI-5905).
//! - `file_path` — only include revisions that touched this file.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};
use crate::time_units::normalize_mixed;

use lore::interface::{LoreEvent, LoreString};
use lore::revision::{LoreRevisionHistoryArgs, LoreRevisionInfoArgs};
use serde::{Deserialize, Serialize};

/// Metadata keys populated by the committing author (see `info.rs`).
const METADATA_KEY_MESSAGE: &str = "message";
const METADATA_KEY_TIMESTAMP: &str = "timestamp";
const METADATA_KEY_CREATED_BY: &str = "created-by";
const METADATA_KEY_COMMITTED_BY: &str = "committed-by";

/// Arguments for [`activity_report`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivityReportArgs {
    /// Start from this revision; empty for current HEAD.
    #[serde(default)]
    pub revision: String,
    /// Restrict to this branch; empty for current.
    #[serde(default)]
    pub branch: String,
    /// Maximum number of revisions to walk; 0 = unlimited.
    #[serde(default)]
    pub length: u32,
    /// Only include revisions by an author whose name contains this substring.
    #[serde(default)]
    pub author: String,
    /// Only include revisions at or after this instant. MIXED UNIT — seconds or
    /// milliseconds, normalized before comparison (SBAI-5905). 0 = unbounded.
    #[serde(default)]
    pub date_from: u64,
    /// Only include revisions at or before this instant. MIXED UNIT — seconds or
    /// milliseconds, normalized before comparison (SBAI-5905). 0 = unbounded.
    #[serde(default)]
    pub date_to: u64,
    /// Only include revisions that touched this file path.
    #[serde(default)]
    pub file_path: String,
}

impl ActivityReportArgs {
    fn to_lore_history(&self) -> LoreRevisionHistoryArgs {
        LoreRevisionHistoryArgs {
            revision: LoreString::from_str(&self.revision),
            branch: LoreString::from_str(&self.branch),
            date: 0,
            length: self.length,
            only_branch: u8::from(!self.branch.is_empty()),
        }
    }

    fn into_lore_info(revision: &str) -> LoreRevisionInfoArgs {
        LoreRevisionInfoArgs {
            revision: LoreString::from_str(revision),
            delta: 1,
            metadata: 1,
        }
    }
}

/// A single file changed in a revision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityFileChange {
    /// Repository-relative file path.
    pub path: String,
    /// File size in bytes.
    pub size: u64,
    /// Action: Add, Delete, Modify, etc.
    pub action: String,
}

/// One row in the activity report — a single revision with its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// Revision hash.
    pub revision: String,
    /// Sequential revision number.
    pub revision_number: u64,
    /// Parent revision hashes (zero hashes omitted).
    pub parents: Vec<String>,
    /// Commit message.
    pub message: String,
    /// Author identity.
    pub author: String,
    /// Commit timestamp in canonical Unix epoch MILLISECONDS (SBAI-5905).
    /// Normalized on the way out, so a caller never sees the mixed stored unit.
    /// 0 means the revision carried no resolvable timestamp.
    pub timestamp: u64,
    /// Files changed in this revision.
    pub files_changed: Vec<ActivityFileChange>,
}

/// Result returned on a successful activity-report query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivityReportResult {
    /// Entries, newest first.
    pub entries: Vec<ActivityEntry>,
    /// Total number of revisions walked before filtering.
    pub total_walked: usize,
    /// Number of entries after filtering.
    pub total_after_filter: usize,
    /// Number of revisions whose info lookup failed and were skipped (not
    /// included in `entries`). Surfaced so a caller can tell a genuinely empty
    /// range from a range where enrichment silently dropped revisions.
    #[serde(default)]
    pub total_skipped: usize,
}

/// Outcome of the non-path filters for one entry.
///
/// `Unplaceable` is deliberately distinct from `Reject`: a revision excluded
/// because we could not place it against an active window is a gap in the
/// answer, not a match that failed, and the caller is told via `total_skipped`.
#[derive(Debug, PartialEq, Eq)]
enum EntryVerdict {
    Keep,
    Reject,
    Unplaceable,
}

/// Pure filter decision, extracted so the window semantics are testable
/// without a `LoreApi` (SBAI-5905). Both cutoffs arrive already normalized to
/// canonical milliseconds, and `entry.timestamp` is normalized on the way in,
/// so every comparison here is ms-vs-ms.
fn classify_entry(
    entry: &ActivityEntry,
    args: &ActivityReportArgs,
    date_from_ms: u64,
    date_to_ms: u64,
) -> EntryVerdict {
    if !args.author.is_empty()
        && !entry
            .author
            .to_lowercase()
            .contains(&args.author.to_lowercase())
    {
        return EntryVerdict::Reject;
    }
    let window_active = date_from_ms != 0 || date_to_ms != 0;
    // A revision with no resolvable timestamp cannot be placed relative to an
    // active window. Excluding it silently would let a caller read a filtered
    // range as complete, so it is counted rather than dropped.
    if window_active && entry.timestamp == 0 {
        return EntryVerdict::Unplaceable;
    }
    if date_from_ms != 0 && entry.timestamp < date_from_ms {
        return EntryVerdict::Reject;
    }
    if date_to_ms != 0 && entry.timestamp > date_to_ms {
        return EntryVerdict::Reject;
    }
    EntryVerdict::Keep
}

/// Render a metadata value as a plain display string.
fn metadata_display(event: &LoreEvent, key: &str) -> Option<String> {
    if let LoreEvent::Metadata(data) = event {
        if data.key.as_str() == key {
            return match &data.value {
                lore::interface::LoreMetadata::String(s) => Some(s.as_str().to_string()),
                lore::interface::LoreMetadata::Numeric(n) => Some(n.to_string()),
                other => Some(serde_json::to_string(other).unwrap_or_default()),
            };
        }
    }
    None
}

/// Extract a metadata value from a slice of events by key.
fn find_metadata(events: &[LoreEvent], key: &str) -> String {
    for event in events {
        if let Some(val) = metadata_display(event, key) {
            return val;
        }
    }
    String::new()
}

/// Retrieve an aggregated activity report for a revision chain.
///
/// For each revision in the chain, fetches rich info (message, author,
/// timestamp, file deltas) and assembles a report.  Optional filters
/// narrow the result to a specific author, date range, or file path.
pub async fn activity_report(
    api: &LoreApi,
    args: ActivityReportArgs,
) -> Result<ActivityReportResult> {
    // Step 1: Walk the revision chain.
    let (callback, rx) = collect_events();
    let status =
        lore::revision::history(api.globals().build(), args.to_lore_history(), callback).await;
    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;
    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision history failed with status {status}"),
        )));
    }

    let history_entries: Vec<_> = stream
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
                Some((format!("{}", data.revision), data.revision_number, parents))
            } else {
                None
            }
        })
        .collect();

    let total_walked = history_entries.len();

    // Step 2: For each revision, fetch rich info (metadata + deltas).
    let mut entries: Vec<ActivityEntry> = Vec::with_capacity(history_entries.len());
    let mut total_skipped: usize = 0;
    for (rev, rev_num, parents) in &history_entries {
        let (cb2, rx2) = collect_events();
        let info_args = ActivityReportArgs::into_lore_info(rev);
        let _ = lore::revision::info(api.globals().build(), info_args, cb2).await;
        let info_stream = rx2
            .await
            .map_err(|e| LoreError::CommandFailed(format!("info stream cancelled: {e}")))?;
        if !info_stream.is_ok() {
            // If info fails for a revision, skip it rather than failing the whole
            // report — but track and surface the count so the drop isn't silent.
            total_skipped += 1;
            tracing::warn!(
                revision = %rev,
                error = info_stream.error.as_deref().unwrap_or("unknown"),
                "activity_report: skipping revision whose info lookup failed"
            );
            continue;
        }

        let message = find_metadata(&info_stream.events, METADATA_KEY_MESSAGE);
        let author = {
            let created = find_metadata(&info_stream.events, METADATA_KEY_CREATED_BY);
            if created.is_empty() {
                find_metadata(&info_stream.events, METADATA_KEY_COMMITTED_BY)
            } else {
                created
            }
        };
        // SBAI-5905: stored history spans Epic 6fd18e6, so this value may be
        // seconds or milliseconds. Normalize on the way in; 0 means the
        // revision carried no resolvable timestamp and stays 0.
        let raw_timestamp: u64 = find_metadata(&info_stream.events, METADATA_KEY_TIMESTAMP)
            .parse()
            .unwrap_or(0);
        let timestamp: u64 = match normalize_mixed(raw_timestamp, "revision metadata timestamp") {
            Ok(Some(ms)) => ms,
            Ok(None) => 0,
            // An out-of-range stored value is not a reason to fail the whole
            // report; it is a revision we cannot place, handled by the window
            // filter below exactly like a missing one.
            Err(_) => 0,
        };

        // Collect file deltas.
        let files_changed: Vec<ActivityFileChange> = info_stream
            .events
            .iter()
            .filter_map(|event| {
                if let LoreEvent::RevisionInfoDelta(data) = event {
                    Some(ActivityFileChange {
                        path: data.path.as_str().to_string(),
                        size: data.size,
                        action: format!("{:?}", data.action),
                    })
                } else {
                    None
                }
            })
            .collect();

        entries.push(ActivityEntry {
            revision: rev.clone(),
            revision_number: *rev_num,
            parents: parents.clone(),
            message,
            author,
            timestamp,
            files_changed,
        });
    }

    // Step 3: Apply filters.
    //
    // SBAI-5905: normalize the caller's window ONCE, before any comparison, so
    // a seconds cutoff and the same instant in milliseconds select the same
    // set. An unusable cutoff is a caller error and fails the request rather
    // than silently filtering against a wrong instant.
    let date_from_ms = normalize_mixed(args.date_from, "date_from")?.unwrap_or(0);
    let date_to_ms = normalize_mixed(args.date_to, "date_to")?.unwrap_or(0);
    let mut unplaceable = 0usize;

    let filtered: Vec<ActivityEntry> = entries
        .into_iter()
        .filter(|entry| {
            match classify_entry(entry, &args, date_from_ms, date_to_ms) {
                EntryVerdict::Keep => {}
                EntryVerdict::Reject => return false,
                EntryVerdict::Unplaceable => {
                    unplaceable += 1;
                    return false;
                }
            }
            // File-path filter (exact match on any changed file).
            if !args.file_path.is_empty()
                && !entry.files_changed.iter().any(|f| f.path == args.file_path)
            {
                return false;
            }
            true
        })
        .collect();

    let total_after_filter = filtered.len();

    Ok(ActivityReportResult {
        entries: filtered,
        total_walked,
        total_after_filter,
        total_skipped: total_skipped + unplaceable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const S_2024: u64 = 1_718_000_000; // seconds
    const MS_2024: u64 = 1_718_000_000_000; // the same instant, milliseconds

    fn entry_at(ts: u64) -> ActivityEntry {
        ActivityEntry {
            revision: "r".into(),
            revision_number: 1,
            parents: vec![],
            message: "m".into(),
            author: "a".into(),
            timestamp: ts,
            files_changed: vec![],
        }
    }

    fn args_window(from: u64, to: u64) -> ActivityReportArgs {
        ActivityReportArgs {
            date_from: from,
            date_to: to,
            ..Default::default()
        }
    }

    /// SBAI-5905, the compatibility property the ruling asks for: a seconds
    /// cutoff and the same instant in milliseconds must select the same set.
    #[test]
    fn mixed_seconds_and_ms_window_select_the_same_entries() {
        let inside = entry_at(MS_2024);
        let before = entry_at(MS_2024 - 86_400_000);
        let after = entry_at(MS_2024 + 86_400_000);

        // Callers may express the SAME window in either unit; both are
        // normalized to ms before classify_entry ever sees them.
        // A one-hour window around the instant, expressed both ways. It must be
        // narrower than the +/-1 day offsets below, or "after" legitimately
        // falls inside and the test proves nothing.
        for (from, to) in [
            (S_2024 - 1_800, S_2024 + 1_800),           // seconds form
            (MS_2024 - 1_800_000, MS_2024 + 1_800_000), // millisecond form
        ] {
            let f = normalize_mixed(from, "date_from").expect("valid").unwrap();
            let t = normalize_mixed(to, "date_to").expect("valid").unwrap();
            let a = args_window(from, to);
            let _ = &a;
            assert_eq!(classify_entry(&inside, &a, f, t), EntryVerdict::Keep);
            assert_eq!(classify_entry(&before, &a, f, t), EntryVerdict::Reject);
            assert_eq!(classify_entry(&after, &a, f, t), EntryVerdict::Reject);
        }
    }

    /// An entry stored in legacy SECONDS and its millisecond twin are the same
    /// instant, so the same window must accept both. This is the case that
    /// silently broke before normalization: the seconds record was numerically
    /// tiny against an ms cutoff and always fell outside.
    #[test]
    fn a_legacy_seconds_record_and_its_ms_twin_are_treated_alike() {
        let from = normalize_mixed(S_2024 - 1, "date_from")
            .expect("valid")
            .unwrap();
        let to = normalize_mixed(S_2024 + 1, "date_to")
            .expect("valid")
            .unwrap();
        let a = args_window(S_2024 - 1, S_2024 + 1);
        // Both spellings normalize to MS_2024 on the way in.
        let from_seconds_record = entry_at(normalize_mixed(S_2024, "ts").expect("v").unwrap());
        let from_ms_record = entry_at(normalize_mixed(MS_2024, "ts").expect("v").unwrap());
        assert_eq!(from_seconds_record.timestamp, from_ms_record.timestamp);
        assert_eq!(
            classify_entry(&from_seconds_record, &a, from, to),
            EntryVerdict::Keep
        );
        assert_eq!(
            classify_entry(&from_ms_record, &a, from, to),
            EntryVerdict::Keep
        );
    }

    /// A revision we cannot place is a GAP, not a non-match — it must be
    /// distinguishable so total_skipped can report it.
    #[test]
    fn unplaceable_entries_are_distinguished_from_rejections() {
        let a = args_window(S_2024, 0);
        let from = normalize_mixed(S_2024, "date_from")
            .expect("valid")
            .unwrap();
        assert_eq!(
            classify_entry(&entry_at(0), &a, from, 0),
            EntryVerdict::Unplaceable
        );
        // With NO window active, a missing timestamp is not a gap — nothing was
        // being filtered on, so the entry is simply kept.
        let no_window = args_window(0, 0);
        assert_eq!(
            classify_entry(&entry_at(0), &no_window, 0, 0),
            EntryVerdict::Keep
        );
    }

    /// An unusable cutoff must fail the request rather than filter against a
    /// wrong instant.
    #[test]
    fn an_out_of_range_cutoff_is_rejected_not_silently_clamped() {
        assert!(normalize_mixed(u64::MAX / 100, "date_from").is_err());
    }

    #[test]
    fn args_defaults() {
        let args: ActivityReportArgs = serde_json::from_str("{}").expect("deserialise");
        assert_eq!(args.revision, "");
        assert_eq!(args.branch, "");
        assert_eq!(args.length, 0);
        assert_eq!(args.author, "");
        assert_eq!(args.date_from, 0);
        assert_eq!(args.date_to, 0);
        assert_eq!(args.file_path, "");
    }

    #[test]
    fn args_to_lore_history() {
        let args = ActivityReportArgs {
            revision: "abc123".into(),
            branch: "main".into(),
            length: 20,
            author: "alice".into(),
            date_from: 1_700_000_000,
            date_to: 1_710_000_000,
            file_path: "src/lib.rs".into(),
        };
        let lore_args = args.to_lore_history();
        assert_eq!(lore_args.revision.as_str(), "abc123");
        assert_eq!(lore_args.branch.as_str(), "main");
        assert_eq!(lore_args.length, 20);
        assert_eq!(lore_args.only_branch, 1);
    }

    #[test]
    fn args_into_lore_info() {
        let lore_args = ActivityReportArgs::into_lore_info("r1");
        assert_eq!(lore_args.revision.as_str(), "r1");
        assert_eq!(lore_args.delta, 1);
        assert_eq!(lore_args.metadata, 1);
    }

    #[test]
    fn args_serializes_with_all_fields() {
        let args = ActivityReportArgs {
            revision: "r1".into(),
            author: "bob".into(),
            date_from: 1_000_000,
            date_to: 2_000_000,
            file_path: "foo.txt".into(),
            ..Default::default()
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("bob"));
        assert!(json.contains("foo.txt"));
        assert!(json.contains("1000000"));
    }

    #[test]
    fn entry_serializes() {
        let entry = ActivityEntry {
            revision: "r42".into(),
            revision_number: 42,
            parents: vec!["r41".into()],
            message: "fix bug".into(),
            author: "alice".into(),
            timestamp: 1_700_000_000,
            files_changed: vec![ActivityFileChange {
                path: "src/main.rs".into(),
                size: 512,
                action: "Modify".into(),
            }],
        };
        let json = serde_json::to_string(&entry).expect("should serialize");
        assert!(json.contains("fix bug"));
        assert!(json.contains("alice"));
        assert!(json.contains("src/main.rs"));
    }

    #[test]
    fn result_serializes_with_counts() {
        let result = ActivityReportResult {
            entries: vec![],
            total_walked: 10,
            total_after_filter: 3,
            total_skipped: 2,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("10"));
        assert!(json.contains("3"));
        assert!(json.contains("total_skipped"));
    }

    #[test]
    fn result_default_is_empty() {
        let result = ActivityReportResult::default();
        assert!(result.entries.is_empty());
        assert_eq!(result.total_walked, 0);
        assert_eq!(result.total_after_filter, 0);
        assert_eq!(result.total_skipped, 0);
    }

    #[test]
    fn file_change_serializes() {
        let fc = ActivityFileChange {
            path: "assets/tex.png".into(),
            size: 2048,
            action: "Add".into(),
        };
        let json = serde_json::to_string(&fc).expect("should serialize");
        assert!(json.contains("assets/tex.png"));
        assert!(json.contains("2048"));
        assert!(json.contains("Add"));
    }
}
