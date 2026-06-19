//! `revision bisect` operation — binds `lore::revision::bisect`.
//!
//! Performs a binary search across a range of revisions, narrowing the search
//! space to locate which revision introduced a change. Emits
//! `LoreEvent::RevisionBisect` events carrying the current search range and
//! target revision at each step.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::LoreRevisionBisectArgs;
use serde::{Deserialize, Serialize};

/// Arguments for [`bisect`].
///
/// Mirrors the `LoreRevisionBisectArgs` from the upstream `lore` crate but uses
/// plain `String` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BisectArgs {
    /// Starting (known-good) revision identifier.
    pub start: String,
    /// Ending (known-bad) revision identifier.
    pub end: String,
}

impl BisectArgs {
    fn into_lore(self) -> LoreRevisionBisectArgs {
        LoreRevisionBisectArgs {
            start: LoreString::from_str(&self.start),
            end: LoreString::from_str(&self.end),
        }
    }
}

/// A single step in the bisect search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BisectStep {
    /// Revision number at the start of the current search range.
    pub start_revision_number: u64,
    /// Revision number selected to test next.
    pub target_revision_number: u64,
    /// Revision number at the end of the current search range.
    pub end_revision_number: u64,
    /// Whether the bisect search has completed.
    pub done: bool,
}

/// Result returned after a bisect operation completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BisectResult {
    /// All bisect steps emitted during the operation.
    pub steps: Vec<BisectStep>,
}

/// Bisect a range of revisions to locate a change.
///
/// Calls the upstream `lore::revision::bisect` in-process and collects the
/// `RevisionBisect` events to return a typed result.
pub async fn bisect(api: &LoreApi, args: BisectArgs) -> Result<BisectResult> {
    let (callback, rx) = collect_events();

    let status = lore::revision::bisect(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("revision bisect failed with status {status}"),
        )));
    }

    let steps: Vec<BisectStep> = stream
        .events
        .iter()
        .filter_map(|event| {
            if let LoreEvent::RevisionBisect(data) = event {
                Some(BisectStep {
                    start_revision_number: data.start_revision_number,
                    target_revision_number: data.target_revision_number,
                    end_revision_number: data.end_revision_number,
                    done: data.done != 0,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(BisectResult { steps })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bisect_args_serializes() {
        let args = BisectArgs {
            start: "rev-1".into(),
            end: "rev-10".into(),
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("rev-1"));
        assert!(json.contains("rev-10"));
    }

    #[test]
    fn bisect_args_into_lore_conversion() {
        let args = BisectArgs {
            start: "good".into(),
            end: "bad".into(),
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.start.as_str(), "good");
        assert_eq!(lore_args.end.as_str(), "bad");
    }

    #[test]
    fn bisect_result_serializes() {
        let result = BisectResult {
            steps: vec![BisectStep {
                start_revision_number: 1,
                target_revision_number: 5,
                end_revision_number: 10,
                done: false,
            }],
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("\"target_revision_number\":5"));
        assert!(json.contains("\"done\":false"));
    }

    #[test]
    fn bisect_result_empty_steps() {
        let result = BisectResult { steps: vec![] };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("\"steps\":[]"));
    }

    #[test]
    fn bisect_step_done_flag() {
        let step = BisectStep {
            start_revision_number: 3,
            target_revision_number: 3,
            end_revision_number: 4,
            done: true,
        };
        assert!(step.done);
    }
}
