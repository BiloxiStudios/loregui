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

strict_exact_request!(DcoValidateRequest);
strict_exact_request!(SubmissionGateCheckRequest);

/// Evidence publication cannot create an attempt for an unrepresentable empty
/// subject. Serde and direct operation entry both call the same validator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidencePreserveRequest {
    pub expected_staged_revision: String,
    pub target_base_revision: String,
}

impl EvidencePreserveRequest {
    pub fn validate(&self) -> std::result::Result<(), &'static str> {
        if self.expected_staged_revision.is_empty() || self.target_base_revision.is_empty() {
            Err("evidence preserve requires nonempty exact revisions")
        } else {
            Ok(())
        }
    }

    pub(crate) fn validated(
        &self,
    ) -> std::result::Result<ValidatedEvidencePreserveRequestV1<'_>, &'static str> {
        self.validate()?;
        Ok(ValidatedEvidencePreserveRequestV1 { request: self })
    }
}

/// Unforgeable state-machine entrance. Its field is private and its sole
/// constructor reuses the strict direct/serde request validator.
pub(crate) struct ValidatedEvidencePreserveRequestV1<'a> {
    request: &'a EvidencePreserveRequest,
}

impl<'de> Deserialize<'de> for EvidencePreserveRequest {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            expected_staged_revision: String,
            target_base_revision: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        let request = Self {
            expected_staged_revision: wire.expected_staged_revision,
            target_base_revision: wire.target_base_revision,
        };
        request.validate().map_err(serde::de::Error::custom)?;
        Ok(request)
    }
}

impl ExactRevisionRequest for EvidencePreserveRequest {
    fn expected_staged_revision(&self) -> &str {
        &self.expected_staged_revision
    }

    fn target_base_revision(&self) -> &str {
        &self.target_base_revision
    }
}

/// Strict future request for the authoritative v2 supersession writer.
///
/// Task 2 intentionally exposes no writer or dispatch route.  The mandatory
/// path selector prevents a future implementation from choosing an arbitrary
/// identity when the exact candidate tree contains more than one file; v2 must
/// resolve this path to exactly one canonical identity before it may write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactMarkSupersededRequest {
    pub expected_staged_revision: String,
    pub target_base_revision: String,
    pub target_path: String,
}

impl ExactRevisionRequest for ArtifactMarkSupersededRequest {
    fn expected_staged_revision(&self) -> &str {
        &self.expected_staged_revision
    }

    fn target_base_revision(&self) -> &str {
        &self.target_base_revision
    }
}

/// Strict v1 metadata payload for a superseded canonical artifact identity.
///
/// The Option-A supersession guarantee is monotonic only across the complete
/// parent DAG reachable from the exact staged subject at evidence-preserve
/// time; any post-preserve change must fail live re-evaluation. Explicit or
/// forced sync, branch reset, force push, forced restore, and cross-line
/// cherry-pick are outside that reachable-DAG guarantee and require the
/// provenance/external-prior-state follow-up tracked by SBAI-6011.
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

/// One strict supersession record observed at an exact revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupersessionObservation {
    pub revision: String,
    pub key: String,
    pub value: String,
    pub identity: String,
}

/// Exact DCO trailer, author identities, and resolved correspondence inputs for
/// one pending revision. These are observations, not a pass/fail assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcoObservation {
    pub revision: String,
    pub message: String,
    pub trailer: String,
    pub signer_name: String,
    pub signer_email: String,
    pub created_by: String,
    pub committed_by: Option<String>,
    pub resolved_authors: Vec<ResolvedAuthor>,
}

/// Exact DCO-relevant metadata cardinality observed for one pending revision.
/// Vectors retain duplicate values so ambiguity cannot be normalized away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcoMetadataObservation {
    pub revision: String,
    pub messages: Vec<String>,
    pub created_by: Vec<String>,
    pub committed_by: Vec<String>,
}

/// Exact request and raw replies from one completed author-resolution call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorResolutionObservation {
    pub requested: Vec<String>,
    pub replies: Vec<ResolvedAuthor>,
}

/// Exact result of one whole-ancestry revision-metadata query. An entry is
/// retained even when `metadata` is empty so a witness can prove that every
/// reachable revision was queried instead of trusting a marker summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupersessionMetadataQueryObservation {
    pub revision: String,
    pub metadata: Vec<MetadataEntry>,
}

/// All raw deterministic inputs actually observed by one evaluator run.
/// Optional fields stay absent when the dependency could not be observed; the
/// corresponding exact failure is recorded in `dependency_observations`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceObservations {
    pub expected_staged_revision: String,
    pub target_base_revision: String,
    pub status: Option<StatusSnapshot>,
    pub base_revision_info: Option<RevisionInfo>,
    /// Exact candidate-to-roots graph used only for whole-ancestry
    /// supersession. The pending submission graph below excludes the base.
    pub supersession_ancestry: Vec<RevisionInfo>,
    pub supersession_ancestry_observed: bool,
    pub revision_graph: Vec<RevisionInfo>,
    pub first_parent_history: Vec<String>,
    pub base_files: Vec<FileIdentity>,
    /// True only after the exact base tree stream and all file identities
    /// completed successfully. An empty tree is otherwise ambiguous.
    pub base_tree_observed: bool,
    pub candidate_files: Vec<FileIdentity>,
    /// True only after the exact candidate tree stream and all file identities
    /// completed successfully. An empty tree is otherwise ambiguous.
    pub candidate_tree_observed: bool,
    /// Effective capture-time identities. Selected filesystem-backed paths use
    /// the exact local content hash plus staged context; unselected paths use
    /// the exact revision identity.
    pub current_files: Vec<FileIdentity>,
    pub upstream_revision_diff: Vec<RevisionDiffObservation>,
    pub revision_diff: Vec<AffectedPath>,
    /// True only after the complete exact base-to-candidate diff was observed.
    pub revision_diff_observed: bool,
    pub affected_paths: Vec<String>,
    pub supersession_markers: Vec<SupersessionObservation>,
    /// One raw metadata-query result for every revision in
    /// `supersession_ancestry`, including revisions whose result was empty.
    pub supersession_metadata_queries: Vec<SupersessionMetadataQueryObservation>,
    /// True only after marker metadata for every whole-ancestry revision was
    /// completely observed. This is dependency presence, not a verdict.
    pub supersession_metadata_observed: bool,
    pub dco_metadata: Vec<DcoMetadataObservation>,
    pub author_resolution: Option<AuthorResolutionObservation>,
    pub dco: Vec<DcoObservation>,
    pub lock_queries: Vec<LockQuery>,
    pub lock_status: Option<LockStatusResponse>,
    /// Evaluator diagnostic only. Witness decisions derive the scope and
    /// remediation exclusively from the validated N+1 raw graph.
    pub history_overflow_scope: Option<HistoryOverflowScope>,
    pub dependency_observations: Vec<String>,
}

impl GovernanceObservations {
    pub fn new(
        expected_staged_revision: impl Into<String>,
        target_base_revision: impl Into<String>,
    ) -> Self {
        Self {
            expected_staged_revision: expected_staged_revision.into(),
            target_base_revision: target_base_revision.into(),
            status: None,
            base_revision_info: None,
            supersession_ancestry: Vec::new(),
            supersession_ancestry_observed: false,
            revision_graph: Vec::new(),
            first_parent_history: Vec::new(),
            base_files: Vec::new(),
            base_tree_observed: false,
            candidate_files: Vec::new(),
            candidate_tree_observed: false,
            current_files: Vec::new(),
            upstream_revision_diff: Vec::new(),
            revision_diff: Vec::new(),
            revision_diff_observed: false,
            affected_paths: Vec::new(),
            supersession_markers: Vec::new(),
            supersession_metadata_queries: Vec::new(),
            supersession_metadata_observed: false,
            dco_metadata: Vec::new(),
            author_resolution: None,
            dco: Vec::new(),
            lock_queries: Vec::new(),
            lock_status: None,
            history_overflow_scope: None,
            dependency_observations: Vec::new(),
        }
    }
}

/// Typed revision reference used only in canonical evidence.  The one staged
/// subject is the sole revision allowed to change during pointer attach.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    tag = "kind",
    content = "revision"
)]
pub enum CanonicalRevisionRefV1 {
    StagedSubject,
    Exact(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalStatusObservationV1 {
    pub branch: String,
    pub staged_revisions: Vec<CanonicalRevisionRefV1>,
    pub scanned_staged_revisions: Vec<CanonicalRevisionRefV1>,
    pub post_scan_staged_revisions: Vec<CanonicalRevisionRefV1>,
    pub staged_paths: Vec<String>,
    pub staged_changes: Vec<StagedPathObservation>,
    pub worktree_files: Vec<CanonicalWorktreeFileObservationV1>,
    pub worktree_clean: bool,
    pub scan_performed: bool,
}

/// Exact staged-revision and local-filesystem values used to prove that one
/// tracked path did not change after staging. No derived cleanliness claim is
/// embedded in this record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalWorktreeFileObservationV1 {
    pub path: String,
    pub revision: CanonicalRevisionRefV1,
    pub revision_hash: String,
    pub revision_context: String,
    pub revision_size: u64,
    pub local_hash: String,
    pub local_size: u64,
    pub filtered_revision_size: u64,
    pub flag_modified: bool,
    pub flag_deleted: bool,
    pub flag_added: bool,
    pub flag_conflict: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalRevisionInfoV1 {
    pub revision: CanonicalRevisionRefV1,
    pub parents: Vec<CanonicalRevisionRefV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalFileIdentityV1 {
    pub path: String,
    pub revision: CanonicalRevisionRefV1,
    pub hash: String,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalSupersessionObservationV1 {
    pub revision: CanonicalRevisionRefV1,
    pub key: String,
    pub value: String,
    pub identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalDcoObservationV1 {
    pub revision: CanonicalRevisionRefV1,
    pub message: String,
    pub trailer: String,
    pub signer_name: String,
    pub signer_email: String,
    pub created_by: String,
    pub committed_by: Option<String>,
    pub resolved_authors: Vec<ResolvedAuthor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalDcoMetadataObservationV1 {
    pub revision: CanonicalRevisionRefV1,
    pub messages: Vec<String>,
    pub created_by: Vec<String>,
    pub committed_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalSupersessionMetadataQueryObservationV1 {
    pub revision: CanonicalRevisionRefV1,
    pub metadata: Vec<MetadataEntry>,
}

/// Canonical, self-reference-free evidence bytes stored before pointer attach.
///
/// Every raw evaluator fact is retained byte-exact except the one known-mutable
/// staged subject.  Exact occurrences of that subject become the typed
/// [`CanonicalRevisionRefV1::StagedSubject`]; its source/result hashes are
/// carried by [`EvidencePointerDeltaV1`].  The pointer cannot be part of its own
/// pre-attach bytes, so its sole allowed key/value delta is also carried by that
/// typed structure. No other field is normalized or omitted. Every vector is
/// sorted and compared byte-for-byte by producer and witness. The snapshot has
/// no actor-issued verdict, pass, validity, or gate decision. This v1
/// representation is a strict subset that v2 subsumes without redefining.
///
/// `evidence_preserve` is the content freeze point; `stage` is not. Under
/// Lore's path/flag staging semantics the pre-preserve window is unbounded.
/// The guarantee is "what was PRESERVED is what is evaluated", never "what
/// was STAGED is what is evaluated".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalEvidenceSnapshotV1 {
    pub version: String,
    pub target_base_revision: String,
    pub status: CanonicalStatusObservationV1,
    pub base_revision_info: CanonicalRevisionInfoV1,
    pub supersession_ancestry: Vec<CanonicalRevisionInfoV1>,
    pub supersession_ancestry_observed: bool,
    pub revision_graph: Vec<CanonicalRevisionInfoV1>,
    pub first_parent_history: Vec<CanonicalRevisionRefV1>,
    pub base_files: Vec<CanonicalFileIdentityV1>,
    pub base_tree_observed: bool,
    pub candidate_files: Vec<CanonicalFileIdentityV1>,
    pub candidate_tree_observed: bool,
    pub current_files: Vec<CanonicalFileIdentityV1>,
    /// Raw exact-terminal stream returned by upstream revision diff.
    pub upstream_revision_diff: Vec<RevisionDiffObservation>,
    /// Exact change endpoints structurally derived from stable identities.
    pub revision_diff: Vec<AffectedPath>,
    pub revision_diff_observed: bool,
    pub affected_paths: Vec<String>,
    pub supersession_markers: Vec<CanonicalSupersessionObservationV1>,
    pub supersession_metadata_queries: Vec<CanonicalSupersessionMetadataQueryObservationV1>,
    pub supersession_metadata_observed: bool,
    pub dco_metadata: Vec<CanonicalDcoMetadataObservationV1>,
    pub author_resolution: AuthorResolutionObservation,
    pub dco: Vec<CanonicalDcoObservationV1>,
    pub lock_queries: Vec<LockQuery>,
    pub lock_status: LockStatusResponse,
    pub dependency_observations: Vec<String>,
}

/// The complete allowlist of state that may change while attaching one v1
/// evidence pointer.  It is verification data, never an authorization claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePointerDeltaV1 {
    pub version: String,
    pub key: String,
    pub source_staged_revision: String,
    pub result_staged_revision: String,
    pub pointer: EvidencePointerV1,
}

/// Structural role at the governance mutation boundary.  Actor evidence is a
/// non-authoritative claim; only a witness re-evaluation can open the gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceRole {
    Actor,
    Witness,
}

/// One raw immutable-put item retained until exact cardinality validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImmutablePutItem {
    pub id: u64,
    pub address: String,
    pub ok: bool,
}

/// One raw immutable-get item retained until exact address/size/byte checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImmutableGetItem {
    pub id: u64,
    pub address: String,
    pub size: u64,
    pub data: Vec<u8>,
    pub ok: bool,
}

/// Raw classification of one operation which may mutate after dispatch.
/// `OutcomeUnknown` is a lower-bound observation: the hidden effect may or may
/// not have happened, and callers must never reconstruct it as no effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationObservation<T> {
    NotDispatched { code: String },
    Completed(T),
    OutcomeUnknown { code: String, observed: T },
}

/// Raw classification of one read. Read unavailability cannot prove or undo a
/// publication, so it is distinct from a mutation's unknown effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadObservation<T> {
    NotDispatched { code: String },
    Completed(T),
    Unavailable { code: String },
}

/// The only three policy rejections that are known before any publication
/// effect boundary is entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRejectionCodeV1 {
    ActorRoleRequired,
    InitialGovernanceClosed,
    PointerAlreadyPresent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRejectionStopV1 {
    pub reason: EvidenceRejectionCodeV1,
    pub code: String,
}

/// Exact operation stop. Whether the stop is verification-incomplete or has
/// an unknown publication effect is checked against the top-level outcome,
/// publication state, and close state during serde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePreserveStopCodeV1 {
    InitialEvaluation,
    SourceMetadata,
    SnapshotSerialization,
    StorageOpenNotDispatched,
    StorageOpenOutcome,
    ActorRoleBeforePut,
    StoragePutNotDispatched,
    StoragePutOutcome,
    StoragePutResponseMalformed,
    PreattachEvaluation,
    ActorRoleBeforeAttach,
    PointerSerialization,
    PointerAttachNotDispatched,
    PointerAttachOutcome,
    PostattachGetUnavailable,
    PostattachGetMalformed,
    PostattachBytesMismatch,
    ResultStatusUnavailable,
    ResultStatusInvalid,
    ResultMetadataUnavailable,
    PointerDeltaInvalid,
    PointerSchemaInvalid,
    PostattachEvaluation,
    PostattachDrift,
    StorageCloseNotDispatched,
    StorageCloseOutcome,
}

impl EvidencePreserveStopCodeV1 {
    fn publication_rank(self) -> u8 {
        match self {
            Self::InitialEvaluation
            | Self::SourceMetadata
            | Self::SnapshotSerialization
            | Self::StorageOpenNotDispatched
            | Self::StorageOpenOutcome
            | Self::ActorRoleBeforePut
            | Self::StoragePutNotDispatched
            | Self::StoragePutOutcome
            | Self::StoragePutResponseMalformed => 0,
            Self::PreattachEvaluation
            | Self::ActorRoleBeforeAttach
            | Self::PointerSerialization
            | Self::PointerAttachNotDispatched
            | Self::PointerAttachOutcome => 1,
            Self::PostattachGetUnavailable
            | Self::PostattachGetMalformed
            | Self::PostattachBytesMismatch => 2,
            Self::ResultStatusUnavailable | Self::ResultStatusInvalid => 3,
            Self::ResultMetadataUnavailable
            | Self::PointerDeltaInvalid
            | Self::PointerSchemaInvalid => 4,
            Self::PostattachEvaluation | Self::PostattachDrift => 5,
            Self::StorageCloseNotDispatched | Self::StorageCloseOutcome => 6,
        }
    }

    fn is_unknown_effect(self) -> bool {
        matches!(
            self,
            Self::StorageOpenOutcome
                | Self::StoragePutOutcome
                | Self::StoragePutResponseMalformed
                | Self::PointerAttachOutcome
                | Self::StorageCloseOutcome
        )
    }

    fn occurs_before_snapshot_hash(self) -> bool {
        matches!(
            self,
            Self::InitialEvaluation | Self::SourceMetadata | Self::SnapshotSerialization
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePreserveStopV1 {
    pub stage: EvidencePreserveStopCodeV1,
    pub code: String,
}

/// Close observation. `OpenOutcomeUnknown` means open was dispatched but no
/// usable handle can be proven; the other close outcomes require a usable
/// handle returned by an exact successful open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidenceCloseStateV1 {
    NotOpened,
    OpenOutcomeUnknown { code: String },
    Closed,
    CloseNotDispatched { code: String },
    CloseOutcomeUnknown { code: String },
}

/// Monotonic lower bound on publication facts observed by the actor. Internal
/// code reaches these variants only through consuming forward-only typestate
/// transitions; this public form is a strict diagnostic wire projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidencePublicationStateV1 {
    None,
    BlobPublished {
        evidence_address: String,
    },
    PointerAttachAcknowledged {
        evidence_address: String,
        pointer: EvidencePointerV1,
    },
    BlobReadbackVerified {
        evidence_address: String,
        pointer: EvidencePointerV1,
    },
    ResultSubjectObserved {
        evidence_address: String,
        pointer: EvidencePointerV1,
        result_staged_revision: String,
    },
    PointerDeltaObserved {
        evidence_address: String,
        pointer: EvidencePointerV1,
        result_staged_revision: String,
    },
    PostattachEquivalent {
        evidence_address: String,
        pointer: EvidencePointerV1,
        result_staged_revision: String,
    },
}

struct PublicationFacts<'a> {
    rank: u8,
    address: Option<&'a str>,
    pointer: Option<&'a EvidencePointerV1>,
    result_revision: Option<&'a str>,
}

impl EvidencePublicationStateV1 {
    fn validate(&self, source_revision: &str) -> std::result::Result<PublicationFacts<'_>, String> {
        let facts = match self {
            Self::None => PublicationFacts {
                rank: 0,
                address: None,
                pointer: None,
                result_revision: None,
            },
            Self::BlobPublished { evidence_address } => PublicationFacts {
                rank: 1,
                address: Some(evidence_address),
                pointer: None,
                result_revision: None,
            },
            Self::PointerAttachAcknowledged {
                evidence_address,
                pointer,
            } => PublicationFacts {
                rank: 2,
                address: Some(evidence_address),
                pointer: Some(pointer),
                result_revision: None,
            },
            Self::BlobReadbackVerified {
                evidence_address,
                pointer,
            } => PublicationFacts {
                rank: 3,
                address: Some(evidence_address),
                pointer: Some(pointer),
                result_revision: None,
            },
            Self::ResultSubjectObserved {
                evidence_address,
                pointer,
                result_staged_revision,
            } => PublicationFacts {
                rank: 4,
                address: Some(evidence_address),
                pointer: Some(pointer),
                result_revision: Some(result_staged_revision),
            },
            Self::PointerDeltaObserved {
                evidence_address,
                pointer,
                result_staged_revision,
            } => PublicationFacts {
                rank: 5,
                address: Some(evidence_address),
                pointer: Some(pointer),
                result_revision: Some(result_staged_revision),
            },
            Self::PostattachEquivalent {
                evidence_address,
                pointer,
                result_staged_revision,
            } => PublicationFacts {
                rank: 6,
                address: Some(evidence_address),
                pointer: Some(pointer),
                result_revision: Some(result_staged_revision),
            },
        };
        if let Some(address) = facts.address {
            if !canonical_evidence_address(address) {
                return Err("publication address is not canonical".into());
            }
        }
        if let Some(pointer) = facts.pointer {
            if pointer.version != "v1" || Some(pointer.address.as_str()) != facts.address {
                return Err("publication pointer does not bind the exact address".into());
            }
        }
        if let Some(result) = facts.result_revision {
            if result.is_empty() || result == source_revision {
                return Err("result staged revision did not advance".into());
            }
        }
        Ok(facts)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePreserveRejectedV1 {
    pub version: String,
    pub attempt_id: String,
    pub source_staged_revision: String,
    pub target_base_revision: String,
    pub snapshot_sha256: Option<String>,
    pub observed_candidate_addresses: Vec<String>,
    pub stopped_at: EvidenceRejectionStopV1,
    pub close: EvidenceCloseStateV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePreserveResidualV1 {
    pub version: String,
    pub attempt_id: String,
    pub source_staged_revision: String,
    pub target_base_revision: String,
    pub snapshot_sha256: Option<String>,
    pub observed_candidate_addresses: Vec<String>,
    pub stopped_at: EvidencePreserveStopV1,
    pub last_confirmed_publication: EvidencePublicationStateV1,
    pub close: EvidenceCloseStateV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePreserveVerifiedV1 {
    pub version: String,
    pub attempt_id: String,
    pub source_staged_revision: String,
    pub target_base_revision: String,
    pub snapshot_sha256: Option<String>,
    pub observed_candidate_addresses: Vec<String>,
    pub result_staged_revision: String,
    pub evidence_address: String,
    pub pointer: EvidencePointerV1,
    pub last_confirmed_publication: EvidencePublicationStateV1,
    pub close: EvidenceCloseStateV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidencePreserveDispositionV1 {
    RejectedBeforePublication(EvidencePreserveRejectedV1),
    Unknown(EvidencePreserveResidualV1),
    VerificationIncomplete(EvidencePreserveResidualV1),
    Verified(EvidencePreserveVerifiedV1),
}

/// Strict checked actor residual state. Actor output never contains a policy
/// verdict and remains non-authoritative until an independent Witness replays
/// every fact. The private wrapper prevents unchecked construction; both serde
/// directions validate the full outcome/publication/close cross-product.
/// Internal typestate and pending reason types are not part of the public API:
///
/// ```compile_fail
/// use lore_vm::ops::governance::contract::{
///     EvidencePublicationAttemptV1, PendingEvidencePreserveOutcomeV1,
///     VerificationIncompleteReasonV1,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidencePreserveOutcomeV1(EvidencePreserveDispositionV1);

pub(crate) type EvidenceOutcomeBuildResultV1 =
    std::result::Result<EvidencePreserveOutcomeV1, String>;

impl EvidencePreserveOutcomeV1 {
    pub fn disposition(&self) -> &EvidencePreserveDispositionV1 {
        &self.0
    }

    pub fn verified(&self) -> Option<&EvidencePreserveVerifiedV1> {
        match &self.0 {
            EvidencePreserveDispositionV1::Verified(verified) => Some(verified),
            EvidencePreserveDispositionV1::RejectedBeforePublication(_)
            | EvidencePreserveDispositionV1::Unknown(_)
            | EvidencePreserveDispositionV1::VerificationIncomplete(_) => None,
        }
    }

    pub fn rejected(&self) -> Option<&EvidencePreserveRejectedV1> {
        match &self.0 {
            EvidencePreserveDispositionV1::RejectedBeforePublication(rejected) => Some(rejected),
            EvidencePreserveDispositionV1::Unknown(_)
            | EvidencePreserveDispositionV1::VerificationIncomplete(_)
            | EvidencePreserveDispositionV1::Verified(_) => None,
        }
    }

    pub fn residual(&self) -> Option<&EvidencePreserveResidualV1> {
        match &self.0 {
            EvidencePreserveDispositionV1::Unknown(residual)
            | EvidencePreserveDispositionV1::VerificationIncomplete(residual) => Some(residual),
            EvidencePreserveDispositionV1::RejectedBeforePublication(_)
            | EvidencePreserveDispositionV1::Verified(_) => None,
        }
    }

    fn checked(disposition: EvidencePreserveDispositionV1) -> std::result::Result<Self, String> {
        validate_evidence_preserve_disposition(&disposition)?;
        Ok(Self(disposition))
    }
}

impl Serialize for EvidencePreserveOutcomeV1 {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        validate_evidence_preserve_disposition(&self.0).map_err(serde::ser::Error::custom)?;
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EvidencePreserveOutcomeV1 {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let disposition = EvidencePreserveDispositionV1::deserialize(deserializer)?;
        Self::checked(disposition).map_err(serde::de::Error::custom)
    }
}

fn canonical_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn canonical_evidence_address(value: &str) -> bool {
    value.len() == 97
        && value.as_bytes()[64] == b'-'
        && canonical_lower_hex(&value[..64], 64)
        && canonical_lower_hex(&value[65..], 32)
}

fn validate_close(close: &EvidenceCloseStateV1) -> std::result::Result<(), String> {
    match close {
        EvidenceCloseStateV1::OpenOutcomeUnknown { code }
        | EvidenceCloseStateV1::CloseNotDispatched { code }
        | EvidenceCloseStateV1::CloseOutcomeUnknown { code }
            if code.is_empty() =>
        {
            Err("close diagnostic code is empty".into())
        }
        EvidenceCloseStateV1::NotOpened
        | EvidenceCloseStateV1::OpenOutcomeUnknown { .. }
        | EvidenceCloseStateV1::Closed
        | EvidenceCloseStateV1::CloseNotDispatched { .. }
        | EvidenceCloseStateV1::CloseOutcomeUnknown { .. } => Ok(()),
    }
}

fn validate_common(
    version: &str,
    attempt_id: &str,
    source: &str,
    base: &str,
    snapshot_sha256: Option<&str>,
) -> std::result::Result<(), String> {
    if version != "v1"
        || !canonical_lower_hex(attempt_id, 64)
        || source.is_empty()
        || base.is_empty()
        || snapshot_sha256.is_some_and(|hash| !canonical_lower_hex(hash, 64))
    {
        return Err("invalid evidence outcome common fields".into());
    }
    Ok(())
}

fn validate_candidates(
    candidates: &[String],
    facts: &PublicationFacts<'_>,
) -> std::result::Result<(), String> {
    if facts.rank > 0 {
        let Some(address) = facts.address else {
            return Err("published state lacks an evidence address".into());
        };
        if candidates != [address] {
            return Err("published address does not match the sole put candidate".into());
        }
    }
    Ok(())
}

fn validate_evidence_preserve_disposition(
    disposition: &EvidencePreserveDispositionV1,
) -> std::result::Result<(), String> {
    match disposition {
        EvidencePreserveDispositionV1::RejectedBeforePublication(outcome) => {
            validate_common(
                &outcome.version,
                &outcome.attempt_id,
                &outcome.source_staged_revision,
                &outcome.target_base_revision,
                outcome.snapshot_sha256.as_deref(),
            )?;
            validate_close(&outcome.close)?;
            if outcome.snapshot_sha256.is_some()
                || !outcome.observed_candidate_addresses.is_empty()
                || !matches!(outcome.close, EvidenceCloseStateV1::NotOpened)
                || outcome.stopped_at.code.is_empty()
            {
                return Err("rejection claimed state beyond pre-dispatch facts".into());
            }
        }
        EvidencePreserveDispositionV1::VerificationIncomplete(outcome) => {
            validate_residual(outcome, false)?;
        }
        EvidencePreserveDispositionV1::Unknown(outcome) => {
            validate_residual(outcome, true)?;
        }
        EvidencePreserveDispositionV1::Verified(outcome) => {
            validate_common(
                &outcome.version,
                &outcome.attempt_id,
                &outcome.source_staged_revision,
                &outcome.target_base_revision,
                outcome.snapshot_sha256.as_deref(),
            )?;
            validate_close(&outcome.close)?;
            let facts = outcome
                .last_confirmed_publication
                .validate(&outcome.source_staged_revision)?;
            validate_candidates(&outcome.observed_candidate_addresses, &facts)?;
            if outcome.snapshot_sha256.is_none()
                || facts.rank != 6
                || facts.address != Some(outcome.evidence_address.as_str())
                || facts.pointer != Some(&outcome.pointer)
                || facts.result_revision != Some(outcome.result_staged_revision.as_str())
                || !matches!(outcome.close, EvidenceCloseStateV1::Closed)
                || outcome.pointer.version != "v1"
                || outcome.pointer.address != outcome.evidence_address
            {
                return Err("verified outcome did not prove the full closed chain".into());
            }
        }
    }
    Ok(())
}

fn validate_residual(
    outcome: &EvidencePreserveResidualV1,
    top_level_unknown: bool,
) -> std::result::Result<(), String> {
    validate_common(
        &outcome.version,
        &outcome.attempt_id,
        &outcome.source_staged_revision,
        &outcome.target_base_revision,
        outcome.snapshot_sha256.as_deref(),
    )?;
    validate_close(&outcome.close)?;
    if outcome.stopped_at.code.is_empty() {
        return Err("stop diagnostic code is empty".into());
    }
    let facts = outcome
        .last_confirmed_publication
        .validate(&outcome.source_staged_revision)?;
    validate_candidates(&outcome.observed_candidate_addresses, &facts)?;
    if facts.rank != outcome.stopped_at.stage.publication_rank() {
        return Err("stop and publication state disagree".into());
    }

    if top_level_unknown {
        if outcome.snapshot_sha256.is_none() {
            return Err("post-dispatch unknown lacks snapshot hash".into());
        }
        if outcome.stopped_at.stage.is_unknown_effect() {
            match outcome.stopped_at.stage {
                EvidencePreserveStopCodeV1::StorageOpenOutcome => {
                    if !matches!(
                        outcome.close,
                        EvidenceCloseStateV1::OpenOutcomeUnknown { .. }
                    ) {
                        return Err("open-outcome unknown has an impossible close state".into());
                    }
                }
                EvidencePreserveStopCodeV1::StorageCloseOutcome => {
                    if !matches!(
                        outcome.close,
                        EvidenceCloseStateV1::CloseOutcomeUnknown { .. }
                    ) {
                        return Err("close-outcome unknown lacks close uncertainty".into());
                    }
                }
                EvidencePreserveStopCodeV1::StoragePutOutcome
                | EvidencePreserveStopCodeV1::StoragePutResponseMalformed
                | EvidencePreserveStopCodeV1::PointerAttachOutcome => {
                    if matches!(
                        outcome.close,
                        EvidenceCloseStateV1::NotOpened
                            | EvidenceCloseStateV1::OpenOutcomeUnknown { .. }
                    ) {
                        return Err("usable-handle unknown has an impossible close state".into());
                    }
                }
                EvidencePreserveStopCodeV1::InitialEvaluation
                | EvidencePreserveStopCodeV1::SourceMetadata
                | EvidencePreserveStopCodeV1::SnapshotSerialization
                | EvidencePreserveStopCodeV1::StorageOpenNotDispatched
                | EvidencePreserveStopCodeV1::ActorRoleBeforePut
                | EvidencePreserveStopCodeV1::StoragePutNotDispatched
                | EvidencePreserveStopCodeV1::PreattachEvaluation
                | EvidencePreserveStopCodeV1::ActorRoleBeforeAttach
                | EvidencePreserveStopCodeV1::PointerSerialization
                | EvidencePreserveStopCodeV1::PointerAttachNotDispatched
                | EvidencePreserveStopCodeV1::PostattachGetUnavailable
                | EvidencePreserveStopCodeV1::PostattachGetMalformed
                | EvidencePreserveStopCodeV1::PostattachBytesMismatch
                | EvidencePreserveStopCodeV1::ResultStatusUnavailable
                | EvidencePreserveStopCodeV1::ResultStatusInvalid
                | EvidencePreserveStopCodeV1::ResultMetadataUnavailable
                | EvidencePreserveStopCodeV1::PointerDeltaInvalid
                | EvidencePreserveStopCodeV1::PointerSchemaInvalid
                | EvidencePreserveStopCodeV1::PostattachEvaluation
                | EvidencePreserveStopCodeV1::PostattachDrift
                | EvidencePreserveStopCodeV1::StorageCloseNotDispatched => {
                    return Err("unknown outcome used a deterministic stop".into())
                }
            }
        } else if !matches!(
            outcome.close,
            EvidenceCloseStateV1::CloseOutcomeUnknown { .. }
        ) || matches!(
            outcome.stopped_at.stage,
            EvidencePreserveStopCodeV1::InitialEvaluation
                | EvidencePreserveStopCodeV1::SourceMetadata
                | EvidencePreserveStopCodeV1::SnapshotSerialization
                | EvidencePreserveStopCodeV1::StorageOpenNotDispatched
        ) {
            return Err(
                "verification stop can become unknown only through close uncertainty".into(),
            );
        }
    } else {
        if outcome.stopped_at.stage.is_unknown_effect()
            || outcome.snapshot_sha256.is_some()
                == outcome.stopped_at.stage.occurs_before_snapshot_hash()
        {
            return Err("verification-incomplete stop has inconsistent diagnostics".into());
        }
        let before_open = matches!(
            outcome.stopped_at.stage,
            EvidencePreserveStopCodeV1::InitialEvaluation
                | EvidencePreserveStopCodeV1::SourceMetadata
                | EvidencePreserveStopCodeV1::SnapshotSerialization
                | EvidencePreserveStopCodeV1::StorageOpenNotDispatched
        );
        if before_open != matches!(outcome.close, EvidenceCloseStateV1::NotOpened)
            || matches!(
                outcome.close,
                EvidenceCloseStateV1::OpenOutcomeUnknown { .. }
                    | EvidenceCloseStateV1::CloseOutcomeUnknown { .. }
            )
            || (outcome.stopped_at.stage == EvidencePreserveStopCodeV1::StorageCloseNotDispatched
                && !matches!(
                    outcome.close,
                    EvidenceCloseStateV1::CloseNotDispatched { .. }
                ))
        {
            return Err("verification-incomplete close state is impossible".into());
        }
    }

    if facts.rank == 0
        && !matches!(
            outcome.stopped_at.stage,
            EvidencePreserveStopCodeV1::StoragePutOutcome
                | EvidencePreserveStopCodeV1::StoragePutResponseMalformed
        )
        && !outcome.observed_candidate_addresses.is_empty()
    {
        return Err("candidate addresses appeared before a put observation".into());
    }
    Ok(())
}

#[derive(Debug)]
struct EvidenceAttemptCommonV1 {
    attempt_id: String,
    source_staged_revision: String,
    target_base_revision: String,
    snapshot_sha256: Option<String>,
    observed_candidate_addresses: Vec<String>,
}

impl EvidenceAttemptCommonV1 {
    fn residual(
        self,
        stopped_at: EvidencePreserveStopV1,
        last_confirmed_publication: EvidencePublicationStateV1,
        close: EvidenceCloseStateV1,
    ) -> EvidencePreserveResidualV1 {
        EvidencePreserveResidualV1 {
            version: "v1".into(),
            attempt_id: self.attempt_id,
            source_staged_revision: self.source_staged_revision,
            target_base_revision: self.target_base_revision,
            snapshot_sha256: self.snapshot_sha256,
            observed_candidate_addresses: self.observed_candidate_addresses,
            stopped_at,
            last_confirmed_publication,
            close,
        }
    }
}

/// Sole authority which can construct `RejectedBeforePublication`. Consuming
/// `enter_effect_boundary` destroys this authority, so no post-dispatch path
/// has a rejection constructor to select accidentally.
pub(crate) struct PredispatchEvidenceAttemptV1 {
    common: EvidenceAttemptCommonV1,
}

enum PredispatchIncompleteReasonV1 {
    InitialEvaluation,
    SourceMetadata,
    SnapshotSerialization,
}

impl PredispatchIncompleteReasonV1 {
    fn stop(&self) -> EvidencePreserveStopCodeV1 {
        match self {
            Self::InitialEvaluation => EvidencePreserveStopCodeV1::InitialEvaluation,
            Self::SourceMetadata => EvidencePreserveStopCodeV1::SourceMetadata,
            Self::SnapshotSerialization => EvidencePreserveStopCodeV1::SnapshotSerialization,
        }
    }

    fn fallback_code(&self) -> &'static str {
        match self {
            Self::InitialEvaluation => "initial_evaluation_without_code",
            Self::SourceMetadata => "source_metadata_without_code",
            Self::SnapshotSerialization => "snapshot_serialization_without_code",
        }
    }
}

impl PredispatchEvidenceAttemptV1 {
    pub(crate) fn new(attempt_id: String, request: ValidatedEvidencePreserveRequestV1<'_>) -> Self {
        Self {
            common: EvidenceAttemptCommonV1 {
                attempt_id,
                source_staged_revision: request.request.expected_staged_revision.clone(),
                target_base_revision: request.request.target_base_revision.clone(),
                snapshot_sha256: None,
                observed_candidate_addresses: Vec::new(),
            },
        }
    }

    fn reject(
        self,
        reason: EvidenceRejectionCodeV1,
        code: impl Into<String>,
    ) -> EvidenceOutcomeBuildResultV1 {
        let fallback = match reason {
            EvidenceRejectionCodeV1::ActorRoleRequired => "actor_role_required_without_code",
            EvidenceRejectionCodeV1::InitialGovernanceClosed => {
                "initial_governance_closed_without_code"
            }
            EvidenceRejectionCodeV1::PointerAlreadyPresent => {
                "pointer_already_present_without_code"
            }
        };
        checked_outcome(EvidencePreserveDispositionV1::RejectedBeforePublication(
            EvidencePreserveRejectedV1 {
                version: "v1".into(),
                attempt_id: self.common.attempt_id,
                source_staged_revision: self.common.source_staged_revision,
                target_base_revision: self.common.target_base_revision,
                snapshot_sha256: None,
                observed_candidate_addresses: Vec::new(),
                stopped_at: EvidenceRejectionStopV1 {
                    reason,
                    code: nonempty_diagnostic(code, fallback),
                },
                close: EvidenceCloseStateV1::NotOpened,
            },
        ))
    }

    fn verification_incomplete(
        self,
        reason: PredispatchIncompleteReasonV1,
        code: impl Into<String>,
    ) -> EvidenceOutcomeBuildResultV1 {
        let code = nonempty_diagnostic(code, reason.fallback_code());
        let residual = self.common.residual(
            EvidencePreserveStopV1 {
                stage: reason.stop(),
                code,
            },
            EvidencePublicationStateV1::None,
            EvidenceCloseStateV1::NotOpened,
        );
        checked_outcome(EvidencePreserveDispositionV1::VerificationIncomplete(
            residual,
        ))
    }

    pub(crate) fn actor_role_rejected(
        self,
        code: impl Into<String>,
    ) -> EvidenceOutcomeBuildResultV1 {
        self.reject(EvidenceRejectionCodeV1::ActorRoleRequired, code)
    }

    pub(crate) fn initial_governance_rejected(
        self,
        code: impl Into<String>,
    ) -> EvidenceOutcomeBuildResultV1 {
        self.reject(EvidenceRejectionCodeV1::InitialGovernanceClosed, code)
    }

    pub(crate) fn pointer_already_present_rejected(
        self,
        code: impl Into<String>,
    ) -> EvidenceOutcomeBuildResultV1 {
        self.reject(EvidenceRejectionCodeV1::PointerAlreadyPresent, code)
    }

    pub(crate) fn initial_evaluation_incomplete(
        self,
        code: impl Into<String>,
    ) -> EvidenceOutcomeBuildResultV1 {
        self.verification_incomplete(PredispatchIncompleteReasonV1::InitialEvaluation, code)
    }

    pub(crate) fn source_metadata_incomplete(
        self,
        code: impl Into<String>,
    ) -> EvidenceOutcomeBuildResultV1 {
        self.verification_incomplete(PredispatchIncompleteReasonV1::SourceMetadata, code)
    }

    pub(crate) fn snapshot_serialization_incomplete(
        self,
        code: impl Into<String>,
    ) -> EvidenceOutcomeBuildResultV1 {
        self.verification_incomplete(PredispatchIncompleteReasonV1::SnapshotSerialization, code)
    }

    pub(crate) fn with_snapshot_sha256(mut self, snapshot_sha256: String) -> Self {
        self.common.snapshot_sha256 = Some(snapshot_sha256);
        self
    }

    pub(crate) fn enter_effect_boundary(self) -> EvidencePublicationAttemptV1<NoPublicationV1> {
        EvidencePublicationAttemptV1 {
            common: self.common,
            stage: NoPublicationV1,
        }
    }
}

pub(crate) trait PublicationStageV1 {
    fn wire_state(&self) -> EvidencePublicationStateV1;
}

pub(crate) struct NoPublicationV1;

impl PublicationStageV1 for NoPublicationV1 {
    fn wire_state(&self) -> EvidencePublicationStateV1 {
        EvidencePublicationStateV1::None
    }
}

pub(crate) struct PutObservedV1;

impl PublicationStageV1 for PutObservedV1 {
    fn wire_state(&self) -> EvidencePublicationStateV1 {
        EvidencePublicationStateV1::None
    }
}

pub(crate) struct BlobPublishedV1 {
    evidence_address: String,
}

impl PublicationStageV1 for BlobPublishedV1 {
    fn wire_state(&self) -> EvidencePublicationStateV1 {
        EvidencePublicationStateV1::BlobPublished {
            evidence_address: self.evidence_address.clone(),
        }
    }
}

pub(crate) struct PointerAttachAcknowledgedV1 {
    evidence_address: String,
    pointer: EvidencePointerV1,
}

impl PublicationStageV1 for PointerAttachAcknowledgedV1 {
    fn wire_state(&self) -> EvidencePublicationStateV1 {
        EvidencePublicationStateV1::PointerAttachAcknowledged {
            evidence_address: self.evidence_address.clone(),
            pointer: self.pointer.clone(),
        }
    }
}

pub(crate) struct BlobReadbackVerifiedV1 {
    evidence_address: String,
    pointer: EvidencePointerV1,
}

impl PublicationStageV1 for BlobReadbackVerifiedV1 {
    fn wire_state(&self) -> EvidencePublicationStateV1 {
        EvidencePublicationStateV1::BlobReadbackVerified {
            evidence_address: self.evidence_address.clone(),
            pointer: self.pointer.clone(),
        }
    }
}

pub(crate) struct ResultSubjectObservedV1 {
    evidence_address: String,
    pointer: EvidencePointerV1,
    result_staged_revision: String,
}

impl PublicationStageV1 for ResultSubjectObservedV1 {
    fn wire_state(&self) -> EvidencePublicationStateV1 {
        EvidencePublicationStateV1::ResultSubjectObserved {
            evidence_address: self.evidence_address.clone(),
            pointer: self.pointer.clone(),
            result_staged_revision: self.result_staged_revision.clone(),
        }
    }
}

pub(crate) struct PointerDeltaObservedV1 {
    evidence_address: String,
    pointer: EvidencePointerV1,
    result_staged_revision: String,
}

impl PublicationStageV1 for PointerDeltaObservedV1 {
    fn wire_state(&self) -> EvidencePublicationStateV1 {
        EvidencePublicationStateV1::PointerDeltaObserved {
            evidence_address: self.evidence_address.clone(),
            pointer: self.pointer.clone(),
            result_staged_revision: self.result_staged_revision.clone(),
        }
    }
}

pub(crate) struct PostattachEquivalentV1 {
    evidence_address: String,
    pointer: EvidencePointerV1,
    result_staged_revision: String,
}

impl PublicationStageV1 for PostattachEquivalentV1 {
    fn wire_state(&self) -> EvidencePublicationStateV1 {
        EvidencePublicationStateV1::PostattachEquivalent {
            evidence_address: self.evidence_address.clone(),
            pointer: self.pointer.clone(),
            result_staged_revision: self.result_staged_revision.clone(),
        }
    }
}

/// Forward-only publication state. There is intentionally no generic state
/// constructor and no backward/skip transition; adding a future stage requires
/// an explicit sealed mapping and transition or compilation fails.
pub(crate) struct EvidencePublicationAttemptV1<S: PublicationStageV1> {
    common: EvidenceAttemptCommonV1,
    stage: S,
}

impl EvidencePublicationAttemptV1<NoPublicationV1> {
    pub(crate) fn storage_open_not_dispatched(
        self,
        code: impl Into<String>,
    ) -> EvidenceOutcomeBuildResultV1 {
        let code = nonempty_diagnostic(code, "storage_open_not_dispatched_without_code");
        let residual = self.common.residual(
            EvidencePreserveStopV1 {
                stage: EvidencePreserveStopCodeV1::StorageOpenNotDispatched,
                code,
            },
            EvidencePublicationStateV1::None,
            EvidenceCloseStateV1::NotOpened,
        );
        checked_outcome(EvidencePreserveDispositionV1::VerificationIncomplete(
            residual,
        ))
    }

    pub(crate) fn storage_open_outcome_unknown(
        self,
        code: impl Into<String>,
    ) -> EvidenceOutcomeBuildResultV1 {
        let code = nonempty_diagnostic(code, "storage_open_outcome_unknown_without_code");
        let residual = self.common.residual(
            EvidencePreserveStopV1 {
                stage: EvidencePreserveStopCodeV1::StorageOpenOutcome,
                code: code.clone(),
            },
            EvidencePublicationStateV1::None,
            EvidenceCloseStateV1::OpenOutcomeUnknown { code },
        );
        checked_outcome(EvidencePreserveDispositionV1::Unknown(residual))
    }

    pub(crate) fn put_observed(
        mut self,
        observed_candidate_addresses: Vec<String>,
    ) -> EvidencePublicationAttemptV1<PutObservedV1> {
        self.common.observed_candidate_addresses = observed_candidate_addresses;
        EvidencePublicationAttemptV1 {
            common: self.common,
            stage: PutObservedV1,
        }
    }

    pub(crate) fn actor_role_before_put(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::no_publication_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                NoPublicationIncompleteReasonV1::ActorRoleBeforePut,
                code,
            ),
        )
    }

    pub(crate) fn storage_put_not_dispatched(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::no_publication_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                NoPublicationIncompleteReasonV1::StoragePutNotDispatched,
                code,
            ),
        )
    }
}

impl EvidencePublicationAttemptV1<PutObservedV1> {
    pub(crate) fn blob_published(
        self,
        evidence_address: String,
    ) -> EvidencePublicationAttemptV1<BlobPublishedV1> {
        EvidencePublicationAttemptV1 {
            common: self.common,
            stage: BlobPublishedV1 { evidence_address },
        }
    }

    pub(crate) fn storage_put_outcome_unknown(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::put_observed_unknown(PendingUnknownV1::new(
            self,
            PutObservedUnknownReasonV1::StoragePutOutcome,
            code,
        ))
    }

    pub(crate) fn storage_put_response_malformed(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::put_observed_unknown(PendingUnknownV1::new(
            self,
            PutObservedUnknownReasonV1::StoragePutResponseMalformed,
            code,
        ))
    }
}

impl EvidencePublicationAttemptV1<BlobPublishedV1> {
    pub(crate) fn pointer_attach_acknowledged(
        self,
        pointer: EvidencePointerV1,
    ) -> EvidencePublicationAttemptV1<PointerAttachAcknowledgedV1> {
        EvidencePublicationAttemptV1 {
            common: self.common,
            stage: PointerAttachAcknowledgedV1 {
                evidence_address: self.stage.evidence_address,
                pointer,
            },
        }
    }

    pub(crate) fn preattach_evaluation_incomplete(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::blob_published_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                BlobPublishedIncompleteReasonV1::PreattachEvaluation,
                code,
            ),
        )
    }

    pub(crate) fn actor_role_before_attach(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::blob_published_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                BlobPublishedIncompleteReasonV1::ActorRoleBeforeAttach,
                code,
            ),
        )
    }

    pub(crate) fn pointer_serialization_incomplete(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::blob_published_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                BlobPublishedIncompleteReasonV1::PointerSerialization,
                code,
            ),
        )
    }

    pub(crate) fn pointer_attach_not_dispatched(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::blob_published_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                BlobPublishedIncompleteReasonV1::PointerAttachNotDispatched,
                code,
            ),
        )
    }

    pub(crate) fn pointer_attach_outcome_unknown(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::blob_published_unknown(PendingUnknownV1::new(
            self,
            BlobPublishedUnknownReasonV1,
            code,
        ))
    }
}

impl EvidencePublicationAttemptV1<PointerAttachAcknowledgedV1> {
    pub(crate) fn blob_readback_verified(
        self,
    ) -> EvidencePublicationAttemptV1<BlobReadbackVerifiedV1> {
        EvidencePublicationAttemptV1 {
            common: self.common,
            stage: BlobReadbackVerifiedV1 {
                evidence_address: self.stage.evidence_address,
                pointer: self.stage.pointer,
            },
        }
    }

    pub(crate) fn postattach_get_unavailable(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::pointer_attach_acknowledged_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                PointerAttachAcknowledgedIncompleteReasonV1::GetUnavailable,
                code,
            ),
        )
    }

    pub(crate) fn postattach_get_malformed(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::pointer_attach_acknowledged_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                PointerAttachAcknowledgedIncompleteReasonV1::GetMalformed,
                code,
            ),
        )
    }

    pub(crate) fn postattach_bytes_mismatch(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::pointer_attach_acknowledged_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                PointerAttachAcknowledgedIncompleteReasonV1::BytesMismatch,
                code,
            ),
        )
    }
}

impl EvidencePublicationAttemptV1<BlobReadbackVerifiedV1> {
    pub(crate) fn result_subject_observed(
        self,
        result_staged_revision: String,
    ) -> EvidencePublicationAttemptV1<ResultSubjectObservedV1> {
        EvidencePublicationAttemptV1 {
            common: self.common,
            stage: ResultSubjectObservedV1 {
                evidence_address: self.stage.evidence_address,
                pointer: self.stage.pointer,
                result_staged_revision,
            },
        }
    }

    pub(crate) fn result_status_unavailable(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::blob_readback_verified_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                BlobReadbackVerifiedIncompleteReasonV1::ResultStatusUnavailable,
                code,
            ),
        )
    }

    pub(crate) fn result_status_invalid(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::blob_readback_verified_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                BlobReadbackVerifiedIncompleteReasonV1::ResultStatusInvalid,
                code,
            ),
        )
    }
}

impl EvidencePublicationAttemptV1<ResultSubjectObservedV1> {
    pub(crate) fn pointer_delta_observed(
        self,
    ) -> EvidencePublicationAttemptV1<PointerDeltaObservedV1> {
        EvidencePublicationAttemptV1 {
            common: self.common,
            stage: PointerDeltaObservedV1 {
                evidence_address: self.stage.evidence_address,
                pointer: self.stage.pointer,
                result_staged_revision: self.stage.result_staged_revision,
            },
        }
    }

    pub(crate) fn result_metadata_unavailable(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::result_subject_observed_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                ResultSubjectObservedIncompleteReasonV1::ResultMetadataUnavailable,
                code,
            ),
        )
    }

    pub(crate) fn pointer_delta_invalid(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::result_subject_observed_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                ResultSubjectObservedIncompleteReasonV1::PointerDeltaInvalid,
                code,
            ),
        )
    }

    pub(crate) fn pointer_schema_invalid(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::result_subject_observed_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                ResultSubjectObservedIncompleteReasonV1::PointerSchemaInvalid,
                code,
            ),
        )
    }
}

impl EvidencePublicationAttemptV1<PointerDeltaObservedV1> {
    pub(crate) fn postattach_equivalent(
        self,
    ) -> EvidencePublicationAttemptV1<PostattachEquivalentV1> {
        EvidencePublicationAttemptV1 {
            common: self.common,
            stage: PostattachEquivalentV1 {
                evidence_address: self.stage.evidence_address,
                pointer: self.stage.pointer,
                result_staged_revision: self.stage.result_staged_revision,
            },
        }
    }

    pub(crate) fn postattach_evaluation_incomplete(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::pointer_delta_observed_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                PointerDeltaObservedIncompleteReasonV1::PostattachEvaluation,
                code,
            ),
        )
    }

    pub(crate) fn postattach_drift(
        self,
        code: impl Into<String>,
    ) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::pointer_delta_observed_incomplete(
            PendingVerificationIncompleteV1::new(
                self,
                PointerDeltaObservedIncompleteReasonV1::PostattachDrift,
                code,
            ),
        )
    }
}

impl EvidencePublicationAttemptV1<PostattachEquivalentV1> {
    pub(crate) fn ready_to_close(self) -> PendingEvidencePreserveOutcomeV1 {
        PendingEvidencePreserveOutcomeV1::ready(ReadyToCloseV1 { attempt: self })
    }
}

pub(crate) enum EvidenceCloseEffectV1 {
    Closed,
    NotDispatched { code: String },
    OutcomeUnknown { code: String },
}

fn nonempty_diagnostic(code: impl Into<String>, fallback: &'static str) -> String {
    let code = code.into();
    if code.trim().is_empty() {
        fallback.into()
    } else {
        code
    }
}

trait VerificationIncompleteReasonV1<S: PublicationStageV1> {
    fn stop(&self) -> EvidencePreserveStopCodeV1;
    fn fallback_code(&self) -> &'static str;
}

trait UnknownReasonV1<S: PublicationStageV1> {
    fn stop(&self) -> EvidencePreserveStopCodeV1;
    fn fallback_code(&self) -> &'static str;
}

enum NoPublicationIncompleteReasonV1 {
    ActorRoleBeforePut,
    StoragePutNotDispatched,
}

impl VerificationIncompleteReasonV1<NoPublicationV1> for NoPublicationIncompleteReasonV1 {
    fn stop(&self) -> EvidencePreserveStopCodeV1 {
        match self {
            Self::ActorRoleBeforePut => EvidencePreserveStopCodeV1::ActorRoleBeforePut,
            Self::StoragePutNotDispatched => EvidencePreserveStopCodeV1::StoragePutNotDispatched,
        }
    }

    fn fallback_code(&self) -> &'static str {
        match self {
            Self::ActorRoleBeforePut => "actor_role_before_put_without_code",
            Self::StoragePutNotDispatched => "storage_put_not_dispatched_without_code",
        }
    }
}

enum PutObservedUnknownReasonV1 {
    StoragePutOutcome,
    StoragePutResponseMalformed,
}

impl UnknownReasonV1<PutObservedV1> for PutObservedUnknownReasonV1 {
    fn stop(&self) -> EvidencePreserveStopCodeV1 {
        match self {
            Self::StoragePutOutcome => EvidencePreserveStopCodeV1::StoragePutOutcome,
            Self::StoragePutResponseMalformed => {
                EvidencePreserveStopCodeV1::StoragePutResponseMalformed
            }
        }
    }

    fn fallback_code(&self) -> &'static str {
        match self {
            Self::StoragePutOutcome => "storage_put_outcome_unknown_without_code",
            Self::StoragePutResponseMalformed => "storage_put_response_malformed_without_code",
        }
    }
}

enum BlobPublishedIncompleteReasonV1 {
    PreattachEvaluation,
    ActorRoleBeforeAttach,
    PointerSerialization,
    PointerAttachNotDispatched,
}

impl VerificationIncompleteReasonV1<BlobPublishedV1> for BlobPublishedIncompleteReasonV1 {
    fn stop(&self) -> EvidencePreserveStopCodeV1 {
        match self {
            Self::PreattachEvaluation => EvidencePreserveStopCodeV1::PreattachEvaluation,
            Self::ActorRoleBeforeAttach => EvidencePreserveStopCodeV1::ActorRoleBeforeAttach,
            Self::PointerSerialization => EvidencePreserveStopCodeV1::PointerSerialization,
            Self::PointerAttachNotDispatched => {
                EvidencePreserveStopCodeV1::PointerAttachNotDispatched
            }
        }
    }

    fn fallback_code(&self) -> &'static str {
        match self {
            Self::PreattachEvaluation => "preattach_evaluation_without_code",
            Self::ActorRoleBeforeAttach => "actor_role_before_attach_without_code",
            Self::PointerSerialization => "pointer_serialization_without_code",
            Self::PointerAttachNotDispatched => "pointer_attach_not_dispatched_without_code",
        }
    }
}

struct BlobPublishedUnknownReasonV1;

impl UnknownReasonV1<BlobPublishedV1> for BlobPublishedUnknownReasonV1 {
    fn stop(&self) -> EvidencePreserveStopCodeV1 {
        EvidencePreserveStopCodeV1::PointerAttachOutcome
    }

    fn fallback_code(&self) -> &'static str {
        "pointer_attach_outcome_unknown_without_code"
    }
}

enum PointerAttachAcknowledgedIncompleteReasonV1 {
    GetUnavailable,
    GetMalformed,
    BytesMismatch,
}

impl VerificationIncompleteReasonV1<PointerAttachAcknowledgedV1>
    for PointerAttachAcknowledgedIncompleteReasonV1
{
    fn stop(&self) -> EvidencePreserveStopCodeV1 {
        match self {
            Self::GetUnavailable => EvidencePreserveStopCodeV1::PostattachGetUnavailable,
            Self::GetMalformed => EvidencePreserveStopCodeV1::PostattachGetMalformed,
            Self::BytesMismatch => EvidencePreserveStopCodeV1::PostattachBytesMismatch,
        }
    }

    fn fallback_code(&self) -> &'static str {
        match self {
            Self::GetUnavailable => "postattach_get_unavailable_without_code",
            Self::GetMalformed => "postattach_get_malformed_without_code",
            Self::BytesMismatch => "postattach_bytes_mismatch_without_code",
        }
    }
}

enum BlobReadbackVerifiedIncompleteReasonV1 {
    ResultStatusUnavailable,
    ResultStatusInvalid,
}

impl VerificationIncompleteReasonV1<BlobReadbackVerifiedV1>
    for BlobReadbackVerifiedIncompleteReasonV1
{
    fn stop(&self) -> EvidencePreserveStopCodeV1 {
        match self {
            Self::ResultStatusUnavailable => EvidencePreserveStopCodeV1::ResultStatusUnavailable,
            Self::ResultStatusInvalid => EvidencePreserveStopCodeV1::ResultStatusInvalid,
        }
    }

    fn fallback_code(&self) -> &'static str {
        match self {
            Self::ResultStatusUnavailable => "result_status_unavailable_without_code",
            Self::ResultStatusInvalid => "result_status_invalid_without_code",
        }
    }
}

enum ResultSubjectObservedIncompleteReasonV1 {
    ResultMetadataUnavailable,
    PointerDeltaInvalid,
    PointerSchemaInvalid,
}

impl VerificationIncompleteReasonV1<ResultSubjectObservedV1>
    for ResultSubjectObservedIncompleteReasonV1
{
    fn stop(&self) -> EvidencePreserveStopCodeV1 {
        match self {
            Self::ResultMetadataUnavailable => {
                EvidencePreserveStopCodeV1::ResultMetadataUnavailable
            }
            Self::PointerDeltaInvalid => EvidencePreserveStopCodeV1::PointerDeltaInvalid,
            Self::PointerSchemaInvalid => EvidencePreserveStopCodeV1::PointerSchemaInvalid,
        }
    }

    fn fallback_code(&self) -> &'static str {
        match self {
            Self::ResultMetadataUnavailable => "result_metadata_unavailable_without_code",
            Self::PointerDeltaInvalid => "pointer_delta_invalid_without_code",
            Self::PointerSchemaInvalid => "pointer_schema_invalid_without_code",
        }
    }
}

enum PointerDeltaObservedIncompleteReasonV1 {
    PostattachEvaluation,
    PostattachDrift,
}

impl VerificationIncompleteReasonV1<PointerDeltaObservedV1>
    for PointerDeltaObservedIncompleteReasonV1
{
    fn stop(&self) -> EvidencePreserveStopCodeV1 {
        match self {
            Self::PostattachEvaluation => EvidencePreserveStopCodeV1::PostattachEvaluation,
            Self::PostattachDrift => EvidencePreserveStopCodeV1::PostattachDrift,
        }
    }

    fn fallback_code(&self) -> &'static str {
        match self {
            Self::PostattachEvaluation => "postattach_evaluation_without_code",
            Self::PostattachDrift => "postattach_drift_without_code",
        }
    }
}

struct PendingVerificationIncompleteV1<S, R>
where
    S: PublicationStageV1,
    R: VerificationIncompleteReasonV1<S>,
{
    attempt: EvidencePublicationAttemptV1<S>,
    reason: R,
    code: String,
}

impl<S, R> PendingVerificationIncompleteV1<S, R>
where
    S: PublicationStageV1,
    R: VerificationIncompleteReasonV1<S>,
{
    fn new(attempt: EvidencePublicationAttemptV1<S>, reason: R, code: impl Into<String>) -> Self {
        let code = nonempty_diagnostic(code, reason.fallback_code());
        Self {
            attempt,
            reason,
            code,
        }
    }

    fn finalize(self, close_effect: EvidenceCloseEffectV1) -> EvidenceOutcomeBuildResultV1 {
        let stop = EvidencePreserveStopV1 {
            stage: self.reason.stop(),
            code: self.code,
        };
        let publication = self.attempt.stage.wire_state();
        let common = self.attempt.common;
        let (disposition_unknown, close) = match close_effect.normalized() {
            EvidenceCloseEffectV1::Closed => (false, EvidenceCloseStateV1::Closed),
            EvidenceCloseEffectV1::NotDispatched { code } => {
                (false, EvidenceCloseStateV1::CloseNotDispatched { code })
            }
            EvidenceCloseEffectV1::OutcomeUnknown { code } => {
                (true, EvidenceCloseStateV1::CloseOutcomeUnknown { code })
            }
        };
        let residual = common.residual(stop, publication, close);
        if disposition_unknown {
            checked_outcome(EvidencePreserveDispositionV1::Unknown(residual))
        } else {
            checked_outcome(EvidencePreserveDispositionV1::VerificationIncomplete(
                residual,
            ))
        }
    }
}

struct PendingUnknownV1<S, R>
where
    S: PublicationStageV1,
    R: UnknownReasonV1<S>,
{
    attempt: EvidencePublicationAttemptV1<S>,
    reason: R,
    code: String,
}

impl<S, R> PendingUnknownV1<S, R>
where
    S: PublicationStageV1,
    R: UnknownReasonV1<S>,
{
    fn new(attempt: EvidencePublicationAttemptV1<S>, reason: R, code: impl Into<String>) -> Self {
        let code = nonempty_diagnostic(code, reason.fallback_code());
        Self {
            attempt,
            reason,
            code,
        }
    }

    fn finalize(self, close_effect: EvidenceCloseEffectV1) -> EvidenceOutcomeBuildResultV1 {
        let stop = EvidencePreserveStopV1 {
            stage: self.reason.stop(),
            code: self.code,
        };
        let publication = self.attempt.stage.wire_state();
        let common = self.attempt.common;
        let close = match close_effect.normalized() {
            EvidenceCloseEffectV1::Closed => EvidenceCloseStateV1::Closed,
            EvidenceCloseEffectV1::NotDispatched { code } => {
                EvidenceCloseStateV1::CloseNotDispatched { code }
            }
            EvidenceCloseEffectV1::OutcomeUnknown { code } => {
                EvidenceCloseStateV1::CloseOutcomeUnknown { code }
            }
        };
        checked_outcome(EvidencePreserveDispositionV1::Unknown(common.residual(
            stop,
            publication,
            close,
        )))
    }
}

struct ReadyToCloseV1 {
    attempt: EvidencePublicationAttemptV1<PostattachEquivalentV1>,
}

impl ReadyToCloseV1 {
    fn finalize(self, close_effect: EvidenceCloseEffectV1) -> EvidenceOutcomeBuildResultV1 {
        let EvidencePublicationAttemptV1 { common, stage } = self.attempt;
        let publication = stage.wire_state();
        match close_effect.normalized() {
            EvidenceCloseEffectV1::Closed => {
                let PostattachEquivalentV1 {
                    evidence_address,
                    pointer,
                    result_staged_revision,
                } = stage;
                checked_outcome(EvidencePreserveDispositionV1::Verified(
                    EvidencePreserveVerifiedV1 {
                        version: "v1".into(),
                        attempt_id: common.attempt_id,
                        source_staged_revision: common.source_staged_revision,
                        target_base_revision: common.target_base_revision,
                        snapshot_sha256: common.snapshot_sha256,
                        observed_candidate_addresses: common.observed_candidate_addresses,
                        result_staged_revision,
                        evidence_address,
                        pointer,
                        last_confirmed_publication: publication,
                        close: EvidenceCloseStateV1::Closed,
                    },
                ))
            }
            EvidenceCloseEffectV1::NotDispatched { code } => {
                let residual = common.residual(
                    EvidencePreserveStopV1 {
                        stage: EvidencePreserveStopCodeV1::StorageCloseNotDispatched,
                        code: code.clone(),
                    },
                    publication,
                    EvidenceCloseStateV1::CloseNotDispatched { code },
                );
                checked_outcome(EvidencePreserveDispositionV1::VerificationIncomplete(
                    residual,
                ))
            }
            EvidenceCloseEffectV1::OutcomeUnknown { code } => {
                let residual = common.residual(
                    EvidencePreserveStopV1 {
                        stage: EvidencePreserveStopCodeV1::StorageCloseOutcome,
                        code: code.clone(),
                    },
                    publication,
                    EvidenceCloseStateV1::CloseOutcomeUnknown { code },
                );
                checked_outcome(EvidencePreserveDispositionV1::Unknown(residual))
            }
        }
    }
}

impl EvidenceCloseEffectV1 {
    fn normalized(self) -> Self {
        match self {
            Self::Closed => Self::Closed,
            Self::NotDispatched { code } => Self::NotDispatched {
                code: nonempty_diagnostic(code, "storage_close_not_dispatched_without_code"),
            },
            Self::OutcomeUnknown { code } => Self::OutcomeUnknown {
                code: nonempty_diagnostic(code, "storage_close_outcome_unknown_without_code"),
            },
        }
    }
}

/// The only close-finalizable states. Each private variant retains a concrete
/// publication stage and a reason type whose trait implementation fixes the
/// outcome class; no runtime `(stage, class)` pair exists to mismatch or leak
/// through the crate's public API.
enum PendingEvidencePreserveStateV1 {
    NoPublicationIncomplete(
        PendingVerificationIncompleteV1<NoPublicationV1, NoPublicationIncompleteReasonV1>,
    ),
    PutObservedUnknown(PendingUnknownV1<PutObservedV1, PutObservedUnknownReasonV1>),
    BlobPublishedIncomplete(
        PendingVerificationIncompleteV1<BlobPublishedV1, BlobPublishedIncompleteReasonV1>,
    ),
    BlobPublishedUnknown(PendingUnknownV1<BlobPublishedV1, BlobPublishedUnknownReasonV1>),
    PointerAttachAcknowledgedIncomplete(
        PendingVerificationIncompleteV1<
            PointerAttachAcknowledgedV1,
            PointerAttachAcknowledgedIncompleteReasonV1,
        >,
    ),
    BlobReadbackVerifiedIncomplete(
        PendingVerificationIncompleteV1<
            BlobReadbackVerifiedV1,
            BlobReadbackVerifiedIncompleteReasonV1,
        >,
    ),
    ResultSubjectObservedIncomplete(
        PendingVerificationIncompleteV1<
            ResultSubjectObservedV1,
            ResultSubjectObservedIncompleteReasonV1,
        >,
    ),
    PointerDeltaObservedIncomplete(
        PendingVerificationIncompleteV1<
            PointerDeltaObservedV1,
            PointerDeltaObservedIncompleteReasonV1,
        >,
    ),
    Ready(ReadyToCloseV1),
}

pub(crate) struct PendingEvidencePreserveOutcomeV1(PendingEvidencePreserveStateV1);

impl PendingEvidencePreserveOutcomeV1 {
    fn no_publication_incomplete(
        pending: PendingVerificationIncompleteV1<NoPublicationV1, NoPublicationIncompleteReasonV1>,
    ) -> Self {
        Self(PendingEvidencePreserveStateV1::NoPublicationIncomplete(
            pending,
        ))
    }

    fn put_observed_unknown(
        pending: PendingUnknownV1<PutObservedV1, PutObservedUnknownReasonV1>,
    ) -> Self {
        Self(PendingEvidencePreserveStateV1::PutObservedUnknown(pending))
    }

    fn blob_published_incomplete(
        pending: PendingVerificationIncompleteV1<BlobPublishedV1, BlobPublishedIncompleteReasonV1>,
    ) -> Self {
        Self(PendingEvidencePreserveStateV1::BlobPublishedIncomplete(
            pending,
        ))
    }

    fn blob_published_unknown(
        pending: PendingUnknownV1<BlobPublishedV1, BlobPublishedUnknownReasonV1>,
    ) -> Self {
        Self(PendingEvidencePreserveStateV1::BlobPublishedUnknown(
            pending,
        ))
    }

    fn pointer_attach_acknowledged_incomplete(
        pending: PendingVerificationIncompleteV1<
            PointerAttachAcknowledgedV1,
            PointerAttachAcknowledgedIncompleteReasonV1,
        >,
    ) -> Self {
        Self(PendingEvidencePreserveStateV1::PointerAttachAcknowledgedIncomplete(pending))
    }

    fn blob_readback_verified_incomplete(
        pending: PendingVerificationIncompleteV1<
            BlobReadbackVerifiedV1,
            BlobReadbackVerifiedIncompleteReasonV1,
        >,
    ) -> Self {
        Self(PendingEvidencePreserveStateV1::BlobReadbackVerifiedIncomplete(pending))
    }

    fn result_subject_observed_incomplete(
        pending: PendingVerificationIncompleteV1<
            ResultSubjectObservedV1,
            ResultSubjectObservedIncompleteReasonV1,
        >,
    ) -> Self {
        Self(PendingEvidencePreserveStateV1::ResultSubjectObservedIncomplete(pending))
    }

    fn pointer_delta_observed_incomplete(
        pending: PendingVerificationIncompleteV1<
            PointerDeltaObservedV1,
            PointerDeltaObservedIncompleteReasonV1,
        >,
    ) -> Self {
        Self(PendingEvidencePreserveStateV1::PointerDeltaObservedIncomplete(pending))
    }

    fn ready(pending: ReadyToCloseV1) -> Self {
        Self(PendingEvidencePreserveStateV1::Ready(pending))
    }

    pub(crate) fn finalize(
        self,
        close_effect: EvidenceCloseEffectV1,
    ) -> EvidenceOutcomeBuildResultV1 {
        match self.0 {
            PendingEvidencePreserveStateV1::NoPublicationIncomplete(pending) => {
                pending.finalize(close_effect)
            }
            PendingEvidencePreserveStateV1::PutObservedUnknown(pending) => {
                pending.finalize(close_effect)
            }
            PendingEvidencePreserveStateV1::BlobPublishedIncomplete(pending) => {
                pending.finalize(close_effect)
            }
            PendingEvidencePreserveStateV1::BlobPublishedUnknown(pending) => {
                pending.finalize(close_effect)
            }
            PendingEvidencePreserveStateV1::PointerAttachAcknowledgedIncomplete(pending) => {
                pending.finalize(close_effect)
            }
            PendingEvidencePreserveStateV1::BlobReadbackVerifiedIncomplete(pending) => {
                pending.finalize(close_effect)
            }
            PendingEvidencePreserveStateV1::ResultSubjectObservedIncomplete(pending) => {
                pending.finalize(close_effect)
            }
            PendingEvidencePreserveStateV1::PointerDeltaObservedIncomplete(pending) => {
                pending.finalize(close_effect)
            }
            PendingEvidencePreserveStateV1::Ready(pending) => pending.finalize(close_effect),
        }
    }
}

fn checked_outcome(disposition: EvidencePreserveDispositionV1) -> EvidenceOutcomeBuildResultV1 {
    EvidencePreserveOutcomeV1::checked(disposition)
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

/// Exact traversal whose independently fixed 1000-revision ceiling overflowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryOverflowScope {
    PendingDco,
    SupersessionAncestry,
}

/// Fixed, machine-actionable remediation codes for the two ratified overflow
/// scopes. Non-overflow failures carry no remediation record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceRemediationCode {
    SplitSubmissionOrAdvanceTargetBase,
    MigrateSupersessionIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceRemediation {
    pub code: GovernanceRemediationCode,
    pub ticket: Option<String>,
}

/// One deterministic criterion observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionResult {
    pub criterion: GovernanceCriterion,
    pub passed: bool,
    pub failure_code: Option<String>,
    pub remediation: Option<GovernanceRemediation>,
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
    pub remediation: Option<GovernanceRemediation>,
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
    /// Exact current branch identity used by both lock-query legs.
    pub branch: String,
    /// Exact revision-only subject read before the scan.
    pub staged_revisions: Vec<String>,
    /// Subject reported by the full staged filesystem scan.
    pub scanned_staged_revisions: Vec<String>,
    /// Exact revision-only subject reread after the scan.
    pub post_scan_staged_revisions: Vec<String>,
    pub staged_paths: Vec<String>,
    pub staged_changes: Vec<StagedPathObservation>,
    /// Exact content and filesystem facts for every tracked staged-revision
    /// file, sorted by repository-relative path.
    pub worktree_files: Vec<WorktreeFileObservation>,
    pub worktree_clean: bool,
    pub scan_performed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernancePathAction {
    Modify,
    Add,
    Delete,
    Move,
    Copy,
}

/// Raw staged status event with both endpoints retained for moves. `Copy`
/// remains representable only so pinned-production-inexpressible input can be
/// retained and rejected with `copy_semantics_unavailable`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedPathObservation {
    pub path: String,
    pub from_path: Option<String>,
    pub action: GovernancePathAction,
    pub dirty: bool,
    pub conflict: bool,
}

/// One raw file event from an exact, terminally-complete upstream revision
/// diff. Pinned Lore projects a staged move as an exact Delete(source) plus
/// Add(target) pair. It cannot project staged Copy; that variant is retained
/// only to reject a future/synthetic raw shape explicitly.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionDiffObservation {
    pub path: String,
    pub action: GovernancePathAction,
    pub old_is_file: bool,
    pub new_is_file: bool,
    pub old_address: String,
    pub new_address: String,
}

/// Raw staged-revision and local-filesystem values observed for one path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorktreeFileObservation {
    pub path: String,
    pub revision: String,
    pub revision_hash: String,
    pub revision_context: String,
    pub revision_size: u64,
    pub local_hash: String,
    pub local_size: u64,
    pub filtered_revision_size: u64,
    pub flag_modified: bool,
    pub flag_deleted: bool,
    pub flag_added: bool,
    pub flag_conflict: bool,
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

/// Exact kind of one revision-metadata value emitted by pinned Lore.
///
/// Inline `Binary` is deliberately absent: Lore 9664606 drops that kind before
/// the public metadata callback. SBAI-6012 owns making that boundary observable,
/// paired with SBAI-6011's provenance/external-prior-state work; until both are
/// resolved, v1 must not pretend Binary state was observed. Every representable
/// emitted kind remains distinct here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataKind {
    Address,
    Boolean,
    Context,
    Hash,
    Numeric,
    String,
}

/// One exact typed revision metadata pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataEntry {
    pub key: String,
    pub kind: MetadataKind,
    pub value: String,
}

impl MetadataEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::with_kind(key, MetadataKind::String, value)
    }

    pub fn with_kind(key: impl Into<String>, kind: MetadataKind, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            kind,
            value: value.into(),
        }
    }

    pub fn string_value(&self) -> Option<&str> {
        (self.kind == MetadataKind::String).then_some(self.value.as_str())
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
    pub remediation: Option<GovernanceRemediation>,
    pub observations: GovernanceObservations,
}

// The authoritative writer -> marker -> evaluator chain remains deliberately
// unproven until the v2 writer lands.  Authenticated remote success coverage is
// tracked by SBAI-6001; until then the reviewed hybrid fixture is corroboration,
// never substitution, and any missing future fixture must fail loudly, not skip.
