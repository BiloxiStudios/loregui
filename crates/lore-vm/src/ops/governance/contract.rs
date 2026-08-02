//! The single authoritative serde boundary for SBAI-5934 Option-A governance.

use serde::{Deserialize, Serialize};

/// Prefix for a strict v1 supersession record, followed by its canonical
/// artifact identity.
pub const SUPERSESSION_MARKER_PREFIX: &str = "studiobrain.governance.v1.superseded.";
/// The sole fixed v1 per-revision immutable-evidence pointer key.
pub const EVIDENCE_POINTER_KEY: &str = "studiobrain.governance.v1.evidence";
/// The largest complete candidate-side first-parent stream accepted by policy.
pub const MAX_GOVERNANCE_HISTORY_REVISIONS: usize = 1000;

/// The exact revision binding shared by every governance operation.
pub trait ExactRevisionRequest {
    /// The staged revision the caller observed and is asking to govern.
    fn expected_staged_revision(&self) -> &str;
    /// The fetched base revision that bounds the candidate-side DAG.
    fn target_base_revision(&self) -> &str;
}

macro_rules! strict_exact_request {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub expected_staged_revision: String,
            pub target_base_revision: String,
        }

        impl ExactRevisionRequest for $name {
            fn expected_staged_revision(&self) -> &str {
                &self.expected_staged_revision
            }

            fn target_base_revision(&self) -> &str {
                &self.target_base_revision
            }
        }
    };
}

strict_exact_request!(ArtifactMarkSupersededRequest);
strict_exact_request!(DcoValidateRequest);
strict_exact_request!(EvidencePreserveRequest);
strict_exact_request!(SubmissionGateCheckRequest);

/// Strict v1 metadata payload for a superseded canonical artifact identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupersessionMarkerV1 {
    pub version: String,
    pub identity: String,
}

/// Result returned by an authoritative supersession write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactMarkSupersededResult {
    pub source_staged_revision: String,
    pub result_staged_revision: String,
    pub identity: String,
}

/// Strict v1 immutable evidence pointer attached to one staged revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePointerV1 {
    pub version: String,
    pub address: String,
}

/// Result returned after a durable evidence-preservation attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePreserveResult {
    pub source_staged_revision: String,
    pub result_staged_revision: String,
    pub evidence_address: String,
}

/// The exact, finite inventory used by submission-gate results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceCriterion {
    ExactSubject,
    HistoryComplete,
    DcoValid,
    NotSuperseded,
    LocksClear,
    WorktreeClean,
    EvidenceValid,
}

/// One deterministic criterion observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionResult {
    pub criterion: GovernanceCriterion,
    pub passed: bool,
    pub failure_code: Option<String>,
}

/// Strict result schema for the eventual canonical submission gate operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmissionGateCheckResult {
    pub gate_open: bool,
    pub criteria: Vec<CriterionResult>,
}

/// Strict result schema for a DCO-only operation using the shared evaluator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcoValidateResult {
    pub valid: bool,
    pub pending_revisions: Vec<String>,
    pub failure_codes: Vec<String>,
}
