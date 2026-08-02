//! The single authoritative serde boundary for SBAI-5934 Option-A governance.

use serde::{Deserialize, Serialize};

/// Prefix for a strict v1 supersession record, followed by its canonical
/// artifact identity.
pub const SUPERSESSION_MARKER_PREFIX: &str = "studiobrain.governance.v1.superseded.";
/// The sole fixed v1 per-revision immutable-evidence pointer key.
pub const EVIDENCE_POINTER_KEY: &str = "studiobrain.governance.v1.evidence";
/// The largest complete unique candidate-side pending DAG accepted by policy.
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

/// A dependency failure reported by a production Lore adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterError {
    pub message: String,
}

impl AdapterError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Exact staged state and proof of the full status scan used by governance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusSnapshot {
    /// Exact revision-only subject read before the scan.
    pub staged_revisions: Vec<String>,
    /// Subject reported by the full staged filesystem scan.
    pub scanned_staged_revisions: Vec<String>,
    /// Exact revision-only subject reread after the scan.
    pub post_scan_staged_revisions: Vec<String>,
    pub staged_paths: Vec<String>,
    pub worktree_clean: bool,
    pub scan_performed: bool,
}

/// Exact revision information required for graph traversal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionInfo {
    pub revision: String,
    pub parents: Vec<String>,
}

/// Every raw `RevisionInfo` event observed through the terminal `End` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionInfoResponse {
    pub revisions: Vec<RevisionInfo>,
}

impl RevisionInfoResponse {
    pub fn exact(info: RevisionInfo) -> Self {
        Self {
            revisions: vec![info],
        }
    }
}

/// One exact revision metadata pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
}

impl MetadataEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// A file identity read at one explicit revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity {
    pub path: String,
    pub revision: String,
    pub hash: String,
    pub context: String,
}

impl FileIdentity {
    pub fn new(
        path: impl Into<String>,
        revision: impl Into<String>,
        hash: impl Into<String>,
        context: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            revision: revision.into(),
            hash: hash.into(),
            context: context.into(),
        }
    }

    pub fn canonical_id(&self) -> String {
        format!("{}:{}", self.hash, self.context)
    }
}

/// An exact base-to-candidate changed path, including rename endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AffectedPath {
    pub source_path: Option<String>,
    pub target_path: Option<String>,
}

impl AffectedPath {
    pub fn modified(path: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            source_path: Some(path.clone()),
            target_path: Some(path),
        }
    }
}

/// One author resolution response from `auth.resolve_user_info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedAuthor {
    pub identity: String,
    pub display_name: String,
}

impl ResolvedAuthor {
    pub fn new(identity: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            identity: identity.into(),
            display_name: display_name.into(),
        }
    }
}

/// Complete response for a single lock query, including the requested identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockQuery {
    pub path: String,
    pub begin_events: usize,
    pub expected_count: usize,
    pub completed: bool,
    pub ignored_paths: Vec<String>,
    pub owners: Vec<String>,
}

impl LockQuery {
    pub fn unlocked(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            begin_events: 1,
            expected_count: 0,
            completed: true,
            ignored_paths: Vec::new(),
            owners: Vec::new(),
        }
    }

    pub fn incomplete(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            begin_events: 0,
            expected_count: 0,
            completed: false,
            ignored_paths: Vec::new(),
            owners: Vec::new(),
        }
    }

    pub fn with_owners(
        path: impl Into<String>,
        expected_count: usize,
        owners: Vec<String>,
    ) -> Self {
        Self {
            path: path.into(),
            begin_events: 1,
            expected_count,
            completed: true,
            ignored_paths: Vec::new(),
            owners,
        }
    }
}

/// One response in the complete lock-status stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockStatus {
    pub path: String,
    pub owner: Option<String>,
}

impl LockStatus {
    pub fn unlocked(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            owner: None,
        }
    }

    pub fn locked(path: impl Into<String>, owner: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            owner: Some(owner.into()),
        }
    }
}

/// Raw, terminally verified lock-status stream. `statuses` contains lock
/// events only; an unlocked path is proved by a zero-count successful stream
/// with no ignored paths, not by fabricating an unlocked event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockStatusResponse {
    pub begin_events: usize,
    pub expected_count: usize,
    pub completed: bool,
    pub ignored_paths: Vec<String>,
    pub statuses: Vec<LockStatus>,
}

impl LockStatusResponse {
    pub fn unlocked() -> Self {
        Self {
            begin_events: 1,
            expected_count: 0,
            completed: true,
            ignored_paths: Vec::new(),
            statuses: Vec::new(),
        }
    }

    pub fn incomplete() -> Self {
        Self {
            begin_events: 0,
            expected_count: 0,
            completed: false,
            ignored_paths: Vec::new(),
            statuses: Vec::new(),
        }
    }

    pub fn with_locks(expected_count: usize, statuses: Vec<LockStatus>) -> Self {
        Self {
            begin_events: 1,
            expected_count,
            completed: true,
            ignored_paths: Vec::new(),
            statuses,
        }
    }

    pub fn ignored(path: impl Into<String>) -> Self {
        Self {
            begin_events: 1,
            expected_count: 0,
            completed: true,
            ignored_paths: vec![path.into()],
            statuses: Vec::new(),
        }
    }
}

/// Deterministic evaluator result. A dependency failure is represented as a
/// closed result so callers cannot accidentally convert it into an open gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationResult {
    pub open: bool,
    pub pending_revisions: Vec<String>,
    pub affected_paths: Vec<String>,
    pub identities: Vec<String>,
    pub superseded_identities: Vec<String>,
    pub failure_codes: Vec<String>,
}
