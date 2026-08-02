//! Injectable, fail-closed Option-A evaluator.

use super::contract::{
    AdapterError, AffectedPath, EvaluationResult, ExactRevisionRequest, FileIdentity, LockQuery,
    LockStatus, LockStatusResponse, MetadataEntry, ResolvedAuthor, RevisionInfo,
    RevisionInfoResponse, StatusSnapshot, SupersessionMarkerV1, MAX_GOVERNANCE_HISTORY_REVISIONS,
    SUPERSESSION_MARKER_PREFIX,
};
use crate::api::LoreApi;
use crate::ops::{auth, file, revision};
use lore::interface::{LoreArray, LoreEvent, LoreEventCallback, LoreString};
use lore::lock::{LoreLockFileQueryArgs, LoreLockFileStatusArgs};
use lore::repository::{LoreRepositoryDumpArgs, LoreRepositoryStatusArgs};
use lore::revision::LoreRevisionInfoArgs;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// The production Lore adapter interface. Implementations must bind every call
/// to the explicit revisions and paths supplied by this evaluator.
#[async_trait::async_trait]
pub trait GovernanceAdapter {
    async fn status(&self) -> Result<StatusSnapshot, AdapterError>;
    async fn revision_info(&self, revision: &str) -> Result<RevisionInfoResponse, AdapterError>;
    async fn revision_metadata(&self, revision: &str) -> Result<Vec<MetadataEntry>, AdapterError>;
    /// Return candidate-side first-parent revisions only, not the target base,
    /// with the supplied sentinel limit applied at the Lore boundary.
    async fn first_parent_history(
        &self,
        candidate: &str,
        target_base: &str,
        max_revisions: usize,
    ) -> Result<Vec<String>, AdapterError>;
    /// Enumerate exact repository-relative paths for `revision`.
    async fn repository_dump(&self, revision: &str) -> Result<Vec<String>, AdapterError>;
    /// Return exactly one exact-revision identity per requested path.
    async fn file_info(
        &self,
        revision: &str,
        paths: &[String],
    ) -> Result<Vec<FileIdentity>, AdapterError>;
    async fn revision_diff(
        &self,
        base: &str,
        candidate: &str,
    ) -> Result<Vec<AffectedPath>, AdapterError>;
    async fn resolve_authors(
        &self,
        identities: &[String],
    ) -> Result<Vec<ResolvedAuthor>, AdapterError>;
    async fn lock_file_query(&self, path: &str) -> Result<LockQuery, AdapterError>;
    async fn lock_file_status(&self, paths: &[String]) -> Result<LockStatusResponse, AdapterError>;
}

/// In-process production adapter for the shipped Lore API. It uses the
/// existing typed bindings where their event contract is lossless, and retains
/// raw lock events through `End` where the older convenience wrappers discard
/// counts and ignored-path evidence.
pub struct ProductionLoreAdapter<'a> {
    api: &'a LoreApi,
    branch: String,
}

impl<'a> ProductionLoreAdapter<'a> {
    pub fn new(api: &'a LoreApi, branch: impl Into<String>) -> Self {
        Self {
            api,
            branch: branch.into(),
        }
    }
}

fn revision_only_status_args() -> LoreRepositoryStatusArgs {
    LoreRepositoryStatusArgs {
        staged: 0,
        scan: 0,
        check_dirty: 0,
        reset: 0,
        sync_point: 0,
        revision_only: 1,
        count: 0,
        paths: LoreArray::from_vec(Vec::new()),
    }
}

fn scanned_status_args() -> LoreRepositoryStatusArgs {
    LoreRepositoryStatusArgs {
        staged: 1,
        scan: 1,
        check_dirty: 0,
        reset: 0,
        sync_point: 0,
        revision_only: 0,
        count: 0,
        paths: LoreArray::from_vec(Vec::new()),
    }
}

async fn repository_status_stream(
    api: &LoreApi,
    args: LoreRepositoryStatusArgs,
) -> Result<RawEventStream, AdapterError> {
    let (callback, receiver) = raw_event_collector();
    let returned = lore::repository::status(api.globals().build(), args, callback).await;
    finished_raw_stream(receiver, returned).await
}

fn status_staged_revisions(
    stream: &RawEventStream,
    context: &str,
) -> Result<Vec<String>, AdapterError> {
    let revisions: Vec<_> = stream
        .events
        .iter()
        .filter_map(|event| match event {
            LoreEvent::RepositoryStatusRevision(data) => Some(data),
            _ => None,
        })
        .collect();
    if revisions.len() != 1 {
        return Err(AdapterError::new(format!(
            "{context} did not emit exactly one status revision"
        )));
    }
    Ok((!revisions[0].revision_staged.is_zero())
        .then_some(format!("{}", revisions[0].revision_staged))
        .into_iter()
        .collect())
}

#[async_trait::async_trait]
impl GovernanceAdapter for ProductionLoreAdapter<'_> {
    async fn status(&self) -> Result<StatusSnapshot, AdapterError> {
        let initial_stream =
            repository_status_stream(self.api, revision_only_status_args()).await?;
        let staged_revisions = status_staged_revisions(&initial_stream, "initial exact status")?;

        let scanned_stream = repository_status_stream(self.api, scanned_status_args()).await?;
        let scanned_staged_revisions =
            status_staged_revisions(&scanned_stream, "full scanned status")?;
        let mut staged_paths = BTreeSet::new();
        let mut worktree_clean = true;
        for event in scanned_stream.events {
            match event {
                LoreEvent::RepositoryStatusFile(data) => {
                    if data.flag_staged != 0 {
                        if !data.path.is_empty() {
                            staged_paths.insert(data.path.as_str().to_string());
                        }
                        if !data.from_path.is_empty() {
                            staged_paths.insert(data.from_path.as_str().to_string());
                        }
                    }
                    if data.flag_dirty != 0 || data.flag_conflict != 0 {
                        worktree_clean = false;
                    }
                }
                LoreEvent::PathIgnore(_) => {
                    return Err(AdapterError::new(
                        "repository status ignored a path during worktree verification",
                    ));
                }
                _ => {}
            }
        }

        let post_scan_stream =
            repository_status_stream(self.api, revision_only_status_args()).await?;
        let post_scan_staged_revisions =
            status_staged_revisions(&post_scan_stream, "post-scan exact status")?;
        if scanned_staged_revisions != staged_revisions
            || post_scan_staged_revisions != staged_revisions
        {
            return Err(AdapterError::new(
                "full status scan changed the exact staged subject",
            ));
        }

        Ok(StatusSnapshot {
            staged_revisions,
            scanned_staged_revisions,
            post_scan_staged_revisions,
            staged_paths: staged_paths.into_iter().collect(),
            worktree_clean,
            scan_performed: true,
        })
    }

    async fn revision_info(&self, revision_id: &str) -> Result<RevisionInfoResponse, AdapterError> {
        let (callback, receiver) = raw_event_collector();
        let returned = lore::revision::info(
            self.api.globals().build(),
            LoreRevisionInfoArgs {
                revision: LoreString::from_str(revision_id),
                delta: 0,
                metadata: 0,
            },
            callback,
        )
        .await;
        let stream = finished_raw_stream(receiver, returned).await?;
        let revisions = stream
            .events
            .into_iter()
            .filter_map(|event| match event {
                LoreEvent::RevisionInfo(data) => Some(RevisionInfo {
                    revision: format!("{}", data.revision),
                    parents: data
                        .parent
                        .iter()
                        .filter(|parent| !parent.is_zero())
                        .map(|parent| format!("{parent}"))
                        .collect(),
                }),
                _ => None,
            })
            .collect();
        Ok(RevisionInfoResponse { revisions })
    }

    async fn revision_metadata(
        &self,
        revision_id: &str,
    ) -> Result<Vec<MetadataEntry>, AdapterError> {
        let result = revision::metadata_list::metadata_list(
            self.api,
            revision::metadata_list::MetadataListArgs {
                revision: revision_id.into(),
            },
        )
        .await
        .map_err(adapter_error)?;
        Ok(result
            .entries
            .into_iter()
            .map(|entry| MetadataEntry::new(entry.key, entry.value))
            .collect())
    }

    async fn first_parent_history(
        &self,
        candidate: &str,
        target_base: &str,
        max_revisions: usize,
    ) -> Result<Vec<String>, AdapterError> {
        let length = u32::try_from(max_revisions)
            .map_err(|_| AdapterError::new("history sentinel exceeds upstream limit"))?;
        let result = revision::history::history(
            self.api,
            revision::history::RevisionHistoryArgs {
                revision: candidate.into(),
                branch: String::new(),
                date: 0,
                length,
                only_branch: false,
            },
        )
        .await
        .map_err(adapter_error)?;
        let mut entries = Vec::new();
        for entry in result.entries {
            if entry.revision == target_base {
                break;
            }
            entries.push(entry.revision);
        }
        Ok(entries)
    }

    async fn repository_dump(&self, revision_id: &str) -> Result<Vec<String>, AdapterError> {
        let (callback, receiver) = raw_event_collector();
        let returned = lore::repository::dump(
            self.api.globals().build(),
            LoreRepositoryDumpArgs {
                revision: LoreString::from_str(revision_id),
                path: LoreString::default(),
                max_depth: 0,
            },
            callback,
        )
        .await;
        let stream = finished_raw_stream(receiver, returned).await?;
        let mut begin = 0;
        let mut state = 0;
        let mut end = 0;
        let mut paths = BTreeSet::new();
        for event in stream.events {
            match event {
                LoreEvent::RepositoryDumpBegin(data) => {
                    begin += 1;
                    if format!("{}", data.revision) != revision_id {
                        return Err(AdapterError::new(
                            "repository dump fell back from exact revision",
                        ));
                    }
                }
                LoreEvent::RepositoryStateDump(data) => {
                    state += 1;
                    if format!("{}", data.revision) != revision_id {
                        return Err(AdapterError::new(
                            "repository dump state did not match exact revision",
                        ));
                    }
                }
                LoreEvent::RepositoryStateDumpNode(data) => {
                    let path = data.name.as_str().to_string();
                    if data.type_data.as_str().starts_with("addr ")
                        && (path.is_empty() || path.ends_with('/') || !paths.insert(path))
                    {
                        return Err(AdapterError::new(
                            "repository dump emitted malformed or duplicate file path",
                        ));
                    }
                }
                LoreEvent::RepositoryDumpEnd(_) => end += 1,
                _ => {}
            }
        }
        if begin != 1 || state != 1 || end != 1 {
            return Err(AdapterError::new("repository dump stream was incomplete"));
        }
        Ok(paths.into_iter().collect())
    }

    async fn file_info(
        &self,
        revision_id: &str,
        paths: &[String],
    ) -> Result<Vec<FileIdentity>, AdapterError> {
        let result = file::info::info(
            self.api,
            file::info::FileInfoArgs {
                paths: paths.to_vec(),
                revision: revision_id.into(),
                local: false,
                filtered: false,
            },
        )
        .await
        .map_err(adapter_error)?;
        if result.entries.iter().any(|entry| !entry.is_file) {
            return Err(AdapterError::new(
                "file info did not return files for every dump path",
            ));
        }
        Ok(result
            .entries
            .into_iter()
            .map(|entry| FileIdentity::new(entry.path, revision_id, entry.hash, entry.context))
            .collect())
    }

    async fn revision_diff(
        &self,
        base: &str,
        candidate: &str,
    ) -> Result<Vec<AffectedPath>, AdapterError> {
        let result = revision::diff::diff(
            self.api,
            revision::diff::RevisionDiffArgs {
                revision_source: base.into(),
                revision_target: candidate.into(),
                paths: Vec::new(),
            },
        )
        .await
        .map_err(adapter_error)?;
        Ok(result
            .files
            .into_iter()
            .map(|file| AffectedPath::modified(file.path))
            .collect())
    }

    async fn resolve_authors(
        &self,
        identities: &[String],
    ) -> Result<Vec<ResolvedAuthor>, AdapterError> {
        let result = auth::resolve_user_info::resolve_user_info(
            self.api,
            auth::resolve_user_info::ResolveUserInfoArgs {
                user_ids: identities.to_vec(),
            },
        )
        .await
        .map_err(adapter_error)?;
        Ok(result
            .users
            .into_iter()
            .map(|user| ResolvedAuthor::new(user.user_id, user.display_name))
            .collect())
    }

    async fn lock_file_query(&self, path: &str) -> Result<LockQuery, AdapterError> {
        let (callback, receiver) = raw_event_collector();
        let returned = lore::lock::file_query(
            self.api.globals().build(),
            LoreLockFileQueryArgs {
                branch: LoreString::from_str(&self.branch),
                owner: LoreString::default(),
                path: LoreString::from_str(path),
            },
            callback,
        )
        .await;
        let stream = finished_raw_stream(receiver, returned).await?;
        let mut begins = 0;
        let mut expected = None;
        let mut owners = Vec::new();
        let mut ignored = Vec::new();
        for event in stream.events {
            match event {
                LoreEvent::LockFileQueryBegin(data) => {
                    begins += 1;
                    expected = Some(data.count);
                }
                LoreEvent::LockFileQuery(data) => {
                    if data.path.as_str() != path {
                        return Err(AdapterError::new("lock query returned a foreign path"));
                    }
                    owners.push(data.owner.as_str().to_string());
                }
                LoreEvent::PathIgnore(data) => ignored.push(data.path.as_str().to_string()),
                _ => {}
            }
        }
        let expected_count = expected
            .ok_or_else(|| AdapterError::new("lock query emitted no begin event"))?
            .try_into()
            .map_err(|_| AdapterError::new("lock query count overflow"))?;
        if begins != 1 || !ignored.is_empty() || expected_count != owners.len() {
            return Err(AdapterError::new("lock query stream was incomplete"));
        }
        Ok(LockQuery {
            path: path.into(),
            begin_events: begins,
            expected_count,
            completed: true,
            ignored_paths: ignored,
            owners,
        })
    }

    async fn lock_file_status(&self, paths: &[String]) -> Result<LockStatusResponse, AdapterError> {
        let (callback, receiver) = raw_event_collector();
        let returned = lore::lock::file_status(
            self.api.globals().build(),
            LoreLockFileStatusArgs {
                paths: LoreArray::from_vec(
                    paths
                        .iter()
                        .map(|path| LoreString::from_str(path))
                        .collect(),
                ),
                branch: LoreString::from_str(&self.branch),
            },
            callback,
        )
        .await;
        let stream = finished_raw_stream(receiver, returned).await?;
        let requested: BTreeSet<&str> = paths.iter().map(String::as_str).collect();
        let mut begins = 0;
        let mut expected = None;
        let mut statuses = Vec::new();
        let mut ignored = Vec::new();
        for event in stream.events {
            match event {
                LoreEvent::LockFileStatusBegin(data) => {
                    begins += 1;
                    expected = Some(data.count);
                }
                LoreEvent::LockFileStatus(data) => {
                    let path = data.path.as_str().to_string();
                    if !requested.contains(path.as_str()) {
                        return Err(AdapterError::new("lock status returned a foreign path"));
                    }
                    statuses.push(LockStatus::locked(path, data.owner.as_str()));
                }
                LoreEvent::PathIgnore(data) => ignored.push(data.path.as_str().to_string()),
                _ => {}
            }
        }
        let expected_count = expected
            .ok_or_else(|| AdapterError::new("lock status emitted no begin event"))?
            .try_into()
            .map_err(|_| AdapterError::new("lock status count overflow"))?;
        if begins != 1 || !ignored.is_empty() || expected_count != statuses.len() {
            return Err(AdapterError::new("lock status stream was incomplete"));
        }
        Ok(LockStatusResponse {
            begin_events: begins,
            expected_count,
            completed: true,
            ignored_paths: ignored,
            statuses,
        })
    }
}

fn adapter_error(error: impl std::fmt::Display) -> AdapterError {
    AdapterError::new(error.to_string())
}

#[derive(Default)]
struct RawEventStream {
    events: Vec<LoreEvent>,
    complete_events: usize,
    complete_status: Option<i32>,
    error_events: usize,
    end_events: usize,
}

fn raw_event_collector() -> (LoreEventCallback, oneshot::Receiver<RawEventStream>) {
    let (sender, receiver) = oneshot::channel();
    let stream = Arc::new(Mutex::new(RawEventStream::default()));
    let sender = Arc::new(Mutex::new(Some(sender)));
    let callback = Box::new(move |event: &LoreEvent| {
        let mut locked = stream.lock().expect("raw event collector mutex poisoned");
        match event {
            LoreEvent::Complete(data) => {
                locked.complete_events += 1;
                locked.complete_status = Some(data.status);
            }
            LoreEvent::Error(_) => locked.error_events += 1,
            LoreEvent::End(_) => locked.end_events += 1,
            _ => {}
        }
        locked.events.push(event.clone());
        if matches!(event, LoreEvent::End(_)) {
            let final_stream = std::mem::take(&mut *locked);
            drop(locked);
            if let Some(sender) = sender
                .lock()
                .expect("raw event sender mutex poisoned")
                .take()
            {
                let _ = sender.send(final_stream);
            }
        }
    });
    (Some(callback), receiver)
}

async fn finished_raw_stream(
    receiver: oneshot::Receiver<RawEventStream>,
    returned: i32,
) -> Result<RawEventStream, AdapterError> {
    let stream = receiver.await.map_err(|error| {
        AdapterError::new(format!("raw event stream ended before End: {error}"))
    })?;
    if returned != 0
        || stream.end_events != 1
        || stream.complete_events != 1
        || stream.complete_status != Some(0)
        || stream.error_events != 0
    {
        return Err(AdapterError::new(
            "raw Lore stream failed terminal validation",
        ));
    }
    Ok(stream)
}

impl EvaluationResult {
    fn closed(code: impl Into<String>) -> Self {
        Self {
            open: false,
            pending_revisions: Vec::new(),
            affected_paths: Vec::new(),
            identities: Vec::new(),
            superseded_identities: Vec::new(),
            failure_codes: vec![code.into()],
        }
    }
}

/// Evaluate the strict Option-A request through an injectable production
/// adapter. Every unavailable or ambiguous dependency closes the result.
pub async fn evaluate<A, R>(adapter: &A, request: &R) -> EvaluationResult
where
    A: GovernanceAdapter,
    R: ExactRevisionRequest,
{
    let expected = request.expected_staged_revision();
    let base = request.target_base_revision();
    if expected.is_empty() || base.is_empty() {
        return EvaluationResult::closed("exact_subject_failed");
    }

    let status = match adapter.status().await {
        Ok(status) => status,
        Err(_) => return EvaluationResult::closed("status_unavailable"),
    };
    if !status.scan_performed {
        return EvaluationResult::closed("worktree_unverified");
    }
    if [
        &status.staged_revisions,
        &status.scanned_staged_revisions,
        &status.post_scan_staged_revisions,
    ]
    .into_iter()
    .any(|revisions| revisions.len() != 1 || revisions[0].is_empty() || revisions[0] != expected)
    {
        return EvaluationResult::closed("exact_subject_failed");
    }

    let base_info = match exact_info(adapter, base).await {
        Ok(info) => info,
        Err(()) => return EvaluationResult::closed("history_incomplete"),
    };
    if base_info.revision != base {
        return EvaluationResult::closed("history_incomplete");
    }

    let pending = match pending_dag(adapter, expected, base).await {
        Ok(revisions) => revisions,
        Err(GraphFailure::Incomplete) => return EvaluationResult::closed("history_incomplete"),
        Err(GraphFailure::Depth) => return EvaluationResult::closed("history_depth_exceeded"),
    };

    let history = match adapter
        .first_parent_history(expected, base, MAX_GOVERNANCE_HISTORY_REVISIONS + 1)
        .await
    {
        Ok(history) => history,
        Err(_) => return EvaluationResult::closed("history_incomplete"),
    };
    if history.len() >= MAX_GOVERNANCE_HISTORY_REVISIONS + 1 {
        return EvaluationResult::closed("history_depth_exceeded");
    }
    let expected_history = match first_parent_history(adapter, expected, base).await {
        Ok(history) => history,
        Err(()) => return EvaluationResult::closed("history_incomplete"),
    };
    if history != expected_history || history.is_empty() {
        return EvaluationResult::closed("history_incomplete");
    }

    let superseded = match validate_metadata_and_dco(adapter, base, &pending).await {
        Ok(superseded) => superseded,
        Err(Failure::Metadata) => return EvaluationResult::closed("metadata_unavailable"),
        Err(Failure::Dco) => return EvaluationResult::closed("dco_invalid"),
        Err(Failure::Auth) => return EvaluationResult::closed("auth_unavailable"),
        Err(Failure::Supersession) => return EvaluationResult::closed("supersession_invalid"),
    };

    let candidate_tree = match exact_tree(adapter, expected).await {
        Ok(tree) => tree,
        Err(TreeFailure::Dependency) => return EvaluationResult::closed("tree_unavailable"),
        Err(TreeFailure::FileInfo) => return EvaluationResult::closed("file_info_unavailable"),
        Err(TreeFailure::Invalid) => return EvaluationResult::closed("tree_invalid"),
    };
    let base_tree = match exact_tree(adapter, base).await {
        Ok(tree) => tree,
        Err(TreeFailure::Dependency) => return EvaluationResult::closed("tree_unavailable"),
        Err(TreeFailure::FileInfo) => return EvaluationResult::closed("file_info_unavailable"),
        Err(TreeFailure::Invalid) => return EvaluationResult::closed("tree_invalid"),
    };

    let mut identities: Vec<String> = candidate_tree.values().cloned().collect();
    identities.sort();
    if superseded
        .iter()
        .any(|identity| candidate_tree.values().any(|id| id == identity))
    {
        return EvaluationResult::closed("not_superseded_failed");
    }

    let affected_paths = match affected_paths(
        adapter,
        base,
        expected,
        &status,
        &base_tree,
        &candidate_tree,
    )
    .await
    {
        Ok(paths) => paths,
        Err(()) => return EvaluationResult::closed("affected_paths_unavailable"),
    };
    if affected_paths.is_empty() {
        return EvaluationResult::closed("empty_submission");
    }

    match validate_locks(adapter, &affected_paths).await {
        Ok(()) => {}
        Err(LockFailure::Dependency) => return EvaluationResult::closed("locks_unavailable"),
        Err(LockFailure::Locked) => return EvaluationResult::closed("locks_clear_failed"),
    }

    if !status.worktree_clean {
        return EvaluationResult::closed("worktree_dirty");
    }

    EvaluationResult {
        open: true,
        pending_revisions: pending,
        affected_paths,
        identities,
        superseded_identities: superseded,
        failure_codes: Vec::new(),
    }
}

async fn exact_info<A: GovernanceAdapter>(adapter: &A, revision: &str) -> Result<RevisionInfo, ()> {
    let response = adapter.revision_info(revision).await.map_err(|_| ())?;
    if response.revisions.len() != 1 {
        return Err(());
    }
    let info = response.revisions.into_iter().next().ok_or(())?;
    if info.revision.is_empty() || info.revision != revision {
        return Err(());
    }
    Ok(info)
}

enum GraphFailure {
    Incomplete,
    Depth,
}

async fn pending_dag<A: GovernanceAdapter>(
    adapter: &A,
    candidate: &str,
    base: &str,
) -> Result<Vec<String>, GraphFailure> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Visit {
        Active,
        Complete,
    }

    let mut visit = BTreeMap::<String, Visit>::new();
    let mut pending = BTreeSet::new();
    let mut stack = vec![(candidate.to_string(), false)];

    while let Some((revision, leaving)) = stack.pop() {
        if revision == base {
            continue;
        }
        if leaving {
            visit.insert(revision, Visit::Complete);
            continue;
        }
        match visit.get(&revision) {
            Some(Visit::Active) => return Err(GraphFailure::Incomplete),
            Some(Visit::Complete) => continue,
            None => {}
        }

        // Fetch and exact-verify the 1001st unique revision as the overflow
        // sentinel, then stop without walking the rest of the arbitrary DAG.
        let info = exact_info(adapter, &revision)
            .await
            .map_err(|_| GraphFailure::Incomplete)?;
        if info.parents.is_empty()
            || info
                .parents
                .iter()
                .any(|parent| parent.is_empty() || parent == &revision)
        {
            return Err(GraphFailure::Incomplete);
        }
        if pending.len() == MAX_GOVERNANCE_HISTORY_REVISIONS {
            return Err(GraphFailure::Depth);
        }
        visit.insert(revision.clone(), Visit::Active);
        pending.insert(revision.clone());
        stack.push((revision, true));
        for parent in info.parents.into_iter().rev() {
            stack.push((parent, false));
        }
    }

    if pending.is_empty() {
        return Err(GraphFailure::Incomplete);
    }
    Ok(pending.into_iter().collect())
}

async fn first_parent_history<A: GovernanceAdapter>(
    adapter: &A,
    candidate: &str,
    base: &str,
) -> Result<Vec<String>, ()> {
    let mut revisions = Vec::new();
    let mut seen = BTreeSet::new();
    let mut revision = candidate.to_string();

    while revision != base {
        if !seen.insert(revision.clone()) || revisions.len() >= MAX_GOVERNANCE_HISTORY_REVISIONS + 1
        {
            return Err(());
        }
        let info = exact_info(adapter, &revision).await?;
        let Some(parent) = info.parents.first() else {
            return Err(());
        };
        if parent.is_empty() {
            return Err(());
        }
        revisions.push(revision);
        revision = parent.clone();
    }

    Ok(revisions)
}

enum Failure {
    Metadata,
    Dco,
    Auth,
    Supersession,
}

async fn validate_metadata_and_dco<A: GovernanceAdapter>(
    adapter: &A,
    base: &str,
    pending: &[String],
) -> Result<Vec<String>, Failure> {
    let mut records = BTreeMap::<String, String>::new();
    let mut authors = BTreeSet::new();
    let mut revision_signers = Vec::<(String, Vec<String>)>::new();

    let base_metadata = adapter
        .revision_metadata(base)
        .await
        .map_err(|_| Failure::Metadata)?;
    scan_supersession_entries(&base_metadata, &mut records)?;

    for revision in pending {
        let metadata = adapter
            .revision_metadata(revision)
            .await
            .map_err(|_| Failure::Metadata)?;
        scan_supersession_entries(&metadata, &mut records)?;
        let mut grouped = BTreeMap::<String, Vec<String>>::new();
        for entry in metadata {
            grouped.entry(entry.key).or_default().push(entry.value);
        }

        let message = exactly_one(&grouped, "message").ok_or(Failure::Dco)?;
        let created_by = exactly_one(&grouped, "created-by").ok_or(Failure::Dco)?;
        if created_by.is_empty()
            || grouped
                .get("committed-by")
                .is_some_and(|values| values.len() != 1)
        {
            return Err(Failure::Dco);
        }
        let mut revision_authors = vec![created_by.to_string()];
        if let Some(committed_by) = grouped
            .get("committed-by")
            .and_then(|values| values.first())
        {
            if committed_by.is_empty() {
                return Err(Failure::Dco);
            }
            revision_authors.push(committed_by.clone());
        }
        let signer = parse_dco_signer(message).ok_or(Failure::Dco)?;
        authors.extend(revision_authors.iter().cloned());
        revision_signers.push((signer, revision_authors));
    }

    if authors.is_empty() {
        return Err(Failure::Dco);
    }
    let requested: Vec<String> = authors.into_iter().collect();
    let replies = adapter
        .resolve_authors(&requested)
        .await
        .map_err(|_| Failure::Auth)?;
    if replies.len() != requested.len() {
        return Err(Failure::Dco);
    }
    let mut resolved = BTreeMap::new();
    for reply in replies {
        if reply.identity.is_empty()
            || reply.display_name.is_empty()
            || !requested.contains(&reply.identity)
            || resolved
                .insert(reply.identity, reply.display_name)
                .is_some()
        {
            return Err(Failure::Dco);
        }
    }
    for (signer, identities) in revision_signers {
        if identities
            .iter()
            .any(|identity| resolved.get(identity).is_none_or(|name| name != &signer))
        {
            return Err(Failure::Dco);
        }
    }

    Ok(records.into_keys().collect())
}

fn scan_supersession_entries(
    metadata: &[MetadataEntry],
    records: &mut BTreeMap<String, String>,
) -> Result<(), Failure> {
    for entry in metadata {
        if !entry.key.starts_with(SUPERSESSION_MARKER_PREFIX) {
            continue;
        }
        let identity = entry.key[SUPERSESSION_MARKER_PREFIX.len()..].to_string();
        let marker: SupersessionMarkerV1 =
            serde_json::from_str(&entry.value).map_err(|_| Failure::Supersession)?;
        if identity.is_empty()
            || marker.version != "v1"
            || marker.identity.is_empty()
            || marker.identity != identity
        {
            return Err(Failure::Supersession);
        }
        match records.get(&identity) {
            Some(existing) if existing != &entry.value => return Err(Failure::Supersession),
            _ => {
                records.insert(identity, entry.value.clone());
            }
        }
    }
    Ok(())
}

fn exactly_one<'a>(metadata: &'a BTreeMap<String, Vec<String>>, key: &str) -> Option<&'a str> {
    let values = metadata.get(key)?;
    (values.len() == 1).then(|| values[0].as_str())
}

fn parse_dco_signer(message: &str) -> Option<String> {
    let mut lines: Vec<&str> = message.lines().collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let end = lines.len();
    let mut start = end;
    while start > 0 && parse_trailer(lines[start - 1]).is_some() {
        start -= 1;
    }
    if start == end
        || start == 0
        || !lines[start - 1].is_empty()
        || !lines[..start - 1].iter().any(|line| !line.is_empty())
        || lines[..start]
            .iter()
            .any(|line| line.starts_with("Signed-off-by:"))
    {
        return None;
    }

    let signers: Vec<&str> = lines[start..end]
        .iter()
        .filter_map(|line| {
            let (key, value) = parse_trailer(line)?;
            (key == "Signed-off-by").then_some(value)
        })
        .collect();
    if signers.len() != 1 {
        return None;
    }
    let signer = signers[0];
    let (name, email_with_bracket) = signer.rsplit_once(" <")?;
    let email = email_with_bracket.strip_suffix('>')?;
    if name.trim().is_empty() || email.is_empty() || email.contains(char::is_whitespace) {
        return None;
    }
    Some(name.trim().to_string())
}

fn parse_trailer(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(": ")?;
    if key.is_empty()
        || !key.as_bytes()[0].is_ascii_alphanumeric()
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || value.is_empty()
        || value.trim() != value
    {
        return None;
    }
    Some((key, value))
}

enum TreeFailure {
    Dependency,
    FileInfo,
    Invalid,
}

async fn exact_tree<A: GovernanceAdapter>(
    adapter: &A,
    revision: &str,
) -> Result<BTreeMap<String, String>, TreeFailure> {
    let paths = adapter
        .repository_dump(revision)
        .await
        .map_err(|_| TreeFailure::Dependency)?;
    let expected_paths: BTreeSet<String> = paths.iter().cloned().collect();
    if paths.iter().any(|path| path.is_empty()) || expected_paths.len() != paths.len() {
        return Err(TreeFailure::Invalid);
    }
    // `file.info` with an empty path set must never be used as a root/tree
    // enumerator. The exact `repository.dump` enumeration above is authoritative.
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }
    let identities = adapter
        .file_info(revision, &paths)
        .await
        .map_err(|_| TreeFailure::FileInfo)?;
    if identities.len() != paths.len() {
        return Err(TreeFailure::Invalid);
    }

    let mut tree = BTreeMap::new();
    let mut canonical_ids = BTreeSet::new();
    for identity in identities {
        if identity.revision != revision
            || identity.path.is_empty()
            || identity.hash.is_empty()
            || identity.context.is_empty()
            || !expected_paths.contains(&identity.path)
        {
            return Err(TreeFailure::Invalid);
        }
        let canonical_id = identity.canonical_id();
        if tree.insert(identity.path, canonical_id.clone()).is_some()
            || !canonical_ids.insert(canonical_id)
        {
            return Err(TreeFailure::Invalid);
        }
    }
    if tree.len() != expected_paths.len() {
        return Err(TreeFailure::Invalid);
    }
    Ok(tree)
}

async fn affected_paths<A: GovernanceAdapter>(
    adapter: &A,
    base: &str,
    candidate: &str,
    status: &StatusSnapshot,
    base_tree: &BTreeMap<String, String>,
    candidate_tree: &BTreeMap<String, String>,
) -> Result<Vec<String>, ()> {
    let diff = adapter
        .revision_diff(base, candidate)
        .await
        .map_err(|_| ())?;
    let mut paths = BTreeSet::new();
    for file in diff {
        if let Some(source) = file.source_path.filter(|path| !path.is_empty()) {
            paths.insert(source);
        }
        if let Some(target) = file.target_path.filter(|path| !path.is_empty()) {
            paths.insert(target);
        }
    }
    paths.extend(
        status
            .staged_paths
            .iter()
            .filter(|path| !path.is_empty())
            .cloned(),
    );
    for path in base_tree.keys().chain(candidate_tree.keys()) {
        if base_tree.get(path) != candidate_tree.get(path) {
            paths.insert(path.clone());
        }
    }
    Ok(paths.into_iter().collect())
}

enum LockFailure {
    Dependency,
    Locked,
}

async fn validate_locks<A: GovernanceAdapter>(
    adapter: &A,
    paths: &[String],
) -> Result<(), LockFailure> {
    let requested: BTreeSet<String> = paths.iter().cloned().collect();
    if requested.len() != paths.len() || requested.is_empty() {
        return Err(LockFailure::Dependency);
    }
    for path in paths {
        let query = adapter
            .lock_file_query(path)
            .await
            .map_err(|_| LockFailure::Dependency)?;
        if query.path != *path {
            return Err(LockFailure::Dependency);
        }
        if query.begin_events != 1
            || !query.completed
            || !query.ignored_paths.is_empty()
            || query.expected_count != query.owners.len()
        {
            return Err(LockFailure::Dependency);
        }
        if !query.owners.is_empty() {
            return Err(LockFailure::Locked);
        }
    }
    let status = adapter
        .lock_file_status(paths)
        .await
        .map_err(|_| LockFailure::Dependency)?;
    if status.begin_events != 1
        || !status.completed
        || !status.ignored_paths.is_empty()
        || status.expected_count != status.statuses.len()
    {
        return Err(LockFailure::Dependency);
    }
    let mut observed = BTreeSet::new();
    for record in status.statuses {
        if !requested.contains(&record.path) || !observed.insert(record.path) {
            return Err(LockFailure::Dependency);
        }
        if record.owner.is_some() {
            return Err(LockFailure::Locked);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{revision_only_status_args, scanned_status_args};

    #[test]
    fn production_status_arguments_pin_exact_reads_and_full_scan() {
        let exact = revision_only_status_args();
        assert_eq!(
            (
                exact.staged,
                exact.scan,
                exact.check_dirty,
                exact.reset,
                exact.sync_point,
                exact.revision_only,
                exact.count,
            ),
            (0, 0, 0, 0, 0, 1, 0)
        );
        assert!(exact.paths.is_empty());

        let scan = scanned_status_args();
        assert_eq!(
            (
                scan.staged,
                scan.scan,
                scan.check_dirty,
                scan.reset,
                scan.sync_point,
                scan.revision_only,
                scan.count,
            ),
            (1, 1, 0, 0, 0, 0, 0)
        );
        assert!(scan.paths.is_empty());
    }
}
