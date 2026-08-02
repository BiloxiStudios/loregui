//! Behavioral contract and fail-closed evaluator matrix for SBAI-5934 Option A.

use lore_vm::ops::governance::contract::{
    ArtifactMarkSupersededRequest, DcoValidateRequest, EvidencePreserveRequest,
    GovernanceCriterion, SubmissionGateCheckRequest, EVIDENCE_POINTER_KEY,
    MAX_GOVERNANCE_HISTORY_REVISIONS, SUPERSESSION_MARKER_PREFIX,
};
use lore_vm::ops::governance::evaluator::{
    evaluate, AdapterError, AffectedPath, FileIdentity, GovernanceAdapter, LockQuery, LockStatus,
    LockStatusResponse, MetadataEntry, ResolvedAuthor, RevisionInfo, StatusSnapshot,
};
use std::collections::BTreeMap;

#[derive(Clone)]
struct FakeLore {
    status: Result<StatusSnapshot, AdapterError>,
    infos: BTreeMap<String, Result<RevisionInfo, AdapterError>>,
    metadata: BTreeMap<String, Result<Vec<MetadataEntry>, AdapterError>>,
    history: Result<Vec<String>, AdapterError>,
    dumps: BTreeMap<String, Result<Vec<String>, AdapterError>>,
    file_info: BTreeMap<String, Result<Vec<FileIdentity>, AdapterError>>,
    diff: Result<Vec<AffectedPath>, AdapterError>,
    authors: Result<Vec<ResolvedAuthor>, AdapterError>,
    lock_queries: BTreeMap<String, Result<LockQuery, AdapterError>>,
    lock_status: Result<LockStatusResponse, AdapterError>,
    history_limits: std::sync::Arc<std::sync::Mutex<Vec<usize>>>,
}

impl FakeLore {
    fn clean() -> Self {
        let mut infos = BTreeMap::new();
        infos.insert(
            "candidate".into(),
            Ok(RevisionInfo {
                revision: "candidate".into(),
                parents: vec!["base".into()],
            }),
        );
        infos.insert(
            "base".into(),
            Ok(RevisionInfo {
                revision: "base".into(),
                parents: vec![],
            }),
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
        dumps.insert("base".into(), Ok(vec![]));

        let mut file_info = BTreeMap::new();
        file_info.insert(
            "candidate".into(),
            Ok(vec![FileIdentity::new(
                "asset.txt",
                "candidate",
                "hash-1",
                "context-1",
            )]),
        );
        file_info.insert("base".into(), Ok(vec![]));

        let mut lock_queries = BTreeMap::new();
        lock_queries.insert("asset.txt".into(), Ok(LockQuery::unlocked("asset.txt")));

        Self {
            status: Ok(StatusSnapshot {
                staged_revisions: vec!["candidate".into()],
                staged_paths: vec!["asset.txt".into()],
                worktree_clean: true,
            }),
            infos,
            metadata,
            history: Ok(vec!["candidate".into()]),
            dumps,
            file_info,
            diff: Ok(vec![AffectedPath::modified("asset.txt")]),
            authors: Ok(vec![ResolvedAuthor::new("alice", "Alice")]),
            lock_queries,
            lock_status: Ok(LockStatusResponse::unlocked()),
            history_limits: Default::default(),
        }
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
                Ok(RevisionInfo {
                    revision: revision.clone(),
                    parents: vec![parent],
                }),
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
            Ok(RevisionInfo {
                revision: "candidate".into(),
                parents: vec![parent],
            }),
        );
        history.extend((1..count).map(|index| format!("pending-{index}")));
        fake.history = Ok(history);
        fake
    }
}

#[async_trait::async_trait]
impl GovernanceAdapter for FakeLore {
    async fn status(&self) -> Result<StatusSnapshot, AdapterError> {
        self.status.clone()
    }

    async fn revision_info(&self, revision: &str) -> Result<RevisionInfo, AdapterError> {
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
    ) -> Result<Vec<AffectedPath>, AdapterError> {
        self.diff.clone()
    }

    async fn resolve_authors(
        &self,
        _identities: &[String],
    ) -> Result<Vec<ResolvedAuthor>, AdapterError> {
        self.authors.clone()
    }

    async fn lock_file_query(&self, path: &str) -> Result<LockQuery, AdapterError> {
        self.lock_queries
            .get(path)
            .cloned()
            .unwrap_or_else(|| Err(Self::error("missing lock query")))
    }

    async fn lock_file_status(
        &self,
        _paths: &[String],
    ) -> Result<LockStatusResponse, AdapterError> {
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
}

#[test]
fn traverses_both_merge_parents_and_rejects_depth_overflow() {
    let mut merge = FakeLore::clean();
    merge.infos.insert(
        "candidate".into(),
        Ok(RevisionInfo {
            revision: "candidate".into(),
            parents: vec!["left".into(), "right".into()],
        }),
    );
    for revision in ["left", "right"] {
        merge.infos.insert(
            revision.into(),
            Ok(RevisionInfo {
                revision: revision.into(),
                parents: vec!["base".into()],
            }),
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
        "history_depth_exceeded",
    );
}

#[test]
fn accepts_500_999_and_1000_clean_pending_nodes() {
    for count in [500_usize, 999, 1000] {
        let fake = FakeLore::with_linear_pending_count(count);
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(evaluate(&fake, &request()));
        assert!(result.open, "count {count}: {result:?}");
    }
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
            fake.status = Ok(StatusSnapshot {
                staged_revisions: vec![],
                staged_paths: vec![],
                worktree_clean: true,
            })
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
            fake.status = Ok(StatusSnapshot {
                staged_revisions: vec!["candidate".into(), "extra".into()],
                staged_paths: vec![],
                worktree_clean: true,
            })
        },
        "exact_subject_failed",
    );
    assert_closed_for(
        FakeLore::clean(),
        |fake| {
            fake.status = Ok(StatusSnapshot {
                staged_revisions: vec!["other".into()],
                staged_paths: vec![],
                worktree_clean: true,
            })
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
                "base".into(),
                Ok(RevisionInfo {
                    revision: "fallback".into(),
                    parents: vec![],
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
                Ok(RevisionInfo {
                    revision: "wrong".into(),
                    parents: vec!["base".into()],
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
                Ok(RevisionInfo {
                    revision: "candidate".into(),
                    parents: vec!["candidate".into()],
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
                Ok(RevisionInfo {
                    revision: "candidate".into(),
                    parents: vec![],
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
                Ok(RevisionInfo {
                    revision: "candidate".into(),
                    parents: vec!["unreadable-parent".into()],
                }),
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
                    "studiobrain.governance.v1.superseded.hash-1:context-1",
                    r#"{"version":"v2","identity":"hash-1:context-1"}"#,
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
                    "studiobrain.governance.v1.superseded.hash-1:context-1",
                    r#"{"version":"v1","identity":"different"}"#,
                ));
        },
        "supersession_invalid",
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
                    FileIdentity::new("asset.txt", "candidate", "hash-1", "context-1"),
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
                    "hash-1",
                    "context-1",
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
                    "context-1",
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
            fake.dumps
                .insert("base".into(), Ok(vec!["asset.txt".into()]));
            fake.file_info.insert(
                "base".into(),
                Ok(vec![FileIdentity::new(
                    "asset.txt",
                    "base",
                    "hash-1",
                    "context-1",
                )]),
            );
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
