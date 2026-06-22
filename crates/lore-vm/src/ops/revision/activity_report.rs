//! `revision activity_report` — aggregated "who did what when" over a revision chain.
//!
//! Walks the revision chain via `lore::revision::history`, then enriches each
//! entry with commit message, author, and timestamp via `lore::revision::info`
//! (with `metadata=true`).  Returns a typed report suitable for the
//! Activity & History UI panel.
//!
//! Optional filters:
//! - `author` — only include revisions whose author contains this substring.
//! - `date_from` / `date_to` — Unix-timestamp window (0 = unbounded).
//! - `file_path` — only include revisions that touched this file.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

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
    /// Only include revisions with timestamp >= this value (Unix seconds; 0 = unbounded).
    #[serde(default)]
    pub date_from: u64,
    /// Only include revisions with timestamp <= this value (Unix seconds; 0 = unbounded).
    #[serde(default)]
    pub date_to: u64,
    /// Only include revisions that touched this file path.
    #[serde(default)]
    pub file_path: String,
}

impl ActivityReportArgs {
    fn into_lore_history(&self) -> LoreRevisionHistoryArgs {
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
    /// Commit Unix timestamp (seconds since epoch).
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
        lore::revision::history(api.globals().build(), args.into_lore_history(), callback).await;
    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;
    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(
            stream
                .error
                .unwrap_or_else(|| format!("revision history failed with status {status}")),
        ));
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
                Some((
                    format!("{}", data.revision),
                    data.revision_number,
                    parents,
                ))
            } else {
                None
            }
        })
        .collect();

    let total_walked = history_entries.len();

    // Step 2: For each revision, fetch rich info (metadata + deltas).
    let mut entries: Vec<ActivityEntry> = Vec::with_capacity(history_entries.len());
    for (rev, rev_num, parents) in &history_entries {
        let (cb2, rx2) = collect_events();
        let info_args = ActivityReportArgs::into_lore_info(rev);
        let _ = lore::revision::info(api.globals().build(), info_args, cb2).await;
        let info_stream = rx2
            .await
            .map_err(|e| LoreError::CommandFailed(format!("info stream cancelled: {e}")))?;
        if !info_stream.is_ok() {
            // If info fails for a revision, skip it rather than failing the whole report.
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
        let timestamp: u64 = find_metadata(&info_stream.events, METADATA_KEY_TIMESTAMP)
            .parse()
            .unwrap_or(0);

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
    let filtered: Vec<ActivityEntry> = entries
        .into_iter()
        .filter(|entry| {
            // Author filter (substring, case-insensitive).
            if !args.author.is_empty()
                && !entry.author.to_lowercase().contains(&args.author.to_lowercase())
            {
                return false;
            }
            // Date-from filter.
            if args.date_from != 0 && entry.timestamp < args.date_from {
                return false;
            }
            // Date-to filter.
            if args.date_to != 0 && entry.timestamp > args.date_to {
                return false;
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn args_into_lore_history() {
        let args = ActivityReportArgs {
            revision: "abc123".into(),
            branch: "main".into(),
            length: 20,
            author: "alice".into(),
            date_from: 1_700_000_000,
            date_to: 1_710_000_000,
            file_path: "src/lib.rs".into(),
        };
        let lore_args = args.into_lore_history();
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
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("10"));
        assert!(json.contains("3"));
    }

    #[test]
    fn result_default_is_empty() {
        let result = ActivityReportResult::default();
        assert!(result.entries.is_empty());
        assert_eq!(result.total_walked, 0);
        assert_eq!(result.total_after_filter, 0);
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
