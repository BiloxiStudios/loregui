//! Behavioral contract and fail-closed evaluator matrix for SBAI-5934 Option A.

use lore_vm::ops::governance::contract::{
    AdapterError, AffectedPath, ArtifactMarkSupersededRequest, AuthorResolutionObservation,
    CanonicalDcoMetadataObservationV1, CanonicalDcoObservationV1, CanonicalEvidenceSnapshotV1,
    CanonicalFileIdentityV1, CanonicalRevisionInfoV1, CanonicalRevisionRefV1,
    CanonicalStatusObservationV1, CanonicalSupersessionMetadataQueryObservationV1, CriterionResult,
    DcoMetadataObservation, DcoValidateRequest, EvidenceCloseStateV1, EvidencePointerDeltaV1,
    EvidencePointerV1, EvidencePreserveDispositionV1, EvidencePreserveOutcomeV1,
    EvidencePreserveRequest, EvidencePreserveStopCodeV1, EvidencePublicationStateV1,
    EvidenceRejectionCodeV1, FileIdentity, GovernanceCriterion, GovernancePathAction,
    GovernanceRemediation, GovernanceRemediationCode, GovernanceRole, HistoryOverflowScope,
    ImmutableGetItem, ImmutablePutItem, LockQuery, LockStatus, LockStatusResponse, MetadataEntry,
    MetadataKind, MutationObservation, ReadObservation, ResolvedAuthor, RevisionDiffObservation,
    RevisionInfo, RevisionInfoResponse, StagedPathObservation, StatusSnapshot,
    SubmissionGateCheckRequest, WorktreeFileObservation, EVIDENCE_POINTER_KEY,
    MAX_GOVERNANCE_HISTORY_REVISIONS, SUPERSESSION_MARKER_PREFIX,
};
use lore_vm::ops::governance::dco_validate::dco_validate_with_adapter;
#[cfg(feature = "integration-tests")]
use lore_vm::ops::governance::evaluator::ProductionLoreAdapter;
use lore_vm::ops::governance::evaluator::{evaluate, GovernanceAdapter};
use lore_vm::ops::governance::evidence_preserve::evidence_preserve_with_adapters;
#[cfg(feature = "integration-tests")]
use lore_vm::ops::governance::evidence_preserve::ProductionGovernanceIo;
use lore_vm::ops::governance::submission_gate_check::submission_gate_check_with_adapters;
use lore_vm::ops::governance::GovernanceIo;
use lore_vm::{dispatch, is_mutating_op, supported_ops, LoreApi};
#[cfg(feature = "integration-tests")]
use lore_vm::{global::LoreGlobal, ops};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

fn raw_modified(path: &str) -> RevisionDiffObservation {
    RevisionDiffObservation {
        path: path.into(),
        action: GovernancePathAction::Modify,
        old_is_file: true,
        new_is_file: true,
        old_address: format!("{}-{}", "4".repeat(64), "2".repeat(32)),
        new_address: format!("{}-{}", "4".repeat(64), "2".repeat(32)),
    }
}

fn raw_deleted(path: &str) -> RevisionDiffObservation {
    RevisionDiffObservation {
        path: path.into(),
        action: GovernancePathAction::Delete,
        old_is_file: true,
        new_is_file: false,
        old_address: format!("{}-{}", "4".repeat(64), "2".repeat(32)),
        new_address: format!("{}-{}", "0".repeat(64), "0".repeat(32)),
    }
}

fn raw_added(path: &str) -> RevisionDiffObservation {
    RevisionDiffObservation {
        path: path.into(),
        action: GovernancePathAction::Add,
        old_is_file: false,
        new_is_file: true,
        old_address: format!("{}-{}", "0".repeat(64), "0".repeat(32)),
        new_address: format!("{}-{}", "4".repeat(64), "2".repeat(32)),
    }
}

#[derive(Clone)]
struct FakeLore {
    status: Result<StatusSnapshot, AdapterError>,
    infos: BTreeMap<String, Result<RevisionInfoResponse, AdapterError>>,
    metadata: BTreeMap<String, Result<Vec<MetadataEntry>, AdapterError>>,
    history: Result<Vec<String>, AdapterError>,
    dumps: BTreeMap<String, Result<Vec<String>, AdapterError>>,
    file_info: BTreeMap<String, Result<Vec<FileIdentity>, AdapterError>>,
    diff: Result<Vec<RevisionDiffObservation>, AdapterError>,
    authors: Result<Vec<ResolvedAuthor>, AdapterError>,
    lock_queries: BTreeMap<String, Result<LockQuery, AdapterError>>,
    lock_status: Result<LockStatusResponse, AdapterError>,
    history_limits: std::sync::Arc<std::sync::Mutex<Vec<usize>>>,
    info_queries: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    lock_branches: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl FakeLore {
    fn clean() -> Self {
        let mut infos = BTreeMap::new();
        infos.insert(
            "candidate".into(),
            Ok(RevisionInfoResponse::exact(RevisionInfo {
                revision: "candidate".into(),
                parents: vec!["base".into()],
            })),
        );
        infos.insert(
            "base".into(),
            Ok(RevisionInfoResponse::exact(RevisionInfo {
                revision: "base".into(),
                parents: vec![],
            })),
        );

        let mut metadata = BTreeMap::new();
        for revision in ["candidate", "base"] {
            metadata.insert(
                revision.into(),
                Ok(vec![
                    MetadataEntry::new(
                        "message",
                        "change\n\nSigned-off-by: Alice <alice@example.test>",
                    ),
                    MetadataEntry::new("created-by", "alice"),
                ]),
            );
        }

        let mut dumps = BTreeMap::new();
        dumps.insert("candidate".into(), Ok(vec!["asset.txt".into()]));
        dumps.insert("base".into(), Ok(vec!["asset.txt".into()]));

        let mut file_info = BTreeMap::new();
        file_info.insert(
            "candidate".into(),
            Ok(vec![FileIdentity::new(
                "asset.txt",
                "candidate",
                "4444444444444444444444444444444444444444444444444444444444444444",
                "22222222222222222222222222222222",
            )]),
        );
        file_info.insert(
            "base".into(),
            Ok(vec![FileIdentity::new(
                "asset.txt",
                "base",
                "4444444444444444444444444444444444444444444444444444444444444444",
                "22222222222222222222222222222222",
            )]),
        );

        let mut lock_queries = BTreeMap::new();
        lock_queries.insert("asset.txt".into(), Ok(LockQuery::unlocked("asset.txt")));

        Self {
            status: Ok(StatusSnapshot {
                branch: "branch".into(),
                staged_revisions: vec!["candidate".into()],
                scanned_staged_revisions: vec!["candidate".into()],
                post_scan_staged_revisions: vec!["candidate".into()],
                staged_paths: vec!["asset.txt".into()],
                staged_changes: vec![StagedPathObservation {
                    path: "asset.txt".into(),
                    from_path: None,
                    action: GovernancePathAction::Modify,
                    dirty: true,
                    conflict: false,
                }],
                worktree_files: vec![WorktreeFileObservation {
                    path: "asset.txt".into(),
                    revision: "candidate".into(),
                    revision_hash:
                        "4444444444444444444444444444444444444444444444444444444444444444".into(),
                    revision_context: "22222222222222222222222222222222".into(),
                    revision_size: 5,
                    local_hash: "1111111111111111111111111111111111111111111111111111111111111111"
                        .into(),
                    local_size: 5,
                    filtered_revision_size: 5,
                    flag_modified: true,
                    flag_deleted: false,
                    flag_added: false,
                    flag_conflict: false,
                }],
                worktree_clean: true,
                scan_performed: true,
            }),
            infos,
            metadata,
            history: Ok(vec!["candidate".into()]),
            dumps,
            file_info,
            diff: Ok(vec![raw_modified("asset.txt")]),
            authors: Ok(vec![ResolvedAuthor::new("alice", "Alice")]),
            lock_queries,
            lock_status: Ok(LockStatusResponse::unlocked()),
            history_limits: Default::default(),
            info_queries: Default::default(),
            lock_branches: Default::default(),
        }
    }

    fn clean_rename() -> Self {
        let mut fake = Self::clean();
        fake.dumps
            .insert("candidate".into(), Ok(vec!["renamed.txt".into()]));
        fake.file_info.insert(
            "candidate".into(),
            Ok(vec![FileIdentity::new(
                "renamed.txt",
                "candidate",
                "4444444444444444444444444444444444444444444444444444444444444444",
                "22222222222222222222222222222222",
            )]),
        );
        let status = fake.status.as_mut().unwrap();
        status.staged_paths = vec!["asset.txt".into(), "renamed.txt".into()];
        status.staged_changes = vec![StagedPathObservation {
            path: "renamed.txt".into(),
            from_path: Some("asset.txt".into()),
            action: GovernancePathAction::Move,
            dirty: true,
            conflict: false,
        }];
        status.worktree_files[0].path = "renamed.txt".into();
        fake.lock_queries
            .insert("renamed.txt".into(), Ok(LockQuery::unlocked("renamed.txt")));
        fake.diff = Ok(vec![raw_deleted("asset.txt"), raw_added("renamed.txt")]);
        fake
    }

    fn error(name: &str) -> AdapterError {
        AdapterError::new(name)
    }

    fn with_linear_pending_count(count: usize) -> Self {
        assert!(count > 0);
        let mut fake = Self::clean();
        let mut history = vec!["candidate".to_string()];
        let mut parent = "base".to_string();

        for index in (1..count).rev() {
            let revision = format!("pending-{index}");
            fake.infos.insert(
                revision.clone(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: revision.clone(),
                    parents: vec![parent],
                })),
            );
            fake.metadata.insert(
                revision.clone(),
                Ok(vec![
                    MetadataEntry::new(
                        "message",
                        "change\n\nSigned-off-by: Alice <alice@example.test>",
                    ),
                    MetadataEntry::new("created-by", "alice"),
                ]),
            );
            parent = revision;
        }

        fake.infos.insert(
            "candidate".into(),
            Ok(RevisionInfoResponse::exact(RevisionInfo {
                revision: "candidate".into(),
                parents: vec![parent],
            })),
        );
        history.extend((1..count).map(|index| format!("pending-{index}")));
        fake.history = Ok(history);
        fake
    }

    fn with_second_parent_pending_count(count: usize) -> Self {
        assert!(count > 1);
        let mut fake = Self::clean();
        let mut parent = "base".to_string();

        for index in (1..count).rev() {
            let revision = format!("side-{index}");
            fake.infos.insert(
                revision.clone(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: revision.clone(),
                    parents: vec![parent],
                })),
            );
            fake.metadata.insert(
                revision.clone(),
                Ok(vec![
                    MetadataEntry::new(
                        "message",
                        "change\n\nSigned-off-by: Alice <alice@example.test>",
                    ),
                    MetadataEntry::new("created-by", "alice"),
                ]),
            );
            parent = revision;
        }

        fake.infos.insert(
            "candidate".into(),
            Ok(RevisionInfoResponse::exact(RevisionInfo {
                revision: "candidate".into(),
                parents: vec!["base".into(), parent],
            })),
        );
        fake.history = Ok(vec!["candidate".into()]);
        fake
    }
}

#[async_trait::async_trait]
impl GovernanceAdapter for FakeLore {
    async fn status(&self) -> Result<StatusSnapshot, AdapterError> {
        self.status.clone()
    }

    async fn revision_info(&self, revision: &str) -> Result<RevisionInfoResponse, AdapterError> {
        self.info_queries.lock().unwrap().push(revision.into());
        self.infos
            .get(revision)
            .cloned()
            .unwrap_or_else(|| Err(Self::error("missing revision info")))
    }

    async fn revision_metadata(&self, revision: &str) -> Result<Vec<MetadataEntry>, AdapterError> {
        self.metadata
            .get(revision)
            .cloned()
            .unwrap_or_else(|| Err(Self::error("missing revision metadata")))
    }

    async fn first_parent_history(
        &self,
        _candidate: &str,
        _target_base: &str,
        max_revisions: usize,
    ) -> Result<Vec<String>, AdapterError> {
        self.history_limits.lock().unwrap().push(max_revisions);
        self.history.clone()
    }

    async fn repository_dump(&self, revision: &str) -> Result<Vec<String>, AdapterError> {
        self.dumps
            .get(revision)
            .cloned()
            .unwrap_or_else(|| Err(Self::error("missing dump")))
    }

    async fn file_info(
        &self,
        revision: &str,
        _paths: &[String],
    ) -> Result<Vec<FileIdentity>, AdapterError> {
        self.file_info
            .get(revision)
            .cloned()
            .unwrap_or_else(|| Err(Self::error("missing file info")))
    }

    async fn revision_diff(
        &self,
        _base: &str,
        _candidate: &str,
    ) -> Result<Vec<RevisionDiffObservation>, AdapterError> {
        self.diff.clone()
    }

    async fn resolve_authors(
        &self,
        _identities: &[String],
    ) -> Result<Vec<ResolvedAuthor>, AdapterError> {
        self.authors.clone()
    }

    async fn lock_file_query(&self, branch: &str, path: &str) -> Result<LockQuery, AdapterError> {
        self.lock_branches
            .lock()
            .unwrap()
            .push(format!("query:{branch}"));
        if branch != "branch" {
            return Err(Self::error("wrong lock query branch"));
        }
        self.lock_queries
            .get(path)
            .cloned()
            .unwrap_or_else(|| Err(Self::error("missing lock query")))
    }

    async fn lock_file_status(
        &self,
        branch: &str,
        _paths: &[String],
    ) -> Result<LockStatusResponse, AdapterError> {
        self.lock_branches
            .lock()
            .unwrap()
            .push(format!("status:{branch}"));
        if branch != "branch" {
            return Err(Self::error("wrong lock status branch"));
        }
        self.lock_status.clone()
    }
}

fn request() -> SubmissionGateCheckRequest {
    SubmissionGateCheckRequest {
        expected_staged_revision: "candidate".into(),
        target_base_revision: "base".into(),
    }
}

fn assert_closed_for(mut fake: FakeLore, mutate: impl FnOnce(&mut FakeLore), expected: &str) {
    mutate(&mut fake);
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&fake, &request()));
    assert!(!result.open, "{expected}: {result:?}");
    assert!(
        result.failure_codes.iter().any(|code| code == expected),
        "expected {expected}, got {:?}",
        result.failure_codes
    );
}

#[test]
fn clean_exact_subject_is_open_and_uses_1001_overflow_sentinel() {
    let fake = FakeLore::clean();
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&fake, &request()));

    assert!(result.open, "{result:?}");
    assert_eq!(result.pending_revisions, vec!["candidate"]);
    assert_eq!(result.affected_paths, vec!["asset.txt"]);
    assert_eq!(*fake.history_limits.lock().unwrap(), vec![1001]);
    assert_eq!(
        *fake.lock_branches.lock().unwrap(),
        vec!["query:branch", "status:branch"]
    );
}

#[test]
fn evaluator_exposes_complete_sorted_raw_observations_without_replacing_them_with_a_verdict() {
    let fake = FakeLore::clean();
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&fake, &request()));
    let observed = result.observations;

    assert_eq!(observed.expected_staged_revision, "candidate");
    assert_eq!(observed.target_base_revision, "base");
    assert_eq!(observed.status.unwrap().staged_paths, vec!["asset.txt"]);
    assert_eq!(
        observed.base_revision_info,
        Some(RevisionInfo {
            revision: "base".into(),
            parents: vec![],
        })
    );
    assert_eq!(
        observed.revision_graph,
        vec![RevisionInfo {
            revision: "candidate".into(),
            parents: vec!["base".into()],
        }]
    );
    assert_eq!(observed.first_parent_history, vec!["candidate"]);
    assert_eq!(observed.base_files.len(), 1);
    assert_eq!(
        observed.base_files[0].canonical_id(),
        "4444444444444444444444444444444444444444444444444444444444444444:22222222222222222222222222222222"
    );
    assert_eq!(observed.candidate_files.len(), 1);
    assert_eq!(
        observed.candidate_files[0].canonical_id(),
        "4444444444444444444444444444444444444444444444444444444444444444:22222222222222222222222222222222"
    );
    assert_eq!(
        observed.current_files[0].canonical_id(),
        "1111111111111111111111111111111111111111111111111111111111111111:22222222222222222222222222222222"
    );
    assert_eq!(
        observed.revision_diff,
        vec![AffectedPath::modified("asset.txt")]
    );
    assert_eq!(observed.affected_paths, vec!["asset.txt"]);
    assert!(observed.supersession_markers.is_empty());
    assert_eq!(
        observed.dco_metadata,
        vec![DcoMetadataObservation {
            revision: "candidate".into(),
            messages: vec!["change\n\nSigned-off-by: Alice <alice@example.test>".into()],
            created_by: vec!["alice".into()],
            committed_by: vec![],
        }]
    );
    assert_eq!(
        observed.author_resolution,
        Some(AuthorResolutionObservation {
            requested: vec!["alice".into()],
            replies: vec![ResolvedAuthor::new("alice", "Alice")],
        })
    );
    assert_eq!(observed.dco.len(), 1);
    assert_eq!(observed.dco[0].revision, "candidate");
    assert_eq!(
        observed.dco[0].trailer,
        "Signed-off-by: Alice <alice@example.test>"
    );
    assert_eq!(observed.dco[0].signer_name, "Alice");
    assert_eq!(observed.dco[0].signer_email, "alice@example.test");
    assert_eq!(
        observed.dco[0].resolved_authors,
        vec![ResolvedAuthor::new("alice", "Alice")]
    );
    assert_eq!(
        observed.lock_queries,
        vec![LockQuery::unlocked("asset.txt")]
    );
    assert_eq!(observed.lock_status, Some(LockStatusResponse::unlocked()));
    assert!(observed.dependency_observations.is_empty());
}

#[test]
fn strict_v1_requests_reject_every_legacy_control_and_pin_ratified_namespaces() {
    assert_eq!(
        SUPERSESSION_MARKER_PREFIX,
        "studiobrain.governance.v1.superseded."
    );
    assert_eq!(EVIDENCE_POINTER_KEY, "studiobrain.governance.v1.evidence");
    assert_eq!(MAX_GOVERNANCE_HISTORY_REVISIONS, 1000);
    assert_eq!(
        serde_json::to_value([
            GovernanceCriterion::ExactSubject,
            GovernanceCriterion::HistoryComplete,
            GovernanceCriterion::DcoValid,
            GovernanceCriterion::NotSuperseded,
            GovernanceCriterion::LocksClear,
            GovernanceCriterion::WorktreeClean,
            GovernanceCriterion::EvidenceValid,
        ])
        .unwrap(),
        serde_json::json!([
            "exact_subject",
            "history_complete",
            "dco_valid",
            "not_superseded",
            "locks_clear",
            "worktree_clean",
            "evidence_valid",
        ])
    );

    for field in [
        "disable_dco",
        "disable_history",
        "force",
        "include",
        "require_dco",
        "reject_superseded",
        "require_clean_workdir",
        "since",
        "limit",
    ] {
        let value = format!(
            r#"{{"expected_staged_revision":"candidate","target_base_revision":"base","{field}":true}}"#
        );
        assert!(serde_json::from_str::<ArtifactMarkSupersededRequest>(&value).is_err());
        assert!(serde_json::from_str::<DcoValidateRequest>(&value).is_err());
        assert!(serde_json::from_str::<EvidencePreserveRequest>(&value).is_err());
        assert!(serde_json::from_str::<SubmissionGateCheckRequest>(&value).is_err());
    }

    for value in [
        serde_json::json!({
            "expected_staged_revision": "",
            "target_base_revision": "base"
        }),
        serde_json::json!({
            "expected_staged_revision": "candidate",
            "target_base_revision": ""
        }),
        serde_json::json!({
            "expected_staged_revision": "",
            "target_base_revision": ""
        }),
    ] {
        assert!(
            serde_json::from_value::<EvidencePreserveRequest>(value).is_err(),
            "serde must reject an empty evidence subject before attempt creation"
        );
    }

    for request in [
        EvidencePreserveRequest {
            expected_staged_revision: String::new(),
            target_base_revision: "base".into(),
        },
        EvidencePreserveRequest {
            expected_staged_revision: "candidate".into(),
            target_base_revision: String::new(),
        },
    ] {
        assert!(
            request.validate().is_err(),
            "direct construction must share the strict serde validator"
        );
    }
}

#[test]
fn remediation_schema_is_a_strict_fixed_enum_and_nonoverflow_failures_have_none() {
    let remediation = GovernanceRemediation {
        code: GovernanceRemediationCode::MigrateSupersessionIndex,
        ticket: Some("SBAI-6010".into()),
    };
    assert_eq!(
        serde_json::to_value(&remediation).unwrap(),
        serde_json::json!({
            "code": "migrate_supersession_index",
            "ticket": "SBAI-6010"
        })
    );
    assert!(
        serde_json::from_value::<GovernanceRemediation>(serde_json::json!({
            "code": "raise_history_limit",
            "ticket": null
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<GovernanceRemediation>(serde_json::json!({
            "code": "migrate_supersession_index",
            "ticket": "SBAI-6010",
            "automatic": true
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<CriterionResult>(serde_json::json!({
            "criterion": "history_complete",
            "passed": false,
            "failure_code": "history_depth_exceeded",
            "remediation": {
                "code": "split_submission_or_advance_target_base",
                "ticket": null,
                "unknown": true
            }
        }))
        .is_err()
    );

    let mut dependency = FakeLore::clean();
    dependency.status = Err(FakeLore::error("status"));
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&dependency, &request()));
    assert_eq!(result.failure_codes, ["status_unavailable"]);
    assert!(result.remediation.is_none());
}

#[test]
fn deferred_supersession_contract_requires_a_target_path_selector() {
    let missing_selector = serde_json::json!({
        "expected_staged_revision": "candidate",
        "target_base_revision": "base"
    });
    assert!(
        serde_json::from_value::<ArtifactMarkSupersededRequest>(missing_selector).is_err(),
        "the deferred v2 writer contract must never select an arbitrary tree identity"
    );

    let request: ArtifactMarkSupersededRequest = serde_json::from_value(serde_json::json!({
        "expected_staged_revision": "candidate",
        "target_base_revision": "base",
        "target_path": "asset.txt"
    }))
    .expect("the strict future contract accepts one mandatory path selector");
    assert_eq!(request.target_path, "asset.txt");
}

#[test]
fn canonical_evidence_and_pointer_delta_are_strict_typed_boundaries() {
    assert_ne!(
        MetadataEntry::new("typed", "1"),
        MetadataEntry::with_kind("typed", MetadataKind::Numeric, "1"),
        "metadata kind is part of exact sole-delta equivalence"
    );

    let snapshot = CanonicalEvidenceSnapshotV1 {
        version: "v1".into(),
        target_base_revision: "base".into(),
        status: CanonicalStatusObservationV1 {
            branch: "branch".into(),
            staged_revisions: vec![CanonicalRevisionRefV1::StagedSubject],
            scanned_staged_revisions: vec![CanonicalRevisionRefV1::StagedSubject],
            post_scan_staged_revisions: vec![CanonicalRevisionRefV1::StagedSubject],
            staged_paths: vec!["asset.txt".into()],
            staged_changes: vec![StagedPathObservation {
                path: "asset.txt".into(),
                from_path: None,
                action: GovernancePathAction::Modify,
                dirty: true,
                conflict: false,
            }],
            worktree_files: vec![
                lore_vm::ops::governance::contract::CanonicalWorktreeFileObservationV1 {
                    path: "asset.txt".into(),
                    revision: CanonicalRevisionRefV1::StagedSubject,
                    revision_hash:
                        "4444444444444444444444444444444444444444444444444444444444444444".into(),
                    revision_context: "22222222222222222222222222222222".into(),
                    revision_size: 5,
                    local_hash: "1111111111111111111111111111111111111111111111111111111111111111"
                        .into(),
                    local_size: 5,
                    filtered_revision_size: 5,
                    flag_modified: true,
                    flag_deleted: false,
                    flag_added: false,
                    flag_conflict: false,
                },
            ],
            worktree_clean: true,
            scan_performed: true,
        },
        base_revision_info: CanonicalRevisionInfoV1 {
            revision: CanonicalRevisionRefV1::Exact("base".into()),
            parents: vec![],
        },
        supersession_ancestry: vec![
            CanonicalRevisionInfoV1 {
                revision: CanonicalRevisionRefV1::Exact("base".into()),
                parents: vec![],
            },
            CanonicalRevisionInfoV1 {
                revision: CanonicalRevisionRefV1::StagedSubject,
                parents: vec![CanonicalRevisionRefV1::Exact("base".into())],
            },
        ],
        supersession_ancestry_observed: true,
        revision_graph: vec![CanonicalRevisionInfoV1 {
            revision: CanonicalRevisionRefV1::StagedSubject,
            parents: vec![CanonicalRevisionRefV1::Exact("base".into())],
        }],
        first_parent_history: vec![CanonicalRevisionRefV1::StagedSubject],
        base_files: vec![CanonicalFileIdentityV1 {
            path: "asset.txt".into(),
            revision: CanonicalRevisionRefV1::Exact("base".into()),
            hash: "4444444444444444444444444444444444444444444444444444444444444444".into(),
            context: "22222222222222222222222222222222".into(),
        }],
        base_tree_observed: true,
        candidate_files: vec![CanonicalFileIdentityV1 {
            path: "asset.txt".into(),
            revision: CanonicalRevisionRefV1::StagedSubject,
            hash: "4444444444444444444444444444444444444444444444444444444444444444".into(),
            context: "22222222222222222222222222222222".into(),
        }],
        candidate_tree_observed: true,
        current_files: vec![CanonicalFileIdentityV1 {
            path: "asset.txt".into(),
            revision: CanonicalRevisionRefV1::StagedSubject,
            hash: "1111111111111111111111111111111111111111111111111111111111111111".into(),
            context: "22222222222222222222222222222222".into(),
        }],
        upstream_revision_diff: vec![raw_modified("asset.txt")],
        revision_diff: vec![AffectedPath::modified("asset.txt")],
        revision_diff_observed: true,
        affected_paths: vec!["asset.txt".into()],
        supersession_markers: vec![],
        supersession_metadata_queries: vec![
            CanonicalSupersessionMetadataQueryObservationV1 {
                revision: CanonicalRevisionRefV1::Exact("base".into()),
                metadata: vec![],
            },
            CanonicalSupersessionMetadataQueryObservationV1 {
                revision: CanonicalRevisionRefV1::StagedSubject,
                metadata: vec![],
            },
        ],
        supersession_metadata_observed: true,
        dco_metadata: vec![CanonicalDcoMetadataObservationV1 {
            revision: CanonicalRevisionRefV1::StagedSubject,
            messages: vec!["change\n\nSigned-off-by: Alice <alice@example.test>".into()],
            created_by: vec!["alice".into()],
            committed_by: vec![],
        }],
        author_resolution: AuthorResolutionObservation {
            requested: vec!["alice".into()],
            replies: vec![ResolvedAuthor::new("alice", "Alice")],
        },
        dco: vec![CanonicalDcoObservationV1 {
            revision: CanonicalRevisionRefV1::StagedSubject,
            message: "change\n\nSigned-off-by: Alice <alice@example.test>".into(),
            trailer: "Signed-off-by: Alice <alice@example.test>".into(),
            signer_name: "Alice".into(),
            signer_email: "alice@example.test".into(),
            created_by: "alice".into(),
            committed_by: None,
            resolved_authors: vec![ResolvedAuthor::new("alice", "Alice")],
        }],
        lock_queries: vec![LockQuery::unlocked("asset.txt")],
        lock_status: LockStatusResponse::unlocked(),
        dependency_observations: vec![],
    };
    let bytes = serde_json::to_vec(&snapshot).expect("canonical snapshot serializes");
    assert_eq!(
        serde_json::from_slice::<CanonicalEvidenceSnapshotV1>(&bytes).unwrap(),
        snapshot
    );

    let mut with_unknown = serde_json::to_value(&snapshot).unwrap();
    with_unknown["gate_open"] = serde_json::json!(true);
    assert!(serde_json::from_value::<CanonicalEvidenceSnapshotV1>(with_unknown).is_err());

    for path in ["/status/staged_revisions/0", "/revision_graph/0/parents/0"] {
        let mut nested_unknown = serde_json::to_value(&snapshot).unwrap();
        nested_unknown
            .pointer_mut(path)
            .expect("canonical revision reference exists")["unknown"] = serde_json::json!(true);
        assert!(
            serde_json::from_value::<CanonicalEvidenceSnapshotV1>(nested_unknown).is_err(),
            "nested canonical revision reference accepted an unknown field at {path}"
        );
    }

    let delta = EvidencePointerDeltaV1 {
        version: "v1".into(),
        key: EVIDENCE_POINTER_KEY.into(),
        source_staged_revision: "candidate".into(),
        result_staged_revision: "candidate-with-pointer".into(),
        pointer: EvidencePointerV1 {
            version: "v1".into(),
            address: format!("{}-{}", "a".repeat(64), "0".repeat(32)),
        },
    };
    let mut encoded = serde_json::to_value(&delta).unwrap();
    encoded["passed"] = serde_json::json!(true);
    assert!(serde_json::from_value::<EvidencePointerDeltaV1>(encoded).is_err());
}

#[test]
fn evidence_outcomes_are_strict_checked_residual_state_not_actor_verdicts() {
    let address = format!("{}-{}", "a".repeat(64), "0".repeat(32));
    let pointer = serde_json::json!({
        "version": "v1",
        "address": address,
    });
    let verified = serde_json::json!({
        "version": "v1",
        "outcome": "verified",
        "attempt_id": "1".repeat(64),
        "source_staged_revision": "candidate",
        "target_base_revision": "base",
        "snapshot_sha256": "2".repeat(64),
        "observed_candidate_addresses": [address],
        "result_staged_revision": "candidate-with-pointer",
        "evidence_address": address,
        "pointer": pointer,
        "last_confirmed_publication": {
            "state": "postattach_equivalent",
            "evidence_address": address,
            "pointer": pointer,
            "result_staged_revision": "candidate-with-pointer"
        },
        "close": { "state": "closed" }
    });
    let parsed: EvidencePreserveOutcomeV1 =
        serde_json::from_value(verified.clone()).expect("the full closed chain is verified");
    assert_eq!(serde_json::to_value(parsed).unwrap(), verified);
    for forbidden in ["gate_open", "passed", "valid", "verdict"] {
        assert!(
            !verified.to_string().contains(forbidden),
            "actor residual outcomes must not contain authority field {forbidden}"
        );
    }

    let mut not_closed = verified.clone();
    not_closed["close"] = serde_json::json!({
        "state": "close_not_dispatched",
        "code": "fixture"
    });
    assert!(serde_json::from_value::<EvidencePreserveOutcomeV1>(not_closed).is_err());

    let mut skipped_stage = verified.clone();
    skipped_stage["last_confirmed_publication"]["state"] =
        serde_json::json!("pointer_delta_observed");
    assert!(serde_json::from_value::<EvidencePreserveOutcomeV1>(skipped_stage).is_err());

    let mut mismatched_address = verified.clone();
    mismatched_address["last_confirmed_publication"]["evidence_address"] =
        serde_json::json!(format!("{}-{}", "b".repeat(64), "0".repeat(32)));
    assert!(serde_json::from_value::<EvidencePreserveOutcomeV1>(mismatched_address).is_err());

    let mut unknown = verified.clone();
    unknown["authoritative"] = serde_json::json!(false);
    assert!(serde_json::from_value::<EvidencePreserveOutcomeV1>(unknown).is_err());

    let impossible_incomplete = serde_json::json!({
        "version": "v1",
        "outcome": "verification_incomplete",
        "attempt_id": "1".repeat(64),
        "source_staged_revision": "candidate",
        "target_base_revision": "base",
        "snapshot_sha256": null,
        "observed_candidate_addresses": [],
        "stopped_at": { "stage": "initial_evaluation", "code": "fixture" },
        "last_confirmed_publication": {
            "state": "blob_published",
            "evidence_address": address
        },
        "close": { "state": "not_opened" }
    });
    assert!(serde_json::from_value::<EvidencePreserveOutcomeV1>(impossible_incomplete).is_err());
}

#[tokio::test]
async fn ruled_governance_dispatch_surface_routes_three_ops_and_defers_writer() {
    let expected = [
        "governance.dco_validate",
        "governance.evidence_preserve",
        "governance.submission_gate_check",
    ];
    for op in expected {
        assert!(supported_ops().contains(&op), "missing canonical op {op}");
        let api = LoreApi::new(std::path::PathBuf::from("/nonexistent-governance-dispatch"));
        let error = dispatch(&api, op, serde_json::Value::Null)
            .await
            .expect_err("null args must fail before touching Lore");
        assert!(
            !error.to_string().contains("unknown op"),
            "{op} is listed but not canonically routed: {error}"
        );
    }

    assert!(!supported_ops().contains(&"governance.artifact_mark_superseded"));
    let api = LoreApi::new(std::path::PathBuf::from("."));
    let error = dispatch(
        &api,
        "governance.artifact_mark_superseded",
        serde_json::json!({
            "expected_staged_revision": "candidate",
            "target_base_revision": "base",
            "target_path": "asset.txt"
        }),
    )
    .await
    .expect_err("the authoritative writer is deferred to v2");
    assert!(error.to_string().contains("unknown op"));

    assert!(!is_mutating_op("governance.dco_validate"));
    assert!(is_mutating_op("governance.evidence_preserve"));
    assert!(!is_mutating_op("governance.submission_gate_check"));
    assert!(!is_mutating_op("governance.artifact_mark_superseded"));
}

#[derive(Clone)]
struct SharedFakeLore(Arc<Mutex<FakeLore>>);

impl SharedFakeLore {
    fn clean() -> Self {
        Self(Arc::new(Mutex::new(FakeLore::clean())))
    }

    fn snapshot(&self) -> FakeLore {
        self.0.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl GovernanceAdapter for SharedFakeLore {
    async fn status(&self) -> Result<StatusSnapshot, AdapterError> {
        GovernanceAdapter::status(&self.snapshot()).await
    }

    async fn revision_info(&self, revision: &str) -> Result<RevisionInfoResponse, AdapterError> {
        GovernanceAdapter::revision_info(&self.snapshot(), revision).await
    }

    async fn revision_metadata(&self, revision: &str) -> Result<Vec<MetadataEntry>, AdapterError> {
        GovernanceAdapter::revision_metadata(&self.snapshot(), revision).await
    }

    async fn first_parent_history(
        &self,
        candidate: &str,
        target_base: &str,
        max_revisions: usize,
    ) -> Result<Vec<String>, AdapterError> {
        GovernanceAdapter::first_parent_history(
            &self.snapshot(),
            candidate,
            target_base,
            max_revisions,
        )
        .await
    }

    async fn repository_dump(&self, revision: &str) -> Result<Vec<String>, AdapterError> {
        GovernanceAdapter::repository_dump(&self.snapshot(), revision).await
    }

    async fn file_info(
        &self,
        revision: &str,
        paths: &[String],
    ) -> Result<Vec<FileIdentity>, AdapterError> {
        GovernanceAdapter::file_info(&self.snapshot(), revision, paths).await
    }

    async fn revision_diff(
        &self,
        base: &str,
        candidate: &str,
    ) -> Result<Vec<RevisionDiffObservation>, AdapterError> {
        GovernanceAdapter::revision_diff(&self.snapshot(), base, candidate).await
    }

    async fn resolve_authors(
        &self,
        identities: &[String],
    ) -> Result<Vec<ResolvedAuthor>, AdapterError> {
        GovernanceAdapter::resolve_authors(&self.snapshot(), identities).await
    }

    async fn lock_file_query(&self, branch: &str, path: &str) -> Result<LockQuery, AdapterError> {
        GovernanceAdapter::lock_file_query(&self.snapshot(), branch, path).await
    }

    async fn lock_file_status(
        &self,
        branch: &str,
        paths: &[String],
    ) -> Result<LockStatusResponse, AdapterError> {
        GovernanceAdapter::lock_file_status(&self.snapshot(), branch, paths).await
    }
}

#[derive(Clone, Default)]
struct FakeIoState {
    failures: BTreeSet<String>,
    mutation_modes: BTreeMap<String, FakeMutationMode>,
    applied_effects: BTreeSet<String>,
    calls: Vec<String>,
    opens: usize,
    puts: usize,
    gets: usize,
    closes: usize,
    metadata_writes: Vec<(String, String)>,
    stored: BTreeMap<String, Vec<u8>>,
    corrupt_get_bytes: bool,
    metadata_type_drift_on_set: bool,
    open_override: Option<MutationObservation<Vec<u64>>>,
    put_override: Option<MutationObservation<Vec<ImmutablePutItem>>>,
    get_override: Option<ReadObservation<Vec<ImmutableGetItem>>>,
    get_mutation: Option<GetMutation>,
    put_metadata_mutation: Option<MetadataEntry>,
}

#[derive(Clone, Copy)]
enum GetMutation {
    Fingerprint,
    Lock,
    Subject,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FakeMutationMode {
    #[default]
    Completed,
    NotDispatched,
    OutcomeUnknownAbsent,
    OutcomeUnknownApplied,
}

#[derive(Clone)]
struct FakeIo {
    lore: SharedFakeLore,
    role: GovernanceRole,
    state: Arc<Mutex<FakeIoState>>,
}

impl FakeIo {
    fn new(lore: SharedFakeLore, role: GovernanceRole) -> Self {
        Self {
            lore,
            role,
            state: Arc::new(Mutex::new(FakeIoState::default())),
        }
    }

    fn with_role(&self, role: GovernanceRole) -> Self {
        Self {
            lore: self.lore.clone(),
            role,
            state: self.state.clone(),
        }
    }

    fn fail(&self, dependency: &str) {
        self.state
            .lock()
            .unwrap()
            .failures
            .insert(dependency.into());
    }

    fn set_mutation_mode(&self, dependency: &str, mode: FakeMutationMode) {
        self.state
            .lock()
            .unwrap()
            .mutation_modes
            .insert(dependency.into(), mode);
    }

    fn state(&self) -> FakeIoState {
        self.state.lock().unwrap().clone()
    }

    fn set_put_override(&self, result: Result<Vec<ImmutablePutItem>, AdapterError>) {
        self.state.lock().unwrap().put_override = Some(match result {
            Ok(items) => MutationObservation::Completed(items),
            Err(error) => MutationObservation::NotDispatched {
                code: error.message,
            },
        });
    }

    fn set_open_override(&self, observation: MutationObservation<Vec<u64>>) {
        self.state.lock().unwrap().open_override = Some(observation);
    }

    fn set_get_override(&self, result: Result<Vec<ImmutableGetItem>, AdapterError>) {
        self.state.lock().unwrap().get_override = Some(match result {
            Ok(items) => ReadObservation::Completed(items),
            Err(error) => ReadObservation::Unavailable {
                code: error.message,
            },
        });
    }

    fn replace_stored_bytes(&self, address: &str, bytes: Vec<u8>) {
        self.state
            .lock()
            .unwrap()
            .stored
            .insert(address.into(), bytes);
    }

    fn corrupt_get_bytes(&self) {
        self.state.lock().unwrap().corrupt_get_bytes = true;
    }

    fn drift_metadata_type_on_set(&self) {
        self.state.lock().unwrap().metadata_type_drift_on_set = true;
    }

    fn mutate_on_get(&self, mutation: GetMutation) {
        self.state.lock().unwrap().get_mutation = Some(mutation);
    }

    fn mutate_metadata_on_put(&self, entry: MetadataEntry) {
        self.state.lock().unwrap().put_metadata_mutation = Some(entry);
    }

    fn has_failure(&self, dependency: &str) -> bool {
        self.state.lock().unwrap().failures.contains(dependency)
    }

    fn mutation_mode(&self, dependency: &str) -> FakeMutationMode {
        self.state
            .lock()
            .unwrap()
            .mutation_modes
            .get(dependency)
            .copied()
            .unwrap_or_default()
    }
}

const FAKE_EVIDENCE_ADDRESS: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-00000000000000000000000000000000";

#[async_trait::async_trait]
impl GovernanceIo for FakeIo {
    fn role(&self) -> GovernanceRole {
        self.role
    }

    async fn revision_metadata_set(&self, key: &str, value: &str) -> MutationObservation<()> {
        self.state.lock().unwrap().calls.push("metadata_set".into());
        if self.has_failure("metadata_set") {
            return MutationObservation::NotDispatched {
                code: "metadata_set".into(),
            };
        }
        let mode = self.mutation_mode("metadata_set");
        if mode == FakeMutationMode::NotDispatched {
            return MutationObservation::NotDispatched {
                code: "metadata_set not dispatched".into(),
            };
        }
        if mode == FakeMutationMode::OutcomeUnknownAbsent {
            return MutationObservation::OutcomeUnknown {
                code: "metadata_set outcome unknown".into(),
                observed: (),
            };
        }

        let applied = (|| -> Result<(), AdapterError> {
            let mut state = self.state.lock().unwrap();
            state.metadata_writes.push((key.into(), value.into()));
            let ordinal = state.metadata_writes.len();
            let metadata_type_drift_on_set = state.metadata_type_drift_on_set;
            drop(state);

            let mut fake = self.lore.0.lock().unwrap();
            let source = fake
                .status
                .as_ref()
                .map_err(Clone::clone)?
                .staged_revisions
                .first()
                .cloned()
                .ok_or_else(|| AdapterError::new("no staged source"))?;
            let result = format!("{source}-metadata-{ordinal}");

            let source_info = fake
                .infos
                .get(&source)
                .cloned()
                .ok_or_else(|| AdapterError::new("missing source info"))??;
            let exact = source_info
                .revisions
                .first()
                .cloned()
                .ok_or_else(|| AdapterError::new("missing exact source info"))?;
            fake.infos.insert(
                result.clone(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: result.clone(),
                    parents: exact.parents,
                })),
            );

            let mut metadata = fake
                .metadata
                .get(&source)
                .cloned()
                .ok_or_else(|| AdapterError::new("missing source metadata"))??;
            if metadata_type_drift_on_set {
                let typed = metadata
                    .iter_mut()
                    .find(|entry| entry.key == "typed")
                    .ok_or_else(|| AdapterError::new("missing typed metadata fixture"))?;
                typed.kind = MetadataKind::Numeric;
            }
            metadata.push(MetadataEntry::new(key, value));
            fake.metadata.insert(result.clone(), Ok(metadata));

            let dump = fake
                .dumps
                .get(&source)
                .cloned()
                .ok_or_else(|| AdapterError::new("missing source dump"))?;
            fake.dumps.insert(result.clone(), dump);
            let mut identities = fake
                .file_info
                .get(&source)
                .cloned()
                .ok_or_else(|| AdapterError::new("missing source identities"))??;
            for identity in &mut identities {
                identity.revision.clone_from(&result);
            }
            fake.file_info.insert(result.clone(), Ok(identities));

            if let Ok(history) = &mut fake.history {
                if let Some(first) = history.first_mut() {
                    *first = result.clone();
                }
            }
            let status = fake.status.as_mut().map_err(|error| error.clone())?;
            status.staged_revisions = vec![result.clone()];
            status.scanned_staged_revisions = vec![result.clone()];
            status.post_scan_staged_revisions = vec![result.clone()];
            for file in &mut status.worktree_files {
                file.revision.clone_from(&result);
            }
            Ok(())
        })();
        match applied {
            Ok(()) => {
                self.state
                    .lock()
                    .unwrap()
                    .applied_effects
                    .insert("metadata_set".into());
                if mode == FakeMutationMode::OutcomeUnknownApplied {
                    MutationObservation::OutcomeUnknown {
                        code: "metadata_set outcome unknown".into(),
                        observed: (),
                    }
                } else {
                    MutationObservation::Completed(())
                }
            }
            Err(error) => MutationObservation::OutcomeUnknown {
                code: error.message,
                observed: (),
            },
        }
    }

    async fn storage_open(&self) -> MutationObservation<Vec<u64>> {
        let mut state = self.state.lock().unwrap();
        state.calls.push("open".into());
        state.opens += 1;
        if state.failures.contains("open") {
            return MutationObservation::NotDispatched {
                code: "open".into(),
            };
        }
        if let Some(observation) = state.open_override.clone() {
            return observation;
        }
        let mode = state
            .mutation_modes
            .get("open")
            .copied()
            .unwrap_or_default();
        match mode {
            FakeMutationMode::NotDispatched => MutationObservation::NotDispatched {
                code: "open not dispatched".into(),
            },
            FakeMutationMode::OutcomeUnknownAbsent => MutationObservation::OutcomeUnknown {
                code: "open outcome unknown".into(),
                observed: vec![],
            },
            FakeMutationMode::OutcomeUnknownApplied => {
                state.applied_effects.insert("open".into());
                MutationObservation::OutcomeUnknown {
                    code: "open outcome unknown".into(),
                    observed: vec![],
                }
            }
            FakeMutationMode::Completed => {
                state.applied_effects.insert("open".into());
                MutationObservation::Completed(vec![5934])
            }
        }
    }

    async fn storage_put(
        &self,
        _handle: u64,
        bytes: &[u8],
    ) -> MutationObservation<Vec<ImmutablePutItem>> {
        let mut state = self.state.lock().unwrap();
        state.calls.push("put".into());
        state.puts += 1;
        if state.failures.contains("put") {
            return MutationObservation::NotDispatched { code: "put".into() };
        }
        if let Some(result) = state.put_override.clone() {
            return result;
        }
        let observed = vec![ImmutablePutItem {
            id: 5934,
            address: FAKE_EVIDENCE_ADDRESS.into(),
            ok: true,
        }];
        let mode = state.mutation_modes.get("put").copied().unwrap_or_default();
        let observation = match mode {
            FakeMutationMode::NotDispatched => MutationObservation::NotDispatched {
                code: "put not dispatched".into(),
            },
            FakeMutationMode::OutcomeUnknownAbsent => MutationObservation::OutcomeUnknown {
                code: "put outcome unknown".into(),
                observed,
            },
            FakeMutationMode::OutcomeUnknownApplied => {
                state
                    .stored
                    .insert(FAKE_EVIDENCE_ADDRESS.into(), bytes.to_vec());
                state.applied_effects.insert("put".into());
                MutationObservation::OutcomeUnknown {
                    code: "put outcome unknown".into(),
                    observed,
                }
            }
            FakeMutationMode::Completed => {
                state
                    .stored
                    .insert(FAKE_EVIDENCE_ADDRESS.into(), bytes.to_vec());
                state.applied_effects.insert("put".into());
                MutationObservation::Completed(observed)
            }
        };
        let metadata_mutation = if matches!(&observation, MutationObservation::Completed(_)) {
            state.put_metadata_mutation.take()
        } else {
            None
        };
        drop(state);
        if let Some(entry) = metadata_mutation {
            let mut lore = self.lore.0.lock().unwrap();
            let subject = lore
                .status
                .as_ref()
                .expect("put-time metadata mutation requires status")
                .staged_revisions[0]
                .clone();
            lore.metadata
                .get_mut(&subject)
                .expect("put-time metadata mutation requires subject")
                .as_mut()
                .expect("put-time metadata mutation requires metadata")
                .push(entry);
        }
        observation
    }

    async fn storage_get(
        &self,
        _handle: u64,
        address: &str,
    ) -> ReadObservation<Vec<ImmutableGetItem>> {
        let mut state = self.state.lock().unwrap();
        state.calls.push("get".into());
        state.gets += 1;
        if state.failures.contains("get") {
            return ReadObservation::Unavailable { code: "get".into() };
        }
        if let Some(result) = state.get_override.clone() {
            return result;
        }
        let mut data = state.stored.get(address).cloned().unwrap_or_default();
        if data.is_empty() && !state.stored.contains_key(address) {
            return ReadObservation::Unavailable {
                code: "missing immutable address".into(),
            };
        }
        if state.corrupt_get_bytes && !data.is_empty() {
            data[0] ^= 1;
        }
        let result = vec![ImmutableGetItem {
            id: 5934,
            address: address.into(),
            size: data.len() as u64,
            data,
            ok: true,
        }];
        let mutation = state.get_mutation.take();
        drop(state);
        if let Some(mutation) = mutation {
            let mut lore = self.lore.0.lock().unwrap();
            match mutation {
                GetMutation::Fingerprint => {
                    lore.status.as_mut().unwrap().worktree_files[0].local_hash =
                        "3333333333333333333333333333333333333333333333333333333333333333".into();
                }
                GetMutation::Lock => {
                    lore.lock_queries.insert(
                        "asset.txt".into(),
                        Ok(LockQuery::with_owners(
                            "asset.txt",
                            1,
                            vec!["foreign".into()],
                        )),
                    );
                }
                GetMutation::Subject => {
                    let status = lore.status.as_mut().unwrap();
                    status.staged_revisions = vec!["other".into()];
                    status.scanned_staged_revisions = vec!["other".into()];
                    status.post_scan_staged_revisions = vec!["other".into()];
                }
            }
        }
        ReadObservation::Completed(result)
    }

    async fn storage_close(&self, _handle: u64) -> MutationObservation<()> {
        let mut state = self.state.lock().unwrap();
        state.calls.push("close".into());
        state.closes += 1;
        if state.failures.contains("close") {
            return MutationObservation::NotDispatched {
                code: "close".into(),
            };
        }
        let mode = state
            .mutation_modes
            .get("close")
            .copied()
            .unwrap_or_default();
        match mode {
            FakeMutationMode::NotDispatched => MutationObservation::NotDispatched {
                code: "close not dispatched".into(),
            },
            FakeMutationMode::OutcomeUnknownAbsent => MutationObservation::OutcomeUnknown {
                code: "close outcome unknown".into(),
                observed: (),
            },
            FakeMutationMode::OutcomeUnknownApplied => {
                state.applied_effects.insert("close".into());
                MutationObservation::OutcomeUnknown {
                    code: "close outcome unknown".into(),
                    observed: (),
                }
            }
            FakeMutationMode::Completed => {
                state.applied_effects.insert("close".into());
                MutationObservation::Completed(())
            }
        }
    }
}

fn evidence_request(expected: impl Into<String>) -> EvidencePreserveRequest {
    EvidencePreserveRequest {
        expected_staged_revision: expected.into(),
        target_base_revision: "base".into(),
    }
}

fn expect_rejected(
    outcome: &EvidencePreserveOutcomeV1,
    reason: EvidenceRejectionCodeV1,
) -> &lore_vm::ops::governance::contract::EvidencePreserveRejectedV1 {
    let rejected = outcome
        .rejected()
        .unwrap_or_else(|| panic!("expected rejected_before_publication, got {outcome:?}"));
    assert_eq!(rejected.stopped_at.reason, reason, "{outcome:?}");
    rejected
}

fn expect_residual(
    outcome: &EvidencePreserveOutcomeV1,
    unknown: bool,
    stage: EvidencePreserveStopCodeV1,
) -> &lore_vm::ops::governance::contract::EvidencePreserveResidualV1 {
    match (unknown, outcome.disposition()) {
        (true, EvidencePreserveDispositionV1::Unknown(residual))
        | (false, EvidencePreserveDispositionV1::VerificationIncomplete(residual)) => {
            assert_eq!(residual.stopped_at.stage, stage, "{outcome:?}");
            residual
        }
        _ => panic!("unexpected evidence residual class: {outcome:?}"),
    }
}

fn normalized_outcome_json(outcome: &EvidencePreserveOutcomeV1) -> serde_json::Value {
    let mut value = serde_json::to_value(outcome).expect("outcome remains serializable");
    value["attempt_id"] = serde_json::json!("normalized-attempt-id");
    value
}

fn gate_request(expected: impl Into<String>) -> SubmissionGateCheckRequest {
    SubmissionGateCheckRequest {
        expected_staged_revision: expected.into(),
        target_base_revision: "base".into(),
    }
}

#[tokio::test]
async fn dco_operation_reuses_exact_evaluation_and_closes_on_every_identity_failure() {
    let clean = FakeLore::clean();
    let result = dco_validate_with_adapter(
        &clean,
        &DcoValidateRequest {
            expected_staged_revision: "candidate".into(),
            target_base_revision: "base".into(),
        },
    )
    .await;
    assert!(result.valid, "{result:?}");
    assert_eq!(result.pending_revisions, vec!["candidate"]);

    let cases: [(&str, fn(&mut FakeLore)); 4] = [
        ("mismatch", |fake: &mut FakeLore| {
            fake.authors = Ok(vec![ResolvedAuthor::new("alice", "Mallory")]);
        }),
        ("malformed", |fake: &mut FakeLore| {
            fake.metadata.insert(
                "candidate".into(),
                Ok(vec![
                    MetadataEntry::new("message", "change\n\nSigned-off-by: malformed"),
                    MetadataEntry::new("created-by", "alice"),
                ]),
            );
        }),
        ("duplicate", |fake: &mut FakeLore| {
            fake.authors = Ok(vec![
                ResolvedAuthor::new("alice", "Alice"),
                ResolvedAuthor::new("alice", "Alice"),
            ]);
        }),
        ("unresolved", |fake: &mut FakeLore| {
            fake.authors = Ok(vec![]);
        }),
    ];
    for (name, mutate) in cases {
        let mut fake = FakeLore::clean();
        mutate(&mut fake);
        let result = dco_validate_with_adapter(
            &fake,
            &DcoValidateRequest {
                expected_staged_revision: "candidate".into(),
                target_base_revision: "base".into(),
            },
        )
        .await;
        assert!(!result.valid, "{name}: {result:?}");
        assert_eq!(result.failure_codes, vec!["dco_invalid"], "{name}");
    }

    let mut auth_unavailable = FakeLore::clean();
    auth_unavailable.authors = Err(FakeLore::error("auth dependency"));
    let result = dco_validate_with_adapter(
        &auth_unavailable,
        &DcoValidateRequest {
            expected_staged_revision: "candidate".into(),
            target_base_revision: "base".into(),
        },
    )
    .await;
    assert!(
        !result.valid,
        "unavailable author resolution must close DCO"
    );
    assert_eq!(result.failure_codes, vec!["auth_unavailable"]);

    let mut history_unavailable = FakeLore::clean();
    history_unavailable.history = Err(FakeLore::error("history dependency"));
    let result = dco_validate_with_adapter(
        &history_unavailable,
        &DcoValidateRequest {
            expected_staged_revision: "candidate".into(),
            target_base_revision: "base".into(),
        },
    )
    .await;
    assert!(!result.valid, "unavailable history must close DCO");
    assert_eq!(result.failure_codes, vec!["history_incomplete"]);
}

#[tokio::test]
async fn dco_result_does_not_conflate_later_supersession_lock_or_worktree_policy() {
    let cases: [(&str, fn(&mut FakeLore)); 9] = [
        ("superseded", |fake| {
            let identity = "1111111111111111111111111111111111111111111111111111111111111111:22222222222222222222222222222222";
            fake.metadata
                .get_mut("base")
                .unwrap()
                .as_mut()
                .unwrap()
                .push(MetadataEntry::new(
                    format!("{SUPERSESSION_MARKER_PREFIX}{identity}"),
                    serde_json::to_string(&serde_json::json!({
                        "version": "v1",
                        "identity": identity,
                    }))
                    .unwrap(),
                ));
        }),
        ("locked", |fake| {
            fake.lock_queries.insert(
                "asset.txt".into(),
                Ok(LockQuery::with_owners(
                    "asset.txt",
                    1,
                    vec!["foreign".into()],
                )),
            );
        }),
        ("dirty", |fake| {
            fake.status.as_mut().unwrap().worktree_clean = false;
        }),
        ("malformed supersession", |fake| {
            fake.metadata
                .get_mut("base")
                .unwrap()
                .as_mut()
                .unwrap()
                .push(MetadataEntry::new(
                    format!("{SUPERSESSION_MARKER_PREFIX}1111111111111111111111111111111111111111111111111111111111111111:22222222222222222222222222222222"),
                    r#"{"version":"v1","identity":"wrong"}"#,
                ));
        }),
        ("marker metadata unavailable", |fake| {
            fake.metadata
                .insert("base".into(), Err(FakeLore::error("base marker metadata")));
        }),
        ("worktree scan unverified", |fake| {
            fake.status.as_mut().unwrap().scan_performed = false;
        }),
        ("candidate tree unavailable", |fake| {
            fake.dumps
                .insert("candidate".into(), Err(FakeLore::error("tree")));
        }),
        ("revision diff unavailable", |fake| {
            fake.diff = Err(FakeLore::error("diff"));
        }),
        ("lock dependency unavailable", |fake| {
            fake.lock_queries
                .insert("asset.txt".into(), Err(FakeLore::error("lock")));
        }),
    ];

    for (name, mutate) in cases {
        let mut fake = FakeLore::clean();
        mutate(&mut fake);
        let result = dco_validate_with_adapter(
            &fake,
            &DcoValidateRequest {
                expected_staged_revision: "candidate".into(),
                target_base_revision: "base".into(),
            },
        )
        .await;
        assert!(result.valid, "{name} is not a DCO failure: {result:?}");
        assert_eq!(result.pending_revisions, vec!["candidate"]);
        assert!(result.failure_codes.is_empty(), "{name}: {result:?}");
    }
}

#[tokio::test]
async fn wrong_role_evidence_call_rejects_before_any_repository_or_storage_state() {
    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore.clone(), GovernanceRole::Witness);
    let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .expect("wrong-role rejection is an exact no-effect outcome");
    let rejected = expect_rejected(&outcome, EvidenceRejectionCodeV1::ActorRoleRequired);
    assert!(matches!(rejected.close, EvidenceCloseStateV1::NotOpened));
    assert!(rejected.snapshot_sha256.is_none());

    let state = io.state();
    assert_eq!(
        (state.opens, state.puts, state.gets, state.closes),
        (0, 0, 0, 0)
    );
    assert!(state.metadata_writes.is_empty());
    assert!(state.stored.is_empty());
    let fake = lore.snapshot();
    assert_eq!(
        fake.status.unwrap().staged_revisions,
        vec!["candidate"],
        "wrong-role rejection must leave the exact staged subject untouched"
    );
}

#[tokio::test]
async fn dirty_only_unselected_path_rejects_before_evidence_state() {
    let lore = SharedFakeLore::clean();
    {
        let mut fake = lore.0.lock().unwrap();
        let status = fake.status.as_mut().unwrap();
        status.worktree_clean = false;
        status.worktree_files.push(WorktreeFileObservation {
            path: "unselected.txt".into(),
            revision: "candidate".into(),
            revision_hash: "base-hash".into(),
            revision_context: "other-context".into(),
            revision_size: 4,
            local_hash: "dirty-hash".into(),
            local_size: 5,
            filtered_revision_size: 4,
            flag_modified: true,
            flag_deleted: false,
            flag_added: false,
            flag_conflict: false,
        });
        assert_eq!(status.staged_paths, ["asset.txt"]);
    }
    let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
    let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .expect("a complete dirty fact is a deterministic policy rejection");
    let rejected = expect_rejected(&outcome, EvidenceRejectionCodeV1::InitialGovernanceClosed);
    assert_eq!(rejected.stopped_at.code, "worktree_dirty");
    let state = io.state();
    assert_eq!(
        (state.opens, state.puts, state.gets, state.closes),
        (0, 0, 0, 0)
    );
    assert!(state.metadata_writes.is_empty());
    assert!(state.stored.is_empty());
}

#[tokio::test]
async fn evidence_rereads_the_exact_blob_only_after_pointer_attachment() {
    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);

    evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .expect("clean actor evidence should preserve");

    assert_eq!(
        io.state().calls,
        ["open", "put", "metadata_set", "get", "close"],
        "the immutable reread must bind the pointer after attachment, not merely preflight the put"
    );
}

#[tokio::test]
async fn preattach_reread_rejects_a_concurrent_pointer_before_actor_attach() {
    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
    io.mutate_metadata_on_put(MetadataEntry::new(
        EVIDENCE_POINTER_KEY,
        serde_json::to_string(&EvidencePointerV1 {
            version: "v1".into(),
            address: FAKE_EVIDENCE_ADDRESS.into(),
        })
        .unwrap(),
    ));

    let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .expect("a preattach metadata race remains an exact residual");
    expect_residual(
        &outcome,
        false,
        EvidencePreserveStopCodeV1::PreattachEvaluation,
    );
    let state = io.state();
    assert_eq!(state.metadata_writes.len(), 0, "the actor must not attach");
    assert_eq!(state.gets, 0, "no postattach read may start");
    assert_eq!(state.closes, 1, "the usable handle must still close");
}

#[tokio::test]
async fn actor_evidence_is_canonical_non_authoritative_and_witness_rederives_it() {
    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
    let preserved = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .expect("clean actor evidence should preserve");
    let preserved = preserved.verified().expect("full actor chain closed");

    assert_eq!(preserved.source_staged_revision, "candidate");
    assert_eq!(preserved.evidence_address, FAKE_EVIDENCE_ADDRESS);
    assert_ne!(preserved.result_staged_revision, "candidate");
    let state = io.state();
    assert_eq!(
        (state.opens, state.puts, state.gets, state.closes),
        (1, 1, 1, 1)
    );
    assert_eq!(state.metadata_writes.len(), 1);
    assert_eq!(state.metadata_writes[0].0, EVIDENCE_POINTER_KEY);

    let stored = state.stored.get(FAKE_EVIDENCE_ADDRESS).unwrap();
    let snapshot: CanonicalEvidenceSnapshotV1 =
        serde_json::from_slice(stored).expect("stored bytes use the strict snapshot schema");
    assert_eq!(snapshot.version, "v1");
    assert_eq!(snapshot.target_base_revision, "base");
    assert_eq!(
        snapshot.first_parent_history,
        vec![CanonicalRevisionRefV1::StagedSubject]
    );
    assert_eq!(snapshot.affected_paths, vec!["asset.txt"]);
    assert_eq!(snapshot.candidate_files.len(), 1);
    let staged_metadata = snapshot
        .supersession_metadata_queries
        .iter()
        .find(|query| query.revision == CanonicalRevisionRefV1::StagedSubject)
        .expect("the exact staged metadata query remains retained");
    assert!(
        staged_metadata
            .metadata
            .iter()
            .all(|entry| entry.key != EVIDENCE_POINTER_KEY),
        "the self-referential pointer is represented only by EvidencePointerDeltaV1"
    );
    assert_eq!(
        staged_metadata
            .metadata
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        ["created-by", "message"],
        "all non-pointer raw metadata remains byte-bound"
    );
    assert_eq!(
        snapshot.candidate_files[0].hash,
        "4444444444444444444444444444444444444444444444444444444444444444"
    );
    assert_eq!(
        snapshot.candidate_files[0].context,
        "22222222222222222222222222222222"
    );
    let json = serde_json::to_value(&snapshot).unwrap();
    for forbidden in ["gate_open", "passed", "valid", "verdict"] {
        assert!(
            json.get(forbidden).is_none(),
            "actor claim contained {forbidden}"
        );
    }

    let before_actor_gate = io.state();
    let actor_gate = submission_gate_check_with_adapters(
        &lore,
        &io,
        &gate_request(&preserved.result_staged_revision),
    )
    .await;
    assert!(
        !actor_gate.gate_open,
        "an Actor must never open the witness gate"
    );
    assert_eq!(
        before_actor_gate.metadata_writes,
        io.state().metadata_writes
    );
    assert_eq!(before_actor_gate.stored, io.state().stored);

    let witness = io.with_role(GovernanceRole::Witness);
    let gate = submission_gate_check_with_adapters(
        &lore,
        &witness,
        &gate_request(&preserved.result_staged_revision),
    )
    .await;
    assert!(gate.gate_open, "{gate:?}");
    assert_eq!(gate.criteria.len(), 7);
    assert!(gate.criteria.iter().all(|criterion| criterion.passed));
    assert_eq!(
        gate.criteria
            .iter()
            .map(|criterion| criterion.criterion)
            .collect::<Vec<_>>(),
        vec![
            GovernanceCriterion::ExactSubject,
            GovernanceCriterion::HistoryComplete,
            GovernanceCriterion::DcoValid,
            GovernanceCriterion::NotSuperseded,
            GovernanceCriterion::LocksClear,
            GovernanceCriterion::WorktreeClean,
            GovernanceCriterion::EvidenceValid,
        ]
    );
}

#[tokio::test]
async fn canonical_pointer_omission_is_limited_to_the_current_staged_subject() {
    let lore = SharedFakeLore::clean();
    lore.0
        .lock()
        .unwrap()
        .metadata
        .get_mut("base")
        .unwrap()
        .as_mut()
        .unwrap()
        .push(MetadataEntry::new(
            EVIDENCE_POINTER_KEY,
            "ancestor-pointer-remains-raw",
        ));
    let io = FakeIo::new(lore, GovernanceRole::Actor);
    let preserved = evidence_preserve_with_adapters(&io.lore, &io, &evidence_request("candidate"))
        .await
        .expect("ancestor pointer metadata is not the active evidence pointer");
    preserved.verified().expect("full actor chain closed");
    let state = io.state();
    let snapshot: CanonicalEvidenceSnapshotV1 =
        serde_json::from_slice(state.stored.get(FAKE_EVIDENCE_ADDRESS).unwrap()).unwrap();
    let base_metadata = snapshot
        .supersession_metadata_queries
        .iter()
        .find(|query| query.revision == CanonicalRevisionRefV1::Exact("base".into()))
        .expect("base metadata query remains retained");
    assert!(base_metadata.metadata.iter().any(|entry| {
        entry.key == EVIDENCE_POINTER_KEY && entry.value == "ancestor-pointer-remains-raw"
    }));
}

#[tokio::test]
async fn witness_rejects_a_well_formed_but_forged_actor_snapshot() {
    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
    let preserved = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .unwrap();
    let preserved = preserved.verified().expect("full actor chain closed");
    let mut forged: CanonicalEvidenceSnapshotV1 =
        serde_json::from_slice(io.state().stored.get(FAKE_EVIDENCE_ADDRESS).unwrap()).unwrap();
    forged.candidate_files[0].hash = "forged".into();
    io.replace_stored_bytes(FAKE_EVIDENCE_ADDRESS, serde_json::to_vec(&forged).unwrap());

    let witness = io.with_role(GovernanceRole::Witness);
    let gate = submission_gate_check_with_adapters(
        &lore,
        &witness,
        &gate_request(&preserved.result_staged_revision),
    )
    .await;
    assert!(
        !gate.gate_open,
        "stored actor bytes must never be authority"
    );
    let evidence = gate
        .criteria
        .iter()
        .find(|criterion| criterion.criterion == GovernanceCriterion::EvidenceValid)
        .unwrap();
    assert!(!evidence.passed);
    assert_eq!(evidence.failure_code.as_deref(), Some("evidence_mismatch"));
    assert!(
        gate.criteria[..6].iter().all(|criterion| criterion.passed),
        "the witness must distinguish a forged claim from live governance failures"
    );
}

#[tokio::test]
async fn witness_full_gate_is_inert_to_poisoned_evaluator_summaries() {
    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
    let preserved = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .expect("seed exact actor evidence");
    let preserved = preserved.verified().expect("full actor chain closed");

    let mut poisoned: CanonicalEvidenceSnapshotV1 =
        serde_json::from_slice(io.state().stored.get(FAKE_EVIDENCE_ADDRESS).unwrap()).unwrap();
    poisoned.current_files[0].hash = "9".repeat(64);
    poisoned.revision_diff = vec![AffectedPath::modified("forged-summary.txt")];
    poisoned.affected_paths = vec!["forged-summary.txt".into()];
    poisoned.supersession_markers.push(
        lore_vm::ops::governance::contract::CanonicalSupersessionObservationV1 {
            revision: CanonicalRevisionRefV1::StagedSubject,
            key: format!(
                "{SUPERSESSION_MARKER_PREFIX}{}:{}",
                "9".repeat(64),
                "8".repeat(32)
            ),
            value: "forged-summary".into(),
            identity: format!("{}:{}", "9".repeat(64), "8".repeat(32)),
        },
    );
    poisoned.dco[0].signer_name = "Mallory".into();
    poisoned.dependency_observations = vec!["forged-summary-label".into()];
    io.replace_stored_bytes(
        FAKE_EVIDENCE_ADDRESS,
        serde_json::to_vec(&poisoned).unwrap(),
    );

    let witness = io.with_role(GovernanceRole::Witness);
    let gate = submission_gate_check_with_adapters(
        &lore,
        &witness,
        &gate_request(&preserved.result_staged_revision),
    )
    .await;

    assert!(
        gate.gate_open,
        "derived evaluator summaries in actor evidence must be diagnostic-only: {gate:?}"
    );
    assert!(gate.criteria.iter().all(|criterion| criterion.passed));
}

#[tokio::test]
async fn witness_final_reread_closes_storage_get_fingerprint_lock_and_subject_races() {
    for mutation in [
        GetMutation::Fingerprint,
        GetMutation::Lock,
        GetMutation::Subject,
    ] {
        let lore = SharedFakeLore::clean();
        let actor = FakeIo::new(lore.clone(), GovernanceRole::Actor);
        let preserved =
            evidence_preserve_with_adapters(&lore, &actor, &evidence_request("candidate"))
                .await
                .expect("seed exact actor evidence");
        let preserved = preserved.verified().expect("full actor chain closed");
        let witness = actor.with_role(GovernanceRole::Witness);
        let before = witness.state();
        witness.mutate_on_get(mutation);
        let gate = submission_gate_check_with_adapters(
            &lore,
            &witness,
            &gate_request(&preserved.result_staged_revision),
        )
        .await;
        assert!(!gate.gate_open, "a get-time live race opened: {gate:?}");
        assert!(
            gate.criteria.iter().any(|criterion| !criterion.passed),
            "the final reread must expose the drift"
        );
        let after = witness.state();
        assert_eq!(before.metadata_writes, after.metadata_writes);
        assert_eq!(before.stored, after.stored, "the witness must never write");
    }
}

#[tokio::test]
async fn witness_rejects_an_uppercase_hex_evidence_address_alias() {
    let lore = SharedFakeLore::clean();
    let actor = FakeIo::new(lore.clone(), GovernanceRole::Actor);
    let preserved = evidence_preserve_with_adapters(&lore, &actor, &evidence_request("candidate"))
        .await
        .unwrap();
    let preserved = preserved.verified().expect("full actor chain closed");
    {
        let mut fake = lore.0.lock().unwrap();
        let metadata = fake
            .metadata
            .get_mut(&preserved.result_staged_revision)
            .unwrap()
            .as_mut()
            .unwrap();
        let pointer = metadata
            .iter_mut()
            .find(|entry| entry.key == EVIDENCE_POINTER_KEY)
            .unwrap();
        pointer.value = serde_json::to_string(&EvidencePointerV1 {
            version: "v1".into(),
            address: format!("{}-{}", "A".repeat(64), "0".repeat(32)),
        })
        .unwrap();
    }
    let witness = actor.with_role(GovernanceRole::Witness);
    let gate = submission_gate_check_with_adapters(
        &lore,
        &witness,
        &gate_request(&preserved.result_staged_revision),
    )
    .await;
    assert!(!gate.gate_open);
    assert_eq!(
        gate.criteria.last().unwrap().failure_code.as_deref(),
        Some("evidence_pointer_invalid")
    );
}

#[tokio::test]
async fn witness_compares_exact_stored_dco_lock_and_worktree_observations() {
    let mutations: [(&str, fn(&mut CanonicalEvidenceSnapshotV1)); 3] = [
        ("dco", |snapshot| {
            snapshot.dco_metadata[0].messages[0].push_str("\nforged")
        }),
        ("lock", |snapshot| {
            snapshot.lock_queries[0].owners.push("forged".into())
        }),
        ("worktree", |snapshot| {
            snapshot.status.worktree_clean = false
        }),
    ];
    for (name, mutate) in mutations {
        let lore = SharedFakeLore::clean();
        let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
        let preserved = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
            .await
            .unwrap();
        let preserved = preserved.verified().expect("full actor chain closed");
        let mut forged: CanonicalEvidenceSnapshotV1 =
            serde_json::from_slice(io.state().stored.get(FAKE_EVIDENCE_ADDRESS).unwrap()).unwrap();
        mutate(&mut forged);
        io.replace_stored_bytes(FAKE_EVIDENCE_ADDRESS, serde_json::to_vec(&forged).unwrap());

        let witness = io.with_role(GovernanceRole::Witness);
        let gate = submission_gate_check_with_adapters(
            &lore,
            &witness,
            &gate_request(&preserved.result_staged_revision),
        )
        .await;
        assert!(!gate.gate_open, "{name}: {gate:?}");
        assert_eq!(
            gate.criteria.last().unwrap().failure_code.as_deref(),
            Some("evidence_mismatch"),
            "{name}: every raw stored fact must compare exactly"
        );
    }
}

#[tokio::test]
async fn post_attach_dco_lock_and_worktree_changes_each_falsify_equivalence() {
    let cases: [(&str, GovernanceCriterion, &str, fn(&mut FakeLore, &str)); 3] = [
        (
            "dco",
            GovernanceCriterion::DcoValid,
            "dco_invalid",
            |fake: &mut FakeLore, current: &str| {
                fake.metadata.insert(
                    current.into(),
                    Ok(vec![
                        MetadataEntry::new("message", "change\n\nSigned-off-by: malformed"),
                        MetadataEntry::new("created-by", "alice"),
                    ]),
                );
            },
        ),
        (
            "lock",
            GovernanceCriterion::LocksClear,
            "locks_clear_failed",
            |fake: &mut FakeLore, _current: &str| {
                fake.lock_queries.insert(
                    "asset.txt".into(),
                    Ok(LockQuery::with_owners(
                        "asset.txt",
                        1,
                        vec!["foreign".into()],
                    )),
                );
            },
        ),
        (
            "worktree",
            GovernanceCriterion::WorktreeClean,
            "worktree_dirty",
            |fake: &mut FakeLore, _current: &str| {
                fake.status.as_mut().unwrap().worktree_clean = false;
            },
        ),
    ];
    for (name, criterion, failure_code, mutate) in cases {
        let lore = SharedFakeLore::clean();
        let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
        let preserved = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
            .await
            .unwrap();
        let preserved = preserved.verified().expect("full actor chain closed");
        mutate(
            &mut lore.0.lock().unwrap(),
            &preserved.result_staged_revision,
        );

        let witness = io.with_role(GovernanceRole::Witness);
        let gate = submission_gate_check_with_adapters(
            &lore,
            &witness,
            &gate_request(&preserved.result_staged_revision),
        )
        .await;
        assert!(!gate.gate_open, "{name}: {gate:?}");
        let observed = gate
            .criteria
            .iter()
            .find(|item| item.criterion == criterion)
            .unwrap();
        assert!(!observed.passed, "{name}: {gate:?}");
        assert_eq!(
            observed.failure_code.as_deref(),
            Some(failure_code),
            "{name}"
        );
        assert!(
            !gate
                .criteria
                .iter()
                .find(|item| item.criterion == GovernanceCriterion::EvidenceValid)
                .unwrap()
                .passed,
            "{name}: a mandatory live-fact change must invalidate equivalence"
        );
    }
}

#[tokio::test]
async fn pointer_delta_rejects_same_text_with_a_different_metadata_kind() {
    let lore = SharedFakeLore::clean();
    lore.0
        .lock()
        .unwrap()
        .metadata
        .get_mut("candidate")
        .unwrap()
        .as_mut()
        .unwrap()
        .push(MetadataEntry::new("typed", "1"));
    let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
    io.drift_metadata_type_on_set();

    let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .expect("metadata drift is an exact residual state");
    let residual = expect_residual(
        &outcome,
        false,
        EvidencePreserveStopCodeV1::PointerDeltaInvalid,
    );
    assert!(matches!(
        residual.last_confirmed_publication,
        EvidencePublicationStateV1::ResultSubjectObserved { .. }
    ));
    assert!(matches!(residual.close, EvidenceCloseStateV1::Closed));
    assert_eq!(io.state().closes, 1);
}

#[tokio::test]
async fn evidence_failures_report_exact_residuals_and_always_close_a_usable_handle() {
    let cases = [
        (
            "put",
            EvidencePreserveStopCodeV1::StoragePutNotDispatched,
            "none",
        ),
        (
            "metadata_set",
            EvidencePreserveStopCodeV1::PointerAttachNotDispatched,
            "blob_published",
        ),
        (
            "get",
            EvidencePreserveStopCodeV1::PostattachGetUnavailable,
            "pointer_attach_acknowledged",
        ),
        (
            "close",
            EvidencePreserveStopCodeV1::StorageCloseNotDispatched,
            "postattach_equivalent",
        ),
    ];
    for (dependency, expected_stop, expected_publication) in cases {
        let lore = SharedFakeLore::clean();
        let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
        io.fail(dependency);
        let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
            .await
            .expect("released failure is represented in-band");
        let residual = expect_residual(&outcome, false, expected_stop);
        assert!(
            serde_json::to_value(&residual.last_confirmed_publication).unwrap()["state"]
                .as_str()
                .is_some_and(|state| state == expected_publication),
            "{dependency}: {outcome:?}"
        );
        if dependency == "close" {
            assert!(matches!(
                residual.close,
                EvidenceCloseStateV1::CloseNotDispatched { .. }
            ));
        } else {
            assert!(matches!(residual.close, EvidenceCloseStateV1::Closed));
        }
        let state = io.state();
        assert_eq!(state.opens, 1, "{dependency}");
        assert_eq!(state.closes, 1, "{dependency}: every opened handle closes");
    }

    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore, GovernanceRole::Actor);
    io.fail("open");
    let outcome = evidence_preserve_with_adapters(&io.lore, &io, &evidence_request("candidate"))
        .await
        .expect("open predispatch failure is representable");
    let residual = expect_residual(
        &outcome,
        false,
        EvidencePreserveStopCodeV1::StorageOpenNotDispatched,
    );
    assert!(matches!(residual.close, EvidenceCloseStateV1::NotOpened));
    assert_eq!(io.state().closes, 0, "no handle existed to close");
}

#[tokio::test]
async fn empty_public_adapter_diagnostic_is_a_typed_stop_not_a_panic() {
    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
    io.set_open_override(MutationObservation::NotDispatched {
        code: String::new(),
    });

    let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .expect("malformed adapter diagnostics remain an in-band residual state");
    let residual = expect_residual(
        &outcome,
        false,
        EvidencePreserveStopCodeV1::StorageOpenNotDispatched,
    );
    assert_eq!(
        residual.stopped_at.code,
        "storage_open_not_dispatched_without_code"
    );
    assert!(matches!(
        residual.last_confirmed_publication,
        EvidencePublicationStateV1::None
    ));
    assert!(matches!(residual.close, EvidenceCloseStateV1::NotOpened));
    assert_eq!(io.state().closes, 0);
}

#[tokio::test]
async fn completed_open_without_exactly_one_usable_handle_is_open_outcome_unknown() {
    for handles in [vec![], vec![0], vec![5934, 5935]] {
        let lore = SharedFakeLore::clean();
        let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
        io.set_open_override(MutationObservation::Completed(handles));
        let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
            .await
            .expect("unusable completed-open data remains an in-band unknown");
        let residual = expect_residual(
            &outcome,
            true,
            EvidencePreserveStopCodeV1::StorageOpenOutcome,
        );
        assert!(matches!(
            residual.last_confirmed_publication,
            EvidencePublicationStateV1::None
        ));
        assert!(matches!(
            residual.close,
            EvidenceCloseStateV1::OpenOutcomeUnknown { .. }
        ));
        assert_eq!(io.state().closes, 0);
    }
}

#[tokio::test]
async fn direct_empty_evidence_request_cannot_create_an_attempt_or_touch_io() {
    for request in [
        EvidencePreserveRequest {
            expected_staged_revision: String::new(),
            target_base_revision: "base".into(),
        },
        EvidencePreserveRequest {
            expected_staged_revision: "candidate".into(),
            target_base_revision: String::new(),
        },
    ] {
        let lore = SharedFakeLore::clean();
        let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
        let error = evidence_preserve_with_adapters(&lore, &io, &request)
            .await
            .expect_err("empty exact revisions are pre-attempt contract errors");
        assert!(error.to_string().contains("nonempty exact revisions"));
        let state = io.state();
        assert!(state.calls.is_empty());
        assert!(state.metadata_writes.is_empty());
        assert!(state.stored.is_empty());
    }
}

#[tokio::test]
async fn unknown_mutation_effects_serialize_identically_whether_hidden_effect_applied_or_absent() {
    let cases = [
        (
            "open",
            EvidencePreserveStopCodeV1::StorageOpenOutcome,
            "none",
        ),
        ("put", EvidencePreserveStopCodeV1::StoragePutOutcome, "none"),
        (
            "metadata_set",
            EvidencePreserveStopCodeV1::PointerAttachOutcome,
            "blob_published",
        ),
        (
            "close",
            EvidencePreserveStopCodeV1::StorageCloseOutcome,
            "postattach_equivalent",
        ),
    ];

    for (dependency, expected_stop, expected_publication) in cases {
        let mut public_outcomes = Vec::new();
        for (mode, hidden_applied) in [
            (FakeMutationMode::OutcomeUnknownAbsent, false),
            (FakeMutationMode::OutcomeUnknownApplied, true),
        ] {
            let lore = SharedFakeLore::clean();
            let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
            io.set_mutation_mode(dependency, mode);

            let outcome =
                evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
                    .await
                    .expect("unknown effects remain an in-band lower bound");
            let residual = expect_residual(&outcome, true, expected_stop);
            assert_eq!(
                serde_json::to_value(&residual.last_confirmed_publication).unwrap()["state"],
                expected_publication,
                "{dependency}: wrong lower-bound publication state"
            );
            assert_eq!(
                io.state().applied_effects.contains(dependency),
                hidden_applied,
                "{dependency}: fake did not establish the hidden-world distinction"
            );
            match dependency {
                "open" => assert!(matches!(
                    residual.close,
                    EvidenceCloseStateV1::OpenOutcomeUnknown { .. }
                )),
                "close" => assert!(matches!(
                    residual.close,
                    EvidenceCloseStateV1::CloseOutcomeUnknown { .. }
                )),
                "put" | "metadata_set" => {
                    assert!(matches!(residual.close, EvidenceCloseStateV1::Closed))
                }
                _ => unreachable!("fixed effect-matrix dependency"),
            }
            public_outcomes.push(normalized_outcome_json(&outcome));
        }
        assert_eq!(
            public_outcomes[0], public_outcomes[1],
            "{dependency}: public residual reconstructed an unknowable hidden effect"
        );
    }
}

#[tokio::test]
async fn close_cross_product_preserves_primary_stop_and_only_full_closed_chain_verifies() {
    for stage in ["ready", "incomplete", "unknown"] {
        for close_mode in [
            FakeMutationMode::Completed,
            FakeMutationMode::NotDispatched,
            FakeMutationMode::OutcomeUnknownAbsent,
        ] {
            let lore = SharedFakeLore::clean();
            let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
            match stage {
                "ready" => {}
                "incomplete" => io.fail("get"),
                "unknown" => io.set_mutation_mode("put", FakeMutationMode::OutcomeUnknownAbsent),
                _ => unreachable!("fixed close-matrix stage"),
            }
            io.set_mutation_mode("close", close_mode);

            let outcome =
                evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
                    .await
                    .expect("every close observation has an exact in-band result");
            if stage == "ready" && close_mode == FakeMutationMode::Completed {
                let verified = outcome
                    .verified()
                    .expect("only the full chain plus Closed is Verified");
                assert!(matches!(verified.close, EvidenceCloseStateV1::Closed));
                continue;
            }

            let expected_unknown =
                stage == "unknown" || close_mode == FakeMutationMode::OutcomeUnknownAbsent;
            let expected_stop = match stage {
                "ready" if close_mode == FakeMutationMode::NotDispatched => {
                    EvidencePreserveStopCodeV1::StorageCloseNotDispatched
                }
                "ready" => EvidencePreserveStopCodeV1::StorageCloseOutcome,
                "incomplete" => EvidencePreserveStopCodeV1::PostattachGetUnavailable,
                "unknown" => EvidencePreserveStopCodeV1::StoragePutOutcome,
                _ => unreachable!("full ready case continued above"),
            };
            let residual = expect_residual(&outcome, expected_unknown, expected_stop);
            match close_mode {
                FakeMutationMode::Completed => {
                    assert!(matches!(residual.close, EvidenceCloseStateV1::Closed))
                }
                FakeMutationMode::NotDispatched => assert!(matches!(
                    residual.close,
                    EvidenceCloseStateV1::CloseNotDispatched { .. }
                )),
                FakeMutationMode::OutcomeUnknownAbsent => assert!(matches!(
                    residual.close,
                    EvidenceCloseStateV1::CloseOutcomeUnknown { .. }
                )),
                FakeMutationMode::OutcomeUnknownApplied => {
                    unreachable!("applied/absent close uncertainty is covered separately")
                }
            }
        }
    }
}

#[tokio::test]
async fn retry_after_unknown_applied_pointer_attach_rejects_existing_pointer_without_resume() {
    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
    io.set_mutation_mode("metadata_set", FakeMutationMode::OutcomeUnknownApplied);
    let first = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .expect("unknown applied pointer attach remains in-band");
    expect_residual(
        &first,
        true,
        EvidencePreserveStopCodeV1::PointerAttachOutcome,
    );
    let result_subject = lore
        .snapshot()
        .status
        .unwrap()
        .staged_revisions
        .into_iter()
        .next()
        .expect("the hidden applied branch advanced the staged subject");
    let before_retry = io.state();

    let retry = evidence_preserve_with_adapters(
        &lore,
        &io,
        &EvidencePreserveRequest {
            expected_staged_revision: result_subject,
            target_base_revision: "base".into(),
        },
    )
    .await
    .expect("retry returns an exact deterministic rejection");
    let rejected = expect_rejected(&retry, EvidenceRejectionCodeV1::PointerAlreadyPresent);
    assert_eq!(rejected.stopped_at.code, "evidence pointer already present");
    let after_retry = io.state();
    assert_eq!(
        after_retry.opens, before_retry.opens,
        "retry must not resume"
    );
    assert_eq!(
        after_retry.puts, before_retry.puts,
        "retry must not republish"
    );
    assert_eq!(
        after_retry.metadata_writes, before_retry.metadata_writes,
        "retry must not attach another pointer"
    );
}

#[tokio::test]
async fn evidence_rejects_ambiguous_put_and_get_streams_and_wrong_bytes() {
    let valid_put = ImmutablePutItem {
        id: 5934,
        address: FAKE_EVIDENCE_ADDRESS.into(),
        ok: true,
    };
    for items in [vec![], vec![valid_put.clone(), valid_put.clone()]] {
        let lore = SharedFakeLore::clean();
        let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
        let addresses: Vec<_> = items.iter().map(|item| item.address.clone()).collect();
        io.set_put_override(Ok(items));
        let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
            .await
            .unwrap();
        let residual = expect_residual(
            &outcome,
            true,
            EvidencePreserveStopCodeV1::StoragePutResponseMalformed,
        );
        assert_eq!(residual.observed_candidate_addresses, addresses);
        assert!(matches!(
            residual.last_confirmed_publication,
            EvidencePublicationStateV1::None
        ));
        assert_eq!(io.state().closes, 1);
        assert!(io.state().metadata_writes.is_empty());
    }

    for invalid in [
        ImmutablePutItem {
            id: 9,
            ..valid_put.clone()
        },
        ImmutablePutItem {
            address: "not-an-address".into(),
            ..valid_put.clone()
        },
        ImmutablePutItem {
            address: format!("{}-{}", "A".repeat(64), "0".repeat(32)),
            ..valid_put.clone()
        },
        ImmutablePutItem {
            ok: false,
            ..valid_put.clone()
        },
    ] {
        let lore = SharedFakeLore::clean();
        let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
        let address = invalid.address.clone();
        io.set_put_override(Ok(vec![invalid]));
        let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
            .await
            .unwrap();
        let residual = expect_residual(
            &outcome,
            true,
            EvidencePreserveStopCodeV1::StoragePutResponseMalformed,
        );
        assert_eq!(residual.observed_candidate_addresses, [address]);
        assert_eq!(io.state().closes, 1);
        assert!(io.state().metadata_writes.is_empty());
    }

    for get in [
        vec![],
        vec![
            ImmutableGetItem {
                id: 5934,
                address: FAKE_EVIDENCE_ADDRESS.into(),
                size: 1,
                data: vec![1],
                ok: true,
            },
            ImmutableGetItem {
                id: 5934,
                address: FAKE_EVIDENCE_ADDRESS.into(),
                size: 1,
                data: vec![1],
                ok: true,
            },
        ],
        vec![ImmutableGetItem {
            id: 7,
            address: FAKE_EVIDENCE_ADDRESS.into(),
            size: 1,
            data: vec![1],
            ok: true,
        }],
        vec![ImmutableGetItem {
            id: 5934,
            address: format!("{}-{}", "b".repeat(64), "0".repeat(32)),
            size: 1,
            data: vec![1],
            ok: true,
        }],
        vec![ImmutableGetItem {
            id: 5934,
            address: FAKE_EVIDENCE_ADDRESS.into(),
            size: 99,
            data: vec![1],
            ok: true,
        }],
        vec![ImmutableGetItem {
            id: 5934,
            address: FAKE_EVIDENCE_ADDRESS.into(),
            size: 1,
            data: vec![1],
            ok: false,
        }],
    ] {
        let lore = SharedFakeLore::clean();
        let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
        io.set_get_override(Ok(get));
        let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
            .await
            .unwrap();
        let residual = expect_residual(
            &outcome,
            false,
            EvidencePreserveStopCodeV1::PostattachGetMalformed,
        );
        assert!(matches!(
            residual.last_confirmed_publication,
            EvidencePublicationStateV1::PointerAttachAcknowledged { .. }
        ));
        assert_eq!(io.state().closes, 1);
        assert_eq!(io.state().metadata_writes.len(), 1);
    }

    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore.clone(), GovernanceRole::Actor);
    io.corrupt_get_bytes();
    let outcome = evidence_preserve_with_adapters(&lore, &io, &evidence_request("candidate"))
        .await
        .unwrap();
    let residual = expect_residual(
        &outcome,
        false,
        EvidencePreserveStopCodeV1::PostattachBytesMismatch,
    );
    assert!(matches!(
        residual.last_confirmed_publication,
        EvidencePublicationStateV1::PointerAttachAcknowledged { .. }
    ));
    assert_eq!(io.state().closes, 1, "same-size wrong bytes are fatal");
    assert_eq!(
        io.state().metadata_writes.len(),
        1,
        "the exact pointer attach precedes the required postattach readback"
    );
}

#[tokio::test]
async fn gate_always_returns_the_exact_inventory_and_closes_missing_evidence() {
    let lore = SharedFakeLore::clean();
    let io = FakeIo::new(lore.clone(), GovernanceRole::Witness);
    let gate = submission_gate_check_with_adapters(&lore, &io, &gate_request("candidate")).await;
    assert!(!gate.gate_open);
    assert_eq!(gate.criteria.len(), 7);
    let mut unique = BTreeSet::new();
    for criterion in &gate.criteria {
        assert!(unique.insert(criterion.criterion));
    }
    assert_eq!(
        gate.criteria
            .last()
            .map(|criterion| criterion.failure_code.as_deref()),
        Some(Some("evidence_pointer_missing"))
    );
}

#[test]
fn traverses_both_merge_parents_and_rejects_depth_overflow() {
    let mut merge = FakeLore::clean();
    merge.infos.insert(
        "candidate".into(),
        Ok(RevisionInfoResponse::exact(RevisionInfo {
            revision: "candidate".into(),
            parents: vec!["left".into(), "right".into()],
        })),
    );
    for revision in ["left", "right"] {
        merge.infos.insert(
            revision.into(),
            Ok(RevisionInfoResponse::exact(RevisionInfo {
                revision: revision.into(),
                parents: vec!["base".into()],
            })),
        );
        merge.metadata.insert(
            revision.into(),
            Ok(vec![
                MetadataEntry::new(
                    "message",
                    "change\n\nSigned-off-by: Alice <alice@example.test>",
                ),
                MetadataEntry::new("created-by", "alice"),
            ]),
        );
    }
    merge.history = Ok(vec!["candidate".into(), "left".into()]);

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&merge, &request()));
    assert!(result.open, "{result:?}");
    assert_eq!(result.pending_revisions, vec!["candidate", "left", "right"]);

    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.history = Ok(vec!["x".to_string(); 1001]),
        "history_incomplete",
    );
}

#[test]
fn separate_pending_and_whole_ancestry_ceilings_map_exact_remediation() {
    for count in [500_usize, 999] {
        let fake = FakeLore::with_linear_pending_count(count);
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(evaluate(&fake, &request()));
        assert!(result.open, "count {count}: {result:?}");
        assert!(result.remediation.is_none());
        if count == 999 {
            assert_eq!(
                result.observations.supersession_ancestry.len(),
                MAX_GOVERNANCE_HISTORY_REVISIONS,
                "whole ancestry accepts exactly 1000 unique revisions"
            );
        }
    }

    let exact_pending = FakeLore::with_linear_pending_count(1000);
    let dco = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(dco_validate_with_adapter(
            &exact_pending,
            &DcoValidateRequest {
                expected_staged_revision: "candidate".into(),
                target_base_revision: "base".into(),
            },
        ));
    assert!(dco.valid, "exact pending ceiling: {dco:?}");
    assert!(dco.remediation.is_none());

    let ancestry_overflow = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&exact_pending, &request()));
    assert!(!ancestry_overflow.open, "{ancestry_overflow:?}");
    assert_eq!(
        ancestry_overflow.observations.supersession_ancestry.len(),
        MAX_GOVERNANCE_HISTORY_REVISIONS + 1,
        "whole ancestry retains its exact N+1 sentinel graph"
    );
    assert_eq!(
        ancestry_overflow.observations.history_overflow_scope,
        Some(HistoryOverflowScope::SupersessionAncestry)
    );
    assert_eq!(
        ancestry_overflow.remediation,
        Some(GovernanceRemediation {
            code: GovernanceRemediationCode::MigrateSupersessionIndex,
            ticket: Some("SBAI-6010".into()),
        })
    );
}

#[test]
fn enforces_1000_unique_pending_limit_across_second_parent_dag() {
    let exact_limit = FakeLore::with_second_parent_pending_count(1000);
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(dco_validate_with_adapter(
            &exact_limit,
            &DcoValidateRequest {
                expected_staged_revision: "candidate".into(),
                target_base_revision: "base".into(),
            },
        ));
    assert!(
        result.valid,
        "exactly 1000 unique pending nodes: {result:?}"
    );

    let overflow = FakeLore::with_second_parent_pending_count(1001);
    let queries = overflow.info_queries.clone();
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(dco_validate_with_adapter(
            &overflow,
            &DcoValidateRequest {
                expected_staged_revision: "candidate".into(),
                target_base_revision: "base".into(),
            },
        ));
    assert!(!result.valid, "{result:?}");
    assert_eq!(result.failure_codes, ["history_depth_exceeded"]);
    assert_eq!(result.pending_revisions.len(), 1001);
    assert_eq!(
        result.remediation,
        Some(GovernanceRemediation {
            code: GovernanceRemediationCode::SplitSubmissionOrAdvanceTargetBase,
            ticket: None,
        })
    );
    let evaluation = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&overflow, &request()));
    assert_eq!(evaluation.failure_codes, ["history_depth_exceeded"]);
    assert_eq!(
        evaluation.observations.history_overflow_scope,
        Some(HistoryOverflowScope::PendingDco)
    );
    assert_eq!(
        evaluation.observations.revision_graph.len(),
        MAX_GOVERNANCE_HISTORY_REVISIONS + 1
    );
    assert!(evaluation.observations.supersession_ancestry.is_empty());
    assert_eq!(evaluation.remediation, result.remediation);
    let queries = queries.lock().unwrap();
    assert!(queries.iter().any(|revision| revision == "side-1000"));
    assert!(!queries.iter().any(|revision| revision == "side-1001"));
}

#[test]
fn malformed_base_never_inherits_pending_overflow_remediation() {
    let malformed_parents = [
        vec!["older".into(), "older".into()],
        vec!["one".into(), "two".into(), "three".into()],
        vec!["base".into()],
    ];
    for parents in malformed_parents {
        let mut fake = FakeLore::with_second_parent_pending_count(1001);
        fake.infos.insert(
            "base".into(),
            Ok(RevisionInfoResponse::exact(RevisionInfo {
                revision: "base".into(),
                parents,
            })),
        );

        let evaluation = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(evaluate(&fake, &request()));
        assert_eq!(evaluation.failure_codes, ["history_incomplete"]);
        assert!(evaluation.remediation.is_none());
        assert!(evaluation.observations.revision_graph.is_empty());

        let dco = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(dco_validate_with_adapter(
                &fake,
                &DcoValidateRequest {
                    expected_staged_revision: "candidate".into(),
                    target_base_revision: "base".into(),
                },
            ));
        assert_eq!(dco.failure_codes, ["history_incomplete"]);
        assert!(dco.remediation.is_none());
        assert!(dco.pending_revisions.is_empty());
    }
}

#[test]
fn genuine_pending_and_whole_ancestry_cycles_fail_before_any_remediation() {
    let mut pending_cycle = FakeLore::clean();
    pending_cycle.infos.insert(
        "candidate".into(),
        Ok(RevisionInfoResponse::exact(RevisionInfo {
            revision: "candidate".into(),
            parents: vec!["loop".into()],
        })),
    );
    pending_cycle.infos.insert(
        "loop".into(),
        Ok(RevisionInfoResponse::exact(RevisionInfo {
            revision: "loop".into(),
            parents: vec!["candidate".into()],
        })),
    );
    let evaluation = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&pending_cycle, &request()));
    assert_eq!(evaluation.failure_codes, ["history_incomplete"]);
    assert!(evaluation.remediation.is_none());
    let dco = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(dco_validate_with_adapter(
            &pending_cycle,
            &DcoValidateRequest {
                expected_staged_revision: "candidate".into(),
                target_base_revision: "base".into(),
            },
        ));
    assert_eq!(dco.failure_codes, ["history_incomplete"]);
    assert!(dco.remediation.is_none());

    let mut ancestry_cycle = FakeLore::clean();
    ancestry_cycle.infos.insert(
        "base".into(),
        Ok(RevisionInfoResponse::exact(RevisionInfo {
            revision: "base".into(),
            parents: vec!["older".into()],
        })),
    );
    ancestry_cycle.infos.insert(
        "older".into(),
        Ok(RevisionInfoResponse::exact(RevisionInfo {
            revision: "older".into(),
            parents: vec!["base".into()],
        })),
    );
    let evaluation = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&ancestry_cycle, &request()));
    assert_eq!(evaluation.failure_codes, ["history_incomplete"]);
    assert!(evaluation.remediation.is_none());
    assert_eq!(evaluation.observations.first_parent_history, ["candidate"]);
}

#[test]
fn closes_for_status_and_history_graph_failures() {
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.status = Err(FakeLore::error("status")),
        "status_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.status.as_mut().unwrap().staged_revisions.clear();
        },
        "exact_subject_failed",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.status.as_mut().unwrap().worktree_clean = false,
        "worktree_dirty",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.status
                .as_mut()
                .unwrap()
                .staged_revisions
                .push("extra".into());
        },
        "exact_subject_failed",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.status.as_mut().unwrap().staged_revisions = vec!["other".into()];
        },
        "exact_subject_failed",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos
                .insert("base".into(), Err(FakeLore::error("base")));
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "candidate".into(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: "candidate".into(),
                    parents: vec!["base".into(), "base".into()],
                })),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "candidate".into(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: "candidate".into(),
                    parents: vec!["base".into(), "left".into(), "right".into()],
                })),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "base".into(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: "base".into(),
                    parents: vec!["older".into(), "older".into()],
                })),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "base".into(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: "base".into(),
                    parents: vec!["one".into(), "two".into(), "three".into()],
                })),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "base".into(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: "fallback".into(),
                    parents: vec![],
                })),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "candidate".into(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: "wrong".into(),
                    parents: vec!["base".into()],
                })),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "candidate".into(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: "candidate".into(),
                    parents: vec!["candidate".into()],
                })),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "candidate".into(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: "candidate".into(),
                    parents: vec![],
                })),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "candidate".into(),
                Ok(RevisionInfoResponse::exact(RevisionInfo {
                    revision: "candidate".into(),
                    parents: vec!["unreadable-parent".into()],
                })),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.history = Err(FakeLore::error("history")),
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.history = Ok(vec![]),
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.history = Ok(vec!["candidate".into(), "candidate".into()]),
        "history_incomplete",
    );
}

#[test]
fn closes_when_status_cannot_prove_a_full_scan_or_stable_exact_subject() {
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.status.as_mut().unwrap().scan_performed = false,
        "worktree_unverified",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.status.as_mut().unwrap().scanned_staged_revisions = vec!["other".into()],
        "exact_subject_failed",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.status.as_mut().unwrap().post_scan_staged_revisions = vec!["other".into()],
        "exact_subject_failed",
    );
}

#[test]
fn missing_deleted_conflicting_duplicate_or_incomplete_fingerprints_close() {
    let cases: [fn(&mut FakeLore); 5] = [
        |fake| fake.status.as_mut().unwrap().worktree_files.clear(),
        |fake| fake.status.as_mut().unwrap().worktree_files[0].flag_deleted = true,
        |fake| fake.status.as_mut().unwrap().worktree_files[0].flag_conflict = true,
        |fake| {
            let duplicate = fake.status.as_ref().unwrap().worktree_files[0].clone();
            fake.status.as_mut().unwrap().worktree_files.push(duplicate);
        },
        |fake| {
            fake.status.as_mut().unwrap().worktree_files[0]
                .local_hash
                .clear()
        },
    ];
    for mutate in cases {
        assert_closed_for(FakeLore::clean(), mutate, "worktree_dirty");
    }
}

#[test]
fn ambiguous_repeated_staged_status_endpoint_closes_instead_of_deduplicating() {
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.status
                .as_mut()
                .unwrap()
                .staged_changes
                .push(StagedPathObservation {
                    path: "asset.txt".into(),
                    from_path: None,
                    action: GovernancePathAction::Add,
                    dirty: true,
                    conflict: false,
                });
        },
        "affected_paths_unavailable",
    );
}

#[test]
fn fake_status_cannot_exceed_pinned_production_endpoint_shapes() {
    for action in [
        GovernancePathAction::Modify,
        GovernancePathAction::Add,
        GovernancePathAction::Delete,
    ] {
        assert_closed_for(
            FakeLore::clean(),
            |fake| {
                let status = fake.status.as_mut().unwrap();
                status.staged_paths = vec!["asset.txt".into(), "fabricated-source.txt".into()];
                status.staged_changes[0].action = action;
                status.staged_changes[0].from_path = Some("fabricated-source.txt".into());
            },
            "affected_paths_unavailable",
        );
    }

    for from_path in [None, Some(String::new()), Some("asset.txt".into())] {
        assert_closed_for(
            FakeLore::clean(),
            |fake| {
                let status = fake.status.as_mut().unwrap();
                status.staged_paths = vec!["asset.txt".into()];
                status.staged_changes[0].action = GovernancePathAction::Move;
                status.staged_changes[0].from_path = from_path;
            },
            "affected_paths_unavailable",
        );
    }
}

#[test]
fn delete_readd_cannot_reuse_the_old_context_as_an_aba_identity() {
    let mut fake = FakeLore::clean();
    fake.dumps
        .insert("base".into(), Ok(vec!["asset.txt".into()]));
    fake.file_info.insert(
        "base".into(),
        Ok(vec![FileIdentity::new(
            "asset.txt",
            "base",
            "1111111111111111111111111111111111111111111111111111111111111111",
            "22222222222222222222222222222222",
        )]),
    );
    fake.status.as_mut().unwrap().staged_changes = vec![StagedPathObservation {
        path: "asset.txt".into(),
        from_path: None,
        action: GovernancePathAction::Add,
        dirty: true,
        conflict: false,
    }];
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&fake, &request()));
    assert_eq!(
        result.failure_codes,
        ["affected_paths_unavailable"],
        "an add event that reuses the old file ID must never collapse into the old identity"
    );
}

#[test]
fn raw_revision_info_boundary_rejects_zero_duplicate_and_over_counted_responses() {
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "candidate".into(),
                Ok(RevisionInfoResponse { revisions: vec![] }),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            let info = RevisionInfo {
                revision: "candidate".into(),
                parents: vec!["base".into()],
            };
            fake.infos.insert(
                "candidate".into(),
                Ok(RevisionInfoResponse {
                    revisions: vec![info.clone(), info],
                }),
            );
        },
        "history_incomplete",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.infos.insert(
                "candidate".into(),
                Ok(RevisionInfoResponse {
                    revisions: vec![
                        RevisionInfo {
                            revision: "candidate".into(),
                            parents: vec!["base".into()],
                        },
                        RevisionInfo {
                            revision: "unexpected".into(),
                            parents: vec![],
                        },
                    ],
                }),
            );
        },
        "history_incomplete",
    );
}

#[test]
fn closes_for_metadata_supersession_and_dco_dependency_failures() {
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.metadata
                .insert("candidate".into(), Err(FakeLore::error("metadata")));
        },
        "metadata_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.metadata.insert(
                "candidate".into(),
                Ok(vec![
                    MetadataEntry::new("message", "x\n\nSigned-off-by: Alice <alice@example.test>"),
                    MetadataEntry::new("message", "duplicate"),
                    MetadataEntry::new("created-by", "alice"),
                ]),
            );
        },
        "dco_invalid",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.metadata.insert(
                "candidate".into(),
                Ok(vec![
                    MetadataEntry::new("message", "x"),
                    MetadataEntry::new("created-by", "alice"),
                ]),
            );
        },
        "dco_invalid",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.metadata.insert(
                "candidate".into(),
                Ok(vec![
                    MetadataEntry::new(
                        "message",
                        "x\n\nSigned-off-by: Mallory <mallory@example.test>",
                    ),
                    MetadataEntry::new("created-by", "alice"),
                ]),
            );
        },
        "dco_invalid",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.authors = Err(FakeLore::error("auth")),
        "auth_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.authors = Ok(vec![]),
        "dco_invalid",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.authors = Ok(vec![
                ResolvedAuthor::new("alice", "Alice"),
                ResolvedAuthor::new("alice", "Alice"),
            ])
        },
        "dco_invalid",
    );

    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.metadata
                .get_mut("candidate")
                .unwrap()
                .as_mut()
                .unwrap()
                .push(MetadataEntry::new(
                    "studiobrain.governance.v1.superseded.1111111111111111111111111111111111111111111111111111111111111111:22222222222222222222222222222222",
                    r#"{"version":"v2","identity":"1111111111111111111111111111111111111111111111111111111111111111:22222222222222222222222222222222"}"#,
                ));
        },
        "supersession_invalid",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.metadata
                .get_mut("candidate")
                .unwrap()
                .as_mut()
                .unwrap()
                .push(MetadataEntry::new(
                    "studiobrain.governance.v1.superseded.1111111111111111111111111111111111111111111111111111111111111111:22222222222222222222222222222222",
                    r#"{"version":"v1","identity":"different"}"#,
                ));
        },
        "supersession_invalid",
    );
}

#[test]
fn scans_supersession_metadata_on_the_verified_base_revision() {
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.metadata.insert(
                "base".into(),
                Ok(vec![MetadataEntry::new(
                    "studiobrain.governance.v1.superseded.1111111111111111111111111111111111111111111111111111111111111111:22222222222222222222222222222222",
                    r#"{"version":"v1","identity":"1111111111111111111111111111111111111111111111111111111111111111:22222222222222222222222222222222"}"#,
                )]),
            );
        },
        "not_superseded_failed",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.metadata.insert(
                "base".into(),
                Ok(vec![MetadataEntry::new(
                    "studiobrain.governance.v1.superseded.1111111111111111111111111111111111111111111111111111111111111111:22222222222222222222222222222222",
                    r#"{"version":"v2","identity":"1111111111111111111111111111111111111111111111111111111111111111:22222222222222222222222222222222"}"#,
                )]),
            );
        },
        "supersession_invalid",
    );
}

#[test]
fn ancestor_marker_survives_cleared_tip_and_base_metadata() {
    let identity = format!("{}:{}", "1".repeat(64), "2".repeat(32));
    let mut fake = FakeLore::clean();
    fake.infos.insert(
        "base".into(),
        Ok(RevisionInfoResponse::exact(RevisionInfo {
            revision: "base".into(),
            parents: vec!["older".into()],
        })),
    );
    fake.infos.insert(
        "older".into(),
        Ok(RevisionInfoResponse::exact(RevisionInfo {
            revision: "older".into(),
            parents: vec![],
        })),
    );
    for revision in ["candidate", "base"] {
        assert!(fake.metadata[revision]
            .as_ref()
            .unwrap()
            .iter()
            .all(|entry| !entry.key.starts_with(SUPERSESSION_MARKER_PREFIX)));
    }
    fake.metadata.insert(
        "older".into(),
        Ok(vec![MetadataEntry::new(
            format!("{SUPERSESSION_MARKER_PREFIX}{identity}"),
            serde_json::to_string(&lore_vm::ops::governance::contract::SupersessionMarkerV1 {
                version: "v1".into(),
                identity,
            })
            .unwrap(),
        )]),
    );

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&fake, &request()));
    assert_eq!(result.failure_codes, ["not_superseded_failed"]);
    assert!(result.remediation.is_none());
    assert_eq!(
        result
            .observations
            .supersession_ancestry
            .iter()
            .map(|info| info.revision.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["base", "candidate", "older"])
    );
}

#[test]
fn marker_on_older_second_parent_of_base_blocks_revival() {
    let identity = format!("{}:{}", "1".repeat(64), "2".repeat(32));
    let mut fake = FakeLore::clean();
    fake.infos.insert(
        "base".into(),
        Ok(RevisionInfoResponse::exact(RevisionInfo {
            revision: "base".into(),
            parents: vec!["left-root".into(), "right-root".into()],
        })),
    );
    for revision in ["left-root", "right-root"] {
        fake.infos.insert(
            revision.into(),
            Ok(RevisionInfoResponse::exact(RevisionInfo {
                revision: revision.into(),
                parents: vec![],
            })),
        );
        fake.metadata.insert(revision.into(), Ok(vec![]));
    }
    fake.metadata.insert(
        "right-root".into(),
        Ok(vec![MetadataEntry::new(
            format!("{SUPERSESSION_MARKER_PREFIX}{identity}"),
            serde_json::to_string(&lore_vm::ops::governance::contract::SupersessionMarkerV1 {
                version: "v1".into(),
                identity,
            })
            .unwrap(),
        )]),
    );

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&fake, &request()));
    assert_eq!(result.failure_codes, ["not_superseded_failed"]);
    assert!(result.remediation.is_none());
    assert!(result
        .observations
        .supersession_markers
        .iter()
        .any(|marker| marker.revision == "right-root"));
}

#[test]
fn supersession_markers_reject_short_uppercase_and_zero_context_identities() {
    for identity in [
        "short:identity".to_string(),
        format!("{}:{}", "A".repeat(64), "2".repeat(32)),
        format!("{}:{}", "1".repeat(64), "0".repeat(32)),
    ] {
        assert_closed_for(
            FakeLore::clean(),
            |fake| {
                fake.metadata
                    .get_mut("base")
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .push(MetadataEntry::new(
                        format!("{SUPERSESSION_MARKER_PREFIX}{identity}"),
                        serde_json::to_string(&serde_json::json!({
                            "version": "v1",
                            "identity": identity,
                        }))
                        .unwrap(),
                    ));
            },
            "supersession_invalid",
        );
    }
}

#[test]
fn dco_requires_exactly_one_signoff_in_the_terminal_trailer_block() {
    let mut valid = FakeLore::clean();
    valid.authors = Ok(vec![ResolvedAuthor::new("alice", "Alice Example")]);
    valid.metadata.insert(
        "candidate".into(),
        Ok(vec![
            MetadataEntry::new(
                "message",
                "change\n\nSigned-off-by: Alice Example <alice@example.test>\nReviewed-by: Bob <bob@example.test>",
            ),
            MetadataEntry::new("created-by", "alice"),
        ]),
    );
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&valid, &request()));
    assert!(
        result.open,
        "other well-formed terminal trailers: {result:?}"
    );

    for message in [
        "Signed-off-by: Alice <alice@example.test>\n\nchange",
        "change\n\nSigned-off-by: Alice <alice@example.test>\ntrailing prose",
        "change\nSigned-off-by: Alice <alice@example.test>\n\nReviewed-by: Bob <bob@example.test>",
        "change\n\nSigned-off-by: Alice <alice@example.test>\nSigned-off-by: Alice <alice@example.test>",
    ] {
        assert_closed_for(
            FakeLore::clean(),
            |fake| {
                fake.metadata.insert(
                    "candidate".into(),
                    Ok(vec![
                        MetadataEntry::new("message", message),
                        MetadataEntry::new("created-by", "alice"),
                    ]),
                );
            },
            "dco_invalid",
        );
    }
}

#[test]
fn dco_rejects_noncanonical_signer_and_email_bytes() {
    let cases = [
        ("double separator spacing", "Alice  <alice@example.test>"),
        ("missing at sign", "Alice <not-an-email>"),
        ("extra angle brackets", "Alice <<alice@example.test>>"),
        (
            "embedded control character",
            "Alice <alice@exam\u{0007}ple.test>",
        ),
        ("empty local part", "Alice <@example.test>"),
        ("empty domain", "Alice <alice@>"),
        ("multiple at signs", "Alice <alice@@example.test>"),
        ("empty domain label", "Alice <alice@example..test>"),
        ("hyphen-bounded domain label", "Alice <alice@-example.test>"),
        ("invalid domain label byte", "Alice <alice@example_test>"),
    ];
    let mut accepted = Vec::new();

    for (case, signer) in cases {
        let mut fake = FakeLore::clean();
        fake.metadata.insert(
            "candidate".into(),
            Ok(vec![
                MetadataEntry::new("message", format!("change\n\nSigned-off-by: {signer}")),
                MetadataEntry::new("created-by", "alice"),
            ]),
        );
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(evaluate(&fake, &request()));
        if result.open {
            accepted.push(case);
        } else {
            assert_eq!(result.failure_codes, vec!["dco_invalid"], "{case}");
        }
    }

    assert!(
        accepted.is_empty(),
        "accepted noncanonical DCO identities: {accepted:?}"
    );
}

#[test]
fn closes_for_tree_file_info_diff_and_lock_dependency_failures() {
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.dumps
                .insert("candidate".into(), Err(FakeLore::error("tree")));
        },
        "tree_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.dumps.insert(
                "candidate".into(),
                Ok(vec!["asset.txt".into(), "asset.txt".into()]),
            );
        },
        "tree_invalid",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.file_info.insert(
                "candidate".into(),
                Ok(vec![
                    FileIdentity::new(
                        "asset.txt",
                        "candidate",
                        "1111111111111111111111111111111111111111111111111111111111111111",
                        "22222222222222222222222222222222",
                    ),
                    FileIdentity::new("asset.txt", "candidate", "hash-2", "context-2"),
                ]),
            );
        },
        "tree_invalid",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.file_info
                .insert("candidate".into(), Err(FakeLore::error("file info")));
        },
        "file_info_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.file_info.insert("candidate".into(), Ok(vec![]));
        },
        "tree_invalid",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.file_info.insert(
                "candidate".into(),
                Ok(vec![FileIdentity::new(
                    "asset.txt",
                    "fallback",
                    "1111111111111111111111111111111111111111111111111111111111111111",
                    "22222222222222222222222222222222",
                )]),
            );
        },
        "tree_invalid",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.file_info.insert(
                "candidate".into(),
                Ok(vec![FileIdentity::new(
                    "asset.txt",
                    "candidate",
                    "",
                    "22222222222222222222222222222222",
                )]),
            );
        },
        "tree_invalid",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.diff = Err(FakeLore::error("diff")),
        "affected_paths_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.diff = Ok(vec![]);
            fake.status.as_mut().unwrap().staged_paths.clear();
            fake.status.as_mut().unwrap().staged_changes.clear();
            let worktree = &mut fake.status.as_mut().unwrap().worktree_files[0];
            worktree.local_hash = worktree.revision_hash.clone();
            worktree.flag_modified = false;
        },
        "empty_submission",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.lock_queries
                .insert("asset.txt".into(), Err(FakeLore::error("query")));
        },
        "locks_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.lock_queries
                .insert("asset.txt".into(), Ok(LockQuery::unlocked("other.txt")));
        },
        "locks_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.lock_status = Err(FakeLore::error("status")),
        "locks_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.lock_status = Ok(LockStatusResponse::incomplete()),
        "locks_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.lock_status = Ok(LockStatusResponse::with_locks(
                2,
                vec![
                    LockStatus::unlocked("asset.txt"),
                    LockStatus::unlocked("extra.txt"),
                ],
            ))
        },
        "locks_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.lock_status = Ok(LockStatusResponse::with_locks(
                1,
                vec![LockStatus::locked("asset.txt", "foreign")],
            ))
        },
        "locks_clear_failed",
    );
}

#[test]
fn raw_revision_diff_rejects_wrong_action_direction_duplicate_and_incomplete_facts() {
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.diff = Err(FakeLore::error("raw diff ended before End"));
        },
        "affected_paths_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            let mut wrong = raw_modified("asset.txt");
            wrong.action = GovernancePathAction::Add;
            wrong.old_is_file = false;
            wrong.old_address = format!("{}-{}", "0".repeat(64), "0".repeat(32));
            fake.diff = Ok(vec![wrong]);
        },
        "affected_paths_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            let mut wrong = raw_modified("asset.txt");
            wrong.action = GovernancePathAction::Delete;
            wrong.new_is_file = false;
            wrong.new_address = format!("{}-{}", "0".repeat(64), "0".repeat(32));
            fake.diff = Ok(vec![wrong]);
        },
        "affected_paths_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.diff = Ok(vec![raw_modified("asset.txt"), raw_modified("asset.txt")]);
        },
        "affected_paths_unavailable",
    );
}

#[test]
fn raw_move_pair_is_exact_and_rejects_orphan_reused_extra_reversed_or_mismatched_facts() {
    let clean = FakeLore::clean_rename();
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(evaluate(&clean, &request()));
    assert!(result.open, "exact raw move pair: {result:?}");
    assert_eq!(
        result.observations.revision_diff,
        vec![AffectedPath {
            source_path: Some("asset.txt".into()),
            target_path: Some("renamed.txt".into()),
        }]
    );
    assert_eq!(result.affected_paths, ["asset.txt", "renamed.txt"]);
    assert_eq!(result.observations.lock_queries.len(), 2);

    let cases: [(&str, fn(&mut FakeLore)); 8] = [
        ("orphan", |fake| {
            fake.diff.as_mut().unwrap().pop();
        }),
        ("raw_move_instead_of_exact_pair", |fake| {
            fake.diff = Ok(vec![RevisionDiffObservation {
                path: "renamed.txt".into(),
                action: GovernancePathAction::Move,
                old_is_file: true,
                new_is_file: true,
                old_address: format!("{}-{}", "4".repeat(64), "2".repeat(32)),
                new_address: format!("{}-{}", "4".repeat(64), "2".repeat(32)),
            }]);
        }),
        ("extra", |fake| {
            fake.diff.as_mut().unwrap().push(raw_added("orphan.txt"));
        }),
        ("reversed_endpoints", |fake| {
            fake.diff = Ok(vec![raw_deleted("renamed.txt"), raw_added("asset.txt")]);
        }),
        ("wrong_flags", |fake| {
            fake.diff.as_mut().unwrap()[0].new_is_file = true;
        }),
        ("wrong_hash", |fake| {
            fake.diff.as_mut().unwrap()[1].new_address =
                format!("{}-{}", "9".repeat(64), "2".repeat(32));
        }),
        ("context_mismatch", |fake| {
            fake.diff.as_mut().unwrap()[1].new_address =
                format!("{}-{}", "4".repeat(64), "3".repeat(32));
        }),
        ("reused_duplicate_event", |fake| {
            fake.diff.as_mut().unwrap().push(raw_added("renamed.txt"));
        }),
    ];
    for (name, mutate) in cases {
        let mut fake = FakeLore::clean_rename();
        mutate(&mut fake);
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(evaluate(&fake, &request()));
        assert!(!result.open, "{name}: {result:?}");
        assert_eq!(
            result.failure_codes,
            ["affected_paths_unavailable"],
            "{name}: {result:?}"
        );
    }
}

#[test]
fn synthetic_status_or_raw_copy_is_explicitly_unsupported_at_pinned_lore() {
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            let status = fake.status.as_mut().unwrap();
            status.staged_paths = vec!["asset.txt".into(), "copied.txt".into()];
            status.staged_changes = vec![StagedPathObservation {
                path: "copied.txt".into(),
                from_path: Some("asset.txt".into()),
                action: GovernancePathAction::Copy,
                dirty: true,
                conflict: false,
            }];
        },
        "copy_semantics_unavailable",
    );

    assert_closed_for(
        FakeLore::clean(),
        |fake| fake.diff.as_mut().unwrap()[0].action = GovernancePathAction::Copy,
        "copy_semantics_unavailable",
    );
}

#[test]
fn exact_tree_rejects_noncanonical_base_and_selected_candidate_identities() {
    for (revision, field, value) in [
        ("base", "hash", "A".repeat(64)),
        ("base", "hash", "abcd".into()),
        ("base", "context", "0".repeat(32)),
        ("candidate", "hash", "A".repeat(64)),
        ("candidate", "hash", "abcd".into()),
        ("candidate", "context", "0".repeat(32)),
    ] {
        let mut fake = FakeLore::clean();
        if revision == "base" {
            fake.dumps
                .insert("base".into(), Ok(vec!["base.txt".into()]));
            fake.file_info.insert(
                "base".into(),
                Ok(vec![FileIdentity::new(
                    "base.txt",
                    "base",
                    "4444444444444444444444444444444444444444444444444444444444444444",
                    "55555555555555555555555555555555",
                )]),
            );
        }
        let identity = fake
            .file_info
            .get_mut(revision)
            .unwrap()
            .as_mut()
            .unwrap()
            .first_mut()
            .unwrap();
        if field == "hash" {
            identity.hash = value;
        } else {
            identity.context = value;
        }
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(evaluate(&fake, &request()));
        assert_eq!(result.failure_codes, ["tree_invalid"], "{revision} {field}");
    }
}

#[test]
fn raw_lock_boundaries_reject_missing_duplicate_over_counted_and_ignored_streams() {
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.lock_queries
                .insert("asset.txt".into(), Ok(LockQuery::incomplete("asset.txt")));
        },
        "locks_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.lock_queries.insert(
                "asset.txt".into(),
                Ok(LockQuery::with_owners(
                    "asset.txt",
                    2,
                    vec!["foreign".into()],
                )),
            );
        },
        "locks_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.lock_status = Ok(LockStatusResponse::ignored("asset.txt"));
        },
        "locks_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.lock_status = Ok(LockStatusResponse::with_locks(
                2,
                vec![LockStatus::unlocked("asset.txt")],
            ));
        },
        "locks_unavailable",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.lock_status = Ok(LockStatusResponse::with_locks(
                2,
                vec![
                    LockStatus::unlocked("asset.txt"),
                    LockStatus::unlocked("asset.txt"),
                ],
            ));
        },
        "locks_unavailable",
    );
}

#[cfg(feature = "integration-tests")]
#[derive(Clone, Copy)]
enum InjectedLockDependency {
    Clear,
    QueryUnavailable,
    StatusUnavailable,
}

#[cfg(feature = "integration-tests")]
#[derive(Clone, Copy)]
enum InjectedAuthorDependency {
    Match,
    Mismatch,
    Duplicate,
    Unresolved,
    Unavailable,
}

/// Truthful hybrid: repository/status/history/tree/diff/metadata are the real
/// in-process Lore engine; only remote auth and lock replies are deterministic
/// injected dependencies. SBAI-6001 tracks authenticated remote corroboration.
#[cfg(feature = "integration-tests")]
struct HybridGovernanceAdapter<'a> {
    real: ProductionLoreAdapter<'a>,
    lock: InjectedLockDependency,
    authors: InjectedAuthorDependency,
}

#[cfg(feature = "integration-tests")]
impl<'a> HybridGovernanceAdapter<'a> {
    fn new(api: &'a LoreApi, branch: &str, lock: InjectedLockDependency) -> Self {
        Self {
            real: ProductionLoreAdapter::new(api, branch),
            lock,
            authors: InjectedAuthorDependency::Match,
        }
    }

    fn with_authors(mut self, authors: InjectedAuthorDependency) -> Self {
        self.authors = authors;
        self
    }
}

#[cfg(feature = "integration-tests")]
#[async_trait::async_trait]
impl GovernanceAdapter for HybridGovernanceAdapter<'_> {
    async fn exact_staged_revisions(&self) -> Result<Vec<String>, AdapterError> {
        self.real.exact_staged_revisions().await
    }

    async fn status(&self) -> Result<StatusSnapshot, AdapterError> {
        self.real.status().await
    }

    async fn revision_info(&self, revision: &str) -> Result<RevisionInfoResponse, AdapterError> {
        self.real.revision_info(revision).await
    }

    async fn revision_metadata(&self, revision: &str) -> Result<Vec<MetadataEntry>, AdapterError> {
        self.real.revision_metadata(revision).await
    }

    async fn first_parent_history(
        &self,
        candidate: &str,
        target_base: &str,
        max_revisions: usize,
    ) -> Result<Vec<String>, AdapterError> {
        self.real
            .first_parent_history(candidate, target_base, max_revisions)
            .await
    }

    async fn repository_dump(&self, revision: &str) -> Result<Vec<String>, AdapterError> {
        self.real.repository_dump(revision).await
    }

    async fn file_info(
        &self,
        revision: &str,
        paths: &[String],
    ) -> Result<Vec<FileIdentity>, AdapterError> {
        self.real.file_info(revision, paths).await
    }

    async fn revision_diff(
        &self,
        base: &str,
        candidate: &str,
    ) -> Result<Vec<RevisionDiffObservation>, AdapterError> {
        self.real.revision_diff(base, candidate).await
    }

    async fn resolve_authors(
        &self,
        identities: &[String],
    ) -> Result<Vec<ResolvedAuthor>, AdapterError> {
        match self.authors {
            InjectedAuthorDependency::Match => Ok(identities
                .iter()
                .map(|identity| ResolvedAuthor::new(identity, "Alice"))
                .collect()),
            InjectedAuthorDependency::Mismatch => Ok(identities
                .iter()
                .map(|identity| ResolvedAuthor::new(identity, "Mallory"))
                .collect()),
            InjectedAuthorDependency::Duplicate => {
                let Some(identity) = identities.first() else {
                    return Ok(Vec::new());
                };
                Ok(vec![
                    ResolvedAuthor::new(identity, "Alice"),
                    ResolvedAuthor::new(identity, "Alice"),
                ])
            }
            InjectedAuthorDependency::Unresolved => Ok(Vec::new()),
            InjectedAuthorDependency::Unavailable => {
                Err(AdapterError::new("injected author resolution unavailable"))
            }
        }
    }

    async fn lock_file_query(&self, _branch: &str, path: &str) -> Result<LockQuery, AdapterError> {
        match self.lock {
            InjectedLockDependency::QueryUnavailable => {
                Err(AdapterError::new("injected lock query unavailable"))
            }
            _ => Ok(LockQuery::unlocked(path)),
        }
    }

    async fn lock_file_status(
        &self,
        _branch: &str,
        _paths: &[String],
    ) -> Result<LockStatusResponse, AdapterError> {
        match self.lock {
            InjectedLockDependency::StatusUnavailable => {
                Err(AdapterError::new("injected lock status unavailable"))
            }
            _ => Ok(LockStatusResponse::unlocked()),
        }
    }
}

#[cfg(feature = "integration-tests")]
struct RealGovernanceFixture {
    _tempdir: tempfile::TempDir,
    _store: tempfile::TempDir,
    api: LoreApi,
    base: String,
    candidate: String,
    branch: String,
}

#[cfg(feature = "integration-tests")]
async fn real_governance_fixture() -> RealGovernanceFixture {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_REPOSITORY: AtomicU64 = AtomicU64::new(1);

    let tempdir = tempfile::tempdir().expect("create governance real-engine tempdir");
    let store = tempfile::tempdir().expect("create governance shared-store tempdir");
    let store_path = store.path().join("shared-store");
    let api = LoreApi::from_global(
        LoreGlobal::new(tempdir.path().to_path_buf())
            .in_memory(false)
            .offline(true)
            .identity("alice"),
    );
    ops::shared_store::create::create(
        &api,
        ops::shared_store::create::SharedStoreCreateArgs {
            remote_url: String::new(),
            path: Some(store_path.to_string_lossy().into_owned()),
            make_default: false,
        },
    )
    .await
    .expect("create real governance shared store");
    let suffix = NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed);
    ops::repository::create::create(
        &api,
        ops::repository::create::CreateArgs {
            repository_url: format!(
                "lore://localhost/governance-{}-{suffix}",
                std::process::id()
            ),
            description: "SBAI-5934 real-engine hybrid fixture".into(),
            id: String::new(),
            use_shared_store: true,
            shared_store_path: store_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("real Lore repository creation must succeed");

    let asset = tempdir.path().join("asset.txt");
    std::fs::write(&asset, b"base bytes").expect("write base fixture");
    ops::file::stage::stage(
        &api,
        ops::file::stage::FileStageArgs {
            paths: vec![asset.to_string_lossy().into_owned()],
            case_change: ops::file::stage::CaseChange::Error,
            scan: true,
        },
    )
    .await
    .expect("stage base fixture");
    let committed = ops::revision::commit::commit(
        &api,
        ops::revision::commit::CommitArgs {
            message: "base\n\nSigned-off-by: Alice <alice@example.test>".into(),
        },
    )
    .await
    .expect("commit base fixture");

    std::fs::write(&asset, b"candidate bytes").expect("write candidate fixture");
    ops::file::stage::stage(
        &api,
        ops::file::stage::FileStageArgs {
            paths: vec![asset.to_string_lossy().into_owned()],
            case_change: ops::file::stage::CaseChange::Error,
            scan: true,
        },
    )
    .await
    .expect("stage candidate fixture");
    ops::revision::metadata_set::metadata_set(
        &api,
        ops::revision::metadata_set::MetadataSetArgs {
            keys: vec!["message".into(), "created-by".into()],
            values: vec![
                "candidate\n\nSigned-off-by: Alice <alice@example.test>".into(),
                "alice".into(),
            ],
            formats: vec![
                ops::revision::metadata_set::MetadataFormat::String,
                ops::revision::metadata_set::MetadataFormat::String,
            ],
        },
    )
    .await
    .expect("attach candidate DCO metadata");

    let real = ProductionLoreAdapter::new(&api, &committed.branch);
    let status = real
        .status()
        .await
        .expect("real status must resolve the exact staged subject");
    assert_eq!(status.staged_revisions.len(), 1);
    let candidate = status.staged_revisions[0].clone();
    assert_ne!(candidate, committed.revision);

    RealGovernanceFixture {
        _tempdir: tempdir,
        _store: store,
        api,
        base: committed.revision,
        candidate,
        branch: committed.branch,
    }
}

#[cfg(feature = "integration-tests")]
#[derive(Clone, Default)]
struct NoWriteIo {
    calls: Arc<Mutex<Vec<String>>>,
}

#[cfg(feature = "integration-tests")]
impl NoWriteIo {
    fn assert_unused(&self, context: &str) {
        assert!(
            self.calls.lock().unwrap().is_empty(),
            "{context}: I/O occurred"
        );
    }

    fn called(&self, name: &str) -> AdapterError {
        self.calls.lock().unwrap().push(name.into());
        AdapterError::new(format!("unexpected {name}"))
    }
}

#[cfg(feature = "integration-tests")]
#[async_trait::async_trait]
impl GovernanceIo for NoWriteIo {
    fn role(&self) -> GovernanceRole {
        GovernanceRole::Actor
    }

    async fn revision_metadata_set(&self, _key: &str, _value: &str) -> MutationObservation<()> {
        MutationObservation::NotDispatched {
            code: self.called("metadata_set").message,
        }
    }

    async fn storage_open(&self) -> MutationObservation<Vec<u64>> {
        MutationObservation::NotDispatched {
            code: self.called("storage_open").message,
        }
    }

    async fn storage_put(
        &self,
        _handle: u64,
        _bytes: &[u8],
    ) -> MutationObservation<Vec<ImmutablePutItem>> {
        MutationObservation::NotDispatched {
            code: self.called("storage_put").message,
        }
    }

    async fn storage_get(
        &self,
        _handle: u64,
        _address: &str,
    ) -> ReadObservation<Vec<ImmutableGetItem>> {
        ReadObservation::NotDispatched {
            code: self.called("storage_get").message,
        }
    }

    async fn storage_close(&self, _handle: u64) -> MutationObservation<()> {
        MutationObservation::NotDispatched {
            code: self.called("storage_close").message,
        }
    }
}

#[cfg(feature = "integration-tests")]
#[tokio::test]
async fn hybrid_real_lore_corroborates_repository_tree_metadata_and_storage_flow() {
    let fixture = real_governance_fixture().await;
    let adapter =
        HybridGovernanceAdapter::new(&fixture.api, &fixture.branch, InjectedLockDependency::Clear);
    let request = EvidencePreserveRequest {
        expected_staged_revision: fixture.candidate.clone(),
        target_base_revision: fixture.base.clone(),
    };

    let direct_history = adapter
        .first_parent_history(&fixture.candidate, &fixture.base, 1001)
        .await
        .expect("real first-parent history dependency");
    assert_eq!(
        direct_history,
        [fixture.candidate.clone()],
        "production staged history must retain its exact staged head"
    );

    let evaluated = evaluate(&adapter, &request).await;
    assert!(
        evaluated.open,
        "real Lore facts did not open: {evaluated:?}"
    );
    assert_eq!(
        evaluated
            .observations
            .status
            .as_ref()
            .unwrap()
            .staged_revisions,
        [fixture.candidate.clone()]
    );
    assert!(!evaluated.observations.candidate_files.is_empty());
    assert!(!evaluated.observations.base_files.is_empty());

    let actor_io = ProductionGovernanceIo::new(&fixture.api, GovernanceRole::Actor);
    let preserved = evidence_preserve_with_adapters(&adapter, &actor_io, &request)
        .await
        .expect("hybrid real Lore actor evidence should preserve");
    let preserved = preserved.verified().expect("full actor chain closed");
    assert_ne!(preserved.result_staged_revision, fixture.candidate);
    let result_metadata = adapter
        .revision_metadata(&preserved.result_staged_revision)
        .await
        .expect("real result metadata");
    assert_eq!(
        result_metadata
            .iter()
            .filter(|entry| entry.key == EVIDENCE_POINTER_KEY)
            .count(),
        1
    );

    let witness_io = ProductionGovernanceIo::new(&fixture.api, GovernanceRole::Witness);
    let gate = submission_gate_check_with_adapters(
        &adapter,
        &witness_io,
        &SubmissionGateCheckRequest {
            expected_staged_revision: preserved.result_staged_revision.clone(),
            target_base_revision: fixture.base.clone(),
        },
    )
    .await;
    assert!(
        gate.gate_open,
        "hybrid real Lore witness did not open: {gate:?}"
    );

    std::fs::write(
        fixture._tempdir.path().join("asset.txt"),
        b"post-evidence edit",
    )
    .expect("write a real post-evidence edit");
    let before_reject = adapter.status().await.expect("pre-reject exact status");
    let before_metadata = adapter
        .revision_metadata(&preserved.result_staged_revision)
        .await
        .expect("pre-reject exact metadata");
    let changed = submission_gate_check_with_adapters(
        &adapter,
        &witness_io,
        &SubmissionGateCheckRequest {
            expected_staged_revision: preserved.result_staged_revision.clone(),
            target_base_revision: fixture.base,
        },
    )
    .await;
    assert!(!changed.gate_open, "post-evidence edit opened: {changed:?}");
    assert_eq!(
        changed.criteria.last().unwrap().failure_code.as_deref(),
        Some("evidence_mismatch"),
        "the witness must compare the exact live filesystem fingerprint"
    );
    let after_reject = adapter.status().await.expect("post-reject exact status");
    let after_metadata = adapter
        .revision_metadata(&preserved.result_staged_revision)
        .await
        .expect("post-reject exact metadata");
    assert_eq!(
        before_reject.staged_revisions,
        after_reject.staged_revisions
    );
    assert_eq!(
        before_metadata, after_metadata,
        "a closed gate has zero effect"
    );
}

#[cfg(feature = "integration-tests")]
#[tokio::test]
async fn real_lore_dirty_copy_projects_only_zero_identity_add_until_commit() {
    let fixture = real_governance_fixture().await;
    let copy_base = ops::revision::commit::commit(
        &fixture.api,
        ops::revision::commit::CommitArgs {
            message: "copy base\n\nSigned-off-by: Alice <alice@example.test>".into(),
        },
    )
    .await
    .expect("commit the existing candidate before exercising dirty_copy");
    let source = fixture._tempdir.path().join("asset.txt");
    let target = fixture._tempdir.path().join("copied.txt");
    std::fs::copy(&source, &target).expect("materialize the filesystem copy");
    ops::file::dirty_copy::dirty_copy(
        &fixture.api,
        ops::file::dirty_copy::FileDirtyCopyArgs {
            from_path: source.to_string_lossy().into_owned(),
            to_path: target.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("record the pinned real-Lore dirty_copy state");

    let before_stage = ops::repository::status::status(
        &fixture.api,
        ops::repository::status::RepositoryStatusArgs {
            staged: true,
            scan: false,
            ..Default::default()
        },
    )
    .await
    .expect("observe dirty_copy before ordinary stage");
    let before_target = before_stage
        .files
        .iter()
        .find(|file| file.path == "copied.txt")
        .expect("dirty_copy target is observable");
    assert_ne!(
        before_target.action,
        ops::repository::status::StatusFileAction::Copy
    );
    assert!(before_target.from_path.is_empty());

    let staged = ops::file::stage::stage(
        &fixture.api,
        ops::file::stage::FileStageArgs {
            paths: vec![target.to_string_lossy().into_owned()],
            case_change: ops::file::stage::CaseChange::Error,
            scan: true,
        },
    )
    .await
    .expect("ordinary stage resolves the dirty_copy projection");
    assert!(staged.files.iter().all(|file| {
        file.action != ops::file::stage::FileStageAction::Copy && file.from_path.is_empty()
    }));

    let after_stage = ops::repository::status::status(
        &fixture.api,
        ops::repository::status::RepositoryStatusArgs {
            staged: true,
            scan: false,
            ..Default::default()
        },
    )
    .await
    .expect("observe staged dirty_copy target");
    let revision = after_stage
        .revision
        .as_ref()
        .expect("status revision")
        .revision_staged
        .clone();
    let target_status = after_stage
        .files
        .iter()
        .find(|file| file.path == "copied.txt")
        .expect("staged target status");
    assert_eq!(
        target_status.action,
        ops::repository::status::StatusFileAction::Add
    );
    assert!(target_status.from_path.is_empty());
    assert!(target_status.staged);

    let diff = ops::revision::diff::diff(
        &fixture.api,
        ops::revision::diff::RevisionDiffArgs {
            revision_source: copy_base.revision.clone(),
            revision_target: revision.clone(),
            paths: vec![],
        },
    )
    .await
    .expect("observe exact base-to-staged revision diff");
    let target_diff = diff
        .files
        .iter()
        .find(|file| file.path == "copied.txt")
        .expect("target-only diff");
    assert_eq!(target_diff.action, ops::revision::diff::DiffFileAction::Add);
    assert!(!target_diff.old_is_file && target_diff.new_is_file);
    let zero_address = format!("{}-{}", "0".repeat(64), "0".repeat(32));
    assert_eq!(target_diff.old_address, zero_address);
    assert_eq!(target_diff.new_address, zero_address);

    let info = ops::file::info::info(
        &fixture.api,
        ops::file::info::FileInfoArgs {
            paths: vec![target.to_string_lossy().into_owned()],
            revision: revision.clone(),
            local: false,
            filtered: false,
        },
    )
    .await
    .expect("observe exact staged target identity");
    assert_eq!(info.entries.len(), 1);
    assert_eq!(info.entries[0].hash, "0".repeat(64));
    assert_eq!(info.entries[0].context, "0".repeat(32));

    ops::revision::metadata_set::metadata_set(
        &fixture.api,
        ops::revision::metadata_set::MetadataSetArgs {
            keys: vec!["message".into(), "created-by".into()],
            values: vec![
                "copy candidate\n\nSigned-off-by: Alice <alice@example.test>".into(),
                "alice".into(),
            ],
            formats: vec![
                ops::revision::metadata_set::MetadataFormat::String,
                ops::revision::metadata_set::MetadataFormat::String,
            ],
        },
    )
    .await
    .expect("attach DCO metadata without committing the unresolved target");
    let adapter = HybridGovernanceAdapter::new(
        &fixture.api,
        &copy_base.branch,
        InjectedLockDependency::Clear,
    );
    // Like stage_move at this pin, the first full scan may rewrite the staged
    // subject. Prime that rewrite, then bind evaluation to the resulting exact
    // staged hash; this must not manufacture a nonzero Copy identity.
    let _ = adapter.status().await;
    let exact_subject = adapter
        .exact_staged_revisions()
        .await
        .expect("read exact staged subject")
        .pop()
        .expect("one staged subject");
    let result = evaluate(
        &adapter,
        &EvidencePreserveRequest {
            expected_staged_revision: exact_subject,
            target_base_revision: copy_base.revision,
        },
    )
    .await;
    assert_eq!(
        result.failure_codes,
        ["tree_invalid"],
        "zero-context staged Add must remain fail closed: {result:?}"
    );
}

#[cfg(feature = "integration-tests")]
#[tokio::test]
async fn real_lore_descendant_metadata_clear_cannot_revive_reachable_marked_identity() {
    let fixture = real_governance_fixture().await;
    let adapter =
        HybridGovernanceAdapter::new(&fixture.api, &fixture.branch, InjectedLockDependency::Clear);
    let initial = evaluate(
        &adapter,
        &EvidencePreserveRequest {
            expected_staged_revision: fixture.candidate.clone(),
            target_base_revision: fixture.base.clone(),
        },
    )
    .await;
    assert!(initial.open, "initial real candidate: {initial:?}");
    let identity = initial.observations.current_files[0].canonical_id();
    let marker_key = format!("{SUPERSESSION_MARKER_PREFIX}{identity}");
    ops::revision::metadata_set::metadata_set(
        &fixture.api,
        ops::revision::metadata_set::MetadataSetArgs {
            keys: vec![marker_key.clone()],
            values: vec![serde_json::to_string(
                &lore_vm::ops::governance::contract::SupersessionMarkerV1 {
                    version: "v1".into(),
                    identity: identity.clone(),
                },
            )
            .unwrap()],
            formats: vec![ops::revision::metadata_set::MetadataFormat::String],
        },
    )
    .await
    .expect("seed strict marker on the exact staged subject");
    let marked = ops::revision::commit::commit(
        &fixture.api,
        ops::revision::commit::CommitArgs {
            message: "marked ancestor\n\nSigned-off-by: Alice <alice@example.test>".into(),
        },
    )
    .await
    .expect("commit the marked reachable ancestor");

    let lineage = fixture._tempdir.path().join("lineage.txt");
    std::fs::write(&lineage, b"ordinary descendant change")
        .expect("write unrelated descendant content");
    ops::file::stage::stage(
        &fixture.api,
        ops::file::stage::FileStageArgs {
            paths: vec![lineage.to_string_lossy().into_owned()],
            case_change: ops::file::stage::CaseChange::Error,
            scan: true,
        },
    )
    .await
    .expect("stage an ordinary descendant before clearing its metadata");
    ops::revision::metadata_clear::metadata_clear(
        &fixture.api,
        ops::revision::metadata_clear::MetadataClearArgs::default(),
    )
    .await
    .expect("clear metadata only on a descendant staged state");
    let cleared = ops::revision::commit::commit(
        &fixture.api,
        ops::revision::commit::CommitArgs {
            message: "cleared descendant\n\nSigned-off-by: Alice <alice@example.test>".into(),
        },
    )
    .await
    .expect("commit the metadata-cleared descendant");
    assert!(adapter
        .revision_metadata(&marked.revision)
        .await
        .expect("marked ancestor metadata")
        .iter()
        .any(|entry| entry.key == marker_key));
    assert!(adapter
        .revision_metadata(&cleared.revision)
        .await
        .expect("cleared descendant metadata")
        .iter()
        .all(|entry| !entry.key.starts_with(SUPERSESSION_MARKER_PREFIX)));

    let source = fixture._tempdir.path().join("asset.txt");
    let target = fixture._tempdir.path().join("renamed-after-clear.txt");
    std::fs::rename(&source, &target).expect("rename identical bytes after metadata clear");
    ops::file::stage_move::stage_move(
        &fixture.api,
        ops::file::stage_move::FileStageMoveArgs {
            from_path: source.to_string_lossy().into_owned(),
            to_path: target.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("stage ordinary same-identity rename on the descendant");
    ops::revision::metadata_set::metadata_set(
        &fixture.api,
        ops::revision::metadata_set::MetadataSetArgs {
            keys: vec!["message".into(), "created-by".into()],
            values: vec![
                "rename after clear\n\nSigned-off-by: Alice <alice@example.test>".into(),
                "alice".into(),
            ],
            formats: vec![
                ops::revision::metadata_set::MetadataFormat::String,
                ops::revision::metadata_set::MetadataFormat::String,
            ],
        },
    )
    .await
    .expect("attach DCO metadata on the later descendant");
    let _ = adapter.status().await;
    let candidate = adapter
        .exact_staged_revisions()
        .await
        .expect("later exact staged subject")
        .pop()
        .expect("one later staged subject");
    let blocked = evaluate(
        &adapter,
        &EvidencePreserveRequest {
            expected_staged_revision: candidate,
            target_base_revision: cleared.revision.clone(),
        },
    )
    .await;
    assert_eq!(blocked.failure_codes, ["not_superseded_failed"]);
    assert!(blocked.remediation.is_none());
    assert_eq!(
        blocked
            .observations
            .current_files
            .iter()
            .find(|file| file.path == "renamed-after-clear.txt")
            .expect("later renamed artifact identity")
            .canonical_id(),
        identity
    );
    assert!(blocked
        .observations
        .supersession_markers
        .iter()
        .any(|marker| marker.revision == marked.revision && marker.key == marker_key));
    assert!(blocked
        .observations
        .supersession_ancestry
        .iter()
        .any(|info| info.revision == marked.revision));
}

#[cfg(feature = "integration-tests")]
#[tokio::test]
async fn real_lore_rename_plus_modify_keeps_context_types_one_move_and_locks_both_endpoints() {
    let fixture = real_governance_fixture().await;
    let source = fixture._tempdir.path().join("asset.txt");
    let target = fixture._tempdir.path().join("renamed.txt");
    std::fs::write(&source, b"explicit rename plus modified bytes")
        .expect("edit the selected bytes before the move");
    std::fs::rename(&source, &target).expect("rename the selected real-Lore file");
    ops::file::stage_move::stage_move(
        &fixture.api,
        ops::file::stage_move::FileStageMoveArgs {
            from_path: source.to_string_lossy().into_owned(),
            to_path: target.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("stage the real-Lore move");
    ops::revision::metadata_set::metadata_set(
        &fixture.api,
        ops::revision::metadata_set::MetadataSetArgs {
            keys: vec!["message".into(), "created-by".into()],
            values: vec![
                "rename candidate\n\nSigned-off-by: Alice <alice@example.test>".into(),
                "alice".into(),
            ],
            formats: vec![
                ops::revision::metadata_set::MetadataFormat::String,
                ops::revision::metadata_set::MetadataFormat::String,
            ],
        },
    )
    .await
    .expect("retain exact DCO metadata after the move");

    let adapter =
        HybridGovernanceAdapter::new(&fixture.api, &fixture.branch, InjectedLockDependency::Clear);
    let priming_scan = adapter
        .status()
        .await
        .expect_err("the first scan must expose and reject Lore's post-move staged-state rewrite");
    assert!(
        priming_scan
            .message
            .contains("full status scan changed the exact staged subject"),
        "{priming_scan:?}"
    );
    let candidate = adapter.status().await.unwrap().staged_revisions[0].clone();
    let evaluated = evaluate(
        &adapter,
        &EvidencePreserveRequest {
            expected_staged_revision: candidate,
            target_base_revision: fixture.base.clone(),
        },
    )
    .await;
    assert!(evaluated.open, "real rename+modify: {evaluated:?}");
    assert_eq!(
        evaluated.observations.revision_diff,
        vec![AffectedPath {
            source_path: Some("asset.txt".into()),
            target_path: Some("renamed.txt".into()),
        }]
    );
    assert_eq!(evaluated.observations.upstream_revision_diff.len(), 2);

    let base = &evaluated.observations.base_files[0];
    let candidate = &evaluated.observations.candidate_files[0];
    let current = &evaluated.observations.current_files[0];
    assert_eq!(base.path, "asset.txt");
    assert_eq!(candidate.path, "renamed.txt");
    assert_eq!(current.path, "renamed.txt");
    assert_eq!(base.context, candidate.context);
    assert_eq!(base.context, current.context);
    assert_ne!(
        base.hash, current.hash,
        "the selected rename also carries changed capture-time bytes"
    );
    let raw_delete = &evaluated.observations.upstream_revision_diff[0];
    let raw_add = &evaluated.observations.upstream_revision_diff[1];
    assert_eq!(
        (raw_delete.path.as_str(), raw_delete.action),
        ("asset.txt", GovernancePathAction::Delete)
    );
    assert!(raw_delete.old_is_file && !raw_delete.new_is_file);
    assert_eq!(
        raw_delete.old_address,
        format!("{}-{}", base.hash, base.context)
    );
    assert_eq!(
        (raw_add.path.as_str(), raw_add.action),
        ("renamed.txt", GovernancePathAction::Add)
    );
    assert!(!raw_add.old_is_file && raw_add.new_is_file);
    assert_eq!(
        raw_add.new_address,
        format!("{}-{}", candidate.hash, candidate.context)
    );
    assert_eq!(raw_delete.old_address, raw_add.new_address);
    assert_eq!(
        evaluated
            .observations
            .lock_queries
            .iter()
            .map(|query| query.path.as_str())
            .collect::<Vec<_>>(),
        vec!["asset.txt", "renamed.txt"],
        "both exact move endpoints must pass the per-file lock query"
    );

    let identity = current.canonical_id();
    ops::revision::metadata_set::metadata_set(
        &fixture.api,
        ops::revision::metadata_set::MetadataSetArgs {
            keys: vec![format!("{SUPERSESSION_MARKER_PREFIX}{identity}")],
            values: vec![serde_json::to_string(
                &lore_vm::ops::governance::contract::SupersessionMarkerV1 {
                    version: "v1".into(),
                    identity: identity.clone(),
                },
            )
            .unwrap()],
            formats: vec![ops::revision::metadata_set::MetadataFormat::String],
        },
    )
    .await
    .expect("seed the strict marker through lower-level real Lore metadata");
    let marked_candidate = adapter.status().await.unwrap().staged_revisions[0].clone();
    let blocked = evaluate(
        &adapter,
        &EvidencePreserveRequest {
            expected_staged_revision: marked_candidate,
            target_base_revision: fixture.base,
        },
    )
    .await;
    assert!(
        !blocked.open,
        "the same moved identity reopened: {blocked:?}"
    );
    assert_eq!(blocked.failure_codes, ["not_superseded_failed"]);
    assert_eq!(
        blocked.observations.current_files[0].canonical_id(),
        identity
    );
}

#[cfg(feature = "integration-tests")]
#[tokio::test]
async fn hybrid_real_lore_dco_matrix_uses_real_history_and_revision_metadata() {
    let fixture = real_governance_fixture().await;
    let request = DcoValidateRequest {
        expected_staged_revision: fixture.candidate.clone(),
        target_base_revision: fixture.base.clone(),
    };
    for (name, authors, expected_code) in [
        ("match", InjectedAuthorDependency::Match, None),
        (
            "mismatch",
            InjectedAuthorDependency::Mismatch,
            Some("dco_invalid"),
        ),
        (
            "duplicate",
            InjectedAuthorDependency::Duplicate,
            Some("dco_invalid"),
        ),
        (
            "unresolved",
            InjectedAuthorDependency::Unresolved,
            Some("dco_invalid"),
        ),
        (
            "unavailable",
            InjectedAuthorDependency::Unavailable,
            Some("auth_unavailable"),
        ),
    ] {
        let adapter = HybridGovernanceAdapter::new(
            &fixture.api,
            &fixture.branch,
            InjectedLockDependency::Clear,
        )
        .with_authors(authors);
        let result = dco_validate_with_adapter(&adapter, &request).await;
        assert_eq!(result.valid, expected_code.is_none(), "{name}: {result:?}");
        assert_eq!(
            result.failure_codes.first().map(String::as_str),
            expected_code,
            "{name}: {result:?}"
        );
    }

    ops::revision::metadata_set::metadata_set(
        &fixture.api,
        ops::revision::metadata_set::MetadataSetArgs {
            keys: vec!["message".into()],
            values: vec!["candidate\n\nSigned-off-by: malformed".into()],
            formats: vec![ops::revision::metadata_set::MetadataFormat::String],
        },
    )
    .await
    .expect("write malformed real candidate DCO metadata");
    let adapter =
        HybridGovernanceAdapter::new(&fixture.api, &fixture.branch, InjectedLockDependency::Clear);
    let malformed_candidate = adapter.status().await.unwrap().staged_revisions[0].clone();
    let malformed = dco_validate_with_adapter(
        &adapter,
        &DcoValidateRequest {
            expected_staged_revision: malformed_candidate,
            target_base_revision: fixture.base,
        },
    )
    .await;
    assert!(!malformed.valid, "malformed real DCO: {malformed:?}");
    assert_eq!(malformed.failure_codes, ["dco_invalid"]);
}

#[cfg(feature = "integration-tests")]
#[tokio::test]
async fn production_status_retains_a_post_stage_worktree_fingerprint_change() {
    let fixture = real_governance_fixture().await;
    let adapter = ProductionLoreAdapter::new(&fixture.api, &fixture.branch);
    let before = adapter
        .status()
        .await
        .expect("read exact pre-edit worktree fingerprint");
    std::fs::write(
        fixture._tempdir.path().join("asset.txt"),
        b"unstaged bytes after the exact staged subject",
    )
    .expect("write post-stage worktree edit");
    let after = adapter.status().await.expect("full production status scan");
    assert_eq!(before.worktree_files.len(), 1);
    assert_eq!(after.worktree_files.len(), 1);
    assert_ne!(
        before.worktree_files[0].local_hash, after.worktree_files[0].local_hash,
        "the exact raw fingerprint must expose a same-path edit even though Lore's staged tree intentionally retains the base hash"
    );
}

#[cfg(feature = "integration-tests")]
#[tokio::test]
async fn production_auth_unavailable_fails_closed_before_any_evidence_io_or_metadata_write() {
    let fixture = real_governance_fixture().await;
    let adapter = ProductionLoreAdapter::new(&fixture.api, &fixture.branch);
    let before = adapter
        .revision_metadata(&fixture.candidate)
        .await
        .expect("read exact metadata before production negative");
    let io = NoWriteIo::default();
    let outcome = evidence_preserve_with_adapters(
        &adapter,
        &io,
        &EvidencePreserveRequest {
            expected_staged_revision: fixture.candidate.clone(),
            target_base_revision: fixture.base,
        },
    )
    .await
    .expect("offline auth unavailability is an exact incomplete outcome");
    let residual = expect_residual(
        &outcome,
        false,
        EvidencePreserveStopCodeV1::InitialEvaluation,
    );
    assert_eq!(residual.stopped_at.code, "auth_unavailable");
    assert!(matches!(residual.close, EvidenceCloseStateV1::NotOpened));
    io.assert_unused("auth unavailable");
    assert_eq!(
        adapter.revision_metadata(&fixture.candidate).await.unwrap(),
        before
    );
}

#[cfg(feature = "integration-tests")]
#[tokio::test]
async fn each_unavailable_lock_leg_fails_before_any_evidence_io_or_metadata_write() {
    for lock in [
        InjectedLockDependency::QueryUnavailable,
        InjectedLockDependency::StatusUnavailable,
    ] {
        let fixture = real_governance_fixture().await;
        let adapter = HybridGovernanceAdapter::new(&fixture.api, &fixture.branch, lock);
        let before = adapter
            .revision_metadata(&fixture.candidate)
            .await
            .expect("read exact metadata before lock negative");
        let io = NoWriteIo::default();
        let outcome = evidence_preserve_with_adapters(
            &adapter,
            &io,
            &EvidencePreserveRequest {
                expected_staged_revision: fixture.candidate.clone(),
                target_base_revision: fixture.base,
            },
        )
        .await
        .expect("lock unavailability is an exact incomplete outcome");
        let residual = expect_residual(
            &outcome,
            false,
            EvidencePreserveStopCodeV1::InitialEvaluation,
        );
        assert_eq!(residual.stopped_at.code, "locks_unavailable");
        io.assert_unused("lock unavailable");
        assert_eq!(
            adapter.revision_metadata(&fixture.candidate).await.unwrap(),
            before
        );
    }
}
