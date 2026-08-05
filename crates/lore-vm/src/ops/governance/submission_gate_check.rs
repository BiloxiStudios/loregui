//! Witness-side submission gate re-evaluation.

use super::contract::{
    AffectedPath, AuthorResolutionObservation, CanonicalDcoMetadataObservationV1,
    CanonicalEvidenceSnapshotV1, CanonicalFileIdentityV1, CanonicalRevisionInfoV1,
    CanonicalRevisionRefV1, CanonicalStatusObservationV1,
    CanonicalSupersessionMetadataQueryObservationV1, CanonicalWorktreeFileObservationV1,
    CriterionResult, DcoMetadataObservation, EvaluationResult, EvidencePointerV1, FileIdentity,
    GovernanceCriterion, GovernancePathAction, GovernanceRemediation, GovernanceRemediationCode,
    GovernanceRole, HistoryOverflowScope, ImmutableGetItem, LockQuery, LockStatusResponse,
    MutationObservation, ReadObservation, ResolvedAuthor, RevisionDiffObservation, RevisionInfo,
    StatusSnapshot, SubmissionGateCheckRequest, SubmissionGateCheckResult, SupersessionMarkerV1,
    SupersessionMetadataQueryObservation, EVIDENCE_POINTER_KEY, MAX_GOVERNANCE_HISTORY_REVISIONS,
    SUPERSESSION_MARKER_PREFIX,
};
use super::evaluator::{evaluate, GovernanceAdapter, ProductionLoreAdapter};
use super::evidence_preserve::ProductionGovernanceIo;
use super::GovernanceIo;
use crate::api::LoreApi;
use std::collections::{BTreeMap, BTreeSet};

const EVIDENCE_ITEM_ID: u64 = 5934;

const CRITERIA: [GovernanceCriterion; 7] = [
    GovernanceCriterion::ExactSubject,
    GovernanceCriterion::HistoryComplete,
    GovernanceCriterion::DcoValid,
    GovernanceCriterion::NotSuperseded,
    GovernanceCriterion::LocksClear,
    GovernanceCriterion::WorktreeClean,
    GovernanceCriterion::EvidenceValid,
];

fn exact_address(address: &str) -> bool {
    let lowercase_hex = |byte: &u8| byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f');
    let bytes = address.as_bytes();
    bytes.len() == 97
        && bytes[64] == b'-'
        && bytes[..64].iter().all(lowercase_hex)
        && bytes[65..].iter().all(lowercase_hex)
}

fn passed(criterion: GovernanceCriterion) -> CriterionResult {
    CriterionResult {
        criterion,
        passed: true,
        failure_code: None,
        remediation: None,
    }
}

fn failed(criterion: GovernanceCriterion, code: &str) -> CriterionResult {
    CriterionResult {
        criterion,
        passed: false,
        failure_code: Some(code.into()),
        remediation: None,
    }
}

#[derive(Debug, Clone, Copy)]
enum FactError {
    Dependency,
    Invalid,
}

/// Private witness-only decision inputs. The evaluator result is converted at
/// the boundary and then discarded; no evaluator verdict, summary, derived
/// path/file/DCO/marker list, failure label, or actor evidence bytes can be
/// named by a live witness criterion.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WitnessRawFacts {
    expected_staged_revision: String,
    target_base_revision: String,
    status: Option<StatusSnapshot>,
    base_revision_info: Option<RevisionInfo>,
    supersession_ancestry: Vec<RevisionInfo>,
    revision_graph: Vec<RevisionInfo>,
    first_parent_history: Vec<String>,
    base_files: Vec<FileIdentity>,
    base_tree_observed: bool,
    candidate_files: Vec<FileIdentity>,
    candidate_tree_observed: bool,
    upstream_revision_diff: Vec<RevisionDiffObservation>,
    supersession_metadata_queries: Vec<SupersessionMetadataQueryObservation>,
    dco_metadata: Vec<DcoMetadataObservation>,
    author_resolution: Option<AuthorResolutionObservation>,
    lock_queries: Vec<LockQuery>,
    lock_status: Option<LockStatusResponse>,
}

impl WitnessRawFacts {
    fn from_evaluation(evaluation: &EvaluationResult) -> Self {
        let observed = &evaluation.observations;
        Self {
            expected_staged_revision: observed.expected_staged_revision.clone(),
            target_base_revision: observed.target_base_revision.clone(),
            status: observed.status.clone(),
            base_revision_info: observed.base_revision_info.clone(),
            supersession_ancestry: observed.supersession_ancestry.clone(),
            revision_graph: observed.revision_graph.clone(),
            first_parent_history: observed.first_parent_history.clone(),
            base_files: observed.base_files.clone(),
            base_tree_observed: observed.base_tree_observed,
            candidate_files: observed.candidate_files.clone(),
            candidate_tree_observed: observed.candidate_tree_observed,
            upstream_revision_diff: observed.upstream_revision_diff.clone(),
            supersession_metadata_queries: observed.supersession_metadata_queries.clone(),
            dco_metadata: observed.dco_metadata.clone(),
            author_resolution: observed.author_resolution.clone(),
            lock_queries: observed.lock_queries.clone(),
            lock_status: observed.lock_status.clone(),
        }
    }
}

/// The only actor-evidence fields visible at the witness decision boundary.
///
/// `CanonicalEvidenceSnapshotV1` intentionally retains evaluator diagnostics
/// for review, but those summaries are neither witness facts nor authority.
/// Convert both the stored claim and the live re-evaluation immediately into
/// this private raw projection; no decision criterion can name the discarded
/// `open`, current-file, affected-path, marker, DCO, failure, or remediation
/// summaries.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WitnessEvidenceProjection {
    version: String,
    target_base_revision: String,
    status: CanonicalStatusObservationV1,
    base_revision_info: CanonicalRevisionInfoV1,
    supersession_ancestry: Vec<CanonicalRevisionInfoV1>,
    revision_graph: Vec<CanonicalRevisionInfoV1>,
    first_parent_history: Vec<CanonicalRevisionRefV1>,
    base_files: Vec<CanonicalFileIdentityV1>,
    base_tree_observed: bool,
    candidate_files: Vec<CanonicalFileIdentityV1>,
    candidate_tree_observed: bool,
    upstream_revision_diff: Vec<RevisionDiffObservation>,
    supersession_metadata_queries: Vec<CanonicalSupersessionMetadataQueryObservationV1>,
    dco_metadata: Vec<CanonicalDcoMetadataObservationV1>,
    author_resolution: AuthorResolutionObservation,
    lock_queries: Vec<LockQuery>,
    lock_status: LockStatusResponse,
}

fn witness_canonical_revision(revision: &str, staged: &str) -> CanonicalRevisionRefV1 {
    if revision == staged {
        CanonicalRevisionRefV1::StagedSubject
    } else {
        CanonicalRevisionRefV1::Exact(revision.to_string())
    }
}

fn witness_canonical_revisions(revisions: &[String], staged: &str) -> Vec<CanonicalRevisionRefV1> {
    revisions
        .iter()
        .map(|revision| witness_canonical_revision(revision, staged))
        .collect()
}

fn witness_sort_resolved_authors(authors: &mut [ResolvedAuthor]) {
    authors.sort_by(|left, right| {
        (&left.identity, &left.display_name).cmp(&(&right.identity, &right.display_name))
    });
}

fn witness_canonical_files(files: &[FileIdentity], staged: &str) -> Vec<CanonicalFileIdentityV1> {
    let mut canonical: Vec<_> = files
        .iter()
        .map(|file| CanonicalFileIdentityV1 {
            path: file.path.clone(),
            revision: witness_canonical_revision(&file.revision, staged),
            hash: file.hash.clone(),
            context: file.context.clone(),
        })
        .collect();
    canonical.sort_by(|left, right| {
        (&left.path, &left.revision, &left.hash, &left.context).cmp(&(
            &right.path,
            &right.revision,
            &right.hash,
            &right.context,
        ))
    });
    canonical
}

impl WitnessEvidenceProjection {
    fn from_raw(observed: &WitnessRawFacts) -> Result<Self, ()> {
        let staged = observed.expected_staged_revision.as_str();
        let status = observed.status.as_ref().ok_or(())?;
        let base_revision_info = observed.base_revision_info.as_ref().ok_or(())?;
        let mut author_resolution = observed.author_resolution.clone().ok_or(())?;
        let mut lock_status = observed.lock_status.clone().ok_or(())?;
        if staged.is_empty() || observed.target_base_revision.is_empty() {
            return Err(());
        }

        let canonical_info = |info: &RevisionInfo| CanonicalRevisionInfoV1 {
            revision: witness_canonical_revision(&info.revision, staged),
            // First-parent order is semantic and must remain exact.
            parents: info
                .parents
                .iter()
                .map(|parent| witness_canonical_revision(parent, staged))
                .collect(),
        };
        let mut supersession_ancestry: Vec<_> = observed
            .supersession_ancestry
            .iter()
            .map(canonical_info)
            .collect();
        supersession_ancestry.sort_by(|left, right| left.revision.cmp(&right.revision));
        let mut revision_graph: Vec<_> =
            observed.revision_graph.iter().map(canonical_info).collect();
        revision_graph.sort_by(|left, right| left.revision.cmp(&right.revision));

        let mut worktree_files: Vec<_> = status
            .worktree_files
            .iter()
            .map(|file| CanonicalWorktreeFileObservationV1 {
                path: file.path.clone(),
                revision: witness_canonical_revision(&file.revision, staged),
                revision_hash: file.revision_hash.clone(),
                revision_context: file.revision_context.clone(),
                revision_size: file.revision_size,
                local_hash: file.local_hash.clone(),
                local_size: file.local_size,
                filtered_revision_size: file.filtered_revision_size,
                flag_modified: file.flag_modified,
                flag_deleted: file.flag_deleted,
                flag_added: file.flag_added,
                flag_conflict: file.flag_conflict,
            })
            .collect();
        worktree_files.sort_by(|left, right| left.path.cmp(&right.path));

        let mut supersession_metadata_queries: Vec<_> = observed
            .supersession_metadata_queries
            .iter()
            .map(|query| {
                let mut metadata: Vec<_> = query
                    .metadata
                    .iter()
                    .filter(|entry| query.revision != staged || entry.key != EVIDENCE_POINTER_KEY)
                    .cloned()
                    .collect();
                metadata.sort_by(|left, right| {
                    (&left.key, left.kind, &left.value).cmp(&(&right.key, right.kind, &right.value))
                });
                CanonicalSupersessionMetadataQueryObservationV1 {
                    revision: witness_canonical_revision(&query.revision, staged),
                    metadata,
                }
            })
            .collect();
        supersession_metadata_queries.sort_by(|left, right| left.revision.cmp(&right.revision));

        let mut dco_metadata: Vec<_> = observed
            .dco_metadata
            .iter()
            .map(|entry| {
                let mut messages = entry.messages.clone();
                messages.sort();
                let mut created_by = entry.created_by.clone();
                created_by.sort();
                let mut committed_by = entry.committed_by.clone();
                committed_by.sort();
                CanonicalDcoMetadataObservationV1 {
                    revision: witness_canonical_revision(&entry.revision, staged),
                    messages,
                    created_by,
                    committed_by,
                }
            })
            .collect();
        dco_metadata.sort_by(|left, right| left.revision.cmp(&right.revision));

        author_resolution.requested.sort();
        witness_sort_resolved_authors(&mut author_resolution.replies);
        let mut lock_queries = observed.lock_queries.clone();
        for query in &mut lock_queries {
            query.ignored_paths.sort();
            query.owners.sort();
        }
        lock_queries.sort_by(|left, right| left.path.cmp(&right.path));
        lock_status.ignored_paths.sort();
        lock_status
            .statuses
            .sort_by(|left, right| (&left.path, &left.owner).cmp(&(&right.path, &right.owner)));
        let mut upstream_revision_diff = observed.upstream_revision_diff.clone();
        upstream_revision_diff.sort();

        Ok(Self {
            version: "v1".into(),
            target_base_revision: observed.target_base_revision.clone(),
            status: CanonicalStatusObservationV1 {
                branch: status.branch.clone(),
                staged_revisions: witness_canonical_revisions(&status.staged_revisions, staged),
                scanned_staged_revisions: witness_canonical_revisions(
                    &status.scanned_staged_revisions,
                    staged,
                ),
                post_scan_staged_revisions: witness_canonical_revisions(
                    &status.post_scan_staged_revisions,
                    staged,
                ),
                staged_paths: {
                    let mut paths = status.staged_paths.clone();
                    paths.sort();
                    paths.dedup();
                    paths
                },
                staged_changes: {
                    let mut changes = status.staged_changes.clone();
                    changes.sort();
                    changes
                },
                worktree_files,
                worktree_clean: status.worktree_clean,
                scan_performed: status.scan_performed,
            },
            base_revision_info: canonical_info(base_revision_info),
            supersession_ancestry,
            revision_graph,
            first_parent_history: witness_canonical_revisions(
                &observed.first_parent_history,
                staged,
            ),
            base_files: witness_canonical_files(&observed.base_files, staged),
            base_tree_observed: observed.base_tree_observed,
            candidate_files: witness_canonical_files(&observed.candidate_files, staged),
            candidate_tree_observed: observed.candidate_tree_observed,
            upstream_revision_diff,
            supersession_metadata_queries,
            dco_metadata,
            author_resolution,
            lock_queries,
            lock_status,
        })
    }

    fn from_stored(stored: &CanonicalEvidenceSnapshotV1) -> Self {
        Self {
            version: stored.version.clone(),
            target_base_revision: stored.target_base_revision.clone(),
            status: stored.status.clone(),
            base_revision_info: stored.base_revision_info.clone(),
            supersession_ancestry: stored.supersession_ancestry.clone(),
            revision_graph: stored.revision_graph.clone(),
            first_parent_history: stored.first_parent_history.clone(),
            base_files: stored.base_files.clone(),
            base_tree_observed: stored.base_tree_observed,
            candidate_files: stored.candidate_files.clone(),
            candidate_tree_observed: stored.candidate_tree_observed,
            upstream_revision_diff: stored.upstream_revision_diff.clone(),
            supersession_metadata_queries: stored.supersession_metadata_queries.clone(),
            dco_metadata: stored.dco_metadata.clone(),
            author_resolution: stored.author_resolution.clone(),
            lock_queries: stored.lock_queries.clone(),
            lock_status: stored.lock_status.clone(),
        }
    }
}

fn canonical_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn witness_artifact_identity(identity: &str) -> bool {
    let Some((hash, context)) = identity.split_once(':') else {
        return false;
    };
    canonical_lower_hex(hash, 64)
        && canonical_lower_hex(context, 32)
        && !context.bytes().all(|byte| byte == b'0')
}

fn witness_worktree_clean(
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

fn witness_current_files(
    status: &StatusSnapshot,
    candidate_files: &[FileIdentity],
    expected_revision: &str,
) -> Result<Vec<FileIdentity>, ()> {
    if !witness_worktree_clean(status, candidate_files, expected_revision) {
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
        if !identities.insert(format!("{hash}:{}", file.context)) {
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

/// Reconstruct the affected path set from raw status, trees, capture-time
/// bytes, and the upstream terminal diff. This is deliberately independent of
/// the evaluator's affected-path implementation and never accepts its
/// `revision_diff` or `affected_paths` summaries.
fn witness_affected_paths(observed: &WitnessRawFacts) -> Result<Vec<String>, ()> {
    if !observed.base_tree_observed || !observed.candidate_tree_observed {
        return Err(());
    }
    let status = observed.status.as_ref().ok_or(())?;
    let current_files = witness_current_files(
        status,
        &observed.candidate_files,
        &observed.expected_staged_revision,
    )?;
    let mut upstream_diff = observed.upstream_revision_diff.clone();
    upstream_diff.sort();
    if upstream_diff
        .iter()
        .any(|raw| raw.action == GovernancePathAction::Copy)
    {
        return Err(());
    }

    let mut base_by_context = BTreeMap::new();
    let mut current_by_context = BTreeMap::new();
    let mut base_by_path = BTreeMap::new();
    let mut candidate_by_path = BTreeMap::new();
    let mut current_by_path = BTreeMap::new();
    for file in &observed.base_files {
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
            return Err(());
        }
    }
    for file in &observed.candidate_files {
        if !canonical_lower_hex(&file.hash, 64)
            || !canonical_lower_hex(&file.context, 32)
            || file.context.bytes().all(|byte| byte == b'0')
            || candidate_by_path.insert(file.path.as_str(), file).is_some()
        {
            return Err(());
        }
    }
    for file in &current_files {
        if current_by_context
            .insert(file.context.as_str(), file.path.as_str())
            .is_some()
            || current_by_path
                .insert(file.path.as_str(), file.canonical_id())
                .is_some()
        {
            return Err(());
        }
    }

    let mut consumed_base = BTreeSet::new();
    let mut consumed_current = BTreeSet::new();
    let mut diff = Vec::<AffectedPath>::new();
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
        return Err(());
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
        return Err(());
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
        return Err(());
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
    let base_file_by_path: BTreeMap<_, _> = observed
        .base_files
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
            return Err(());
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
                    return Err(());
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
                    return Err(());
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
                    return Err(());
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
            GovernancePathAction::Copy => return Err(()),
        }
    }
    if consumed_raw.iter().any(|consumed| !consumed) {
        return Err(());
    }
    let paths: BTreeSet<_> = diff
        .iter()
        .flat_map(|entry| entry.source_path.iter().chain(entry.target_path.iter()))
        .cloned()
        .collect();
    if paths.is_empty() {
        return Err(());
    }
    Ok(paths.into_iter().collect())
}

fn witness_remediation_for_overflow(scope: HistoryOverflowScope) -> GovernanceRemediation {
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

fn exact_subject(observed: &WitnessRawFacts) -> CriterionResult {
    let criterion = GovernanceCriterion::ExactSubject;
    let Some(status) = observed.status.as_ref() else {
        return failed(criterion, "exact_subject_dependency_failed");
    };
    let expected = observed.expected_staged_revision.as_str();
    if expected.is_empty()
        || observed.target_base_revision.is_empty()
        || [
            &status.staged_revisions,
            &status.scanned_staged_revisions,
            &status.post_scan_staged_revisions,
        ]
        .into_iter()
        .any(|revisions| revisions.as_slice() != [expected])
    {
        failed(criterion, "exact_subject_failed")
    } else {
        passed(criterion)
    }
}

fn exact_history(observed: &WitnessRawFacts) -> Result<BTreeSet<String>, FactError> {
    let Some(base_info) = observed.base_revision_info.as_ref() else {
        return Err(FactError::Dependency);
    };
    if observed.revision_graph.is_empty() {
        return Err(FactError::Dependency);
    }
    let candidate = observed.expected_staged_revision.as_str();
    let base = observed.target_base_revision.as_str();
    if candidate.is_empty()
        || base.is_empty()
        || candidate == base
        || base_info.revision != base
        || observed.revision_graph.len() > MAX_GOVERNANCE_HISTORY_REVISIONS
        || observed.first_parent_history.len() > MAX_GOVERNANCE_HISTORY_REVISIONS
    {
        return Err(FactError::Invalid);
    }

    let mut graph = BTreeMap::new();
    for info in &observed.revision_graph {
        let unique_parents: BTreeSet<_> = info.parents.iter().collect();
        if info.revision.is_empty()
            || info.revision == base
            || info.parents.is_empty()
            || info.parents.len() > 2
            || unique_parents.len() != info.parents.len()
            || info
                .parents
                .iter()
                .any(|parent| parent.is_empty() || parent == &info.revision)
            || graph.insert(info.revision.as_str(), info).is_some()
        {
            return Err(FactError::Invalid);
        }
    }

    let mut states = BTreeMap::<&str, u8>::new();
    let mut stack = vec![(candidate, false)];
    while let Some((revision, leaving)) = stack.pop() {
        if revision == base {
            continue;
        }
        if leaving {
            states.insert(revision, 2);
            continue;
        }
        match states.get(revision) {
            Some(1) => return Err(FactError::Invalid),
            Some(2) => continue,
            _ => {}
        }
        let Some(info) = graph.get(revision) else {
            return Err(FactError::Invalid);
        };
        states.insert(revision, 1);
        stack.push((revision, true));
        for parent in info.parents.iter().rev() {
            stack.push((parent.as_str(), false));
        }
    }
    if states.len() != graph.len() || states.values().any(|state| *state != 2) {
        return Err(FactError::Invalid);
    }
    if observed.first_parent_history.is_empty() {
        return Err(FactError::Dependency);
    }

    let mut reconstructed = Vec::new();
    let mut seen = BTreeSet::new();
    let mut revision = candidate;
    while revision != base {
        if !seen.insert(revision) || reconstructed.len() == MAX_GOVERNANCE_HISTORY_REVISIONS {
            return Err(FactError::Invalid);
        }
        let Some(info) = graph.get(revision) else {
            return Err(FactError::Invalid);
        };
        reconstructed.push(revision.to_string());
        revision = info.parents[0].as_str();
    }
    if reconstructed != observed.first_parent_history {
        return Err(FactError::Invalid);
    }
    Ok(graph
        .keys()
        .map(|revision| (*revision).to_string())
        .collect())
}

fn exact_revision_shape(info: &RevisionInfo, allow_root: bool) -> bool {
    let parents: BTreeSet<_> = info.parents.iter().map(String::as_str).collect();
    !info.revision.is_empty()
        && (allow_root || !info.parents.is_empty())
        && info.parents.len() <= 2
        && parents.len() == info.parents.len()
        && info
            .parents
            .iter()
            .all(|parent| !parent.is_empty() && parent != &info.revision)
}

/// Validate a retained N+1 traversal prefix without assuming its frontier is
/// complete. Every retained node must be exact, structurally valid, acyclic,
/// and reachable from the candidate through retained edges. Parent references
/// outside the prefix are the unvisited frontier after the sentinel.
fn exact_overflow_prefix(
    infos: &[RevisionInfo],
    candidate: &str,
    base: &str,
    pending: bool,
) -> Result<(), FactError> {
    if infos.len() != MAX_GOVERNANCE_HISTORY_REVISIONS + 1 {
        return Err(FactError::Invalid);
    }
    let mut graph = BTreeMap::new();
    for info in infos {
        if !exact_revision_shape(info, !pending)
            || (pending && info.revision == base)
            || graph.insert(info.revision.as_str(), info).is_some()
        {
            return Err(FactError::Invalid);
        }
    }
    if !graph.contains_key(candidate) {
        return Err(FactError::Invalid);
    }

    let mut states = BTreeMap::<&str, u8>::new();
    let mut stack = vec![(candidate, false)];
    while let Some((revision, leaving)) = stack.pop() {
        if leaving {
            states.insert(revision, 2);
            continue;
        }
        match states.get(revision) {
            Some(1) => return Err(FactError::Invalid),
            Some(2) => continue,
            _ => {}
        }
        let Some(info) = graph.get(revision) else {
            continue;
        };
        states.insert(revision, 1);
        stack.push((revision, true));
        for parent in info.parents.iter().rev() {
            if graph.contains_key(parent.as_str()) {
                stack.push((parent.as_str(), false));
            }
        }
    }
    if states.len() != graph.len() || states.values().any(|state| *state != 2) {
        return Err(FactError::Invalid);
    }
    Ok(())
}

/// Derive overflow scope solely from the exact N+1 raw graph. The evaluator's
/// scope label is deliberately inert and can never select or alter witness
/// reason/remediation.
fn raw_overflow_scope(
    observed: &WitnessRawFacts,
) -> Result<Option<HistoryOverflowScope>, FactError> {
    let pending_overflow = observed.revision_graph.len() == MAX_GOVERNANCE_HISTORY_REVISIONS + 1;
    let ancestry_overflow =
        observed.supersession_ancestry.len() == MAX_GOVERNANCE_HISTORY_REVISIONS + 1;
    if observed.revision_graph.len() > MAX_GOVERNANCE_HISTORY_REVISIONS + 1
        || observed.supersession_ancestry.len() > MAX_GOVERNANCE_HISTORY_REVISIONS + 1
        || (pending_overflow && ancestry_overflow)
    {
        return Err(FactError::Invalid);
    }
    let candidate = observed.expected_staged_revision.as_str();
    let base = observed.target_base_revision.as_str();
    let Some(base_info) = observed.base_revision_info.as_ref() else {
        return Err(FactError::Dependency);
    };
    if base_info.revision != base || !exact_revision_shape(base_info, true) {
        return Err(FactError::Invalid);
    }

    if pending_overflow {
        if !observed.supersession_ancestry.is_empty() || !observed.first_parent_history.is_empty() {
            return Err(FactError::Invalid);
        }
        exact_overflow_prefix(&observed.revision_graph, candidate, base, true)?;
        return Ok(Some(HistoryOverflowScope::PendingDco));
    }
    if ancestry_overflow {
        let pending_graph: BTreeMap<_, _> = observed
            .revision_graph
            .iter()
            .map(|info| (info.revision.as_str(), info))
            .collect();
        if exact_history(observed).is_err()
            || observed
                .supersession_ancestry
                .iter()
                .find(|info| info.revision == base)
                != Some(base_info)
            || observed.supersession_ancestry.iter().any(|info| {
                pending_graph
                    .get(info.revision.as_str())
                    .is_some_and(|pending| *pending != info)
            })
        {
            return Err(FactError::Invalid);
        }
        exact_overflow_prefix(&observed.supersession_ancestry, candidate, base, false)?;
        return Ok(Some(HistoryOverflowScope::SupersessionAncestry));
    }
    Ok(None)
}

fn history_criterion(observed: &WitnessRawFacts) -> CriterionResult {
    let raw_scope = match raw_overflow_scope(observed) {
        Ok(scope) => scope,
        Err(FactError::Dependency) => {
            return failed(
                GovernanceCriterion::HistoryComplete,
                "history_dependency_failed",
            )
        }
        Err(FactError::Invalid) => {
            return failed(GovernanceCriterion::HistoryComplete, "history_incomplete")
        }
    };
    if let Some(scope) = raw_scope {
        return CriterionResult {
            criterion: GovernanceCriterion::HistoryComplete,
            passed: false,
            failure_code: Some("history_depth_exceeded".into()),
            remediation: Some(witness_remediation_for_overflow(scope)),
        };
    }
    match (
        exact_history(observed),
        exact_supersession_ancestry(observed),
    ) {
        (Ok(_), Ok(_)) => passed(GovernanceCriterion::HistoryComplete),
        (Err(FactError::Invalid), _) | (_, Err(FactError::Invalid)) => {
            failed(GovernanceCriterion::HistoryComplete, "history_incomplete")
        }
        (Err(FactError::Dependency), _) | (_, Err(FactError::Dependency)) => failed(
            GovernanceCriterion::HistoryComplete,
            "history_dependency_failed",
        ),
    }
}

fn exact_supersession_ancestry(observed: &WitnessRawFacts) -> Result<BTreeSet<String>, FactError> {
    if observed.supersession_ancestry.is_empty() {
        return Err(FactError::Dependency);
    }
    if observed.supersession_ancestry.len() > MAX_GOVERNANCE_HISTORY_REVISIONS {
        return Err(FactError::Invalid);
    }
    let mut pending_graph = BTreeMap::new();
    for info in &observed.revision_graph {
        if pending_graph.insert(info.revision.as_str(), info).is_some() {
            return Err(FactError::Invalid);
        }
    }
    let mut graph = BTreeMap::new();
    for info in &observed.supersession_ancestry {
        let parents: BTreeSet<_> = info.parents.iter().map(String::as_str).collect();
        if info.revision.is_empty()
            || info.parents.len() > 2
            || parents.len() != info.parents.len()
            || info
                .parents
                .iter()
                .any(|parent| parent.is_empty() || parent == &info.revision)
            || pending_graph
                .get(info.revision.as_str())
                .is_some_and(|pending| *pending != info)
            || graph.insert(info.revision.as_str(), info).is_some()
        {
            return Err(FactError::Invalid);
        }
    }
    let candidate = observed.expected_staged_revision.as_str();
    let base = observed.target_base_revision.as_str();
    let Some(base_info) = observed.base_revision_info.as_ref() else {
        return Err(FactError::Dependency);
    };
    if graph.get(base).copied() != Some(base_info) || !graph.contains_key(candidate) {
        return Err(FactError::Invalid);
    }

    let mut states = BTreeMap::<&str, u8>::new();
    let mut stack = vec![(candidate, false)];
    while let Some((revision, leaving)) = stack.pop() {
        if leaving {
            states.insert(revision, 2);
            continue;
        }
        match states.get(revision) {
            Some(1) => return Err(FactError::Invalid),
            Some(2) => continue,
            _ => {}
        }
        let Some(info) = graph.get(revision) else {
            return Err(FactError::Invalid);
        };
        states.insert(revision, 1);
        stack.push((revision, true));
        for parent in info.parents.iter().rev() {
            stack.push((parent.as_str(), false));
        }
    }
    if states.len() != graph.len() || states.values().any(|state| *state != 2) {
        return Err(FactError::Invalid);
    }
    Ok(graph
        .keys()
        .map(|revision| (*revision).to_string())
        .collect())
}

struct WitnessDcoSigner {
    name: String,
}

fn witness_parse_trailer(line: &str) -> Option<(&str, &str)> {
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

fn witness_canonical_dco_email(email: &str) -> bool {
    if email.is_empty()
        || email.chars().any(|character| {
            character.is_whitespace() || character.is_control() || matches!(character, '<' | '>')
        })
    {
        return false;
    }
    let mut parts = email.split('@');
    let (Some(local), Some(domain)) = (parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && parts.next().is_none()
        && domain.split('.').all(|label| {
            let (Some(first), Some(last)) = (label.as_bytes().first(), label.as_bytes().last())
            else {
                return false;
            };
            first.is_ascii_alphanumeric()
                && last.is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn witness_parse_dco_signer(message: &str) -> Option<WitnessDcoSigner> {
    let mut lines: Vec<&str> = message.lines().collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let end = lines.len();
    let mut start = end;
    while start > 0 && witness_parse_trailer(lines[start - 1]).is_some() {
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
            let (key, value) = witness_parse_trailer(line)?;
            (key == "Signed-off-by").then_some(value)
        })
        .collect();
    let [signer] = signers.as_slice() else {
        return None;
    };
    let identity = signer.strip_suffix('>')?;
    let (name, email) = identity.split_once(" <")?;
    let first = name.chars().next()?;
    let last = name.chars().last()?;
    if first.is_whitespace()
        || last.is_whitespace()
        || name
            .chars()
            .any(|character| character.is_control() || matches!(character, '<' | '>'))
        || !witness_canonical_dco_email(email)
    {
        return None;
    }
    Some(WitnessDcoSigner { name: name.into() })
}

fn dco_criterion(observed: &WitnessRawFacts) -> CriterionResult {
    let criterion = GovernanceCriterion::DcoValid;
    let pending = match exact_history(observed) {
        Ok(pending) => pending,
        Err(_) => return failed(criterion, "dco_dependency_failed"),
    };
    if observed.dco_metadata.is_empty() || observed.dco_metadata.len() != pending.len() {
        return failed(criterion, "dco_dependency_failed");
    }
    let mut revisions = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut identity_signer_names = Vec::new();
    for raw in &observed.dco_metadata {
        if !pending.contains(&raw.revision)
            || !revisions.insert(raw.revision.as_str())
            || raw.messages.len() != 1
            || raw.created_by.len() != 1
            || raw.committed_by.len() > 1
            || raw.messages[0].is_empty()
            || raw.created_by[0].is_empty()
            || raw.committed_by.iter().any(String::is_empty)
        {
            return failed(criterion, "dco_invalid");
        }
        let Some(signer) = witness_parse_dco_signer(&raw.messages[0]) else {
            return failed(criterion, "dco_invalid");
        };
        identities.insert(raw.created_by[0].clone());
        identities.extend(raw.committed_by.iter().cloned());
        identity_signer_names.push((raw.created_by[0].clone(), signer.name.clone()));
        identity_signer_names.extend(
            raw.committed_by
                .iter()
                .cloned()
                .map(|identity| (identity, signer.name.clone())),
        );
    }
    let Some(auth) = observed.author_resolution.as_ref() else {
        return failed(criterion, "dco_dependency_failed");
    };
    let requested: BTreeSet<_> = auth.requested.iter().cloned().collect();
    if requested != identities || requested.len() != auth.requested.len() {
        return failed(criterion, "dco_invalid");
    }
    let mut replies = BTreeMap::new();
    if auth.replies.len() != requested.len()
        || auth.replies.iter().any(|reply| {
            reply.identity.is_empty()
                || reply.display_name.is_empty()
                || !requested.contains(&reply.identity)
                || replies
                    .insert(reply.identity.as_str(), reply.display_name.as_str())
                    .is_some()
        })
        || identity_signer_names.iter().any(|(identity, signer_name)| {
            replies.get(identity.as_str()).copied() != Some(signer_name.as_str())
        })
    {
        return failed(criterion, "dco_invalid");
    }
    passed(criterion)
}

fn not_superseded_criterion(observed: &WitnessRawFacts) -> CriterionResult {
    let criterion = GovernanceCriterion::NotSuperseded;
    if !observed.candidate_tree_observed {
        return failed(criterion, "not_superseded_dependency_failed");
    }
    let Some(status) = observed.status.as_ref() else {
        return failed(criterion, "not_superseded_dependency_failed");
    };
    let current_files = match witness_current_files(
        status,
        &observed.candidate_files,
        &observed.expected_staged_revision,
    ) {
        Ok(files) => files,
        _ => return failed(criterion, "not_superseded_dependency_failed"),
    };
    let ancestry = match exact_supersession_ancestry(observed) {
        Ok(ancestry) => ancestry,
        Err(_) => return failed(criterion, "not_superseded_dependency_failed"),
    };
    let mut candidate_paths = BTreeSet::new();
    let mut candidate_identities = BTreeSet::new();
    for file in &current_files {
        if file.path.is_empty()
            || file.revision != observed.expected_staged_revision
            || file.hash.is_empty()
            || file.context.is_empty()
            || !candidate_paths.insert(file.path.as_str())
            || !candidate_identities.insert(file.canonical_id())
        {
            return failed(criterion, "not_superseded_failed");
        }
    }
    let mut queries = BTreeMap::new();
    for query in &observed.supersession_metadata_queries {
        if query.revision.is_empty()
            || !ancestry.contains(&query.revision)
            || queries
                .insert(query.revision.as_str(), query.metadata.as_slice())
                .is_some()
        {
            return failed(criterion, "not_superseded_failed");
        }
    }
    if queries.len() != ancestry.len()
        || ancestry
            .iter()
            .any(|revision| !queries.contains_key(revision.as_str()))
    {
        return failed(criterion, "not_superseded_dependency_failed");
    }

    let mut records = BTreeMap::<String, String>::new();
    for metadata in queries.values() {
        let mut keys = BTreeSet::new();
        for entry in *metadata {
            if entry.key.is_empty() || !keys.insert(entry.key.as_str()) {
                return failed(criterion, "not_superseded_failed");
            }
            if !entry.key.starts_with(SUPERSESSION_MARKER_PREFIX) {
                continue;
            }
            let identity = entry.key[SUPERSESSION_MARKER_PREFIX.len()..].to_string();
            let Some(value) = entry.string_value() else {
                return failed(criterion, "not_superseded_failed");
            };
            let marker: SupersessionMarkerV1 = match serde_json::from_str(value) {
                Ok(marker) => marker,
                Err(_) => return failed(criterion, "not_superseded_failed"),
            };
            if !witness_artifact_identity(&identity)
                || marker.version != "v1"
                || marker.identity != identity
                || records
                    .insert(identity, value.to_string())
                    .is_some_and(|existing| existing != value)
            {
                return failed(criterion, "not_superseded_failed");
            }
        }
    }
    if records
        .keys()
        .any(|identity| candidate_identities.contains(identity))
    {
        failed(criterion, "not_superseded_failed")
    } else {
        passed(criterion)
    }
}

fn locks_criterion(observed: &WitnessRawFacts) -> CriterionResult {
    let criterion = GovernanceCriterion::LocksClear;
    let affected_paths = match witness_affected_paths(observed) {
        Ok(paths) => paths,
        Err(()) => return failed(criterion, "locks_dependency_failed"),
    };
    let paths: BTreeSet<_> = affected_paths.iter().map(String::as_str).collect();
    let Some(status) = observed.lock_status.as_ref() else {
        return failed(criterion, "locks_dependency_failed");
    };
    if paths.is_empty()
        || observed
            .status
            .as_ref()
            .map(|status| status.branch.is_empty())
            .unwrap_or(true)
        || paths.len() != affected_paths.len()
        || observed.lock_queries.len() != paths.len()
    {
        return failed(criterion, "locks_dependency_failed");
    }
    let mut queried = BTreeSet::new();
    for query in &observed.lock_queries {
        if query.path.is_empty()
            || !paths.contains(query.path.as_str())
            || !queried.insert(query.path.as_str())
            || query.begin_events != 1
            || !query.completed
            || !query.ignored_paths.is_empty()
            || query.expected_count != query.owners.len()
        {
            return failed(criterion, "locks_dependency_failed");
        }
        if !query.owners.is_empty() {
            return failed(criterion, "locks_clear_failed");
        }
    }
    if status.begin_events != 1
        || !status.completed
        || !status.ignored_paths.is_empty()
        || status.expected_count != status.statuses.len()
    {
        return failed(criterion, "locks_dependency_failed");
    }
    let mut status_paths = BTreeSet::new();
    for entry in &status.statuses {
        if !paths.contains(entry.path.as_str()) || !status_paths.insert(entry.path.as_str()) {
            return failed(criterion, "locks_dependency_failed");
        }
        if entry.owner.is_some() {
            return failed(criterion, "locks_clear_failed");
        }
    }
    passed(criterion)
}

fn worktree_criterion(observed: &WitnessRawFacts) -> CriterionResult {
    let criterion = GovernanceCriterion::WorktreeClean;
    let Some(status) = observed.status.as_ref() else {
        return failed(criterion, "worktree_dependency_failed");
    };
    if !observed.candidate_tree_observed || !status.scan_performed {
        return failed(criterion, "worktree_dependency_failed");
    }
    if witness_worktree_clean(
        status,
        &observed.candidate_files,
        &observed.expected_staged_revision,
    ) {
        passed(criterion)
    } else {
        failed(criterion, "worktree_dirty")
    }
}

fn live_criteria(observed: &WitnessRawFacts) -> Vec<CriterionResult> {
    vec![
        exact_subject(observed),
        history_criterion(observed),
        dco_criterion(observed),
        not_superseded_criterion(observed),
        locks_criterion(observed),
        worktree_criterion(observed),
    ]
}

fn witness_role_rejection() -> SubmissionGateCheckResult {
    SubmissionGateCheckResult {
        gate_open: false,
        criteria: CRITERIA
            .into_iter()
            .map(|criterion| failed(criterion, "witness_role_required"))
            .collect(),
    }
}

fn validate_get(items: Vec<ImmutableGetItem>, address: &str) -> Result<Vec<u8>, String> {
    if items.len() != 1 {
        return Err("evidence_get_cardinality".into());
    }
    let item = items.into_iter().next().expect("length checked");
    if item.id != EVIDENCE_ITEM_ID
        || !item.ok
        || item.address != address
        || item.size != item.data.len() as u64
    {
        return Err("evidence_get_invalid".into());
    }
    Ok(item.data)
}

async fn evidence_criterion<A: GovernanceAdapter, I: GovernanceIo>(
    adapter: &A,
    io: &I,
    request: &SubmissionGateCheckRequest,
    observed: &WitnessRawFacts,
    live: &[CriterionResult],
) -> CriterionResult {
    let failure = |code: &str| CriterionResult {
        criterion: GovernanceCriterion::EvidenceValid,
        passed: false,
        failure_code: Some(code.into()),
        remediation: None,
    };
    if live.iter().any(|criterion| !criterion.passed) {
        return failure("dependency_failed");
    }
    let expected = match WitnessEvidenceProjection::from_raw(observed) {
        Ok(projection) => projection,
        Err(_) => return failure("evidence_live_snapshot_invalid"),
    };

    let metadata = match adapter
        .revision_metadata(&request.expected_staged_revision)
        .await
    {
        Ok(metadata) => metadata,
        Err(_) => return failure("evidence_pointer_unavailable"),
    };
    let pointers: Vec<_> = metadata
        .iter()
        .filter(|entry| entry.key == EVIDENCE_POINTER_KEY)
        .collect();
    if pointers.is_empty() {
        return failure("evidence_pointer_missing");
    }
    if pointers.len() != 1 {
        return failure("evidence_pointer_invalid");
    }
    let Some(pointer_value) = pointers[0].string_value() else {
        return failure("evidence_pointer_invalid");
    };
    let pointer: EvidencePointerV1 = match serde_json::from_str(pointer_value) {
        Ok(pointer) => pointer,
        Err(_) => return failure("evidence_pointer_invalid"),
    };
    if pointer.version != "v1" || !exact_address(&pointer.address) {
        return failure("evidence_pointer_invalid");
    }

    let handle = match io.storage_open().await {
        MutationObservation::Completed(handles) => match handles.as_slice() {
            [handle] if *handle != 0 => *handle,
            _ => return failure("evidence_storage_open_failed"),
        },
        MutationObservation::NotDispatched { .. } | MutationObservation::OutcomeUnknown { .. } => {
            return failure("evidence_storage_open_failed")
        }
    };
    let read = match io.storage_get(handle, &pointer.address).await {
        ReadObservation::Completed(items) => validate_get(items, &pointer.address),
        ReadObservation::NotDispatched { .. } | ReadObservation::Unavailable { .. } => {
            Err("evidence_get_failed".into())
        }
    };
    if !matches!(
        io.storage_close(handle).await,
        MutationObservation::Completed(())
    ) {
        return failure("evidence_storage_close_failed");
    }
    let bytes = match read {
        Ok(bytes) => bytes,
        Err(code) => return failure(&code),
    };
    let stored_snapshot: CanonicalEvidenceSnapshotV1 = match serde_json::from_slice(&bytes) {
        Ok(snapshot) => snapshot,
        Err(_) => return failure("evidence_schema_invalid"),
    };
    let stored = WitnessEvidenceProjection::from_stored(&stored_snapshot);
    if stored != expected {
        return failure("evidence_mismatch");
    }
    // The actor claim proves only schema/raw-fact equality/binding. Discard
    // the complete summary-bearing claim and its bytes before the post-claim
    // live witness pass; no decision criterion can recover either.
    drop(stored);
    drop(stored_snapshot);
    drop(bytes);

    CriterionResult {
        criterion: GovernanceCriterion::EvidenceValid,
        passed: true,
        failure_code: None,
        remediation: None,
    }
}

pub async fn submission_gate_check_with_adapters<A: GovernanceAdapter, I: GovernanceIo>(
    adapter: &A,
    io: &I,
    request: &SubmissionGateCheckRequest,
) -> SubmissionGateCheckResult {
    if io.role() != GovernanceRole::Witness {
        return witness_role_rejection();
    }
    let initial = evaluate(adapter, request).await;
    let initial_facts = WitnessRawFacts::from_evaluation(&initial);
    // The summary-bearing evaluator result is deliberately unavailable to
    // every evidence and terminal decision below this boundary.
    drop(initial);
    let initial_live = live_criteria(&initial_facts);
    let mut evidence =
        evidence_criterion(adapter, io, request, &initial_facts, &initial_live).await;

    // Evidence retrieval is not atomic with repository state. Re-read every
    // live dependency after the storage handle has closed and require both the
    // independently derived criteria and canonical raw snapshot to be stable.
    let final_evaluation = evaluate(adapter, request).await;
    let final_facts = WitnessRawFacts::from_evaluation(&final_evaluation);
    drop(final_evaluation);
    let criteria = live_criteria(&final_facts);
    // Stability is a property of the retained raw inputs, not merely of the
    // verdicts they happen to derive. Two different fact sets that produce the
    // same criteria are still live drift and must close the gate.
    let live_stable = final_facts == initial_facts;
    let projections_stable = match (
        WitnessEvidenceProjection::from_raw(&initial_facts),
        WitnessEvidenceProjection::from_raw(&final_facts),
    ) {
        (Ok(initial), Ok(final_projection)) => initial == final_projection,
        _ => false,
    };
    if !live_stable || !projections_stable {
        evidence = failed(GovernanceCriterion::EvidenceValid, "evidence_live_drift");
    }
    if io.role() != GovernanceRole::Witness {
        evidence = failed(GovernanceCriterion::EvidenceValid, "witness_role_required");
    }
    let mut criteria = criteria;
    criteria.push(evidence);
    debug_assert_eq!(criteria.len(), CRITERIA.len());
    let gate_open = criteria.len() == CRITERIA.len()
        && criteria
            .iter()
            .zip(CRITERIA)
            .all(|(result, expected)| result.criterion == expected && result.passed);
    SubmissionGateCheckResult {
        gate_open,
        criteria,
    }
}

pub async fn submission_gate_check(
    api: &LoreApi,
    request: SubmissionGateCheckRequest,
) -> crate::error::Result<SubmissionGateCheckResult> {
    let adapter = ProductionLoreAdapter::new(api, "");
    let io = ProductionGovernanceIo::new(api, GovernanceRole::Witness);
    Ok(submission_gate_check_with_adapters(&adapter, &io, &request).await)
}

#[cfg(test)]
mod tests {
    use super::{
        dco_criterion, exact_history, history_criterion, live_criteria, not_superseded_criterion,
        raw_overflow_scope, WitnessRawFacts,
    };
    use crate::ops::governance::contract::{
        AffectedPath, AuthorResolutionObservation, DcoMetadataObservation, DcoObservation,
        EvaluationResult, FileIdentity, GovernanceCriterion, GovernanceObservations,
        GovernancePathAction, GovernanceRemediation, GovernanceRemediationCode,
        HistoryOverflowScope, LockQuery, LockStatusResponse, MetadataEntry, ResolvedAuthor,
        RevisionDiffObservation, RevisionInfo, StagedPathObservation, StatusSnapshot,
        SupersessionMarkerV1, SupersessionMetadataQueryObservation, WorktreeFileObservation,
        MAX_GOVERNANCE_HISTORY_REVISIONS,
    };

    fn pending_chain(count: usize) -> (Vec<RevisionInfo>, Vec<String>) {
        assert!(count > 0);
        let revisions: Vec<_> = (0..count)
            .map(|index| {
                if index == 0 {
                    "candidate".into()
                } else {
                    format!("pending-{index}")
                }
            })
            .collect();
        let graph = revisions
            .iter()
            .enumerate()
            .map(|(index, revision)| RevisionInfo {
                revision: revision.clone(),
                parents: vec![revisions
                    .get(index + 1)
                    .cloned()
                    .unwrap_or_else(|| "base".into())],
            })
            .collect();
        (graph, revisions)
    }

    fn raw(evaluation: &EvaluationResult) -> WitnessRawFacts {
        WitnessRawFacts::from_evaluation(evaluation)
    }

    fn ancestry_chain(count: usize) -> Vec<RevisionInfo> {
        assert!(count >= 2);
        let revisions: Vec<_> = (0..count)
            .map(|index| match index {
                0 => "candidate".into(),
                1 => "base".into(),
                _ => format!("ancestor-{}", index - 1),
            })
            .collect();
        revisions
            .iter()
            .enumerate()
            .map(|(index, revision)| RevisionInfo {
                revision: revision.clone(),
                parents: revisions.get(index + 1).cloned().into_iter().collect(),
            })
            .collect()
    }

    fn complete_evaluation(failure_codes: Vec<String>) -> EvaluationResult {
        let mut observations = GovernanceObservations::new("candidate", "base");
        observations.status = Some(StatusSnapshot {
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
                revision_hash: "1111111111111111111111111111111111111111111111111111111111111111"
                    .into(),
                revision_context: "22222222222222222222222222222222".into(),
                revision_size: 5,
                local_hash: "1111111111111111111111111111111111111111111111111111111111111111"
                    .into(),
                local_size: 5,
                filtered_revision_size: 5,
                flag_modified: false,
                flag_deleted: false,
                flag_added: false,
                flag_conflict: false,
            }],
            worktree_clean: true,
            scan_performed: true,
        });
        observations.base_revision_info = Some(RevisionInfo {
            revision: "base".into(),
            parents: Vec::new(),
        });
        observations.supersession_ancestry = vec![
            RevisionInfo {
                revision: "base".into(),
                parents: Vec::new(),
            },
            RevisionInfo {
                revision: "candidate".into(),
                parents: vec!["base".into()],
            },
        ];
        observations.supersession_ancestry_observed = true;
        observations.revision_graph = vec![RevisionInfo {
            revision: "candidate".into(),
            parents: vec!["base".into()],
        }];
        observations.first_parent_history = vec!["candidate".into()];
        observations.candidate_files = vec![FileIdentity::new(
            "asset.txt",
            "candidate",
            "1111111111111111111111111111111111111111111111111111111111111111",
            "22222222222222222222222222222222",
        )];
        observations.base_files = vec![FileIdentity::new(
            "asset.txt",
            "base",
            "3333333333333333333333333333333333333333333333333333333333333333",
            "22222222222222222222222222222222",
        )];
        observations.candidate_tree_observed = true;
        observations.current_files = observations.candidate_files.clone();
        observations.base_tree_observed = true;
        observations.upstream_revision_diff = vec![RevisionDiffObservation {
            path: "asset.txt".into(),
            action: GovernancePathAction::Modify,
            old_is_file: true,
            new_is_file: true,
            old_address: format!("{}-{}", "3".repeat(64), "2".repeat(32)),
            new_address: format!("{}-{}", "1".repeat(64), "2".repeat(32)),
        }];
        observations.revision_diff = vec![AffectedPath::modified("asset.txt")];
        observations.revision_diff_observed = true;
        observations.affected_paths = vec!["asset.txt".into()];
        observations.supersession_metadata_queries = vec![
            SupersessionMetadataQueryObservation {
                revision: "base".into(),
                metadata: vec![],
            },
            SupersessionMetadataQueryObservation {
                revision: "candidate".into(),
                metadata: vec![],
            },
        ];
        observations.supersession_metadata_observed = true;
        observations.dco_metadata = vec![DcoMetadataObservation {
            revision: "candidate".into(),
            messages: vec!["change\n\nSigned-off-by: Alice <alice@example.test>".into()],
            created_by: vec!["alice".into()],
            committed_by: vec![],
        }];
        observations.author_resolution = Some(AuthorResolutionObservation {
            requested: vec!["alice".into()],
            replies: vec![ResolvedAuthor::new("alice", "Alice")],
        });
        observations.dco = vec![DcoObservation {
            revision: "candidate".into(),
            message: "change\n\nSigned-off-by: Alice <alice@example.test>".into(),
            trailer: "Signed-off-by: Alice <alice@example.test>".into(),
            signer_name: "Alice".into(),
            signer_email: "alice@example.test".into(),
            created_by: "alice".into(),
            committed_by: None,
            resolved_authors: vec![ResolvedAuthor::new("alice", "Alice")],
        }];
        observations.lock_queries = vec![LockQuery::unlocked("asset.txt")];
        observations.lock_status = Some(LockStatusResponse::unlocked());
        EvaluationResult {
            open: false,
            pending_revisions: Vec::new(),
            affected_paths: Vec::new(),
            identities: Vec::new(),
            superseded_identities: Vec::new(),
            failure_codes,
            remediation: None,
            observations,
        }
    }

    #[test]
    fn shuffled_failure_codes_cannot_change_complete_raw_criteria() {
        let left = complete_evaluation(vec!["dco_invalid".into(), "worktree_dirty".into()]);
        let right = complete_evaluation(vec![
            "unknown_future_code".into(),
            "worktree_dirty".into(),
            "dco_invalid".into(),
        ]);
        let left_criteria = live_criteria(&super::WitnessRawFacts::from_evaluation(&left));
        let right_criteria = live_criteria(&super::WitnessRawFacts::from_evaluation(&right));
        assert_eq!(left_criteria, right_criteria);
        assert!(left_criteria.iter().all(|criterion| criterion.passed));
    }

    #[test]
    fn evaluator_summaries_are_inert_at_the_witness_decision_boundary() {
        let baseline = complete_evaluation(Vec::new());
        let expected = live_criteria(&super::WitnessRawFacts::from_evaluation(&baseline));
        assert!(expected.iter().all(|criterion| criterion.passed));

        let mut changed = baseline.clone();
        changed.open = true;
        changed.pending_revisions = vec!["forged-pending-summary".into()];
        changed.affected_paths = vec!["forged-top-level-summary.txt".into()];
        changed.identities = vec!["forged-identity-summary".into()];
        changed.superseded_identities = vec!["forged-superseded-summary".into()];
        changed.failure_codes = vec!["forged-failure-summary".into()];
        changed.remediation = Some(GovernanceRemediation {
            code: GovernanceRemediationCode::MigrateSupersessionIndex,
            ticket: Some("forged-ticket".into()),
        });
        changed.observations.affected_paths = vec!["forged-summary.txt".into()];
        changed.observations.dco[0].signer_name = "Mallory".into();
        changed.observations.supersession_markers.push(
            crate::ops::governance::contract::SupersessionObservation {
                revision: "candidate".into(),
                key: format!(
                    "{}{}:{}",
                    crate::ops::governance::contract::SUPERSESSION_MARKER_PREFIX,
                    "1".repeat(64),
                    "2".repeat(32)
                ),
                value: serde_json::to_string(
                    &crate::ops::governance::contract::SupersessionMarkerV1 {
                        version: "v1".into(),
                        identity: format!("{}:{}", "1".repeat(64), "2".repeat(32)),
                    },
                )
                .unwrap(),
                identity: format!("{}:{}", "1".repeat(64), "2".repeat(32)),
            },
        );
        changed.observations.supersession_metadata_observed = false;
        changed.observations.history_overflow_scope =
            Some(crate::ops::governance::contract::HistoryOverflowScope::PendingDco);

        assert_eq!(
            live_criteria(&super::WitnessRawFacts::from_evaluation(&changed)),
            expected,
            "witness criteria must derive only from retained raw facts, never evaluator summaries",
        );
        assert_eq!(
            super::WitnessEvidenceProjection::from_raw(&super::WitnessRawFacts::from_evaluation(
                &changed
            )),
            super::WitnessEvidenceProjection::from_raw(&super::WitnessRawFacts::from_evaluation(
                &baseline
            )),
            "evidence equality and stability must use the private raw projection, not summaries",
        );
    }

    #[test]
    fn witness_requires_exact_supersession_metadata_query_coverage() {
        let baseline = complete_evaluation(Vec::new());

        let mut missing = baseline.clone();
        missing.observations.supersession_metadata_queries.pop();
        let result = not_superseded_criterion(&WitnessRawFacts::from_evaluation(&missing));
        assert!(!result.passed);
        assert_eq!(
            result.failure_code.as_deref(),
            Some("not_superseded_dependency_failed")
        );

        let mut duplicate = baseline.clone();
        duplicate
            .observations
            .supersession_metadata_queries
            .push(duplicate.observations.supersession_metadata_queries[0].clone());
        let result = not_superseded_criterion(&WitnessRawFacts::from_evaluation(&duplicate));
        assert!(!result.passed);
        assert_eq!(
            result.failure_code.as_deref(),
            Some("not_superseded_failed")
        );

        let mut foreign = baseline;
        foreign.observations.supersession_metadata_queries[0].revision = "foreign".into();
        let result = not_superseded_criterion(&WitnessRawFacts::from_evaluation(&foreign));
        assert!(!result.passed);
        assert_eq!(
            result.failure_code.as_deref(),
            Some("not_superseded_failed")
        );
    }

    #[test]
    fn witness_rederives_supersession_from_an_ancestor_raw_query() {
        let mut evaluation = complete_evaluation(Vec::new());
        let identity = format!("{}:{}", "1".repeat(64), "2".repeat(32));
        evaluation.observations.supersession_metadata_queries[0]
            .metadata
            .push(MetadataEntry::new(
                format!("{}{}", super::SUPERSESSION_MARKER_PREFIX, identity),
                serde_json::to_string(&SupersessionMarkerV1 {
                    version: "v1".into(),
                    identity,
                })
                .unwrap(),
            ));
        let result = not_superseded_criterion(&WitnessRawFacts::from_evaluation(&evaluation));
        assert!(!result.passed);
        assert_eq!(
            result.failure_code.as_deref(),
            Some("not_superseded_failed")
        );
    }

    #[test]
    fn witness_rejects_dco_row_and_reply_multiplicity_without_normalizing() {
        let baseline = complete_evaluation(Vec::new());

        let mut foreign = baseline.clone();
        foreign.observations.dco_metadata[0].revision = "foreign".into();
        let result = dco_criterion(&WitnessRawFacts::from_evaluation(&foreign));
        assert!(!result.passed);
        assert_eq!(result.failure_code.as_deref(), Some("dco_invalid"));

        let mut duplicate_row = baseline.clone();
        duplicate_row
            .observations
            .dco_metadata
            .push(duplicate_row.observations.dco_metadata[0].clone());
        let result = dco_criterion(&WitnessRawFacts::from_evaluation(&duplicate_row));
        assert!(!result.passed);
        assert_eq!(
            result.failure_code.as_deref(),
            Some("dco_dependency_failed")
        );

        let mut duplicate_reply = baseline;
        let reply = duplicate_reply
            .observations
            .author_resolution
            .as_ref()
            .unwrap()
            .replies[0]
            .clone();
        duplicate_reply
            .observations
            .author_resolution
            .as_mut()
            .unwrap()
            .replies
            .push(reply);
        let result = dco_criterion(&WitnessRawFacts::from_evaluation(&duplicate_reply));
        assert!(!result.passed);
        assert_eq!(result.failure_code.as_deref(), Some("dco_invalid"));
    }

    #[test]
    fn multiple_raw_failures_are_criterion_local_regardless_of_code_order() {
        let mut left = complete_evaluation(vec!["worktree_dirty".into(), "dco_invalid".into()]);
        left.observations.dco_metadata[0].messages[0] = "malformed raw message".into();
        left.observations.lock_queries[0].owners = vec!["foreign".into()];
        left.observations.lock_queries[0].expected_count = 1;
        let mut right = left.clone();
        right.failure_codes.reverse();

        let left_criteria = live_criteria(&super::WitnessRawFacts::from_evaluation(&left));
        assert_eq!(
            left_criteria,
            live_criteria(&super::WitnessRawFacts::from_evaluation(&right))
        );
        for result in left_criteria {
            let expected = matches!(
                result.criterion,
                GovernanceCriterion::DcoValid | GovernanceCriterion::LocksClear
            );
            assert_eq!(!result.passed, expected, "{result:?}");
        }
    }

    #[test]
    fn witness_accepts_exact_1000_pending_nodes_and_derives_1001_remediation() {
        let mut evaluation = complete_evaluation(Vec::new());
        let (graph, history) = pending_chain(MAX_GOVERNANCE_HISTORY_REVISIONS);
        evaluation.observations.revision_graph = graph;
        evaluation.observations.first_parent_history = history;
        assert!(exact_history(&raw(&evaluation)).is_ok());

        let (graph, _) = pending_chain(MAX_GOVERNANCE_HISTORY_REVISIONS + 1);
        evaluation.observations.revision_graph = graph;
        evaluation.observations.first_parent_history.clear();
        evaluation.observations.supersession_ancestry.clear();
        evaluation.observations.supersession_ancestry_observed = false;
        evaluation.observations.history_overflow_scope = Some(HistoryOverflowScope::PendingDco);
        assert_eq!(
            raw_overflow_scope(&raw(&evaluation)).unwrap(),
            Some(HistoryOverflowScope::PendingDco)
        );
        let result = history_criterion(&raw(&evaluation));
        assert_eq!(
            result.failure_code.as_deref(),
            Some("history_depth_exceeded")
        );
        assert_eq!(
            result.remediation,
            Some(GovernanceRemediation {
                code: GovernanceRemediationCode::SplitSubmissionOrAdvanceTargetBase,
                ticket: None,
            })
        );
    }

    #[test]
    fn witness_ignores_lied_pending_scope_but_rejects_missing_and_malformed_sentinel() {
        let mut evaluation = complete_evaluation(Vec::new());
        let (graph, _) = pending_chain(MAX_GOVERNANCE_HISTORY_REVISIONS + 1);
        evaluation.observations.revision_graph = graph;
        evaluation.observations.first_parent_history.clear();
        evaluation.observations.supersession_ancestry.clear();
        evaluation.observations.supersession_ancestry_observed = false;
        evaluation.observations.history_overflow_scope =
            Some(HistoryOverflowScope::SupersessionAncestry);
        let lied = history_criterion(&raw(&evaluation));
        assert_eq!(lied.failure_code.as_deref(), Some("history_depth_exceeded"));
        assert_eq!(
            lied.remediation,
            Some(GovernanceRemediation {
                code: GovernanceRemediationCode::SplitSubmissionOrAdvanceTargetBase,
                ticket: None,
            })
        );

        evaluation.observations.history_overflow_scope = Some(HistoryOverflowScope::PendingDco);
        evaluation.observations.revision_graph.pop();
        let missing = history_criterion(&raw(&evaluation));
        assert_eq!(missing.failure_code.as_deref(), Some("history_incomplete"));
        assert!(missing.remediation.is_none());

        let (mut graph, _) = pending_chain(MAX_GOVERNANCE_HISTORY_REVISIONS + 1);
        graph.last_mut().unwrap().parents.clear();
        evaluation.observations.revision_graph = graph;
        let malformed = history_criterion(&raw(&evaluation));
        assert_eq!(
            malformed.failure_code.as_deref(),
            Some("history_incomplete")
        );
        assert!(malformed.remediation.is_none());

        let (mut cycle, _) = pending_chain(MAX_GOVERNANCE_HISTORY_REVISIONS + 1);
        cycle.last_mut().unwrap().parents = vec!["candidate".into()];
        evaluation.observations.revision_graph = cycle;
        evaluation.observations.history_overflow_scope =
            Some(HistoryOverflowScope::SupersessionAncestry);
        let cycle = history_criterion(&raw(&evaluation));
        assert_eq!(cycle.failure_code.as_deref(), Some("history_incomplete"));
        assert!(cycle.remediation.is_none());

        let (mut duplicate, _) = pending_chain(MAX_GOVERNANCE_HISTORY_REVISIONS + 1);
        duplicate.last_mut().unwrap().revision = "pending-1".into();
        evaluation.observations.revision_graph = duplicate;
        let duplicate = history_criterion(&raw(&evaluation));
        assert_eq!(
            duplicate.failure_code.as_deref(),
            Some("history_incomplete")
        );
        assert!(duplicate.remediation.is_none());
    }

    #[test]
    fn witness_accepts_exact_1000_ancestry_nodes_and_derives_1001_remediation() {
        let mut evaluation = complete_evaluation(Vec::new());
        let graph = ancestry_chain(MAX_GOVERNANCE_HISTORY_REVISIONS);
        evaluation.observations.base_revision_info = Some(graph[1].clone());
        evaluation.observations.supersession_ancestry = graph;
        evaluation.observations.supersession_ancestry_observed = true;
        assert!(history_criterion(&raw(&evaluation)).passed);

        let graph = ancestry_chain(MAX_GOVERNANCE_HISTORY_REVISIONS + 1);
        evaluation.observations.base_revision_info = Some(graph[1].clone());
        evaluation.observations.supersession_ancestry = graph;
        evaluation.observations.history_overflow_scope =
            Some(HistoryOverflowScope::SupersessionAncestry);
        assert_eq!(
            raw_overflow_scope(&raw(&evaluation)).unwrap(),
            Some(HistoryOverflowScope::SupersessionAncestry)
        );
        let result = history_criterion(&raw(&evaluation));
        assert_eq!(
            result.failure_code.as_deref(),
            Some("history_depth_exceeded")
        );
        assert_eq!(
            result.remediation,
            Some(GovernanceRemediation {
                code: GovernanceRemediationCode::MigrateSupersessionIndex,
                ticket: Some("SBAI-6010".into()),
            })
        );
    }

    #[test]
    fn witness_ignores_lied_ancestry_scope_but_rejects_missing_and_malformed_sentinel() {
        let mut evaluation = complete_evaluation(Vec::new());
        let graph = ancestry_chain(MAX_GOVERNANCE_HISTORY_REVISIONS + 1);
        evaluation.observations.base_revision_info = Some(graph[1].clone());
        evaluation.observations.supersession_ancestry = graph;
        evaluation.observations.supersession_ancestry_observed = true;
        evaluation.observations.history_overflow_scope = Some(HistoryOverflowScope::PendingDco);
        let lied = history_criterion(&raw(&evaluation));
        assert_eq!(lied.failure_code.as_deref(), Some("history_depth_exceeded"));
        assert_eq!(
            lied.remediation,
            Some(GovernanceRemediation {
                code: GovernanceRemediationCode::MigrateSupersessionIndex,
                ticket: Some("SBAI-6010".into()),
            })
        );

        evaluation.observations.history_overflow_scope =
            Some(HistoryOverflowScope::SupersessionAncestry);
        evaluation.observations.supersession_ancestry.pop();
        let missing = history_criterion(&raw(&evaluation));
        assert_eq!(missing.failure_code.as_deref(), Some("history_incomplete"));
        assert!(missing.remediation.is_none());

        let mut graph = ancestry_chain(MAX_GOVERNANCE_HISTORY_REVISIONS + 1);
        graph.last_mut().unwrap().parents = vec!["duplicate".into(), "duplicate".into()];
        evaluation.observations.supersession_ancestry = graph;
        let malformed = history_criterion(&raw(&evaluation));
        assert_eq!(
            malformed.failure_code.as_deref(),
            Some("history_incomplete")
        );
        assert!(malformed.remediation.is_none());

        let mut cycle = ancestry_chain(MAX_GOVERNANCE_HISTORY_REVISIONS + 1);
        cycle.last_mut().unwrap().parents = vec!["candidate".into()];
        evaluation.observations.supersession_ancestry = cycle;
        evaluation.observations.history_overflow_scope = Some(HistoryOverflowScope::PendingDco);
        let cycle = history_criterion(&raw(&evaluation));
        assert_eq!(cycle.failure_code.as_deref(), Some("history_incomplete"));
        assert!(cycle.remediation.is_none());

        let mut duplicate = ancestry_chain(MAX_GOVERNANCE_HISTORY_REVISIONS + 1);
        duplicate.last_mut().unwrap().revision = "ancestor-1".into();
        evaluation.observations.supersession_ancestry = duplicate;
        let duplicate = history_criterion(&raw(&evaluation));
        assert_eq!(
            duplicate.failure_code.as_deref(),
            Some("history_incomplete")
        );
        assert!(duplicate.remediation.is_none());
    }

    #[test]
    fn witness_rejects_ancestry_overflow_fact_that_disagrees_with_pending_graph() {
        let mut evaluation = complete_evaluation(Vec::new());
        let mut graph = ancestry_chain(MAX_GOVERNANCE_HISTORY_REVISIONS);
        graph[0].parents.push("side-root".into());
        graph.push(RevisionInfo {
            revision: "side-root".into(),
            parents: vec![],
        });
        evaluation.observations.base_revision_info = Some(graph[1].clone());
        evaluation.observations.supersession_ancestry = graph;
        evaluation.observations.supersession_ancestry_observed = true;
        evaluation.observations.history_overflow_scope =
            Some(HistoryOverflowScope::SupersessionAncestry);

        let result = history_criterion(&raw(&evaluation));
        assert_eq!(result.failure_code.as_deref(), Some("history_incomplete"));
        assert!(result.remediation.is_none());
    }
}
