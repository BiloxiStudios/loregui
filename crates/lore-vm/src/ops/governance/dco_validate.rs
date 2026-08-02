//! `governance::dco_validate` — validate DCO (Developer Certificate of Origin)
//! sign-offs on one or more revisions.
//!
//! The DCO requires a `Signed-off-by:` line in every commit message. This op
//! inspects the commit message metadata (via `lore::revision::info`) and
//! reports which revisions pass / fail the check. No upstream `lore` function
//! performs this check — it is a lore-vm native governance binding.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreString};
use lore::revision::{LoreRevisionHistoryArgs, history};
use serde::{Deserialize, Serialize};

/// DCO sign-off prefix required in every commit message.
pub const DCO_SIGNOFF_PREFIX: &str = "Signed-off-by:";

/// Arguments for [`dco_validate`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DcoValidateArgs {
    /// Starting revision; empty targets the current HEAD.
    #[serde(default)]
    pub revision: String,
    /// Stop at this revision (exclusive); empty walks to branch root.
    #[serde(default)]
    pub since: String,
    /// Maximum number of revisions to check; 0 for unlimited.
    #[serde(default)]
    pub limit: u32,
}

impl DcoValidateArgs {
    fn into_lore(self) -> LoreRevisionHistoryArgs {
        LoreRevisionHistoryArgs {
            revision: LoreString::from_str(&self.revision),
            branch: LoreString::from_str(""),
            date: 0,
            length: self.limit,
            only_branch: 0,
        }
    }
}

/// Per-revision DCO validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcoRevisionResult {
    /// Revision hash.
    pub revision: String,
    /// Revision number.
    pub revision_number: u64,
    /// Whether the commit message contains a `Signed-off-by:` line.
    pub dco_signed: bool,
    /// The sign-off line if present (e.g. `Signed-off-by: Alice <alice@example.com>`).
    pub signoff_line: Option<String>,
}

/// Result returned after validating DCO on a range of revisions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DcoValidateResult {
    /// Per-revision results, newest first.
    pub revisions: Vec<DcoRevisionResult>,
    /// Total number of revisions checked.
    pub total_checked: u64,
    /// Number of revisions that passed DCO validation.
    pub passed: u64,
    /// Number of revisions missing a DCO sign-off.
    pub failed: u64,
}

/// Check if a commit message contains a valid DCO sign-off line.
pub(crate) fn check_dco_signoff(message: &str) -> (bool, Option<String>) {
    for line in message.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(DCO_SIGNOFF_PREFIX) && trimmed.len() > DCO_SIGNOFF_PREFIX.len() + 1
        {
            return (true, Some(trimmed.to_string()));
        }
    }
    (false, None)
}

/// Validate DCO sign-offs across a range of revisions.
///
/// Walks the revision history and checks each commit message for a
/// `Signed-off-by:` line. Returns a per-revision breakdown plus aggregate
/// pass/fail counts.
pub async fn dco_validate(api: &LoreApi, args: DcoValidateArgs) -> Result<DcoValidateResult> {
    let since_rev = args.since.clone();

    let (callback, rx) = collect_events();

    let status = history(api.globals().build(), args.into_lore(), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("dco_validate (history) failed with status {status}"),
        )));
    }

    // Collect revision hashes from the history.
    let mut rev_hashes: Vec<(String, u64)> = Vec::new();
    for event in &stream.events {
        if let LoreEvent::RevisionHistoryEntry(data) = event {
            let hash = format!("{}", data.revision);
            // Stop if we've hit the since boundary.
            if !since_rev.is_empty() && hash == since_rev {
                break;
            }
            rev_hashes.push((hash, data.revision_number));
        }
    }

    // For each revision, fetch its info (with metadata) to check the commit message.
    let mut result = DcoValidateResult::default();

    for (rev_hash, rev_number) in &rev_hashes {
        let info_args = lore::revision::LoreRevisionInfoArgs {
            revision: LoreString::from_str(rev_hash),
            delta: 0,
            metadata: 1,
        };

        let (cb, rx2) = collect_events();
        let info_status =
            lore::revision::info(api.globals().build(), info_args, cb).await;

        let info_stream = rx2
            .await
            .map_err(|e| {
                LoreError::CommandFailed(format!("info event stream cancelled: {e}"))
            })?;

        if !info_stream.is_ok() {
            return Err(LoreError::CommandFailed(info_stream.error.unwrap_or_else(
                || format!("revision info for {rev_hash} failed with status {info_status}"),
            )));
        }

        // Extract the commit message from metadata.
        let mut message = String::new();
        for event in &info_stream.events {
            if let LoreEvent::Metadata(data) = event {
                if data.key.as_str() == "message" {
                    if let lore::interface::LoreMetadata::String(s) = &data.value {
                        message = s.as_str().to_string();
                    }
                }
            }
        }

        let (dco_signed, signoff_line) = check_dco_signoff(&message);

        if dco_signed {
            result.passed += 1;
        } else {
            result.failed += 1;
        }
        result.total_checked += 1;

        result.revisions.push(DcoRevisionResult {
            revision: rev_hash.clone(),
            revision_number: *rev_number,
            dco_signed,
            signoff_line,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_defaults() {
        let args = DcoValidateArgs::default();
        assert!(args.revision.is_empty());
        assert!(args.since.is_empty());
        assert_eq!(args.limit, 0);
    }

    #[test]
    fn args_into_lore_conversion() {
        let args = DcoValidateArgs {
            revision: "abc123".into(),
            since: "old456".into(),
            limit: 10,
        };
        let lore_args = args.into_lore();
        assert_eq!(lore_args.revision.as_str(), "abc123");
        assert_eq!(lore_args.length, 10);
    }

    #[test]
    fn dco_check_valid_signoff() {
        let msg = "feat: add user authentication\n\nSigned-off-by: Alice <alice@example.com>";
        let (signed, line) = check_dco_signoff(msg);
        assert!(signed);
        assert_eq!(line, Some("Signed-off-by: Alice <alice@example.com>".into()));
    }

    #[test]
    fn dco_check_missing_signoff() {
        let msg = "feat: add user authentication\n\nCo-authored-by: Bob <bob@example.com>";
        let (signed, line) = check_dco_signoff(msg);
        assert!(!signed);
        assert!(line.is_none());
    }

    #[test]
    fn dco_check_empty_message() {
        let (signed, line) = check_dco_signoff("");
        assert!(!signed);
        assert!(line.is_none());
    }

    #[test]
    fn dco_check_multiple_signoffs_takes_first() {
        let msg = "fix: patch vulnerability\n\nSigned-off-by: Alice <alice@example.com>\nSigned-off-by: Bob <bob@example.com>";
        let (signed, line) = check_dco_signoff(msg);
        assert!(signed);
        assert_eq!(line, Some("Signed-off-by: Alice <alice@example.com>".into()));
    }

    #[test]
    fn dco_check_whitespace_tolerant() {
        let msg = "chore: cleanup\n\n  Signed-off-by: Carol <carol@example.com>";
        let (signed, line) = check_dco_signoff(msg);
        assert!(signed);
        assert!(line.is_some());
    }

    #[test]
    fn dco_result_serialises() {
        let result = DcoValidateResult {
            revisions: vec![DcoRevisionResult {
                revision: "abc".into(),
                revision_number: 1,
                dco_signed: true,
                signoff_line: Some("Signed-off-by: Test <t@e.com>".into()),
            }],
            total_checked: 1,
            passed: 1,
            failed: 0,
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("abc"));
        assert!(json.contains("total_checked"));
    }

    #[test]
    fn result_defaults_are_zero() {
        let result = DcoValidateResult::default();
        assert!(result.revisions.is_empty());
        assert_eq!(result.total_checked, 0);
        assert_eq!(result.passed, 0);
        assert_eq!(result.failed, 0);
    }
}
