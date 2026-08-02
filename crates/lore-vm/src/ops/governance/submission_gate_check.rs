//! `governance::submission_gate_check` — check whether a branch is ready for
//! submission / merge.
//!
//! Evaluates a set of gate conditions against the current worktree state:
//! - No uncommitted changes (clean working directory)
//! - No unresolved conflicts
//! - DCO sign-off present on HEAD
//! - No superseded-artifact metadata on HEAD
//!
//! All checks use lore primitives (`lore::repository::status`,
//! `lore::revision::info`). No upstream `lore` function implements this gate.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::interface::{LoreEvent, LoreMetadata, LoreString};
use lore::repository::{LoreRepositoryStatusArgs, status};
use lore::revision::{LoreRevisionInfoArgs, info};
use serde::{Deserialize, Serialize};

use super::artifact_mark_superseded::METADATA_KEY_SUPERSEDED_BY;
use super::dco_validate::check_dco_signoff;

/// Arguments for [`submission_gate_check`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubmissionGateCheckArgs {
    /// Require a DCO sign-off on HEAD. Default: true.
    #[serde(default = "default_require_dco")]
    pub require_dco: bool,
    /// Reject if HEAD carries superseded-by metadata. Default: true.
    #[serde(default = "default_reject_superseded")]
    pub reject_superseded: bool,
    /// Require a clean working directory (no uncommitted changes). Default: true.
    #[serde(default = "default_require_clean")]
    pub require_clean_workdir: bool,
}

fn default_require_dco() -> bool {
    true
}

fn default_reject_superseded() -> bool {
    true
}

fn default_require_clean() -> bool {
    true
}

/// Individual gate check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCheck {
    /// Human-readable name of the check.
    pub name: String,
    /// Whether this check passed.
    pub passed: bool,
    /// Detail message (reason for failure or success note).
    pub detail: String,
}

/// Result returned after evaluating all submission gate checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionGateCheckResult {
    /// Whether ALL checks passed.
    pub gate_open: bool,
    /// Individual check results.
    pub checks: Vec<GateCheck>,
    /// Revision hash of HEAD at check time.
    pub head_revision: String,
    /// Branch name at check time.
    pub branch_name: String,
}

/// Evaluate the submission gate for the current worktree.
///
/// Returns a `gate_open: bool` plus per-check details so callers can report
/// exactly which conditions failed and why.
pub async fn submission_gate_check(
    api: &LoreApi,
    args: SubmissionGateCheckArgs,
) -> Result<SubmissionGateCheckResult> {
    let mut checks: Vec<GateCheck> = Vec::new();
    let mut head_revision = String::new();
    let mut branch_name = String::new();

    // 1. Check working directory cleanliness (no uncommitted changes, no conflicts).
    if args.require_clean_workdir {
        let (cb, rx) = collect_events();
        let status_args = LoreRepositoryStatusArgs {
            staged: 0,
            scan: 1,
            check_dirty: 0,
            reset: 0,
            sync_point: 0,
            revision_only: 0,
            count: 0,
            paths: lore::interface::LoreArray::from_vec(vec![]),
        };
        let _status_val = status(api.globals().build(), status_args, cb).await;

        let stream = rx
            .await
            .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

        if stream.is_ok() {
            let mut dirty_count = 0u64;
            let mut conflict_count = 0u64;

            for event in &stream.events {
                match event {
                    LoreEvent::RepositoryStatusRevision(data) => {
                        head_revision = format!("{}", data.revision);
                        branch_name = data.branch_name.as_str().to_string();
                    }
                    LoreEvent::RepositoryStatusFile(data) => {
                        if data.flag_dirty != 0 {
                            dirty_count += 1;
                        }
                        if data.flag_conflict != 0 {
                            conflict_count += 1;
                        }
                    }
                    _ => {}
                }
            }

            if dirty_count == 0 && conflict_count == 0 {
                checks.push(GateCheck {
                    name: "clean_workdir".into(),
                    passed: true,
                    detail: "no uncommitted changes or conflicts".into(),
                });
            } else {
                checks.push(GateCheck {
                    name: "clean_workdir".into(),
                    passed: false,
                    detail: format!("{dirty_count} dirty file(s), {conflict_count} conflict(s)"),
                });
            }
        } else {
            checks.push(GateCheck {
                name: "clean_workdir".into(),
                passed: false,
                detail: stream
                    .error
                    .unwrap_or_else(|| "status check failed".into()),
            });
        }
    } else {
        // Still fetch revision info for head_revision / branch_name.
        checks.push(GateCheck {
            name: "clean_workdir".into(),
            passed: true,
            detail: "skipped (not required)".into(),
        });
    }

    // If we didn't get head_revision from status, try revision info.
    if head_revision.is_empty() {
        let (cb2, rx2) = collect_events();
        let info_args = LoreRevisionInfoArgs {
            revision: LoreString::from_str(""),
            delta: 0,
            metadata: 1,
        };
        let _info_status = info(api.globals().build(), info_args, cb2).await;

        let info_stream = rx2.await.map_err(|e| {
            LoreError::CommandFailed(format!("info event stream cancelled: {e}"))
        })?;

        if info_stream.is_ok() {
            for event in &info_stream.events {
                if let LoreEvent::RevisionInfo(data) = event {
                    head_revision = format!("{}", data.revision);
                }
            }
        }
    }

    if branch_name.is_empty() {
        branch_name = "<unknown>".into();
    }

    // 2. DCO sign-off check on HEAD.
    if args.require_dco {
        let (cb, rx) = collect_events();
        let info_args = LoreRevisionInfoArgs {
            revision: LoreString::from_str(""),
            delta: 0,
            metadata: 1,
        };
        let _dco_status = info(api.globals().build(), info_args, cb).await;

        let dco_stream = rx
            .await
            .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

        if dco_stream.is_ok() {
            let mut message = String::new();
            for event in &dco_stream.events {
                if let LoreEvent::Metadata(data) = event {
                    if data.key.as_str() == "message" {
                        if let LoreMetadata::String(s) = &data.value {
                            message = s.as_str().to_string();
                        }
                    }
                }
            }

            let (signed, _) = check_dco_signoff(&message);
            if signed {
                checks.push(GateCheck {
                    name: "dco_signoff".into(),
                    passed: true,
                    detail: "HEAD has Signed-off-by line".into(),
                });
            } else {
                checks.push(GateCheck {
                    name: "dco_signoff".into(),
                    passed: false,
                    detail: "HEAD commit message missing Signed-off-by line".into(),
                });
            }
        } else {
            checks.push(GateCheck {
                name: "dco_signoff".into(),
                passed: false,
                detail: dco_stream
                    .error
                    .unwrap_or_else(|| "revision info failed".into()),
            });
        }
    } else {
        checks.push(GateCheck {
            name: "dco_signoff".into(),
            passed: true,
            detail: "skipped (not required)".into(),
        });
    }

    // 3. Superseded-artifact check on HEAD.
    if args.reject_superseded {
        let (cb, rx) = collect_events();
        let info_args = LoreRevisionInfoArgs {
            revision: LoreString::from_str(""),
            delta: 0,
            metadata: 1,
        };
        let _super_status = info(api.globals().build(), info_args, cb).await;

        let super_stream = rx.await.map_err(|e| {
            LoreError::CommandFailed(format!("event stream cancelled: {e}"))
        })?;

        if super_stream.is_ok() {
            let mut is_superseded = false;
            let mut superseded_by = String::new();

            for event in &super_stream.events {
                if let LoreEvent::Metadata(data) = event {
                    if data.key.as_str() == METADATA_KEY_SUPERSEDED_BY {
                        is_superseded = true;
                        if let LoreMetadata::String(s) = &data.value {
                            superseded_by = s.as_str().to_string();
                        }
                    }
                }
            }

            if is_superseded {
                checks.push(GateCheck {
                    name: "not_superseded".into(),
                    passed: false,
                    detail: format!("HEAD is superseded by {superseded_by}"),
                });
            } else {
                checks.push(GateCheck {
                    name: "not_superseded".into(),
                    passed: true,
                    detail: "HEAD has no superseded-by metadata".into(),
                });
            }
        } else {
            checks.push(GateCheck {
                name: "not_superseded".into(),
                passed: false,
                detail: super_stream
                    .error
                    .unwrap_or_else(|| "revision info failed".into()),
            });
        }
    } else {
        checks.push(GateCheck {
            name: "not_superseded".into(),
            passed: true,
            detail: "skipped (not required)".into(),
        });
    }

    let gate_open = checks.iter().all(|c| c.passed);

    Ok(SubmissionGateCheckResult {
        gate_open,
        checks,
        head_revision,
        branch_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_defaults_all_true() {
        let json = r#"{}"#;
        let args: SubmissionGateCheckArgs =
            serde_json::from_str(json).expect("should deserialize");
        assert!(args.require_dco);
        assert!(args.reject_superseded);
        assert!(args.require_clean_workdir);
    }

    #[test]
    fn args_can_disable_individual_checks() {
        let json = r#"{"require_dco":false,"reject_superseded":false}"#;
        let args: SubmissionGateCheckArgs =
            serde_json::from_str(json).expect("should deserialize");
        assert!(!args.require_dco);
        assert!(!args.reject_superseded);
        assert!(args.require_clean_workdir);
    }

    #[test]
    fn gate_result_all_pass_is_open() {
        let result = SubmissionGateCheckResult {
            gate_open: true,
            checks: vec![
                GateCheck {
                    name: "clean_workdir".into(),
                    passed: true,
                    detail: "ok".into(),
                },
                GateCheck {
                    name: "dco_signoff".into(),
                    passed: true,
                    detail: "ok".into(),
                },
            ],
            head_revision: "abc".into(),
            branch_name: "main".into(),
        };
        assert!(result.gate_open);
        assert!(result.checks.iter().all(|c| c.passed));
    }

    #[test]
    fn gate_result_one_fail_is_closed() {
        let result = SubmissionGateCheckResult {
            gate_open: false,
            checks: vec![
                GateCheck {
                    name: "clean_workdir".into(),
                    passed: true,
                    detail: "ok".into(),
                },
                GateCheck {
                    name: "dco_signoff".into(),
                    passed: false,
                    detail: "missing signoff".into(),
                },
            ],
            head_revision: "abc".into(),
            branch_name: "main".into(),
        };
        assert!(!result.gate_open);
    }

    #[test]
    fn result_serialises() {
        let result = SubmissionGateCheckResult {
            gate_open: true,
            checks: vec![GateCheck {
                name: "clean_workdir".into(),
                passed: true,
                detail: "clean".into(),
            }],
            head_revision: "rev1".into(),
            branch_name: "feature".into(),
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("gate_open"));
        assert!(json.contains("rev1"));
        assert!(json.contains("feature"));
    }
}
