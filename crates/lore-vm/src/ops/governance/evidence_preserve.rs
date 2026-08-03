//! Actor-produced, non-authoritative canonical evidence preservation.
//!
//! The stored bytes are a claim, never a gate verdict. The operation proves an
//! exact immutable put/get round trip and the sole metadata-pointer delta, then
//! re-evaluates the resulting staged subject. `storage_close` only releases the
//! handle and starts upstream's fire-and-forget flush; this module makes no
//! independent cross-process durability claim.

use super::contract::{
    AdapterError, CanonicalDcoMetadataObservationV1, CanonicalDcoObservationV1,
    CanonicalEvidenceSnapshotV1, CanonicalFileIdentityV1, CanonicalRevisionInfoV1,
    CanonicalRevisionRefV1, CanonicalStatusObservationV1,
    CanonicalSupersessionMetadataQueryObservationV1, CanonicalSupersessionObservationV1,
    CanonicalWorktreeFileObservationV1, EvaluationResult, EvidenceCloseEffectV1,
    EvidencePointerDeltaV1, EvidencePointerV1, EvidencePreserveOutcomeV1, EvidencePreserveRequest,
    EvidencePublicationAttemptV1, GovernanceRole, ImmutableGetItem, ImmutablePutItem, LockQuery,
    LockStatusResponse, MetadataEntry, MutationObservation, NoPublicationV1,
    PendingEvidencePreserveOutcomeV1, PredispatchEvidenceAttemptV1, ReadObservation,
    ResolvedAuthor, EVIDENCE_POINTER_KEY,
};
use super::evaluator::{
    evaluate, raw_event_collector, raw_stream_completed_exactly, take_raw_stream,
    GovernanceAdapter, ProductionLoreAdapter,
};
use super::GovernanceIo;
use crate::api::LoreApi;
use crate::error::{LoreError, Result};
use lore::interface::{LoreArray, LoreEvent, LoreEventCallback, LoreMetadataType, LoreString};
use lore::revision::LoreRevisionMetadataSetArgs;
use lore::storage::close::LoreStorageCloseArgs;
use lore::storage::get::{LoreStorageGetArgs, LoreStorageGetItem};
use lore::storage::handle::LoreStore;
use lore::storage::open::{LoreStorageOpenArgs, LoreStorageRemoteConfig};
use lore::storage::put::{LoreStoragePutArgs, LoreStoragePutItem};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const EVIDENCE_VERSION: &str = "v1";
const EVIDENCE_ITEM_ID: u64 = 5934;
const EVIDENCE_PARTITION: &str = "00000000000000000000000000005934";
static EVIDENCE_ATTEMPT_COUNTER: AtomicU64 = AtomicU64::new(1);

fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut message = bytes.to_vec();
    let bit_len = (message.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let mut hash = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in message.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(
                chunk[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("SHA-256 chunk word"),
            );
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = hash;
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(sum1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in hash.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    hash.iter().map(|word| format!("{word:08x}")).collect()
}

fn new_attempt_id(request: &EvidencePreserveRequest) -> String {
    let counter = EVIDENCE_ATTEMPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    sha256_hex(
        format!(
            "studiobrain-governance-evidence-v1\0{}\0{}\0{}\0{}\0{}",
            std::process::id(),
            counter,
            nanos,
            request.expected_staged_revision,
            request.target_base_revision
        )
        .as_bytes(),
    )
}

fn command_failed(context: &str, error: impl std::fmt::Display) -> LoreError {
    LoreError::CommandFailed(format!("governance evidence {context}: {error}"))
}

fn canonical_revision(revision: &str, staged: &str) -> CanonicalRevisionRefV1 {
    if revision == staged {
        CanonicalRevisionRefV1::StagedSubject
    } else {
        CanonicalRevisionRefV1::Exact(revision.to_string())
    }
}

fn canonical_status_revisions(revisions: &[String], staged: &str) -> Vec<CanonicalRevisionRefV1> {
    revisions
        .iter()
        .map(|revision| canonical_revision(revision, staged))
        .collect()
}

fn sort_resolved_authors(authors: &mut [ResolvedAuthor]) {
    authors.sort_by(|left, right| {
        (&left.identity, &left.display_name).cmp(&(&right.identity, &right.display_name))
    });
}

fn canonical_files(
    files: &[super::contract::FileIdentity],
    staged: &str,
) -> Vec<CanonicalFileIdentityV1> {
    let mut canonical: Vec<_> = files
        .iter()
        .map(|file| CanonicalFileIdentityV1 {
            path: file.path.clone(),
            revision: canonical_revision(&file.revision, staged),
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

/// Convert a successful evaluator run into the sole canonical v1 evidence
/// representation. Normalization is limited to the typed staged-subject token
/// and omission of that subject's self-referential evidence-pointer entry;
/// all other strings and scalar observations remain exact.
pub(crate) fn canonical_snapshot(
    evaluation: &EvaluationResult,
) -> Result<CanonicalEvidenceSnapshotV1> {
    if !evaluation.open {
        return Err(command_failed(
            "snapshot rejected closed evaluation",
            evaluation.failure_codes.join(","),
        ));
    }
    let observed = &evaluation.observations;
    let staged = observed.expected_staged_revision.as_str();
    let status = observed
        .status
        .as_ref()
        .ok_or_else(|| command_failed("snapshot missing status", "status_unavailable"))?;
    let base_revision_info = observed
        .base_revision_info
        .as_ref()
        .ok_or_else(|| command_failed("snapshot missing base ancestry", "history_incomplete"))?;
    if observed.target_base_revision.is_empty()
        || staged.is_empty()
        || base_revision_info.revision != observed.target_base_revision
        || !observed.dependency_observations.is_empty()
        || !observed.base_tree_observed
        || !observed.candidate_tree_observed
        || !observed.revision_diff_observed
        || !observed.supersession_metadata_observed
        || [
            &status.staged_revisions,
            &status.scanned_staged_revisions,
            &status.post_scan_staged_revisions,
        ]
        .into_iter()
        .any(|revisions| revisions.as_slice() != [staged])
    {
        return Err(command_failed(
            "snapshot contained incomplete raw observations",
            "canonicalization_failed",
        ));
    }

    let mut revision_graph: Vec<_> = observed
        .revision_graph
        .iter()
        .map(|info| CanonicalRevisionInfoV1 {
            revision: canonical_revision(&info.revision, staged),
            // Parent ordering is an exact first-parent input and must not be sorted.
            parents: info
                .parents
                .iter()
                .map(|parent| canonical_revision(parent, staged))
                .collect(),
        })
        .collect();
    revision_graph.sort_by(|left, right| left.revision.cmp(&right.revision));
    let mut supersession_ancestry: Vec<_> = observed
        .supersession_ancestry
        .iter()
        .map(|info| CanonicalRevisionInfoV1 {
            revision: canonical_revision(&info.revision, staged),
            parents: info
                .parents
                .iter()
                .map(|parent| canonical_revision(parent, staged))
                .collect(),
        })
        .collect();
    supersession_ancestry.sort_by(|left, right| left.revision.cmp(&right.revision));
    let base_revision_info = CanonicalRevisionInfoV1 {
        revision: canonical_revision(&base_revision_info.revision, staged),
        parents: base_revision_info
            .parents
            .iter()
            .map(|parent| canonical_revision(parent, staged))
            .collect(),
    };

    let mut supersession_markers: Vec<_> = observed
        .supersession_markers
        .iter()
        .map(|entry| CanonicalSupersessionObservationV1 {
            revision: canonical_revision(&entry.revision, staged),
            key: entry.key.clone(),
            value: entry.value.clone(),
            identity: entry.identity.clone(),
        })
        .collect();
    supersession_markers.sort_by(|left, right| {
        (&left.revision, &left.key, &left.value, &left.identity).cmp(&(
            &right.revision,
            &right.key,
            &right.value,
            &right.identity,
        ))
    });

    let mut ancestry_revisions: Vec<_> = observed
        .supersession_ancestry
        .iter()
        .map(|info| info.revision.as_str())
        .collect();
    ancestry_revisions.sort_unstable();
    let mut query_revisions: Vec<_> = observed
        .supersession_metadata_queries
        .iter()
        .map(|query| query.revision.as_str())
        .collect();
    query_revisions.sort_unstable();
    if query_revisions != ancestry_revisions {
        return Err(command_failed(
            "snapshot metadata-query coverage mismatch",
            "metadata_unavailable",
        ));
    }
    let mut supersession_metadata_queries: Vec<_> = observed
        .supersession_metadata_queries
        .iter()
        .map(|query| {
            // The evidence pointer is the one typed metadata delta produced by
            // this operation. It cannot be included in the bytes that its own
            // address identifies, so omit only that exact key from the staged
            // subject's raw-query projection. The actor validates the complete
            // source/result metadata delta before this comparison, and the
            // witness separately validates the exact pointer before accepting
            // the stored snapshot. Every other metadata entry remains bound.
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
                revision: canonical_revision(&query.revision, staged),
                metadata,
            }
        })
        .collect();
    supersession_metadata_queries.sort_by(|left, right| left.revision.cmp(&right.revision));

    let mut dco: Vec<_> = observed
        .dco
        .iter()
        .map(|entry| {
            let mut resolved_authors = entry.resolved_authors.clone();
            sort_resolved_authors(&mut resolved_authors);
            CanonicalDcoObservationV1 {
                revision: canonical_revision(&entry.revision, staged),
                message: entry.message.clone(),
                trailer: entry.trailer.clone(),
                signer_name: entry.signer_name.clone(),
                signer_email: entry.signer_email.clone(),
                created_by: entry.created_by.clone(),
                committed_by: entry.committed_by.clone(),
                resolved_authors,
            }
        })
        .collect();
    dco.sort_by(|left, right| left.revision.cmp(&right.revision));
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
                revision: canonical_revision(&entry.revision, staged),
                messages,
                created_by,
                committed_by,
            }
        })
        .collect();
    dco_metadata.sort_by(|left, right| left.revision.cmp(&right.revision));
    let mut author_resolution = observed
        .author_resolution
        .clone()
        .ok_or_else(|| command_failed("snapshot missing author resolution", "auth_unavailable"))?;
    author_resolution.requested.sort();
    sort_resolved_authors(&mut author_resolution.replies);

    let mut lock_queries: Vec<LockQuery> = observed.lock_queries.clone();
    for query in &mut lock_queries {
        query.ignored_paths.sort();
        query.owners.sort();
    }
    lock_queries.sort_by(|left, right| left.path.cmp(&right.path));
    let mut lock_status: LockStatusResponse = observed
        .lock_status
        .clone()
        .ok_or_else(|| command_failed("snapshot missing lock status", "locks_unavailable"))?;
    lock_status.ignored_paths.sort();
    lock_status
        .statuses
        .sort_by(|left, right| (&left.path, &left.owner).cmp(&(&right.path, &right.owner)));

    let mut affected_paths = observed.affected_paths.clone();
    affected_paths.sort();
    affected_paths.dedup();
    let mut dependency_observations = observed.dependency_observations.clone();
    dependency_observations.sort();
    dependency_observations.dedup();
    let mut revision_diff = observed.revision_diff.clone();
    revision_diff.sort_by(|left, right| {
        (&left.source_path, &left.target_path).cmp(&(&right.source_path, &right.target_path))
    });
    let mut worktree_files: Vec<_> = status
        .worktree_files
        .iter()
        .map(|file| CanonicalWorktreeFileObservationV1 {
            path: file.path.clone(),
            revision: canonical_revision(&file.revision, staged),
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

    Ok(CanonicalEvidenceSnapshotV1 {
        version: EVIDENCE_VERSION.into(),
        target_base_revision: observed.target_base_revision.clone(),
        status: CanonicalStatusObservationV1 {
            branch: status.branch.clone(),
            staged_revisions: canonical_status_revisions(&status.staged_revisions, staged),
            scanned_staged_revisions: canonical_status_revisions(
                &status.scanned_staged_revisions,
                staged,
            ),
            post_scan_staged_revisions: canonical_status_revisions(
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
        base_revision_info,
        supersession_ancestry,
        supersession_ancestry_observed: observed.supersession_ancestry_observed,
        revision_graph,
        // First-parent order is semantic and remains byte-exact.
        first_parent_history: observed
            .first_parent_history
            .iter()
            .map(|revision| canonical_revision(revision, staged))
            .collect(),
        base_files: canonical_files(&observed.base_files, staged),
        base_tree_observed: observed.base_tree_observed,
        candidate_files: canonical_files(&observed.candidate_files, staged),
        candidate_tree_observed: observed.candidate_tree_observed,
        current_files: canonical_files(&observed.current_files, staged),
        upstream_revision_diff: {
            let mut diff = observed.upstream_revision_diff.clone();
            diff.sort();
            diff
        },
        revision_diff,
        revision_diff_observed: observed.revision_diff_observed,
        affected_paths,
        supersession_markers,
        supersession_metadata_queries,
        supersession_metadata_observed: observed.supersession_metadata_observed,
        dco_metadata,
        author_resolution,
        dco,
        lock_queries,
        lock_status,
        dependency_observations,
    })
}

fn exact_metadata(mut metadata: Vec<MetadataEntry>) -> Vec<MetadataEntry> {
    metadata.sort_by(|left, right| {
        (&left.key, left.kind, &left.value).cmp(&(&right.key, right.kind, &right.value))
    });
    metadata
}

fn exact_address(address: &str) -> bool {
    let lowercase_hex = |byte: &u8| byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f');
    let bytes = address.as_bytes();
    bytes.len() == 97
        && bytes[64] == b'-'
        && bytes[..64].iter().all(lowercase_hex)
        && bytes[65..].iter().all(lowercase_hex)
}

fn one_put(items: &[ImmutablePutItem]) -> std::result::Result<String, String> {
    if items.len() != 1 {
        return Err(format!("put cardinality {}", items.len()));
    }
    let item = items.first().expect("length checked");
    if item.id != EVIDENCE_ITEM_ID || !item.ok || !exact_address(&item.address) {
        return Err("put item validation failed".into());
    }
    Ok(item.address.clone())
}

enum GetValidation {
    Exact,
    Malformed(String),
    BytesMismatch,
}

fn one_get(items: &[ImmutableGetItem], address: &str, expected: &[u8]) -> GetValidation {
    if items.len() != 1 {
        return GetValidation::Malformed(format!("get cardinality {}", items.len()));
    }
    let item = items.first().expect("length checked");
    if item.id != EVIDENCE_ITEM_ID
        || !item.ok
        || item.address != address
        || item.size != expected.len() as u64
    {
        return GetValidation::Malformed("get item validation failed".into());
    }
    if item.data != expected {
        return GetValidation::BytesMismatch;
    }
    GetValidation::Exact
}

fn observed_candidate_addresses(items: &[ImmutablePutItem]) -> Vec<String> {
    items.iter().map(|item| item.address.clone()).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CapturedStorageGetEvent {
    Header {
        id: u64,
        address: String,
        size: u64,
    },
    Data {
        id: u64,
        address: String,
        offset: u64,
        bytes: Vec<u8>,
    },
    ItemComplete {
        id: u64,
        address: String,
        ok: bool,
    },
    Error,
    Complete {
        status: i32,
    },
    End,
    Other,
}

fn raw_storage_get_collector() -> (LoreEventCallback, Arc<Mutex<Vec<CapturedStorageGetEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    let callback = Box::new(move |event: &LoreEvent| {
        let captured = match event {
            LoreEvent::StorageGetHeader(data) => CapturedStorageGetEvent::Header {
                id: data.id,
                address: format!("{}", data.address),
                size: data.size_content,
            },
            LoreEvent::StorageGetData(data) => CapturedStorageGetEvent::Data {
                id: data.id,
                address: format!("{}", data.address),
                offset: data.offset,
                // SAFETY: the upstream buffer is callback-scoped. Copy it
                // before returning from this invocation.
                bytes: unsafe { data.bytes.as_slice() }.to_vec(),
            },
            LoreEvent::StorageGetItemComplete(data) => CapturedStorageGetEvent::ItemComplete {
                id: data.id,
                address: format!("{}", data.address),
                ok: data.error_code as i32 == 0,
            },
            LoreEvent::Error(_) => CapturedStorageGetEvent::Error,
            LoreEvent::Complete(data) => CapturedStorageGetEvent::Complete {
                status: data.status,
            },
            LoreEvent::End(_) => CapturedStorageGetEvent::End,
            _ => CapturedStorageGetEvent::Other,
        };
        observed
            .lock()
            .expect("raw storage get collector mutex poisoned")
            .push(captured);
    });
    (Some(callback), events)
}

fn finish_captured_storage_get(
    events: Vec<CapturedStorageGetEvent>,
    returned: i32,
) -> std::result::Result<Vec<ImmutableGetItem>, AdapterError> {
    let complete_positions: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            CapturedStorageGetEvent::Complete { status } => Some((index, *status)),
            _ => None,
        })
        .collect();
    let end_positions: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| matches!(event, CapturedStorageGetEvent::End).then_some(index))
        .collect();
    if returned != 0
        || complete_positions.as_slice() != [(events.len().saturating_sub(2), 0)]
        || end_positions.as_slice() != [events.len().saturating_sub(1)]
        || events
            .iter()
            .any(|event| matches!(event, CapturedStorageGetEvent::Error))
    {
        return Err(AdapterError::new(
            "raw storage get stream failed terminal validation",
        ));
    }

    let headers: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            CapturedStorageGetEvent::Header { id, address, size } => {
                Some((index, *id, address.as_str(), *size))
            }
            _ => None,
        })
        .collect();
    let completions: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            CapturedStorageGetEvent::ItemComplete { id, address, ok } => {
                Some((index, *id, address.as_str(), *ok))
            }
            _ => None,
        })
        .collect();
    let (header_index, id, address, size) = match headers.as_slice() {
        [header] => *header,
        _ => return Err(AdapterError::new("raw storage get header cardinality")),
    };
    let (completion_index, completion_id, completion_address, ok) = match completions.as_slice() {
        [completion] => *completion,
        _ => return Err(AdapterError::new("raw storage get completion cardinality")),
    };
    if !ok
        || id != completion_id
        || address != completion_address
        || header_index >= completion_index
        || completion_index >= complete_positions[0].0
    {
        return Err(AdapterError::new(
            "raw storage get item sequence was invalid",
        ));
    }

    let mut data = Vec::new();
    let mut next_offset = 0u64;
    for (index, event) in events.iter().enumerate() {
        let CapturedStorageGetEvent::Data {
            id: data_id,
            address: data_address,
            offset,
            bytes,
        } = event
        else {
            continue;
        };
        if index <= header_index
            || index >= completion_index
            || *data_id != id
            || data_address != address
            || *offset != next_offset
        {
            return Err(AdapterError::new(
                "raw storage get data sequence was invalid",
            ));
        }
        next_offset = next_offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| AdapterError::new("raw storage get size overflow"))?;
        data.extend_from_slice(bytes);
    }
    if next_offset != size || data.len() as u64 != size {
        return Err(AdapterError::new("raw storage get payload was incomplete"));
    }
    Ok(vec![ImmutableGetItem {
        id,
        address: address.into(),
        size,
        data,
        ok,
    }])
}

fn exact_staged_subject(status: &super::contract::StatusSnapshot) -> Result<String> {
    let Some(staged) = status.staged_revisions.first() else {
        return Err(command_failed("metadata result", "missing staged revision"));
    };
    if staged.is_empty()
        || status.staged_revisions.len() != 1
        || status.scanned_staged_revisions.len() != 1
        || status.scanned_staged_revisions.first() != Some(staged)
        || status.post_scan_staged_revisions.len() != 1
        || status.post_scan_staged_revisions.first() != Some(staged)
    {
        return Err(command_failed(
            "metadata result",
            "ambiguous staged revision",
        ));
    }
    Ok(staged.clone())
}

struct PreserveOnOpenInputs<'a> {
    request: &'a EvidencePreserveRequest,
    snapshot: CanonicalEvidenceSnapshotV1,
    bytes: Vec<u8>,
    source_metadata: Vec<MetadataEntry>,
}

async fn preserve_on_open<A: GovernanceAdapter, I: GovernanceIo>(
    adapter: &A,
    io: &I,
    handle: u64,
    attempt: EvidencePublicationAttemptV1<NoPublicationV1>,
    inputs: PreserveOnOpenInputs<'_>,
) -> PendingEvidencePreserveOutcomeV1 {
    let PreserveOnOpenInputs {
        request,
        snapshot,
        bytes,
        source_metadata,
    } = inputs;
    let source = request.expected_staged_revision.as_str();

    if io.role() != GovernanceRole::Actor {
        return attempt.actor_role_before_put("role changed before immutable put");
    }
    let (attempt, address) = match io.storage_put(handle, &bytes).await {
        MutationObservation::NotDispatched { code } => {
            return attempt.storage_put_not_dispatched(code)
        }
        MutationObservation::OutcomeUnknown { code, observed } => {
            return attempt
                .put_observed(observed_candidate_addresses(&observed))
                .storage_put_outcome_unknown(code)
        }
        MutationObservation::Completed(items) => {
            let candidate_addresses = observed_candidate_addresses(&items);
            let attempt = attempt.put_observed(candidate_addresses);
            match one_put(&items) {
                Ok(address) => (attempt.blob_published(address.clone()), address),
                Err(code) => return attempt.storage_put_response_malformed(code),
            }
        }
    };

    // Re-read every live dependency after storage I/O and before the mutation.
    let preattach = evaluate(adapter, request).await;
    match canonical_snapshot(&preattach) {
        Ok(observed) if observed == snapshot => {}
        Ok(_) => {
            return attempt
                .preattach_evaluation_incomplete("live observations changed before pointer attach")
        }
        Err(error) => return attempt.preattach_evaluation_incomplete(error.to_string()),
    }
    let live_source_metadata = match adapter.revision_metadata(source).await {
        Ok(metadata) => exact_metadata(metadata),
        Err(error) => return attempt.preattach_evaluation_incomplete(error.message),
    };
    if live_source_metadata != source_metadata {
        return attempt
            .preattach_evaluation_incomplete("source metadata changed before pointer attach");
    }
    if io.role() != GovernanceRole::Actor {
        return attempt.actor_role_before_attach("role changed before pointer attach");
    }

    let pointer = EvidencePointerV1 {
        version: EVIDENCE_VERSION.into(),
        address: address.clone(),
    };
    let pointer_json = match serde_json::to_string(&pointer) {
        Ok(pointer_json) => pointer_json,
        Err(error) => return attempt.pointer_serialization_incomplete(error.to_string()),
    };
    let attempt = match io
        .revision_metadata_set(EVIDENCE_POINTER_KEY, &pointer_json)
        .await
    {
        MutationObservation::NotDispatched { code } => {
            return attempt.pointer_attach_not_dispatched(code)
        }
        MutationObservation::OutcomeUnknown { code, observed: () } => {
            return attempt.pointer_attach_outcome_unknown(code)
        }
        MutationObservation::Completed(()) => attempt.pointer_attach_acknowledged(pointer.clone()),
    };

    // The pointer has been acknowledged. Bind it immediately to one exact
    // immutable reread before observing a result subject or metadata delta.
    let attempt = match io.storage_get(handle, &address).await {
        ReadObservation::NotDispatched { code } | ReadObservation::Unavailable { code } => {
            return attempt.postattach_get_unavailable(code)
        }
        ReadObservation::Completed(items) => match one_get(&items, &address, &bytes) {
            GetValidation::Exact => attempt.blob_readback_verified(),
            GetValidation::Malformed(code) => return attempt.postattach_get_malformed(code),
            GetValidation::BytesMismatch => {
                return attempt.postattach_bytes_mismatch(
                    "same-size immutable bytes differed from the canonical snapshot",
                )
            }
        },
    };

    let result_status = match adapter.status().await {
        Ok(status) => status,
        Err(error) => return attempt.result_status_unavailable(error.message),
    };
    let result_revision = match exact_staged_subject(&result_status) {
        Ok(revision) if revision != source => revision,
        Ok(_) => {
            return attempt
                .result_status_invalid("pointer attach did not produce a new staged revision")
        }
        Err(error) => return attempt.result_status_invalid(error.to_string()),
    };
    let attempt = attempt.result_subject_observed(result_revision.clone());

    let result_metadata = match adapter.revision_metadata(&result_revision).await {
        Ok(metadata) => exact_metadata(metadata),
        Err(error) => return attempt.result_metadata_unavailable(error.message),
    };
    let pointer_entries: Vec<_> = result_metadata
        .iter()
        .filter(|entry| entry.key == EVIDENCE_POINTER_KEY)
        .collect();
    if pointer_entries.len() != 1
        || pointer_entries[0].string_value() != Some(pointer_json.as_str())
    {
        return attempt
            .pointer_delta_invalid("result did not contain exactly the attached string pointer");
    }
    let mut expected_metadata = source_metadata.clone();
    expected_metadata.push(MetadataEntry::new(EVIDENCE_POINTER_KEY, &pointer_json));
    if exact_metadata(expected_metadata) != result_metadata {
        return attempt.pointer_delta_invalid("metadata changed outside the sole evidence key");
    }
    let Some(pointer_value) = pointer_entries[0].string_value() else {
        return attempt.pointer_schema_invalid("pointer value was not a string");
    };
    let parsed_pointer: EvidencePointerV1 = match serde_json::from_str(pointer_value) {
        Ok(pointer) => pointer,
        Err(error) => return attempt.pointer_schema_invalid(error.to_string()),
    };
    if parsed_pointer != pointer {
        return attempt.pointer_schema_invalid("pointer schema round-trip mismatch");
    }

    let delta = EvidencePointerDeltaV1 {
        version: EVIDENCE_VERSION.into(),
        key: EVIDENCE_POINTER_KEY.into(),
        source_staged_revision: source.into(),
        result_staged_revision: result_revision.clone(),
        pointer: pointer.clone(),
    };
    let delta_bytes = match serde_json::to_vec(&delta) {
        Ok(bytes) => bytes,
        Err(error) => return attempt.pointer_schema_invalid(error.to_string()),
    };
    match serde_json::from_slice::<EvidencePointerDeltaV1>(&delta_bytes) {
        Ok(roundtrip) if roundtrip == delta => {}
        Ok(_) => return attempt.pointer_schema_invalid("pointer delta round-trip mismatch"),
        Err(error) => return attempt.pointer_schema_invalid(error.to_string()),
    }
    let attempt = attempt.pointer_delta_observed();

    let result_request = EvidencePreserveRequest {
        expected_staged_revision: result_revision.clone(),
        target_base_revision: request.target_base_revision.clone(),
    };
    let postattach = evaluate(adapter, &result_request).await;
    match canonical_snapshot(&postattach) {
        Ok(observed) if observed == snapshot => attempt.postattach_equivalent().ready_to_close(),
        Ok(_) => {
            attempt.postattach_drift("live observations changed outside the typed pointer delta")
        }
        Err(error) => attempt.postattach_evaluation_incomplete(error.to_string()),
    }
}

pub async fn evidence_preserve_with_adapters<A: GovernanceAdapter, I: GovernanceIo>(
    adapter: &A,
    io: &I,
    request: &EvidencePreserveRequest,
) -> Result<EvidencePreserveOutcomeV1> {
    let validated = request
        .validated()
        .map_err(|error| LoreError::Parse(error.into()))?;
    let predispatch = PredispatchEvidenceAttemptV1::new(new_attempt_id(request), validated);
    // This check precedes evaluation and every storage/repository side effect.
    if io.role() != GovernanceRole::Actor {
        return predispatch
            .actor_role_rejected("only an actor may preserve a non-authoritative claim")
            .map_err(LoreError::Parse);
    }
    let initial = evaluate(adapter, request).await;
    if !initial.open {
        let code = initial.failure_codes.join(",");
        return if complete_policy_rejection(&initial) {
            predispatch
                .initial_governance_rejected(code)
                .map_err(LoreError::Parse)
        } else {
            predispatch
                .initial_evaluation_incomplete(code)
                .map_err(LoreError::Parse)
        };
    }
    let snapshot = match canonical_snapshot(&initial) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return predispatch
                .initial_evaluation_incomplete(error.to_string())
                .map_err(LoreError::Parse)
        }
    };
    let source_metadata = match adapter
        .revision_metadata(&request.expected_staged_revision)
        .await
    {
        Ok(metadata) => exact_metadata(metadata),
        Err(error) => {
            return predispatch
                .source_metadata_incomplete(error.message)
                .map_err(LoreError::Parse)
        }
    };
    if source_metadata
        .iter()
        .any(|entry| entry.key == EVIDENCE_POINTER_KEY)
    {
        return predispatch
            .pointer_already_present_rejected("evidence pointer already present")
            .map_err(LoreError::Parse);
    }
    let bytes = match serde_json::to_vec(&snapshot) {
        Ok(bytes) => bytes,
        Err(error) => {
            return predispatch
                .snapshot_serialization_incomplete(error.to_string())
                .map_err(LoreError::Parse)
        }
    };
    let attempt = predispatch
        .with_snapshot_sha256(sha256_hex(&bytes))
        .enter_effect_boundary();
    let handle = match io.storage_open().await {
        MutationObservation::NotDispatched { code } => {
            return attempt
                .storage_open_not_dispatched(code)
                .map_err(LoreError::Parse)
        }
        MutationObservation::OutcomeUnknown { code, observed: _ } => {
            return attempt
                .storage_open_outcome_unknown(code)
                .map_err(LoreError::Parse)
        }
        MutationObservation::Completed(handles) => match handles.as_slice() {
            [handle] if *handle != 0 => *handle,
            _ => {
                return attempt
                    .storage_open_outcome_unknown("storage open returned an unusable handle set")
                    .map_err(LoreError::Parse)
            }
        },
    };
    let operation = preserve_on_open(
        adapter,
        io,
        handle,
        attempt,
        PreserveOnOpenInputs {
            request,
            snapshot,
            bytes,
            source_metadata,
        },
    )
    .await;
    let close = match io.storage_close(handle).await {
        MutationObservation::Completed(()) => EvidenceCloseEffectV1::Closed,
        MutationObservation::NotDispatched { code } => {
            EvidenceCloseEffectV1::NotDispatched { code }
        }
        MutationObservation::OutcomeUnknown { code, observed: () } => {
            EvidenceCloseEffectV1::OutcomeUnknown { code }
        }
    };
    operation.finalize(close).map_err(LoreError::Parse)
}

fn complete_policy_rejection(evaluation: &EvaluationResult) -> bool {
    if evaluation.open || evaluation.failure_codes.len() != 1 {
        return false;
    }
    match evaluation.failure_codes[0].as_str() {
        "dco_invalid"
        | "not_superseded_failed"
        | "worktree_dirty"
        | "empty_submission"
        | "locks_clear_failed" => true,
        "exact_subject_failed" => evaluation.observations.status.is_some(),
        _ => false,
    }
}

/// Production I/O binding. Its role is operation-scoped, not an authorization
/// assertion; actor claims remain non-authoritative until a witness re-derives
/// every live fact.
pub struct ProductionGovernanceIo<'a> {
    api: &'a LoreApi,
    role: GovernanceRole,
}

impl<'a> ProductionGovernanceIo<'a> {
    pub fn new(api: &'a LoreApi, role: GovernanceRole) -> Self {
        Self { api, role }
    }
}

#[async_trait::async_trait]
impl GovernanceIo for ProductionGovernanceIo<'_> {
    fn role(&self) -> GovernanceRole {
        self.role
    }

    async fn revision_metadata_set(&self, key: &str, value: &str) -> MutationObservation<()> {
        let (callback, stream) = raw_event_collector();
        let returned = lore::revision::metadata_set(
            self.api.globals().build(),
            LoreRevisionMetadataSetArgs {
                keys: LoreArray::from_vec(vec![LoreString::from_str(key)]),
                values: LoreArray::from_vec(vec![LoreString::from_str(value)]),
                formats: LoreArray::from_vec(vec![LoreMetadataType::String]),
            },
            callback,
        )
        .await;
        let stream = take_raw_stream(stream);
        if raw_stream_completed_exactly(&stream, returned) {
            MutationObservation::Completed(())
        } else {
            MutationObservation::OutcomeUnknown {
                code: "raw metadata-set terminal outcome was unknown".into(),
                observed: (),
            }
        }
    }

    async fn storage_open(&self) -> MutationObservation<Vec<u64>> {
        let in_memory = self.api.global().in_memory;
        let repository_path = if in_memory {
            String::new()
        } else {
            self.api
                .global()
                .repository_path
                .to_string_lossy()
                .into_owned()
        };
        let (callback, stream) = raw_event_collector();
        let returned = lore::storage::open::open(
            self.api.globals().build(),
            LoreStorageOpenArgs {
                repository_path: LoreString::from_str(&repository_path),
                in_memory: u8::from(in_memory),
                remote_config: LoreStorageRemoteConfig {
                    remote_url: LoreString::default(),
                },
                has_remote_config: 0,
                cache_target_bytes: 0,
                cache_target_fragments: 0,
            },
            callback,
        )
        .await;
        let stream = take_raw_stream(stream);
        let handles: Vec<_> = stream
            .events
            .iter()
            .filter_map(|event| match event {
                LoreEvent::StorageOpened(opened) => Some(opened.handle_id),
                _ => None,
            })
            .collect();
        if raw_stream_completed_exactly(&stream, returned) {
            MutationObservation::Completed(handles)
        } else {
            MutationObservation::OutcomeUnknown {
                code: "raw storage-open terminal outcome was unknown".into(),
                observed: handles,
            }
        }
    }

    async fn storage_put(
        &self,
        handle: u64,
        bytes: &[u8],
    ) -> MutationObservation<Vec<ImmutablePutItem>> {
        let partition =
            match serde_json::from_value(serde_json::Value::String(EVIDENCE_PARTITION.into())) {
                Ok(partition) => partition,
                Err(error) => {
                    return MutationObservation::NotDispatched {
                        code: format!("evidence partition: {error}"),
                    }
                }
            };
        let context = match serde_json::from_value(serde_json::Value::String("0".repeat(32))) {
            Ok(context) => context,
            Err(error) => {
                return MutationObservation::NotDispatched {
                    code: format!("evidence context: {error}"),
                }
            }
        };
        let mut item = LoreStoragePutItem {
            id: EVIDENCE_ITEM_ID,
            partition,
            context,
            // SAFETY: LoreBytes is a pointer/length POD. The borrowed `bytes`
            // slice remains alive until the awaited put call returns.
            data: unsafe { std::mem::zeroed() },
            remote_write: 0,
            local_cache: 1,
            fixed_size_chunk: 0,
        };
        item.data.ptr = bytes.as_ptr().cast();
        item.data.len = bytes.len();
        let (callback, stream) = raw_event_collector();
        let returned = lore::storage::put::put(
            self.api.globals().build(),
            LoreStoragePutArgs {
                handle: LoreStore { handle_id: handle },
                items: LoreArray::from_vec(vec![item]),
            },
            callback,
        )
        .await;
        let stream = take_raw_stream(stream);
        let observed: Vec<_> = stream
            .events
            .iter()
            .filter_map(|event| match event {
                LoreEvent::StoragePutItemComplete(item) => {
                    let ok = item.error_code as i32 == 0;
                    Some(ImmutablePutItem {
                        id: item.id,
                        address: format!("{}", item.address),
                        ok,
                    })
                }
                _ => None,
            })
            .collect();
        if raw_stream_completed_exactly(&stream, returned) {
            MutationObservation::Completed(observed)
        } else {
            MutationObservation::OutcomeUnknown {
                code: "raw storage-put terminal outcome was unknown".into(),
                observed,
            }
        }
    }

    async fn storage_get(
        &self,
        handle: u64,
        address: &str,
    ) -> ReadObservation<Vec<ImmutableGetItem>> {
        let item: LoreStorageGetItem = match serde_json::from_value(serde_json::json!({
            "id": EVIDENCE_ITEM_ID,
            "partition": EVIDENCE_PARTITION,
            "address": address,
            "streaming": 0,
            "local_cache": 1,
        })) {
            Ok(item) => item,
            Err(error) => {
                return ReadObservation::NotDispatched {
                    code: format!("evidence get item: {error}"),
                }
            }
        };
        let (callback, events) = raw_storage_get_collector();
        let returned = lore::storage::get::get(
            self.api.globals().build(),
            LoreStorageGetArgs {
                handle: LoreStore { handle_id: handle },
                items: LoreArray::from_vec(vec![item]),
            },
            callback,
        )
        .await;
        let events = std::mem::take(
            &mut *events
                .lock()
                .expect("raw storage get collector mutex poisoned"),
        );
        match finish_captured_storage_get(events, returned) {
            Ok(items) => ReadObservation::Completed(items),
            Err(error) => ReadObservation::Unavailable {
                code: error.message,
            },
        }
    }

    async fn storage_close(&self, handle: u64) -> MutationObservation<()> {
        let (callback, stream) = raw_event_collector();
        let returned = lore::storage::close::close(
            self.api.globals().build(),
            LoreStorageCloseArgs {
                handle: LoreStore { handle_id: handle },
            },
            callback,
        )
        .await;
        let stream = take_raw_stream(stream);
        if raw_stream_completed_exactly(&stream, returned) {
            MutationObservation::Completed(())
        } else {
            MutationObservation::OutcomeUnknown {
                code: "raw storage-close terminal outcome was unknown".into(),
                observed: (),
            }
        }
    }
}

pub async fn evidence_preserve(
    api: &LoreApi,
    request: EvidencePreserveRequest,
) -> Result<EvidencePreserveOutcomeV1> {
    let adapter = ProductionLoreAdapter::new(api, "");
    let io = ProductionGovernanceIo::new(api, GovernanceRole::Actor);
    evidence_preserve_with_adapters(&adapter, &io, &request).await
}

#[cfg(test)]
mod tests {
    use super::{finish_captured_storage_get, CapturedStorageGetEvent};

    const ADDRESS: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-00000000000000000000000000000000";

    fn exact_events() -> Vec<CapturedStorageGetEvent> {
        vec![
            CapturedStorageGetEvent::Header {
                id: 5934,
                address: ADDRESS.into(),
                size: 3,
            },
            CapturedStorageGetEvent::Data {
                id: 5934,
                address: ADDRESS.into(),
                offset: 0,
                bytes: b"raw".to_vec(),
            },
            CapturedStorageGetEvent::ItemComplete {
                id: 5934,
                address: ADDRESS.into(),
                ok: true,
            },
            CapturedStorageGetEvent::Complete { status: 0 },
            CapturedStorageGetEvent::End,
        ]
    }

    #[test]
    fn raw_storage_get_retains_duplicate_item_events_and_exact_terminal_shape() {
        let exact = finish_captured_storage_get(exact_events(), 0).expect("exact raw stream");
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0].data, b"raw");

        let mut duplicate_header = exact_events();
        duplicate_header.insert(1, duplicate_header[0].clone());
        assert!(finish_captured_storage_get(duplicate_header, 0).is_err());

        let mut duplicate_item_complete = exact_events();
        duplicate_item_complete.insert(3, duplicate_item_complete[2].clone());
        assert!(finish_captured_storage_get(duplicate_item_complete, 0).is_err());

        let mut missing_end = exact_events();
        missing_end.pop();
        assert!(finish_captured_storage_get(missing_end, 0).is_err());

        let mut duplicate_end = exact_events();
        duplicate_end.push(CapturedStorageGetEvent::End);
        assert!(finish_captured_storage_get(duplicate_end, 0).is_err());

        let mut post_end = exact_events();
        post_end.push(CapturedStorageGetEvent::Complete { status: 0 });
        assert!(finish_captured_storage_get(post_end, 0).is_err());
    }
}
