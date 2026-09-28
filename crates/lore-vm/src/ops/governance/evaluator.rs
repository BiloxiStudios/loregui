//! Injectable, fail-closed Option-A evaluator.

use super::contract::{
    AdapterError, AffectedPath, AuthorResolutionObservation, DcoMetadataObservation,
    DcoObservation, DcoValidateResult, EvaluationResult, ExactRevisionRequest, FileIdentity,
    GovernanceObservations, GovernancePathAction, GovernanceRemediation, GovernanceRemediationCode,
    HistoryOverflowScope, LockQuery, LockStatus, LockStatusResponse, MetadataEntry, MetadataKind,
    ResolvedAuthor, RevisionDiffObservation, RevisionInfo, RevisionInfoResponse,
    StagedPathObservation, StatusSnapshot, SupersessionMarkerV1,
    SupersessionMetadataQueryObservation, SupersessionObservation, WorktreeFileObservation,
    MAX_GOVERNANCE_HISTORY_REVISIONS, SUPERSESSION_MARKER_PREFIX,
};
use crate::api::LoreApi;
use lore::auth::LoreAuthUserInfoArgs;
use lore::file::LoreFileInfoArgs;
use lore::interface::{LoreArray, LoreEvent, LoreEventCallback, LoreMetadata, LoreString};
use lore::lock::{LoreLockFileQueryArgs, LoreLockFileStatusArgs};
use lore::repository::{LoreRepositoryDumpArgs, LoreRepositoryStatusArgs};
use lore::revision::{
    LoreRevisionDiffArgs, LoreRevisionHistoryArgs, LoreRevisionInfoArgs,
    LoreRevisionMetadataListArgs,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// The production Lore adapter interface. Implementations must bind every call
/// to the explicit revisions and paths supplied by this evaluator.
#[async_trait::async_trait]
pub trait GovernanceAdapter {
    /// Read only the exact staged subject. Production overrides this with a
    /// revision-only call so DCO does not depend on a filesystem scan.
    async fn exact_staged_revisions(&self) -> Result<Vec<String>, AdapterError> {
        Ok(self.status().await?.staged_revisions)
    }
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
    ) -> Result<Vec<RevisionDiffObservation>, AdapterError>;
    async fn resolve_authors(
        &self,
        identities: &[String],
    ) -> Result<Vec<ResolvedAuthor>, AdapterError>;
    async fn lock_file_query(&self, branch: &str, path: &str) -> Result<LockQuery, AdapterError>;
    async fn lock_file_status(
        &self,
        branch: &str,
        paths: &[String],
    ) -> Result<LockStatusResponse, AdapterError>;
}

/// In-process production adapter for the shipped Lore API. It uses the
/// existing typed bindings where their event contract is lossless, and retains
/// raw lock events through `End` where the older convenience wrappers discard
/// counts and ignored-path evidence.
pub struct ProductionLoreAdapter<'a> {
    api: &'a LoreApi,
}

impl<'a> ProductionLoreAdapter<'a> {
    pub fn new(api: &'a LoreApi, _branch: impl Into<String>) -> Self {
        Self { api }
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
    let (callback, stream) = raw_event_collector();
    let returned = lore::repository::status(api.globals().build(), args, callback).await;
    finished_raw_stream(stream, returned)
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

fn status_branch(stream: &RawEventStream, context: &str) -> Result<String, AdapterError> {
    let branches: Vec<_> = stream
        .events
        .iter()
        .filter_map(|event| match event {
            LoreEvent::RepositoryStatusRevision(data) => Some(&data.branch),
            _ => None,
        })
        .collect();
    if branches.len() != 1 || branches[0].is_zero() {
        return Err(AdapterError::new(format!(
            "{context} did not emit exactly one nonzero branch"
        )));
    }
    Ok(format!("{}", branches[0]))
}

#[derive(Debug)]
struct RawFileInfoEntry {
    path: String,
    context: String,
    hash: String,
    is_file: bool,
    flag_modified: bool,
    flag_deleted: bool,
    flag_added: bool,
    flag_conflict: bool,
    size: u64,
    local_size: u64,
    local_hash: String,
    filter_size: u64,
}

fn lore_file_info_path(path: &str, repository_path: &Path) -> LoreString {
    if Path::new(path).is_absolute() {
        LoreString::from_str(path)
    } else {
        LoreString::from_path(repository_path.join(path))
    }
}

async fn raw_file_info(
    api: &LoreApi,
    paths: &[String],
    revision: &str,
    local: bool,
    filtered: bool,
) -> Result<Vec<RawFileInfoEntry>, AdapterError> {
    let globals = api.globals();
    let repository_path = globals.repository_path.clone();
    let (callback, stream) = raw_event_collector();
    let returned = lore::file::info(
        globals.build(),
        LoreFileInfoArgs {
            paths: LoreArray::from_vec(
                paths
                    .iter()
                    .map(|path| lore_file_info_path(path, &repository_path))
                    .collect(),
            ),
            revision: LoreString::from_str(revision),
            local: u8::from(local),
            filtered: u8::from(filtered),
        },
        callback,
    )
    .await;
    let stream = finished_raw_stream(stream, returned)?;
    let mut entries = Vec::new();
    for event in stream.events {
        match event {
            LoreEvent::FileInfo(data) => entries.push(RawFileInfoEntry {
                path: data.path.as_str().to_string(),
                context: format!("{}", data.context),
                hash: format!("{}", data.hash),
                is_file: data.is_file != 0,
                flag_modified: data.flag_modified != 0,
                flag_deleted: data.flag_deleted != 0,
                flag_added: data.flag_added != 0,
                flag_conflict: data.flag_conflict != 0,
                size: data.size,
                local_size: data.local_size,
                local_hash: format!("{}", data.local_hash),
                filter_size: data.filter_size,
            }),
            LoreEvent::PathIgnore(_) => {
                return Err(AdapterError::new("file info ignored a requested path"))
            }
            _ => {}
        }
    }
    Ok(entries)
}

#[derive(Default)]
struct RawMetadataStream {
    entries: Vec<MetadataEntry>,
    event_count: usize,
    complete_events: usize,
    complete_status: Option<i32>,
    complete_position: Option<usize>,
    error_events: usize,
    end_events: usize,
    end_position: Option<usize>,
    last_was_end: bool,
    unrepresentable: bool,
}

fn raw_metadata_collector() -> (LoreEventCallback, Arc<Mutex<RawMetadataStream>>) {
    let stream = Arc::new(Mutex::new(RawMetadataStream::default()));
    let observed = Arc::clone(&stream);
    let callback = Box::new(move |event: &LoreEvent| {
        let mut locked = observed
            .lock()
            .expect("raw metadata collector mutex poisoned");
        let position = locked.event_count;
        locked.event_count += 1;
        locked.last_was_end = matches!(event, LoreEvent::End(_));
        match event {
            LoreEvent::Metadata(data) => {
                let key = data.key.as_str().to_string();
                let entry = match &data.value {
                    LoreMetadata::Address(value) => Some(MetadataEntry::with_kind(
                        key,
                        MetadataKind::Address,
                        format!("{value}"),
                    )),
                    LoreMetadata::Boolean(value) => Some(MetadataEntry::with_kind(
                        key,
                        MetadataKind::Boolean,
                        if *value == 0 { "false" } else { "true" },
                    )),
                    LoreMetadata::Context(value) => Some(MetadataEntry::with_kind(
                        key,
                        MetadataKind::Context,
                        format!("{value}"),
                    )),
                    LoreMetadata::Hash(value) => Some(MetadataEntry::with_kind(
                        key,
                        MetadataKind::Hash,
                        format!("{value}"),
                    )),
                    LoreMetadata::Numeric(value) => Some(MetadataEntry::with_kind(
                        key,
                        MetadataKind::Numeric,
                        value.to_string(),
                    )),
                    LoreMetadata::String(value) => Some(MetadataEntry::with_kind(
                        key,
                        MetadataKind::String,
                        value.as_str(),
                    )),
                    // Pinned Lore's metadata-list emitter omits inline Binary
                    // before callback delivery. SBAI-6012 owns making that
                    // boundary observable, paired with SBAI-6011's provenance
                    // work; a future emitting pin must fail closed until then.
                    LoreMetadata::Binary(_) => {
                        locked.unrepresentable = true;
                        None
                    }
                };
                if let Some(entry) = entry {
                    locked.entries.push(entry);
                }
            }
            LoreEvent::Complete(data) => {
                locked.complete_events += 1;
                locked.complete_status = Some(data.status);
                locked.complete_position = Some(position);
            }
            LoreEvent::Error(_) => locked.error_events += 1,
            LoreEvent::End(_) => {
                locked.end_events += 1;
                locked.end_position = Some(position);
            }
            _ => {}
        }
    });
    (Some(callback), stream)
}

fn finished_raw_metadata(
    observed: Arc<Mutex<RawMetadataStream>>,
    returned: i32,
) -> Result<Vec<MetadataEntry>, AdapterError> {
    let stream = std::mem::take(
        &mut *observed
            .lock()
            .expect("raw metadata collector mutex poisoned"),
    );
    if returned != 0
        || stream.complete_events != 1
        || stream.complete_status != Some(0)
        || stream.error_events != 0
        || stream.end_events != 1
        || stream
            .complete_position
            .zip(stream.end_position)
            .is_none_or(|(complete, end)| complete + 1 != end || end + 1 != stream.event_count)
        || !stream.last_was_end
        || stream.unrepresentable
    {
        return Err(AdapterError::new(
            "raw revision metadata stream failed exact validation",
        ));
    }
    let mut keys = BTreeSet::new();
    if stream
        .entries
        .iter()
        .any(|entry| entry.key.is_empty() || !keys.insert(entry.key.as_str()))
    {
        return Err(AdapterError::new(
            "raw revision metadata emitted a duplicate or empty key",
        ));
    }
    Ok(stream.entries)
}

#[async_trait::async_trait]
impl GovernanceAdapter for ProductionLoreAdapter<'_> {
    async fn exact_staged_revisions(&self) -> Result<Vec<String>, AdapterError> {
        let stream = repository_status_stream(self.api, revision_only_status_args()).await?;
        status_staged_revisions(&stream, "exact DCO status")
    }

    async fn status(&self) -> Result<StatusSnapshot, AdapterError> {
        let initial_stream =
            repository_status_stream(self.api, revision_only_status_args()).await?;
        let staged_revisions = status_staged_revisions(&initial_stream, "initial exact status")?;
        let branch = status_branch(&initial_stream, "initial exact status")?;

        let scanned_stream = repository_status_stream(self.api, scanned_status_args()).await?;
        let scanned_staged_revisions =
            status_staged_revisions(&scanned_stream, "full scanned status")?;
        let scanned_branch = status_branch(&scanned_stream, "full scanned status")?;
        let mut staged_paths = BTreeSet::new();
        let mut staged_changes = Vec::new();
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
                        staged_changes.push(StagedPathObservation {
                            path: data.path.as_str().to_string(),
                            from_path: (!data.from_path.is_empty())
                                .then(|| data.from_path.as_str().to_string()),
                            action: match data.action {
                                lore::interface::LoreFileAction::Keep => {
                                    GovernancePathAction::Modify
                                }
                                lore::interface::LoreFileAction::Add => GovernancePathAction::Add,
                                lore::interface::LoreFileAction::Delete => {
                                    GovernancePathAction::Delete
                                }
                                lore::interface::LoreFileAction::Move => GovernancePathAction::Move,
                                lore::interface::LoreFileAction::Copy => GovernancePathAction::Copy,
                            },
                            dirty: data.flag_dirty != 0,
                            conflict: data.flag_conflict != 0,
                        });
                    }
                    // A staged change is, by definition, different from the
                    // committed revision and upstream may retain its dirty
                    // bit on the staged-diff event. Only a dirty-only event is
                    // an unstaged worktree delta; conflicts always close.
                    if (data.flag_dirty != 0 && data.flag_staged == 0) || data.flag_conflict != 0 {
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
        staged_changes.sort();
        if staged_changes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(AdapterError::new(
                "repository status emitted a duplicate staged change",
            ));
        }
        let mut staged_endpoints = BTreeSet::new();
        for change in &staged_changes {
            let from_valid = match change.action {
                GovernancePathAction::Move | GovernancePathAction::Copy => change
                    .from_path
                    .as_ref()
                    .is_some_and(|from| !from.is_empty() && from != &change.path),
                _ => change.from_path.is_none(),
            };
            if change.path.is_empty()
                || !from_valid
                || !staged_endpoints.insert(change.path.as_str())
                || change
                    .from_path
                    .as_ref()
                    .is_some_and(|from| !staged_endpoints.insert(from.as_str()))
            {
                return Err(AdapterError::new(
                    "repository status emitted ambiguous staged endpoints",
                ));
            }
        }
        if staged_endpoints.len() != staged_paths.len() {
            return Err(AdapterError::new(
                "repository status collapsed a staged endpoint",
            ));
        }

        let post_scan_stream =
            repository_status_stream(self.api, revision_only_status_args()).await?;
        let post_scan_staged_revisions =
            status_staged_revisions(&post_scan_stream, "post-scan exact status")?;
        let post_scan_branch = status_branch(&post_scan_stream, "post-scan exact status")?;
        if scanned_staged_revisions != staged_revisions
            || post_scan_staged_revisions != staged_revisions
            || scanned_branch != branch
            || post_scan_branch != branch
        {
            return Err(AdapterError::new(
                "full status scan changed the exact staged subject",
            ));
        }

        // Lore 9664606 stages path/flag selections, not immutable content:
        // repository/status.rs documents staged hashes as zero/current and
        // commit writes selected paths from the live filesystem. Therefore a
        // same-path edit before evidence capture is part of the selected
        // submission, not a distinguishable unstaged delta. Retain every exact
        // local fingerprint here so evidence binds the capture-time bytes and
        // a witness can reject any later change. Non-selected paths must still
        // match their staged-tree identity immediately.
        let mut worktree_files = Vec::new();
        if let [staged_revision] = staged_revisions.as_slice() {
            let paths = self.repository_dump(staged_revision).await?;
            if !paths.is_empty() {
                let entries = raw_file_info(self.api, &paths, staged_revision, true, true).await?;
                if entries.len() != paths.len() {
                    return Err(AdapterError::new(
                        "worktree file info cardinality did not match the staged tree",
                    ));
                }
                let expected_paths: BTreeSet<_> = paths.into_iter().collect();
                let mut observed_paths = BTreeSet::new();
                for entry in entries {
                    if !entry.is_file
                        || entry.path.is_empty()
                        || entry.hash.is_empty()
                        || entry.context.is_empty()
                        || entry.local_hash.is_empty()
                        || !expected_paths.contains(&entry.path)
                        || !observed_paths.insert(entry.path.clone())
                    {
                        return Err(AdapterError::new(
                            "worktree file info did not exactly cover the staged tree",
                        ));
                    }
                    let is_staged_path = staged_paths.contains(&entry.path);
                    if (!is_staged_path
                        && (entry.hash != entry.local_hash
                            || entry.flag_modified
                            || entry.flag_added))
                        || entry.flag_deleted
                        || entry.flag_conflict
                    {
                        worktree_clean = false;
                    }
                    worktree_files.push(WorktreeFileObservation {
                        path: entry.path,
                        revision: staged_revision.clone(),
                        revision_hash: entry.hash,
                        revision_context: entry.context,
                        revision_size: entry.size,
                        local_hash: entry.local_hash,
                        local_size: entry.local_size,
                        filtered_revision_size: entry.filter_size,
                        flag_modified: entry.flag_modified,
                        flag_deleted: entry.flag_deleted,
                        flag_added: entry.flag_added,
                        flag_conflict: entry.flag_conflict,
                    });
                }
                if observed_paths != expected_paths {
                    return Err(AdapterError::new(
                        "worktree file info omitted a staged tree path",
                    ));
                }
                worktree_files.sort_by(|left, right| left.path.cmp(&right.path));
            }
        }

        Ok(StatusSnapshot {
            branch,
            staged_revisions,
            scanned_staged_revisions,
            post_scan_staged_revisions,
            staged_paths: staged_paths.into_iter().collect(),
            staged_changes,
            worktree_files,
            worktree_clean,
            scan_performed: true,
        })
    }

    async fn revision_info(&self, revision_id: &str) -> Result<RevisionInfoResponse, AdapterError> {
        let (callback, stream) = raw_event_collector();
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
        let stream = finished_raw_stream(stream, returned)?;
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
        let (callback, stream) = raw_metadata_collector();
        let returned = lore::revision::metadata_list(
            self.api.globals().build(),
            LoreRevisionMetadataListArgs {
                revision: LoreString::from_str(revision_id),
            },
            callback,
        )
        .await;
        finished_raw_metadata(stream, returned)
    }

    async fn first_parent_history(
        &self,
        candidate: &str,
        target_base: &str,
        max_revisions: usize,
    ) -> Result<Vec<String>, AdapterError> {
        if candidate == target_base {
            return Ok(Vec::new());
        }
        let candidate_info = self.revision_info(candidate).await?;
        if candidate_info.revisions.len() != 1
            || candidate_info.revisions[0].revision != candidate
            || candidate_info.revisions[0].parents.is_empty()
        {
            return Err(AdapterError::new(
                "history candidate did not resolve to one exact first parent",
            ));
        }
        let first_parent = candidate_info.revisions[0].parents[0].clone();
        if first_parent.is_empty() {
            return Err(AdapterError::new("history candidate had an empty parent"));
        }
        let length = u32::try_from(max_revisions)
            .map_err(|_| AdapterError::new("history sentinel exceeds upstream limit"))?;
        let (callback, stream) = raw_event_collector();
        let returned = lore::revision::history(
            self.api.globals().build(),
            LoreRevisionHistoryArgs {
                // Upstream history requires branch metadata and therefore
                // rejects an uncommitted staged hash. Query its exact first
                // parent, then retain the staged head explicitly below.
                revision: LoreString::from_str(&first_parent),
                branch: LoreString::default(),
                date: 0,
                length,
                only_branch: 0,
            },
            callback,
        )
        .await;
        let stream = finished_raw_stream(stream, returned)?;
        let headers = stream
            .events
            .iter()
            .filter(|event| matches!(event, LoreEvent::RevisionHistory(_)))
            .count();
        if headers != 1 {
            return Err(AdapterError::new(
                "revision history emitted an inexact header count",
            ));
        }
        // Upstream history starts at the committed ancestor when `candidate`
        // is the staged subject. Preserve that real response but explicitly
        // retain the exact staged head the caller requested; the evaluator
        // independently reconstructs and compares the first-parent chain.
        let history_revisions: Vec<_> = stream
            .events
            .iter()
            .filter_map(|event| match event {
                LoreEvent::RevisionHistoryEntry(entry) => Some(format!("{}", entry.revision)),
                _ => None,
            })
            .collect();
        let mut unique_history = BTreeSet::new();
        if history_revisions
            .iter()
            .any(|revision| revision.is_empty() || !unique_history.insert(revision.as_str()))
        {
            return Err(AdapterError::new(
                "revision history emitted a duplicate or empty revision",
            ));
        }
        let mut entries = vec![candidate.to_string()];
        for revision in history_revisions {
            if revision == target_base {
                break;
            }
            if revision != candidate {
                entries.push(revision);
            }
        }
        Ok(entries)
    }

    async fn repository_dump(&self, revision_id: &str) -> Result<Vec<String>, AdapterError> {
        let (callback, stream) = raw_event_collector();
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
        let stream = finished_raw_stream(stream, returned)?;
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
        let entries = raw_file_info(self.api, paths, revision_id, false, false).await?;
        if entries.iter().any(|entry| !entry.is_file) {
            return Err(AdapterError::new(
                "file info did not return files for every dump path",
            ));
        }
        Ok(entries
            .into_iter()
            .map(|entry| FileIdentity::new(entry.path, revision_id, entry.hash, entry.context))
            .collect())
    }

    async fn revision_diff(
        &self,
        base: &str,
        candidate: &str,
    ) -> Result<Vec<RevisionDiffObservation>, AdapterError> {
        let (callback, stream) = raw_event_collector();
        let returned = lore::revision::diff(
            self.api.globals().build(),
            LoreRevisionDiffArgs {
                revision_source: LoreString::from_str(base),
                revision_target: LoreString::from_str(candidate),
                paths: LoreArray::from_vec(Vec::new()),
            },
            callback,
        )
        .await;
        let stream = finished_raw_stream(stream, returned)?;
        let mut observations = Vec::new();
        for event in stream.events {
            if let LoreEvent::RevisionDiffFile(data) = event {
                observations.push(RevisionDiffObservation {
                    path: data.path.as_str().to_string(),
                    action: match data.action {
                        lore::interface::LoreFileAction::Keep => GovernancePathAction::Modify,
                        lore::interface::LoreFileAction::Add => GovernancePathAction::Add,
                        lore::interface::LoreFileAction::Delete => GovernancePathAction::Delete,
                        lore::interface::LoreFileAction::Move => GovernancePathAction::Move,
                        lore::interface::LoreFileAction::Copy => GovernancePathAction::Copy,
                    },
                    old_is_file: data.old_is_file != 0,
                    new_is_file: data.new_is_file != 0,
                    old_address: format!("{}", data.old_address),
                    new_address: format!("{}", data.new_address),
                });
            }
        }
        Ok(observations)
    }

    async fn resolve_authors(
        &self,
        identities: &[String],
    ) -> Result<Vec<ResolvedAuthor>, AdapterError> {
        let (callback, stream) = raw_event_collector();
        let returned = lore::auth::resolve_user_info(
            self.api.globals().build(),
            LoreAuthUserInfoArgs {
                user_ids: LoreArray::from_vec(
                    identities
                        .iter()
                        .map(|identity| LoreString::from_str(identity))
                        .collect(),
                ),
            },
            callback,
        )
        .await;
        let stream = finished_raw_stream(stream, returned)?;
        Ok(stream
            .events
            .into_iter()
            .filter_map(|event| match event {
                LoreEvent::AuthUserInfo(user) => {
                    Some(ResolvedAuthor::new(user.id.as_str(), user.name.as_str()))
                }
                _ => None,
            })
            .collect())
    }

    async fn lock_file_query(&self, branch: &str, path: &str) -> Result<LockQuery, AdapterError> {
        let (callback, stream) = raw_event_collector();
        let returned = lore::lock::file_query(
            self.api.globals().build(),
            LoreLockFileQueryArgs {
                branch: LoreString::from_str(branch),
                owner: LoreString::default(),
                path: LoreString::from_str(path),
            },
            callback,
        )
        .await;
        let stream = finished_raw_stream(stream, returned)?;
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

    async fn lock_file_status(
        &self,
        branch: &str,
        paths: &[String],
    ) -> Result<LockStatusResponse, AdapterError> {
        let (callback, stream) = raw_event_collector();
        let returned = lore::lock::file_status(
            self.api.globals().build(),
            LoreLockFileStatusArgs {
                paths: LoreArray::from_vec(
                    paths
                        .iter()
                        .map(|path| LoreString::from_str(path))
                        .collect(),
                ),
                branch: LoreString::from_str(branch),
            },
            callback,
        )
        .await;
        let stream = finished_raw_stream(stream, returned)?;
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

#[derive(Default)]
pub(crate) struct RawEventStream {
    pub(crate) events: Vec<LoreEvent>,
    pub(crate) complete_events: usize,
    pub(crate) complete_status: Option<i32>,
    pub(crate) error_events: usize,
    pub(crate) end_events: usize,
}

pub(crate) fn raw_event_collector() -> (LoreEventCallback, Arc<Mutex<RawEventStream>>) {
    let stream = Arc::new(Mutex::new(RawEventStream::default()));
    let observed = Arc::clone(&stream);
    let callback = Box::new(move |event: &LoreEvent| {
        let mut locked = observed.lock().expect("raw event collector mutex poisoned");
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
    });
    (Some(callback), stream)
}

pub(crate) fn finished_raw_stream(
    observed: Arc<Mutex<RawEventStream>>,
    returned: i32,
) -> Result<RawEventStream, AdapterError> {
    // Upstream interface calls are awaited before this snapshot, so every
    // synchronous callback through API return is retained. In particular, a
    // duplicate End or any event after End cannot disappear into a fresh
    // post-send buffer as it could with a first-End oneshot collector.
    let stream = take_raw_stream(observed);
    if !raw_stream_completed_exactly(&stream, returned) {
        return Err(AdapterError::new(
            "raw Lore stream failed terminal validation",
        ));
    }
    Ok(stream)
}

pub(crate) fn take_raw_stream(observed: Arc<Mutex<RawEventStream>>) -> RawEventStream {
    std::mem::take(&mut *observed.lock().expect("raw event collector mutex poisoned"))
}

pub(crate) fn raw_stream_completed_exactly(stream: &RawEventStream, returned: i32) -> bool {
    returned == 0
        && stream.end_events == 1
        && stream.complete_events == 1
        && stream.complete_status == Some(0)
        && stream.error_events == 0
        && matches!(
            stream.events.get(stream.events.len().saturating_sub(2)),
            Some(LoreEvent::Complete(_))
        )
        && matches!(stream.events.last(), Some(LoreEvent::End(_)))
}

impl EvaluationResult {
    fn closed(code: impl Into<String>, mut observations: GovernanceObservations) -> Self {
        let code = code.into();
        observations.dependency_observations.push(code.clone());
        observations.dependency_observations.sort();
        observations.dependency_observations.dedup();
        Self {
            open: false,
            pending_revisions: Vec::new(),
            affected_paths: Vec::new(),
            identities: Vec::new(),
            superseded_identities: Vec::new(),
            failure_codes: vec![code],
            remediation: None,
            observations,
        }
    }

    fn history_overflow(
        scope: HistoryOverflowScope,
        mut observations: GovernanceObservations,
    ) -> Self {
        let code = "history_depth_exceeded".to_string();
        observations.history_overflow_scope = Some(scope);
        observations.dependency_observations.push(code.clone());
        observations.dependency_observations.sort();
        observations.dependency_observations.dedup();
        Self {
            open: false,
            pending_revisions: Vec::new(),
            affected_paths: Vec::new(),
            identities: Vec::new(),
            superseded_identities: Vec::new(),
            failure_codes: vec![code],
            remediation: Some(remediation_for_overflow(scope)),
            observations,
        }
    }
}

pub(crate) fn remediation_for_overflow(scope: HistoryOverflowScope) -> GovernanceRemediation {
    match scope {
        HistoryOverflowScope::PendingDco => GovernanceRemediation {
            code: GovernanceRemediationCode::SplitSubmissionOrAdvanceTargetBase,
            ticket: None,
        },
        HistoryOverflowScope::SupersessionAncestry => GovernanceRemediation {
            code: GovernanceRemediationCode::MigrateSupersessionIndex,
            ticket: Some("SBAI-6010".into()),
        },
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
    let mut observations = GovernanceObservations::new(expected, base);
    if expected.is_empty() || base.is_empty() {
        return EvaluationResult::closed("exact_subject_failed", observations);
    }

    let status = match adapter.status().await {
        Ok(status) => status,
        Err(_) => return EvaluationResult::closed("status_unavailable", observations),
    };
    observations.status = Some(status.clone());
    if !status.scan_performed {
        return EvaluationResult::closed("worktree_unverified", observations);
    }
    if [
        &status.staged_revisions,
        &status.scanned_staged_revisions,
        &status.post_scan_staged_revisions,
    ]
    .into_iter()
    .any(|revisions| revisions.len() != 1 || revisions[0].is_empty() || revisions[0] != expected)
    {
        return EvaluationResult::closed("exact_subject_failed", observations);
    }
    // Pinned Lore 9664606 cannot project a source-backed staged Copy. Its
    // dirty_copy lifecycle projects a target-only Add whose identity remains
    // zero/unresolved until commit. Treat any Copy event as unsupported rather
    // than granting the fake adapter capabilities absent from production.
    if status
        .staged_changes
        .iter()
        .any(|change| change.action == GovernancePathAction::Copy)
    {
        return EvaluationResult::closed("copy_semantics_unavailable", observations);
    }
    if !production_expressible_staged_changes(&status) {
        return EvaluationResult::closed("affected_paths_unavailable", observations);
    }
    let base_info = match exact_info(adapter, base).await {
        Ok(info) => info,
        Err(()) => return EvaluationResult::closed("history_incomplete", observations),
    };
    if base_info.revision != base || !exact_parent_shape(&base_info, true) {
        return EvaluationResult::closed("history_incomplete", observations);
    }
    observations.base_revision_info = Some(base_info.clone());

    let pending = match pending_dag(adapter, expected, base).await {
        Ok((revisions, graph)) => {
            observations.revision_graph = graph;
            revisions
        }
        Err(GraphFailure::Incomplete) => {
            return EvaluationResult::closed("history_incomplete", observations)
        }
        Err(GraphFailure::Depth(graph)) => {
            observations.revision_graph = graph;
            return EvaluationResult::history_overflow(
                HistoryOverflowScope::PendingDco,
                observations,
            );
        }
    };

    let history = match adapter
        .first_parent_history(expected, base, MAX_GOVERNANCE_HISTORY_REVISIONS + 1)
        .await
    {
        Ok(history) => history,
        Err(_) => return EvaluationResult::closed("history_incomplete", observations),
    };
    observations.first_parent_history = history.clone();
    if history.len() > MAX_GOVERNANCE_HISTORY_REVISIONS {
        // The exact pending DAG above is authoritative for this ceiling. A
        // contradictory convenience-history stream is an incomplete
        // dependency, not graph-backed overflow evidence.
        return EvaluationResult::closed("history_incomplete", observations);
    }
    let expected_history = match first_parent_history(adapter, expected, base).await {
        Ok(history) => history,
        Err(()) => return EvaluationResult::closed("history_incomplete", observations),
    };
    if history != expected_history || history.is_empty() {
        return EvaluationResult::closed("history_incomplete", observations);
    }

    let ancestry = match complete_supersession_ancestry(adapter, expected).await {
        Ok(ancestry) => ancestry,
        Err(GraphFailure::Incomplete) => {
            return EvaluationResult::closed("history_incomplete", observations)
        }
        Err(GraphFailure::Depth(ancestry)) => {
            observations.supersession_ancestry = ancestry;
            observations.supersession_ancestry_observed = true;
            return EvaluationResult::history_overflow(
                HistoryOverflowScope::SupersessionAncestry,
                observations,
            );
        }
    };
    if ancestry
        .iter()
        .find(|info| info.revision == base)
        .is_none_or(|info| info != &base_info)
    {
        return EvaluationResult::closed("history_incomplete", observations);
    }
    observations.supersession_ancestry = ancestry.clone();
    observations.supersession_ancestry_observed = true;

    let dco_facts = observe_dco(adapter, &pending).await;
    observations.dco_metadata = dco_facts.metadata_observations.clone();
    observations.author_resolution = dco_facts.author_resolution.clone();
    observations.dco = dco_facts.observations.clone();
    match dco_facts.failure {
        Some(Failure::Metadata) => {
            return EvaluationResult::closed("metadata_unavailable", observations)
        }
        Some(Failure::Dco) => return EvaluationResult::closed("dco_invalid", observations),
        Some(Failure::Auth) => return EvaluationResult::closed("auth_unavailable", observations),
        None => {}
    }
    let metadata_facts = match validate_supersession_metadata(adapter, &ancestry).await {
        Ok(facts) => facts,
        Err(()) => return EvaluationResult::closed("metadata_unavailable", observations),
    };
    observations.supersession_markers = metadata_facts.observations;
    observations.supersession_metadata_queries = metadata_facts.metadata_queries;
    observations.supersession_metadata_observed = true;
    if !metadata_facts.valid {
        return EvaluationResult::closed("supersession_invalid", observations);
    }
    let superseded = metadata_facts.identities;

    let candidate_tree = match exact_tree(adapter, expected).await {
        Ok(tree) => tree,
        Err(TreeFailure::Dependency) => {
            return EvaluationResult::closed("tree_unavailable", observations)
        }
        Err(TreeFailure::FileInfo) => {
            return EvaluationResult::closed("file_info_unavailable", observations)
        }
        Err(TreeFailure::Invalid) => return EvaluationResult::closed("tree_invalid", observations),
    };
    observations.candidate_files = candidate_tree.files.clone();
    observations.candidate_tree_observed = true;
    let current_files = match derive_current_files(&status, &candidate_tree.files, expected) {
        Ok(files) => files,
        Err(()) => return EvaluationResult::closed("worktree_dirty", observations),
    };
    observations.current_files = current_files.clone();
    let base_tree = match exact_tree(adapter, base).await {
        Ok(tree) => tree,
        Err(TreeFailure::Dependency) => {
            return EvaluationResult::closed("tree_unavailable", observations)
        }
        Err(TreeFailure::FileInfo) => {
            return EvaluationResult::closed("file_info_unavailable", observations)
        }
        Err(TreeFailure::Invalid) => return EvaluationResult::closed("tree_invalid", observations),
    };
    observations.base_files = base_tree.files.clone();
    observations.base_tree_observed = true;

    let mut identities: Vec<String> = current_files
        .iter()
        .map(FileIdentity::canonical_id)
        .collect();
    identities.sort();
    if superseded
        .iter()
        .any(|identity| identities.iter().any(|current| current == identity))
    {
        return EvaluationResult::closed("not_superseded_failed", observations);
    }

    let affected_facts = match affected_paths(
        adapter,
        base,
        expected,
        &status,
        &base_tree.files,
        &candidate_tree.files,
        &current_files,
    )
    .await
    {
        Ok(facts) => facts,
        Err(AffectedFailure::Unavailable) => {
            return EvaluationResult::closed("affected_paths_unavailable", observations)
        }
        Err(AffectedFailure::CopySemanticsUnavailable) => {
            return EvaluationResult::closed("copy_semantics_unavailable", observations)
        }
    };
    observations.upstream_revision_diff = affected_facts.upstream_diff;
    observations.revision_diff = affected_facts.diff;
    observations.revision_diff_observed = true;
    let affected_paths = affected_facts.paths;
    observations.affected_paths = affected_paths.clone();
    if affected_paths.is_empty() {
        return EvaluationResult::closed("empty_submission", observations);
    }

    match validate_locks(adapter, &status.branch, &affected_paths).await {
        Ok(facts) => {
            observations.lock_queries = facts.queries;
            observations.lock_status = facts.status;
        }
        Err((LockFailure::Dependency, facts)) => {
            observations.lock_queries = facts.queries;
            observations.lock_status = facts.status;
            return EvaluationResult::closed("locks_unavailable", observations);
        }
        Err((LockFailure::Locked, facts)) => {
            observations.lock_queries = facts.queries;
            observations.lock_status = facts.status;
            return EvaluationResult::closed("locks_clear_failed", observations);
        }
    }

    EvaluationResult {
        open: true,
        pending_revisions: pending,
        affected_paths,
        identities,
        superseded_identities: superseded,
        failure_codes: Vec::new(),
        remediation: None,
        observations,
    }
}

fn canonical_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// Reapply the production status adapter's endpoint rules at the shared
/// evaluator boundary so an injected/fake adapter cannot authorize a staged
/// event shape that pinned Lore cannot emit.
fn production_expressible_staged_changes(status: &StatusSnapshot) -> bool {
    let staged_paths: BTreeSet<_> = status.staged_paths.iter().map(String::as_str).collect();
    if staged_paths.len() != status.staged_paths.len()
        || staged_paths.iter().any(|path| path.is_empty())
    {
        return false;
    }
    let mut endpoints = BTreeSet::new();
    for change in &status.staged_changes {
        let exact_from = match change.action {
            GovernancePathAction::Move => change
                .from_path
                .as_ref()
                .is_some_and(|from| !from.is_empty() && from != &change.path),
            GovernancePathAction::Copy => false,
            GovernancePathAction::Modify
            | GovernancePathAction::Add
            | GovernancePathAction::Delete => change.from_path.is_none(),
        };
        if change.path.is_empty()
            || !exact_from
            || !endpoints.insert(change.path.as_str())
            || change
                .from_path
                .as_ref()
                .is_some_and(|from| !endpoints.insert(from.as_str()))
        {
            return false;
        }
    }
    endpoints == staged_paths
}

pub(crate) fn canonical_artifact_identity(identity: &str) -> bool {
    let Some((hash, context)) = identity.split_once(':') else {
        return false;
    };
    canonical_lower_hex(hash, 64)
        && canonical_lower_hex(context, 32)
        && !context.bytes().all(|byte| byte == b'0')
}

pub(crate) fn derive_current_files(
    status: &StatusSnapshot,
    candidate_files: &[FileIdentity],
    expected_revision: &str,
) -> Result<Vec<FileIdentity>, ()> {
    if !exact_worktree_clean(status, candidate_files, expected_revision) {
        return Err(());
    }
    let staged_paths: BTreeSet<_> = status.staged_paths.iter().map(String::as_str).collect();
    let worktree: BTreeMap<_, _> = status
        .worktree_files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let mut identities = BTreeSet::new();
    let mut current = Vec::with_capacity(candidate_files.len());
    for file in candidate_files {
        let observed = worktree.get(file.path.as_str()).ok_or(())?;
        let hash = if staged_paths.contains(file.path.as_str()) {
            observed.local_hash.clone()
        } else {
            file.hash.clone()
        };
        if !canonical_lower_hex(&hash, 64)
            || !canonical_lower_hex(&file.context, 32)
            || file.context.bytes().all(|byte| byte == b'0')
        {
            return Err(());
        }
        let identity = format!("{hash}:{}", file.context);
        if !identities.insert(identity) {
            return Err(());
        }
        current.push(FileIdentity::new(
            &file.path,
            expected_revision,
            hash,
            &file.context,
        ));
    }
    current.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(current)
}

/// Evaluate only the ratified DCO dependency set. This deliberately shares
/// exact-subject, DAG/history, metadata parsing, and author-resolution helpers
/// with the full gate evaluator while never querying marker metadata, trees,
/// diffs, locks, or worktree scan state.
pub(crate) async fn evaluate_dco<A, R>(adapter: &A, request: &R) -> DcoValidateResult
where
    A: GovernanceAdapter + Sync,
    R: ExactRevisionRequest,
{
    let expected = request.expected_staged_revision();
    let base = request.target_base_revision();
    let closed = |code: &str, pending_revisions: Vec<String>| DcoValidateResult {
        valid: false,
        pending_revisions,
        failure_codes: vec![code.into()],
        remediation: None,
    };
    let overflow = |pending_revisions: Vec<String>| DcoValidateResult {
        valid: false,
        pending_revisions,
        failure_codes: vec!["history_depth_exceeded".into()],
        remediation: Some(remediation_for_overflow(HistoryOverflowScope::PendingDco)),
    };
    if expected.is_empty() || base.is_empty() {
        return closed("exact_subject_failed", Vec::new());
    }
    let staged = match adapter.exact_staged_revisions().await {
        Ok(staged) => staged,
        Err(_) => return closed("status_unavailable", Vec::new()),
    };
    if staged.as_slice() != [expected] {
        return closed("exact_subject_failed", Vec::new());
    }
    match exact_info(adapter, base).await {
        Ok(info) if exact_parent_shape(&info, true) => {}
        _ => return closed("history_incomplete", Vec::new()),
    }
    let (pending, _) = match pending_dag(adapter, expected, base).await {
        Ok(facts) => facts,
        Err(GraphFailure::Incomplete) => return closed("history_incomplete", Vec::new()),
        Err(GraphFailure::Depth(graph)) => {
            return overflow(graph.into_iter().map(|info| info.revision).collect())
        }
    };
    let history = match adapter
        .first_parent_history(expected, base, MAX_GOVERNANCE_HISTORY_REVISIONS + 1)
        .await
    {
        Ok(history) => history,
        Err(_) => return closed("history_incomplete", pending),
    };
    if history.len() > MAX_GOVERNANCE_HISTORY_REVISIONS {
        return closed("history_incomplete", pending);
    }
    let reconstructed = match first_parent_history(adapter, expected, base).await {
        Ok(history) => history,
        Err(_) => return closed("history_incomplete", pending),
    };
    if history.is_empty() || history != reconstructed {
        return closed("history_incomplete", pending);
    }

    let facts = observe_dco(adapter, &pending).await;
    if let Some(failure) = facts.failure {
        return closed(
            match failure {
                Failure::Metadata => "metadata_unavailable",
                Failure::Dco => "dco_invalid",
                Failure::Auth => "auth_unavailable",
            },
            pending,
        );
    }
    if facts.metadata_observations.len() != pending.len()
        || facts.observations.len() != pending.len()
        || facts.author_resolution.is_none()
    {
        return closed("dco_dependency_incomplete", pending);
    }
    DcoValidateResult {
        valid: true,
        pending_revisions: pending,
        failure_codes: Vec::new(),
        remediation: None,
    }
}

pub(crate) fn exact_worktree_clean(
    status: &StatusSnapshot,
    candidate_files: &[FileIdentity],
    expected_revision: &str,
) -> bool {
    if !status.scan_performed || !status.worktree_clean {
        return false;
    }
    let expected: BTreeMap<_, _> = candidate_files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    if expected.len() != candidate_files.len() || status.worktree_files.len() != expected.len() {
        return false;
    }
    let mut observed = BTreeSet::new();
    let staged_paths: BTreeSet<_> = status.staged_paths.iter().map(String::as_str).collect();
    if staged_paths.len() != status.staged_paths.len()
        || staged_paths.iter().any(|path| path.is_empty())
    {
        return false;
    }
    status.worktree_files.iter().all(|file| {
        let Some(candidate) = expected.get(file.path.as_str()) else {
            return false;
        };
        let is_staged_path = staged_paths.contains(file.path.as_str());
        !file.path.is_empty()
            && observed.insert(file.path.as_str())
            && file.revision == expected_revision
            && file.revision_hash == candidate.hash
            && file.revision_context == candidate.context
            && !file.revision_hash.is_empty()
            && !file.revision_context.is_empty()
            && !file.local_hash.is_empty()
            && (is_staged_path
                || (file.local_hash == file.revision_hash
                    && !file.flag_modified
                    && !file.flag_added))
            && !file.flag_deleted
            && !file.flag_conflict
    })
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
    /// The exact, shape-validated N+1 prefix retained as raw overflow
    /// evidence. Its frontier may reference revisions outside the prefix.
    Depth(Vec<RevisionInfo>),
}

fn exact_parent_shape(info: &RevisionInfo, allow_root: bool) -> bool {
    let parents: BTreeSet<_> = info.parents.iter().map(String::as_str).collect();
    (allow_root || !info.parents.is_empty())
        && info.parents.len() <= 2
        && parents.len() == info.parents.len()
        && info
            .parents
            .iter()
            .all(|parent| !parent.is_empty() && parent != &info.revision)
}

/// Independent whole-ancestry traversal for supersession. Unlike the pending
/// DCO graph, this includes the candidate, explicit base, all older ancestors,
/// and every merge parent through roots. The 1001st unique revision is fetched
/// and shape-validated as the overflow sentinel, but is not accepted.
async fn complete_supersession_ancestry<A: GovernanceAdapter>(
    adapter: &A,
    candidate: &str,
) -> Result<Vec<RevisionInfo>, GraphFailure> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Visit {
        Active,
        Complete,
    }

    let mut visit = BTreeMap::<String, Visit>::new();
    let mut graph = BTreeMap::<String, RevisionInfo>::new();
    let mut stack = vec![(candidate.to_string(), false)];
    while let Some((revision, leaving)) = stack.pop() {
        if leaving {
            visit.insert(revision, Visit::Complete);
            continue;
        }
        match visit.get(&revision) {
            Some(Visit::Active) => return Err(GraphFailure::Incomplete),
            Some(Visit::Complete) => continue,
            None => {}
        }
        let info = exact_info(adapter, &revision)
            .await
            .map_err(|_| GraphFailure::Incomplete)?;
        if !exact_parent_shape(&info, true) {
            return Err(GraphFailure::Incomplete);
        }
        graph.insert(revision.clone(), info.clone());
        if graph.len() == MAX_GOVERNANCE_HISTORY_REVISIONS + 1 {
            return Err(GraphFailure::Depth(graph.into_values().collect()));
        }
        visit.insert(revision.clone(), Visit::Active);
        stack.push((revision, true));
        for parent in info.parents.into_iter().rev() {
            stack.push((parent, false));
        }
    }
    if graph.is_empty() {
        return Err(GraphFailure::Incomplete);
    }
    Ok(graph.into_values().collect())
}

async fn pending_dag<A: GovernanceAdapter>(
    adapter: &A,
    candidate: &str,
    base: &str,
) -> Result<(Vec<String>, Vec<RevisionInfo>), GraphFailure> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Visit {
        Active,
        Complete,
    }

    let mut visit = BTreeMap::<String, Visit>::new();
    let mut pending = BTreeSet::new();
    let mut graph = BTreeMap::<String, RevisionInfo>::new();
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
        if !exact_parent_shape(&info, false) {
            return Err(GraphFailure::Incomplete);
        }
        graph.insert(revision.clone(), info.clone());
        pending.insert(revision.clone());
        if pending.len() == MAX_GOVERNANCE_HISTORY_REVISIONS + 1 {
            return Err(GraphFailure::Depth(graph.into_values().collect()));
        }
        visit.insert(revision.clone(), Visit::Active);
        stack.push((revision, true));
        for parent in info.parents.into_iter().rev() {
            stack.push((parent, false));
        }
    }

    if pending.is_empty() {
        return Err(GraphFailure::Incomplete);
    }
    Ok((pending.into_iter().collect(), graph.into_values().collect()))
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
        if !seen.insert(revision.clone()) || revisions.len() > MAX_GOVERNANCE_HISTORY_REVISIONS {
            return Err(());
        }
        let info = exact_info(adapter, &revision).await?;
        if !exact_parent_shape(&info, false) {
            return Err(());
        }
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

#[derive(Clone, Copy)]
enum Failure {
    Metadata,
    Dco,
    Auth,
}

struct DcoFacts {
    metadata_observations: Vec<DcoMetadataObservation>,
    author_resolution: Option<AuthorResolutionObservation>,
    observations: Vec<DcoObservation>,
    failure: Option<Failure>,
}

async fn observe_dco<A: GovernanceAdapter>(adapter: &A, pending: &[String]) -> DcoFacts {
    let mut facts = DcoFacts {
        metadata_observations: Vec::new(),
        author_resolution: None,
        observations: Vec::new(),
        failure: None,
    };
    let mut authors = BTreeSet::new();

    for revision in pending {
        let metadata = match adapter.revision_metadata(revision).await {
            Ok(metadata) => metadata,
            Err(_) => {
                facts.failure = Some(Failure::Metadata);
                return facts;
            }
        };
        let mut grouped = BTreeMap::<String, Vec<String>>::new();
        for entry in &metadata {
            if matches!(
                entry.key.as_str(),
                "message" | "created-by" | "committed-by"
            ) && entry.kind != MetadataKind::String
            {
                facts.failure = Some(Failure::Dco);
                return facts;
            }
            grouped
                .entry(entry.key.clone())
                .or_default()
                .push(entry.value.clone());
        }
        let mut observation = DcoMetadataObservation {
            revision: revision.clone(),
            messages: grouped.remove("message").unwrap_or_default(),
            created_by: grouped.remove("created-by").unwrap_or_default(),
            committed_by: grouped.remove("committed-by").unwrap_or_default(),
        };
        observation.messages.sort();
        observation.created_by.sort();
        observation.committed_by.sort();
        facts.metadata_observations.push(observation);
    }
    facts
        .metadata_observations
        .sort_by(|left, right| left.revision.cmp(&right.revision));

    for raw in &facts.metadata_observations {
        let ([message], [created_by], committed_by) = (
            raw.messages.as_slice(),
            raw.created_by.as_slice(),
            raw.committed_by.as_slice(),
        ) else {
            facts.failure = Some(Failure::Dco);
            return facts;
        };
        let committed_by = match committed_by {
            [] => None,
            [identity] if !identity.is_empty() => Some(identity.clone()),
            _ => {
                facts.failure = Some(Failure::Dco);
                return facts;
            }
        };
        if created_by.is_empty() {
            facts.failure = Some(Failure::Dco);
            return facts;
        }
        let Some(signer) = parse_dco_signer(message) else {
            facts.failure = Some(Failure::Dco);
            return facts;
        };
        authors.insert(created_by.clone());
        if let Some(identity) = &committed_by {
            authors.insert(identity.clone());
        }
        facts.observations.push(DcoObservation {
            revision: raw.revision.clone(),
            message: message.clone(),
            trailer: signer.trailer,
            signer_name: signer.name,
            signer_email: signer.email,
            created_by: created_by.clone(),
            committed_by,
            resolved_authors: Vec::new(),
        });
    }

    if authors.is_empty() {
        facts.failure = Some(Failure::Dco);
        return facts;
    }
    let requested: Vec<String> = authors.into_iter().collect();
    let replies = match adapter.resolve_authors(&requested).await {
        Ok(replies) => replies,
        Err(_) => {
            facts.failure = Some(Failure::Auth);
            return facts;
        }
    };
    facts.author_resolution = Some(AuthorResolutionObservation {
        requested: requested.clone(),
        replies: replies.clone(),
    });
    if replies.len() != requested.len() {
        facts.failure = Some(Failure::Dco);
        return facts;
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
            facts.failure = Some(Failure::Dco);
            return facts;
        }
    }
    for observation in &mut facts.observations {
        let identities: BTreeSet<String> = std::iter::once(observation.created_by.clone())
            .chain(observation.committed_by.iter().cloned())
            .collect();
        observation.resolved_authors = identities
            .iter()
            .filter_map(|identity| {
                resolved
                    .get(identity)
                    .map(|name| ResolvedAuthor::new(identity, name))
            })
            .collect();
        if observation.resolved_authors.len() != identities.len()
            || observation
                .resolved_authors
                .iter()
                .any(|author| author.display_name != observation.signer_name)
        {
            facts.failure = Some(Failure::Dco);
            return facts;
        }
    }

    facts
        .observations
        .sort_by(|left, right| left.revision.cmp(&right.revision));
    facts
}

struct SupersessionFacts {
    identities: Vec<String>,
    observations: Vec<SupersessionObservation>,
    metadata_queries: Vec<SupersessionMetadataQueryObservation>,
    valid: bool,
}

async fn validate_supersession_metadata<A: GovernanceAdapter>(
    adapter: &A,
    ancestry: &[RevisionInfo],
) -> Result<SupersessionFacts, ()> {
    let mut records = BTreeMap::<String, String>::new();
    let mut observations = Vec::new();
    let mut metadata_queries = Vec::with_capacity(ancestry.len());
    let mut valid = true;
    for info in ancestry {
        let metadata = adapter
            .revision_metadata(&info.revision)
            .await
            .map_err(|_| ())?;
        metadata_queries.push(SupersessionMetadataQueryObservation {
            revision: info.revision.clone(),
            metadata: metadata.clone(),
        });
        valid &=
            scan_supersession_entries(&info.revision, &metadata, &mut records, &mut observations)
                .is_ok();
    }
    observations.sort_by(|left, right| {
        (&left.revision, &left.key, &left.value, &left.identity).cmp(&(
            &right.revision,
            &right.key,
            &right.value,
            &right.identity,
        ))
    });
    Ok(SupersessionFacts {
        identities: records.into_keys().collect(),
        observations,
        metadata_queries,
        valid,
    })
}

fn scan_supersession_entries(
    revision: &str,
    metadata: &[MetadataEntry],
    records: &mut BTreeMap<String, String>,
    observations: &mut Vec<SupersessionObservation>,
) -> Result<(), ()> {
    let mut valid = true;
    for entry in metadata {
        if !entry.key.starts_with(SUPERSESSION_MARKER_PREFIX) {
            continue;
        }
        let identity = entry.key[SUPERSESSION_MARKER_PREFIX.len()..].to_string();
        observations.push(SupersessionObservation {
            revision: revision.into(),
            key: entry.key.clone(),
            value: entry.value.clone(),
            identity: identity.clone(),
        });
        match entry
            .string_value()
            .and_then(|value| serde_json::from_str::<SupersessionMarkerV1>(value).ok())
        {
            Some(marker)
                if canonical_artifact_identity(&identity)
                    && marker.version == "v1"
                    && !marker.identity.is_empty()
                    && marker.identity == identity =>
            {
                match records.get(&identity) {
                    Some(existing) if existing != &entry.value => valid = false,
                    _ => {
                        records.insert(identity, entry.value.clone());
                    }
                }
            }
            _ => valid = false,
        }
    }
    if valid {
        Ok(())
    } else {
        Err(())
    }
}

struct ParsedDco {
    trailer: String,
    name: String,
    email: String,
}

fn parse_dco_signer(message: &str) -> Option<ParsedDco> {
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
    let identity = signer.strip_suffix('>')?;
    let (name, email) = identity.split_once(" <")?;
    let first_name_char = name.chars().next()?;
    let last_name_char = name.chars().last()?;
    if first_name_char.is_whitespace()
        || last_name_char.is_whitespace()
        || name
            .chars()
            .any(|character| character.is_control() || matches!(character, '<' | '>'))
        || !is_canonical_dco_email(email)
    {
        return None;
    }
    Some(ParsedDco {
        trailer: format!("Signed-off-by: {signer}"),
        name: name.to_string(),
        email: email.to_string(),
    })
}

fn is_canonical_dco_email(email: &str) -> bool {
    if email.is_empty()
        || email.chars().any(|character| {
            character.is_whitespace() || character.is_control() || matches!(character, '<' | '>')
        })
    {
        return false;
    }

    let mut address_parts = email.split('@');
    let Some(local) = address_parts.next() else {
        return false;
    };
    let Some(domain) = address_parts.next() else {
        return false;
    };
    if local.is_empty() || domain.is_empty() || address_parts.next().is_some() {
        return false;
    }

    domain.split('.').all(|label| {
        let (Some(first), Some(last)) = (label.as_bytes().first(), label.as_bytes().last()) else {
            return false;
        };
        first.is_ascii_alphanumeric()
            && last.is_ascii_alphanumeric()
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
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

struct ExactTree {
    files: Vec<FileIdentity>,
}

async fn exact_tree<A: GovernanceAdapter>(
    adapter: &A,
    revision: &str,
) -> Result<ExactTree, TreeFailure> {
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
        return Ok(ExactTree { files: Vec::new() });
    }
    let mut identities = adapter
        .file_info(revision, &paths)
        .await
        .map_err(|_| TreeFailure::FileInfo)?;
    if identities.len() != paths.len() {
        return Err(TreeFailure::Invalid);
    }

    let mut tree = BTreeMap::new();
    let mut canonical_ids = BTreeSet::new();
    identities.sort_by(|left, right| {
        (&left.path, &left.revision, &left.hash, &left.context).cmp(&(
            &right.path,
            &right.revision,
            &right.hash,
            &right.context,
        ))
    });
    for identity in &identities {
        if identity.revision != revision
            || identity.path.is_empty()
            || !canonical_lower_hex(&identity.hash, 64)
            || !canonical_lower_hex(&identity.context, 32)
            || identity.context.bytes().all(|byte| byte == b'0')
            || !expected_paths.contains(&identity.path)
        {
            return Err(TreeFailure::Invalid);
        }
        let canonical_id = identity.canonical_id();
        if tree
            .insert(identity.path.clone(), canonical_id.clone())
            .is_some()
            || !canonical_ids.insert(canonical_id)
        {
            return Err(TreeFailure::Invalid);
        }
    }
    if tree.len() != expected_paths.len() {
        return Err(TreeFailure::Invalid);
    }
    Ok(ExactTree { files: identities })
}

struct AffectedFacts {
    upstream_diff: Vec<RevisionDiffObservation>,
    diff: Vec<AffectedPath>,
    paths: Vec<String>,
}

enum AffectedFailure {
    Unavailable,
    CopySemanticsUnavailable,
}

impl From<()> for AffectedFailure {
    fn from(_: ()) -> Self {
        Self::Unavailable
    }
}

async fn affected_paths<A: GovernanceAdapter>(
    adapter: &A,
    base: &str,
    candidate: &str,
    status: &StatusSnapshot,
    base_files: &[FileIdentity],
    candidate_files: &[FileIdentity],
    current_files: &[FileIdentity],
) -> Result<AffectedFacts, AffectedFailure> {
    let mut upstream_diff = adapter
        .revision_diff(base, candidate)
        .await
        .map_err(|_| ())?;
    upstream_diff.sort();
    if upstream_diff
        .iter()
        .any(|raw| raw.action == GovernancePathAction::Copy)
    {
        return Err(AffectedFailure::CopySemanticsUnavailable);
    }
    let mut base_by_context = BTreeMap::new();
    let mut current_by_context = BTreeMap::new();
    let mut base_by_path = BTreeMap::new();
    let mut candidate_by_path = BTreeMap::new();
    let mut current_by_path = BTreeMap::new();
    for file in base_files {
        if !canonical_lower_hex(&file.hash, 64)
            || !canonical_lower_hex(&file.context, 32)
            || file.context.bytes().all(|byte| byte == b'0')
            || base_by_context
                .insert(file.context.as_str(), file.path.as_str())
                .is_some()
            || base_by_path
                .insert(file.path.as_str(), file.canonical_id())
                .is_some()
        {
            return Err(AffectedFailure::Unavailable);
        }
    }
    for file in candidate_files {
        if !canonical_lower_hex(&file.hash, 64)
            || !canonical_lower_hex(&file.context, 32)
            || file.context.bytes().all(|byte| byte == b'0')
            || candidate_by_path.insert(file.path.as_str(), file).is_some()
        {
            return Err(AffectedFailure::Unavailable);
        }
    }
    for file in current_files {
        if current_by_context
            .insert(file.context.as_str(), file.path.as_str())
            .is_some()
            || current_by_path
                .insert(file.path.as_str(), file.canonical_id())
                .is_some()
        {
            return Err(AffectedFailure::Unavailable);
        }
    }

    let mut consumed_base = BTreeSet::new();
    let mut consumed_current = BTreeSet::new();
    let mut diff = Vec::new();
    for (context, source) in &base_by_context {
        if let Some(target) = current_by_context.get(context) {
            consumed_base.insert(*source);
            consumed_current.insert(*target);
            if source != target {
                diff.push(AffectedPath {
                    source_path: Some((*source).to_string()),
                    target_path: Some((*target).to_string()),
                });
            } else if base_by_path.get(source) != current_by_path.get(target) {
                diff.push(AffectedPath::modified(*source));
            }
        }
    }
    for (path, base_identity) in &base_by_path {
        if consumed_base.contains(path) {
            continue;
        }
        if let Some(current_identity) = current_by_path.get(path) {
            if !consumed_current.contains(path) && current_identity != base_identity {
                consumed_base.insert(*path);
                consumed_current.insert(*path);
                diff.push(AffectedPath::modified(*path));
            }
        }
    }
    for path in base_by_path.keys() {
        if !consumed_base.contains(path) {
            diff.push(AffectedPath {
                source_path: Some((*path).to_string()),
                target_path: None,
            });
        }
    }
    for path in current_by_path.keys() {
        if !consumed_current.contains(path) {
            diff.push(AffectedPath {
                source_path: None,
                target_path: Some((*path).to_string()),
            });
        }
    }

    diff.sort_by(|left, right| {
        (&left.source_path, &left.target_path).cmp(&(&right.source_path, &right.target_path))
    });
    if diff.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(AffectedFailure::Unavailable);
    }

    let expected_status: BTreeSet<_> = status
        .staged_changes
        .iter()
        .map(|change| match change.action {
            GovernancePathAction::Modify => (Some(change.path.clone()), Some(change.path.clone())),
            GovernancePathAction::Add => (None, Some(change.path.clone())),
            GovernancePathAction::Delete => (Some(change.path.clone()), None),
            GovernancePathAction::Move | GovernancePathAction::Copy => {
                (change.from_path.clone(), Some(change.path.clone()))
            }
        })
        .collect();
    let exact_diff: BTreeSet<_> = diff
        .iter()
        .map(|entry| (entry.source_path.clone(), entry.target_path.clone()))
        .collect();
    if expected_status.len() != status.staged_changes.len() || expected_status != exact_diff {
        return Err(AffectedFailure::Unavailable);
    }
    let status_paths: BTreeSet<_> = status
        .staged_changes
        .iter()
        .flat_map(|change| {
            std::iter::once(change.path.as_str()).chain(change.from_path.iter().map(String::as_str))
        })
        .collect();
    if status_paths.len() != status.staged_paths.len()
        || status_paths != status.staged_paths.iter().map(String::as_str).collect()
    {
        return Err(AffectedFailure::Unavailable);
    }

    let zero_address = format!("{}-{}", "0".repeat(64), "0".repeat(32));
    let address = |file: &FileIdentity| format!("{}-{}", file.hash, file.context);
    let canonical_address = |value: &str| {
        value.len() == 97
            && value.as_bytes()[64] == b'-'
            && value.bytes().enumerate().all(|(index, byte)| {
                index == 64 || byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
            })
    };
    let base_file_by_path: BTreeMap<_, _> = base_files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let mut raw_paths = BTreeSet::new();
    for raw in &upstream_diff {
        if raw.path.is_empty()
            || !raw_paths.insert(raw.path.as_str())
            || !canonical_address(&raw.old_address)
            || !canonical_address(&raw.new_address)
        {
            return Err(AffectedFailure::Unavailable);
        }
    }

    let mut consumed_raw = vec![false; upstream_diff.len()];
    let mut consume = |path: &str,
                       action: GovernancePathAction,
                       old_is_file: bool,
                       new_is_file: bool,
                       old_address: &str,
                       new_address: &str|
     -> Result<(), ()> {
        let matches: Vec<_> = upstream_diff
            .iter()
            .enumerate()
            .filter(|(index, raw)| {
                !consumed_raw[*index]
                    && raw.path == path
                    && raw.action == action
                    && raw.old_is_file == old_is_file
                    && raw.new_is_file == new_is_file
                    && raw.old_address == old_address
                    && raw.new_address == new_address
            })
            .map(|(index, _)| index)
            .collect();
        if let [index] = matches.as_slice() {
            consumed_raw[*index] = true;
            Ok(())
        } else {
            Err(())
        }
    };
    for change in &status.staged_changes {
        match change.action {
            GovernancePathAction::Modify => {
                let old = base_file_by_path.get(change.path.as_str()).ok_or(())?;
                let new = candidate_by_path.get(change.path.as_str()).ok_or(())?;
                consume(
                    &change.path,
                    GovernancePathAction::Modify,
                    true,
                    true,
                    &address(old),
                    &address(new),
                )?;
            }
            GovernancePathAction::Add => {
                if base_file_by_path.contains_key(change.path.as_str()) {
                    return Err(AffectedFailure::Unavailable);
                }
                let new = candidate_by_path.get(change.path.as_str()).ok_or(())?;
                consume(
                    &change.path,
                    GovernancePathAction::Add,
                    false,
                    true,
                    &zero_address,
                    &address(new),
                )?;
            }
            GovernancePathAction::Delete => {
                let old = base_file_by_path.get(change.path.as_str()).ok_or(())?;
                if candidate_by_path.contains_key(change.path.as_str()) {
                    return Err(AffectedFailure::Unavailable);
                }
                consume(
                    &change.path,
                    GovernancePathAction::Delete,
                    true,
                    false,
                    &address(old),
                    &zero_address,
                )?;
            }
            GovernancePathAction::Move => {
                let source = change.from_path.as_ref().ok_or(())?;
                let old = base_file_by_path.get(source.as_str()).ok_or(())?;
                let new = candidate_by_path.get(change.path.as_str()).ok_or(())?;
                let old_address = address(old);
                let new_address = address(new);
                if old_address != new_address {
                    return Err(AffectedFailure::Unavailable);
                }
                consume(
                    source,
                    GovernancePathAction::Delete,
                    true,
                    false,
                    &old_address,
                    &zero_address,
                )?;
                consume(
                    &change.path,
                    GovernancePathAction::Add,
                    false,
                    true,
                    &zero_address,
                    &new_address,
                )?;
            }
            GovernancePathAction::Copy => {
                return Err(AffectedFailure::CopySemanticsUnavailable);
            }
        }
    }
    if consumed_raw.iter().any(|consumed| !consumed) {
        return Err(AffectedFailure::Unavailable);
    }
    let derived_paths: BTreeSet<_> = diff
        .iter()
        .flat_map(|entry| entry.source_path.iter().chain(entry.target_path.iter()))
        .cloned()
        .collect();
    Ok(AffectedFacts {
        upstream_diff,
        diff,
        paths: derived_paths.into_iter().collect(),
    })
}

enum LockFailure {
    Dependency,
    Locked,
}

#[derive(Default)]
struct LockFacts {
    queries: Vec<LockQuery>,
    status: Option<LockStatusResponse>,
}

async fn validate_locks<A: GovernanceAdapter>(
    adapter: &A,
    branch: &str,
    paths: &[String],
) -> Result<LockFacts, (LockFailure, LockFacts)> {
    let mut facts = LockFacts::default();
    let requested: BTreeSet<String> = paths.iter().cloned().collect();
    if branch.is_empty() || requested.len() != paths.len() || requested.is_empty() {
        return Err((LockFailure::Dependency, facts));
    }
    let mut locked = false;
    for path in paths {
        let mut query = match adapter.lock_file_query(branch, path).await {
            Ok(query) => query,
            Err(_) => return Err((LockFailure::Dependency, facts)),
        };
        query.owners.sort();
        query.ignored_paths.sort();
        if query.path != *path {
            facts.queries.push(query);
            return Err((LockFailure::Dependency, facts));
        }
        if query.begin_events != 1
            || !query.completed
            || !query.ignored_paths.is_empty()
            || query.expected_count != query.owners.len()
        {
            facts.queries.push(query);
            return Err((LockFailure::Dependency, facts));
        }
        if !query.owners.is_empty() {
            locked = true;
        }
        facts.queries.push(query);
    }
    let mut status = match adapter.lock_file_status(branch, paths).await {
        Ok(status) => status,
        Err(_) => return Err((LockFailure::Dependency, facts)),
    };
    status.ignored_paths.sort();
    status
        .statuses
        .sort_by(|left, right| (&left.path, &left.owner).cmp(&(&right.path, &right.owner)));
    facts.status = Some(status.clone());
    if status.begin_events != 1
        || !status.completed
        || !status.ignored_paths.is_empty()
        || status.expected_count != status.statuses.len()
    {
        return Err((LockFailure::Dependency, facts));
    }
    let mut observed = BTreeSet::new();
    for record in &status.statuses {
        if !requested.contains(&record.path) || !observed.insert(record.path.clone()) {
            return Err((LockFailure::Dependency, facts));
        }
        if record.owner.is_some() {
            locked = true;
        }
    }
    if locked {
        Err((LockFailure::Locked, facts))
    } else {
        Ok(facts)
    }
}

#[cfg(test)]
mod tests {
    use super::MetadataKind;
    use super::{
        finished_raw_metadata, finished_raw_stream, raw_event_collector, raw_metadata_collector,
        revision_only_status_args, scanned_status_args,
    };
    use lore::interface::{
        LoreCompleteEventData, LoreErrorEventData, LoreEvent, LoreMetadata, LoreMetadataEventData,
        LoreString,
    };

    fn complete(status: i32) -> LoreEvent {
        LoreEvent::Complete(LoreCompleteEventData {
            status,
            error: Default::default(),
        })
    }

    fn end() -> LoreEvent {
        LoreEvent::End(Default::default())
    }

    fn collect(events: Vec<LoreEvent>, returned: i32) -> bool {
        let (callback, observed) = raw_event_collector();
        let callback = callback.expect("collector callback");
        for event in events {
            callback(&event);
        }
        finished_raw_stream(observed, returned).is_ok()
    }

    fn collect_metadata(
        events: Vec<LoreEvent>,
        returned: i32,
    ) -> Result<Vec<super::MetadataEntry>, super::AdapterError> {
        let (callback, observed) = raw_metadata_collector();
        let callback = callback.expect("metadata collector callback");
        for event in events {
            callback(&event);
        }
        finished_raw_metadata(observed, returned)
    }

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

    #[test]
    fn raw_collector_rejects_every_inexact_terminal_shape_after_api_return() {
        assert!(collect(vec![complete(0), end()], 0));
        assert!(!collect(vec![complete(0), end(), end()], 0));
        assert!(!collect(vec![end(), complete(0)], 0));
        assert!(!collect(vec![complete(0)], 0));
        assert!(!collect(vec![end()], 0));
        assert!(!collect(
            vec![
                LoreEvent::Error(LoreErrorEventData {
                    error_type: 3,
                    error_inner: LoreString::from_str("injected"),
                }),
                complete(0),
                end(),
            ],
            0,
        ));
        assert!(!collect(vec![complete(1), end()], 0));
        assert!(!collect(vec![complete(0), end()], 1));
    }

    #[test]
    fn raw_metadata_retains_kind_value_and_multiplicity_without_binary_fiction() {
        let string = LoreEvent::Metadata(LoreMetadataEventData {
            key: LoreString::from_str("typed"),
            value: LoreMetadata::String(LoreString::from_str("1")),
        });
        let numeric = LoreEvent::Metadata(LoreMetadataEventData {
            key: LoreString::from_str("typed"),
            value: LoreMetadata::Numeric(1),
        });
        let entries =
            collect_metadata(vec![string.clone(), numeric.clone(), complete(0), end()], 0)
                .expect_err("duplicate metadata keys are an over-counted raw stream");
        assert!(entries.message.contains("duplicate"));

        let string = LoreEvent::Metadata(LoreMetadataEventData {
            key: LoreString::from_str("as_string"),
            value: LoreMetadata::String(LoreString::from_str("1")),
        });
        let numeric = LoreEvent::Metadata(LoreMetadataEventData {
            key: LoreString::from_str("as_numeric"),
            value: LoreMetadata::Numeric(1),
        });
        let entries =
            collect_metadata(vec![string.clone(), numeric.clone(), complete(0), end()], 0)
                .expect("distinct exact metadata events");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, MetadataKind::String);
        assert_eq!(entries[1].kind, MetadataKind::Numeric);
        assert_ne!(entries[0], entries[1]);

        assert!(
            collect_metadata(vec![string.clone(), string, numeric, complete(0), end()], 0,)
                .is_err()
        );

        let (_, missing_end) = raw_metadata_collector();
        *missing_end.lock().unwrap() = super::RawMetadataStream {
            complete_events: 1,
            complete_status: Some(0),
            ..Default::default()
        };
        assert!(finished_raw_metadata(missing_end, 0).is_err());
    }
}
