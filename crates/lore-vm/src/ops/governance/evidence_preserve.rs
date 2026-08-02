//! `governance::evidence_preserve` — capture and serialise worktree state as
//! an evidence snapshot for audit / incident-response purposes.
//!
//! Collects revision info, working-directory status, and governance metadata
//! into a single JSON-serialisable snapshot. This op does not call any
//! upstream `lore` governance function (none exists) — it composes
//! `lore::revision::info` and `lore::repository::status`.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreMetadata, LoreString};
use lore::repository::{LoreRepositoryStatusArgs, status};
use lore::revision::{LoreRevisionInfoArgs, info};
use serde::{Deserialize, Serialize};

use super::artifact_mark_superseded::{METADATA_KEY_SUPERSEDED_BY, METADATA_KEY_SUPERSEDED_REASON};

/// Arguments for [`evidence_preserve`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvidencePreserveArgs {
    /// Snapshot label / incident identifier (e.g. "INC-2026-042").
    #[serde(default)]
    pub label: String,
    /// Revision to snapshot; empty targets HEAD.
    #[serde(default)]
    pub revision: String,
    /// Include per-file status in the snapshot. Default: true.
    #[serde(default = "default_include_status")]
    pub include_file_status: bool,
}

fn default_include_status() -> bool {
    true
}

impl EvidencePreserveArgs {
    fn status_args(&self) -> LoreRepositoryStatusArgs {
        LoreRepositoryStatusArgs {
            staged: 1,
            scan: 1,
            check_dirty: 0,
            reset: 0,
            sync_point: 1,
            revision_only: 0,
            count: 1,
            paths: lore::interface::LoreArray::from_vec(vec![]),
        }
    }

    fn info_args(&self) -> LoreRevisionInfoArgs {
        LoreRevisionInfoArgs {
            revision: LoreString::from_str(&self.revision),
            delta: 0,
            metadata: 1,
        }
    }
}

/// A file entry captured in the evidence snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceFileEntry {
    /// Repository-relative path.
    pub path: String,
    /// File size in bytes.
    pub size: u64,
    /// Change action.
    pub action: String,
    /// Whether the change is staged.
    pub staged: bool,
    /// Whether the file is in conflict.
    pub conflict: bool,
    /// Whether the file differs from recorded state.
    pub dirty: bool,
}

/// Metadata key/value pair captured in the snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceMetadataEntry {
    pub key: String,
    pub value: String,
}

/// Governance-specific fields captured in the snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvidenceGovernance {
    /// Whether the revision carries superseded-by metadata.
    pub is_superseded: bool,
    /// The superseding revision hash, if any.
    pub superseded_by: Option<String>,
    /// Reason for supersession, if any.
    pub superseded_reason: Option<String>,
    /// Whether the commit message contains a DCO sign-off.
    pub dco_signed: bool,
    /// The DCO sign-off line, if present.
    pub dco_signoff_line: Option<String>,
}

/// A self-contained evidence snapshot of worktree state at a point in time.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvidencePreserveResult {
    /// Snapshot label (from args).
    pub label: String,
    /// Timestamp when the snapshot was captured (ISO 8601).
    pub captured_at: String,
    /// Repository identifier.
    pub repository: String,
    /// Branch name.
    pub branch_name: String,
    /// Revision hash.
    pub revision: String,
    /// Revision number.
    pub revision_number: u64,
    /// Staged revision hash (empty if none).
    pub revision_staged: String,
    /// Governance-specific fields.
    pub governance: EvidenceGovernance,
    /// Metadata key/value pairs from the revision.
    pub metadata: Vec<EvidenceMetadataEntry>,
    /// File status entries (when `include_file_status` is true).
    pub files: Vec<EvidenceFileEntry>,
    /// Total directory count at snapshot time.
    pub dir_count: Option<u64>,
    /// Total file count at snapshot time.
    pub file_count: Option<u64>,
}

fn hash_or_empty(hash: &lore::interface::Hash) -> String {
    if hash.is_zero() {
        String::new()
    } else {
        format!("{hash}")
    }
}

fn metadata_display(value: &LoreMetadata) -> String {
    match value {
        LoreMetadata::String(s) => s.as_str().to_string(),
        LoreMetadata::Numeric(n) => n.to_string(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Capture a self-contained evidence snapshot of worktree state.
///
/// Collects revision info, working-directory status, and governance metadata
/// into a single serialisable result suitable for archival, incident response,
/// or audit trails.
pub async fn evidence_preserve(
    api: &LoreApi,
    args: EvidencePreserveArgs,
) -> Result<EvidencePreserveResult> {
    let captured_at = chrono_offset_datetime();
    let mut result = EvidencePreserveResult {
        label: args.label.clone(),
        captured_at,
        ..Default::default()
    };

    // 1. Revision info — get metadata (commit message, author, etc.).
    let (cb, rx) = collect_events();
    let info_status = info(api.globals().build(), args.info_args(), cb).await;

    let info_stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("info event stream cancelled: {e}")))?;

    if !info_stream.is_ok() {
        return Err(LoreError::CommandFailed(info_stream.error.unwrap_or_else(
            || format!("evidence_preserve (info) failed with status {info_status}"),
        )));
    }

    for event in &info_stream.events {
        match event {
            LoreEvent::RevisionInfo(data) => {
                result.repository = format!("{}", data.repository);
                result.revision = hash_or_empty(&data.revision);
                result.revision_number = data.revision_number;
                // branch_name and revision_staged come from the status call below
            }
            LoreEvent::Metadata(data) => {
                let value = metadata_display(&data.value);
                result.metadata.push(EvidenceMetadataEntry {
                    key: data.key.as_str().to_string(),
                    value: value.clone(),
                });

                // Governance: check for DCO sign-off.
                if data.key.as_str() == "message" {
                    let (signed, line) = super::dco_validate::check_dco_signoff(&value);
                    result.governance.dco_signed = signed;
                    result.governance.dco_signoff_line = line;
                }

                // Governance: check for superseded metadata.
                if data.key.as_str() == METADATA_KEY_SUPERSEDED_BY {
                    result.governance.is_superseded = true;
                    result.governance.superseded_by = Some(value.clone());
                }
                if data.key.as_str() == METADATA_KEY_SUPERSEDED_REASON {
                    result.governance.superseded_reason = Some(value);
                }
            }
            _ => {}
        }
    }

    // 2. Working directory status — if requested.
    if args.include_file_status {
        let (cb2, rx2) = collect_events();
        let _status_val =
            status(api.globals().build(), args.status_args(), cb2).await;

        let status_stream = rx2.await.map_err(|e| {
            LoreError::CommandFailed(format!("status event stream cancelled: {e}"))
        })?;

        if status_stream.is_ok() {
            for event in &status_stream.events {
                match event {
                    LoreEvent::RepositoryStatusRevision(data) => {
                        result.branch_name = data.branch_name.as_str().to_string();
                        result.revision_staged = hash_or_empty(&data.revision_staged);
                    }
                    LoreEvent::RepositoryStatusFile(data) => {
                        result.files.push(EvidenceFileEntry {
                            path: data.path.as_str().to_string(),
                            size: data.size,
                            action: format!("{:?}", data.action),
                            staged: data.flag_staged != 0,
                            conflict: data.flag_conflict != 0,
                            dirty: data.flag_dirty != 0,
                        });
                    }
                    LoreEvent::RepositoryStatusCount(data) => {
                        result.dir_count = Some(data.directories);
                        result.file_count = Some(data.files);
                    }
                    _ => {}
                }
            }
        }
        // Status failure is non-fatal for evidence capture — we still have
        // revision info and metadata.
    }

    Ok(result)
}

/// Best-effort UTC timestamp in ISO 8601 format.
///
/// Uses `std::time::SystemTime` so we don't pull in `chrono` as a dependency.
fn chrono_offset_datetime() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let nanos = duration.subsec_nanos();

    // Calculate date/time from Unix timestamp (simplified UTC).
    let (year, month, day, hour, minute, second) = unix_timestamp_to_utc(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        year, month, day, hour, minute, second, nanos
    )
}

/// Convert a Unix timestamp to (year, month, day, hour, minute, second) in UTC.
fn unix_timestamp_to_utc(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    const SECS_PER_MINUTE: u64 = 60;
    const SECS_PER_HOUR: u64 = 3600;
    const SECS_PER_DAY: u64 = 86400;

    let total_days = secs / SECS_PER_DAY;
    let remaining_secs = secs % SECS_PER_DAY;

    let hour = (remaining_secs / SECS_PER_HOUR) as u32;
    let minute = ((remaining_secs % SECS_PER_HOUR) / SECS_PER_MINUTE) as u32;
    let second = (remaining_secs % SECS_PER_MINUTE) as u32;

    // Calculate year/month/day from total_days since epoch (1970-01-01).
    let mut days = total_days as i64;
    let mut year: i64 = 1970;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let month_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month: usize = 0;
    for (i, &dm) in month_days.iter().enumerate() {
        if days < dm as i64 {
            month = i;
            break;
        }
        days -= dm as i64;
    }

    (year as u32, (month + 1) as u32, (days + 1) as u32, hour, minute, second)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_defaults() {
        let json = r#"{}"#;
        let args: EvidencePreserveArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.label.is_empty());
        assert!(args.revision.is_empty());
        assert!(args.include_file_status);
    }

    #[test]
    fn args_with_label() {
        let json = r#"{"label":"INC-2026-042","include_file_status":false}"#;
        let args: EvidencePreserveArgs = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(args.label, "INC-2026-042");
        assert!(!args.include_file_status);
    }

    #[test]
    fn result_defaults() {
        let result = EvidencePreserveResult::default();
        assert!(result.label.is_empty());
        // Default struct has empty captured_at; the real fn populates it.
        assert!(result.files.is_empty());
        assert!(result.metadata.is_empty());
        assert!(!result.governance.is_superseded);
        assert!(!result.governance.dco_signed);
    }

    #[test]
    fn result_serialises() {
        let result = EvidencePreserveResult {
            label: "TEST-001".into(),
            captured_at: "2026-08-02T12:00:00.000000000Z".into(),
            repository: "repo1".into(),
            branch_name: "main".into(),
            revision: "abc123".into(),
            revision_number: 42,
            revision_staged: String::new(),
            governance: EvidenceGovernance {
                is_superseded: false,
                superseded_by: None,
                superseded_reason: None,
                dco_signed: true,
                dco_signoff_line: Some("Signed-off-by: Test <t@e.com>".into()),
            },
            metadata: vec![EvidenceMetadataEntry {
                key: "message".into(),
                value: "test commit".into(),
            }],
            files: vec![EvidenceFileEntry {
                path: "src/lib.rs".into(),
                size: 1024,
                action: "Add".into(),
                staged: true,
                conflict: false,
                dirty: false,
            }],
            dir_count: Some(5),
            file_count: Some(20),
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("TEST-001"));
        assert!(json.contains("abc123"));
        assert!(json.contains("Signed-off-by"));
        assert!(json.contains("src/lib.rs"));
    }

    #[test]
    fn unix_timestamp_to_utc_epoch() {
        let (y, m, d, h, min, s) = unix_timestamp_to_utc(0);
        assert_eq!(y, 1970);
        assert_eq!(m, 1);
        assert_eq!(d, 1);
        assert_eq!(h, 0);
        assert_eq!(min, 0);
        assert_eq!(s, 0);
    }

    #[test]
    fn unix_timestamp_to_utc_known_date() {
        // 2026-08-02 00:00:00 UTC = 20667 days from epoch
        let secs: u64 = 1785628800;
        let (y, m, d, h, min, s) = unix_timestamp_to_utc(secs);
        assert_eq!(y, 2026);
        assert_eq!(m, 8);
        assert_eq!(d, 2);
        assert_eq!(h, 0);
        assert_eq!(min, 0);
        assert_eq!(s, 0);
    }

    #[test]
    fn is_leap_year_checks() {
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(2025));
        assert!(!is_leap_year(1900));
        assert!(is_leap_year(2000));
    }

    #[test]
    fn captured_at_is_iso8601() {
        let ts = chrono_offset_datetime();
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('T'));
        assert!(ts.len() > 20);
    }

    #[test]
    fn hash_or_empty_zero_is_empty() {
        let zero = lore::interface::Hash::default();
        assert!(hash_or_empty(&zero).is_empty());
    }

    #[test]
    fn metadata_display_string_and_numeric() {
        use lore::interface::LoreString;
        assert_eq!(
            metadata_display(&LoreMetadata::String(LoreString::from("hello"))),
            "hello"
        );
        assert_eq!(metadata_display(&LoreMetadata::Numeric(42)), "42");
    }

    #[test]
    fn governance_defaults_are_false() {
        let gov = EvidenceGovernance::default();
        assert!(!gov.is_superseded);
        assert!(gov.superseded_by.is_none());
        assert!(gov.superseded_reason.is_none());
        assert!(!gov.dco_signed);
        assert!(gov.dco_signoff_line.is_none());
    }
}
