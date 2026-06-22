//! Tauri command layer. Each command is a thin wrapper that builds a backend fo
//! the currently-open working directory and forwards to `lore-vm`. No business
//! logic lives here — that's the whole point of the lore-vm seam.

use lore_vm::{default_backend, Branch, LoreApi, LoreError, RepoStatus, Revision};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, State};

use crate::operations::SubscriptionId;

/// Storage session opened by the onboarding "validate connectivity" flow.
///
/// The frontend speaks in terms of opaque string keys (`storage_put(key, data)`
/// / `storage_get(key)`), but the lore storage layer is content-addressed: a
/// `put` returns a content address that a later `get`/`obliterate` must supply.
/// This session holds the open store handle plus the `key -> (partition,
/// address)` mapping that bridges the two models for the duration of the wizard.
#[derive(Default)]
pub struct StorageSession {
    /// Handle id returned by the most recent `storage_open`, if any.
    pub handle: Option<u64>,
    /// Map from frontend key to the `(partition, address)` produced by `put`.
    pub keys: HashMap<String, (String, String)>,
}

/// The only mutable app state: which working tree we're looking at,
/// notification subscription tracking, and the onboarding storage session.
pub struct AppState {
    pub working_dir: Mutex<PathBuf>,
    /// Monotonically increasing counter for subscription IDs.
    pub(crate) subscription_counter: AtomicU64,
    /// Currently active subscription IDs.
    pub(crate) subscriptions: Mutex<HashSet<SubscriptionId>>,
    /// Storage session for the server-setup onboarding flow.
    pub(crate) storage_session: Mutex<StorageSession>,
    /// The `loreserver` process the "Host a server" flow launched, if any.
    pub(crate) hosted_server: Mutex<Option<crate::server_host::HostedServer>>,
}

impl AppState {
    pub(crate) fn dir(&self) -> PathBuf {
        self.working_dir.lock().unwrap().clone()
    }

    /// Allocate a new subscription ID and register it.
    pub(crate) fn next_subscription_id(&self) -> SubscriptionId {
        let id = self.subscription_counter.fetch_add(1, Ordering::SeqCst) + 1;
        self.subscriptions.lock().unwrap().insert(id);
        id
    }

    /// Remove a subscription. Returns true if it existed, false if it was
    /// already gone (idempotent).
    pub(crate) fn remove_subscription(&self, id: SubscriptionId) -> bool {
        self.subscriptions.lock().unwrap().remove(&id)
    }
}

/// Point the app at a different working tree (e.g. after a folder picker).
#[tauri::command]
pub fn open_repository(state: State<'_, AppState>, path: String) -> Result<(), LoreError> {
    *state.working_dir.lock().unwrap() = PathBuf::from(path);
    Ok(())
}

#[tauri::command]
pub fn current_repository(state: State<'_, AppState>) -> String {
    state.dir().to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn status(state: State<'_, AppState>) -> Result<RepoStatus, LoreError> {
    default_backend(state.dir()).status().await
}

#[tauri::command]
pub async fn log(state: State<'_, AppState>, limit: usize) -> Result<Vec<Revision>, LoreError> {
    default_backend(state.dir()).log(limit).await
}

#[tauri::command]
pub async fn branches(state: State<'_, AppState>) -> Result<Vec<Branch>, LoreError> {
    default_backend(state.dir()).branches().await
}

#[tauri::command]
pub async fn stage(state: State<'_, AppState>, paths: Vec<String>) -> Result<(), LoreError> {
    default_backend(state.dir()).stage(&paths).await
}

#[tauri::command]
pub async fn unstage(state: State<'_, AppState>, paths: Vec<String>) -> Result<(), LoreError> {
    default_backend(state.dir()).unstage(&paths).await
}

#[tauri::command]
pub async fn commit(state: State<'_, AppState>, message: String) -> Result<String, LoreError> {
    default_backend(state.dir()).commit(&message).await
}

#[tauri::command]
pub async fn create_branch(state: State<'_, AppState>, name: String) -> Result<(), LoreError> {
    default_backend(state.dir()).create_branch(&name).await
}

#[tauri::command]
pub async fn switch_branch(state: State<'_, AppState>, name: String) -> Result<(), LoreError> {
    default_backend(state.dir()).switch_branch(&name).await
}

#[tauri::command]
pub async fn merge_branch(state: State<'_, AppState>, name: String) -> Result<(), LoreError> {
    default_backend(state.dir()).merge_branch(&name).await
}

#[tauri::command]
pub async fn push(state: State<'_, AppState>) -> Result<(), LoreError> {
    default_backend(state.dir()).push().await
}

#[tauri::command]
pub async fn sync(state: State<'_, AppState>) -> Result<(), LoreError> {
    default_backend(state.dir()).sync().await
}

#[tauri::command]
pub async fn create_repository(
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> Result<String, LoreError> {
    let p = PathBuf::from(&path);
    let id = default_backend(state.dir())
        .create_repository(p.clone(), &name)
        .await?;
    *state.working_dir.lock().unwrap() = p;
    Ok(id)
}

#[tauri::command]
pub async fn clone(state: State<'_, AppState>, url: String, dest: String) -> Result<(), LoreError> {
    let d = PathBuf::from(&dest);
    default_backend(state.dir()).clone(&url, d.clone()).await?;
    *state.working_dir.lock().unwrap() = d;
    Ok(())
}

// --- branch info ---

use lore_vm::ops::branch::info::{info as op_branch_info, BranchInfoArgs, BranchInfoResult};

#[tauri::command]
pub async fn branch_info(
    state: State<'_, AppState>,
    branch: String,
) -> Result<BranchInfoResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_info(&api, BranchInfoArgs { branch }).await
}

// --- branch protect ---

use lore_vm::ops::branch::protect::{
    protect as op_branch_protect, BranchProtectArgs, BranchProtectResult,
};

#[tauri::command]
pub async fn branch_protect(
    state: State<'_, AppState>,
    branch: String,
) -> Result<BranchProtectResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_protect(&api, BranchProtectArgs { branch }).await
}

// --- branch unprotect ---

use lore_vm::ops::branch::unprotect::{
    unprotect as op_branch_unprotect, BranchUnprotectArgs, BranchUnprotectResult,
};

#[tauri::command]
pub async fn branch_unprotect(
    state: State<'_, AppState>,
    branch: String,
) -> Result<BranchUnprotectResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_unprotect(&api, BranchUnprotectArgs { branch }).await
}

// --- branch archive ---

use lore_vm::ops::branch::archive::{
    archive as op_branch_archive, BranchArchiveArgs, BranchArchiveResult,
};

#[tauri::command]
pub async fn branch_archive(
    state: State<'_, AppState>,
    branch: String,
) -> Result<BranchArchiveResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_archive(&api, BranchArchiveArgs { branch }).await
}

// --- branch metadata_get ---

use lore_vm::ops::branch::metadata_get::{
    metadata_get as op_branch_metadata_get, BranchMetadataGetArgs, BranchMetadataGetResult,
};

#[tauri::command]
pub async fn branch_metadata_get(
    state: State<'_, AppState>,
    branch: String,
    key: String,
) -> Result<BranchMetadataGetResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_metadata_get(&api, BranchMetadataGetArgs { branch, key }).await
}

// --- branch merge_abort ---

use lore_vm::ops::branch::merge_abort::{
    merge_abort as op_branch_merge_abort, BranchMergeAbortArgs, BranchMergeAbortResult,
};

#[tauri::command]
pub async fn branch_merge_abort(
    state: State<'_, AppState>,
    link: String,
    ignore_links: bool,
) -> Result<BranchMergeAbortResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_merge_abort(&api, BranchMergeAbortArgs { link, ignore_links }).await
}

// --- branch merge_unresolve ---

use lore_vm::ops::branch::merge_unresolve::{
    merge_unresolve as op_branch_merge_unresolve, BranchMergeUnresolveArgs,
    BranchMergeUnresolveResult,
};

#[tauri::command]
pub async fn branch_merge_unresolve(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<BranchMergeUnresolveResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_merge_unresolve(&api, BranchMergeUnresolveArgs { paths }).await
}

// --- file info ---

use lore_vm::ops::file::info::{info as op_file_info, FileInfoArgs, FileInfoResult};

#[tauri::command]
pub async fn file_info(
    state: State<'_, AppState>,
    paths: Vec<String>,
    revision: String,
    local: bool,
    filtered: bool,
) -> Result<FileInfoResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_info(
        &api,
        FileInfoArgs {
            paths,
            revision,
            local,
            filtered,
        },
    )
    .await
}

// --- file obliterate ---

use lore_vm::ops::file::obliterate::{
    obliterate as op_file_obliterate, FileObliterateArgs, FileObliterateResult,
};

#[tauri::command]
pub async fn file_obliterate(
    state: State<'_, AppState>,
    path: String,
    address: String,
) -> Result<FileObliterateResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_obliterate(&api, FileObliterateArgs { address, path }).await
}

// --- branch merge_into ---

use lore_vm::ops::branch::merge_into::{
    merge_into as op_branch_merge_into, BranchMergeIntoArgs, BranchMergeIntoResult,
};

#[tauri::command]
pub async fn branch_merge_into(
    state: State<'_, AppState>,
    branch: String,
    branch_id: String,
    message: String,
    link: String,
    ignore_links: bool,
) -> Result<BranchMergeIntoResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_merge_into(
        &api,
        BranchMergeIntoArgs {
            branch,
            branch_id,
            message,
            link,
            ignore_links,
        },
    )
    .await
}

// --- repository verify_state ---

#[tauri::command]
pub async fn repository_verify_state(
    state: State<'_, AppState>,
    path: String,
    heal: bool,
) -> Result<VerifyStateResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_verify_state(&api, VerifyStateArgs { path, heal }).await
}

// --- revision diff ---

use lore_vm::ops::revision::diff::{
    diff as op_revision_diff, RevisionDiffArgs, RevisionDiffResult,
};

#[tauri::command]
pub async fn revision_diff(
    state: State<'_, AppState>,
    revision_source: String,
    revision_target: String,
    paths: Vec<String>,
) -> Result<RevisionDiffResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_diff(
        &api,
        RevisionDiffArgs {
            revision_source,
            revision_target,
            paths,
        },
    )
    .await
}

// --- revision find ---

use lore_vm::ops::revision::find::{
    find as op_revision_find, RevisionFindArgs, RevisionFindResult,
};

#[tauri::command]
pub async fn revision_find(
    state: State<'_, AppState>,
    key: String,
    value: String,
    number: u64,
) -> Result<RevisionFindResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_find(&api, RevisionFindArgs { key, value, number }).await
}

// --- revision find_local ---

use lore_vm::ops::revision::find_local::{
    find_local as op_revision_find_local, RevisionFindLocalArgs, RevisionFindLocalResult,
};

#[tauri::command]
pub async fn revision_find_local(
    state: State<'_, AppState>,
    key: String,
    value: String,
    number: u64,
) -> Result<RevisionFindLocalResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_find_local(&api, RevisionFindLocalArgs { key, value, number }).await
}

// --- repository dump ---

use lore_vm::ops::repository::dump::{
    dump as op_repository_dump, RepositoryDumpArgs, RepositoryDumpResult,
};

#[tauri::command]
pub async fn repository_dump(
    state: State<'_, AppState>,
    revision: String,
    path: String,
    max_depth: usize,
) -> Result<RepositoryDumpResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_dump(
        &api,
        RepositoryDumpArgs {
            revision,
            path,
            max_depth,
        },
    )
    .await
}

// --- repository delete ---

use lore_vm::ops::repository::delete::{delete as op_repository_delete, DeleteArgs, DeleteResult};

#[tauri::command]
pub async fn repository_delete(
    state: State<'_, AppState>,
    repository_url: String,
) -> Result<DeleteResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_delete(&api, DeleteArgs { repository_url }).await
}

// --- repository metadata_get ---

use lore_vm::ops::repository::metadata_get::{
    metadata_get as op_repository_metadata_get, RepositoryMetadataGetArgs,
    RepositoryMetadataGetResult,
};

#[tauri::command]
pub async fn repository_metadata_get(
    state: State<'_, AppState>,
    key: String,
) -> Result<RepositoryMetadataGetResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_metadata_get(&api, RepositoryMetadataGetArgs { key }).await
}

// --- repository metadata_set ---

use lore_vm::ops::repository::metadata_set::{
    metadata_set as op_repository_metadata_set, MetadataFormat, RepositoryMetadataSetArgs,
    RepositoryMetadataSetResult,
};

#[tauri::command]
pub async fn repository_metadata_set(
    state: State<'_, AppState>,
    keys: Vec<String>,
    values: Vec<String>,
    formats: Vec<MetadataFormat>,
) -> Result<RepositoryMetadataSetResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_metadata_set(
        &api,
        RepositoryMetadataSetArgs {
            keys,
            values,
            formats,
        },
    )
    .await
}

// --- repository instance_list ---

use lore_vm::ops::repository::instance_list::{
    instance_list as op_repository_instance_list, InstanceListResult,
};

#[tauri::command]
pub async fn repository_instance_list(
    state: State<'_, AppState>,
) -> Result<InstanceListResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_instance_list(&api).await
}

// --- repository list ---

use lore_vm::ops::repository::list::{list as op_repository_list, ListArgs, ListResult};

#[tauri::command]
pub async fn repository_list(
    state: State<'_, AppState>,
    url: String,
) -> Result<ListResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_list(&api, ListArgs { url }).await
}

// --- repository flush ---

use lore_vm::ops::repository::flush::{flush as op_repository_flush, FlushResult};

#[tauri::command]
pub async fn repository_flush(state: State<'_, AppState>) -> Result<FlushResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_flush(&api).await
}

// --- repository gc ---

use lore_vm::ops::repository::gc::{gc as op_repository_gc, GcResult};

#[tauri::command]
pub async fn repository_gc(state: State<'_, AppState>) -> Result<GcResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_gc(&api).await
}

// --- repository instance_prune ---

use lore_vm::ops::repository::instance_prune::{
    instance_prune as op_repository_instance_prune, InstancePruneResult, PrunedInstance,
};

#[tauri::command]
pub async fn repository_instance_prune(
    state: State<'_, AppState>,
) -> Result<InstancePruneResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_instance_prune(&api).await
}

// --- revision revert_local ---

use lore_vm::ops::repository::verify_state::{
    verify_state as op_verify_state, VerifyStateArgs, VerifyStateResult,
};

use lore_vm::ops::revision::revert_local::{
    revert_local as op_revision_revert_local, RevertLocalArgs, RevertLocalResult,
};

#[tauri::command]
pub async fn revision_revert_local(
    state: State<'_, AppState>,
    revision: String,
    message: String,
    no_commit: bool,
) -> Result<RevertLocalResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_revert_local(
        &api,
        RevertLocalArgs {
            revision,
            message,
            no_commit,
        },
    )
    .await
}

// --- revision revert_resolve ---

use lore_vm::ops::revision::revert_resolve::{
    revert_resolve as op_revision_revert_resolve, RevertResolveArgs, RevertResolveResult,
};

#[tauri::command]
pub async fn revision_revert_resolve(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<RevertResolveResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_revert_resolve(&api, RevertResolveArgs { paths }).await
}

// --- link add ---

use lore_vm::ops::link::add::{
    add as op_link_add, AddArgs as LinkAddArgs, AddResult as LinkAddResult,
};

#[tauri::command]
pub async fn link_add(
    state: State<'_, AppState>,
    link: String,
    link_path: String,
    source_path: String,
    pin: String,
    disable_branching: bool,
) -> Result<LinkAddResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_link_add(
        &api,
        LinkAddArgs {
            link,
            link_path,
            source_path,
            pin,
            disable_branching,
        },
    )
    .await
}

// --- link remove ---

use lore_vm::ops::link::remove::{remove as op_link_remove, RemoveArgs, RemoveResult};

#[tauri::command]
pub async fn link_remove(
    state: State<'_, AppState>,
    link_path: String,
) -> Result<RemoveResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_link_remove(&api, RemoveArgs { link_path }).await
}

// --- lock file_release ---

use lore_vm::ops::lock::file_release::{
    file_release as op_lock_file_release, FileReleaseArgs, FileReleaseResult,
};

#[tauri::command]
pub async fn lock_file_release(
    state: State<'_, AppState>,
    paths: Vec<String>,
    branch: String,
    owner: String,
    owner_id: String,
) -> Result<FileReleaseResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_lock_file_release(
        &api,
        FileReleaseArgs {
            paths,
            branch,
            owner,
            owner_id,
        },
    )
    .await
}

// --- auth local_user_info ---

use lore_vm::ops::auth::local_user_info::{
    local_user_info as op_auth_local_user_info, LocalUserInfoArgs, LocalUserInfoResult,
};

#[tauri::command]
pub async fn auth_local_user_info(
    state: State<'_, AppState>,
    auth_endpoint: String,
    user_ids: Vec<String>,
    with_token: bool,
) -> Result<LocalUserInfoResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_auth_local_user_info(
        &api,
        LocalUserInfoArgs {
            auth_endpoint,
            user_ids,
            with_token,
        },
    )
    .await
}

// --- lock file_acquire_as_owner ---

use lore_vm::ops::lock::file_acquire_as_owner::{
    file_acquire_as_owner as op_lock_file_acquire_as_owner, FileAcquireAsOwnerArgs,
    FileAcquireAsOwnerResult,
};

#[tauri::command]
pub async fn lock_file_acquire_as_owner(
    state: State<'_, AppState>,
    paths: Vec<String>,
    branch: String,
    owner: String,
) -> Result<FileAcquireAsOwnerResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_lock_file_acquire_as_owner(
        &api,
        FileAcquireAsOwnerArgs {
            paths,
            branch,
            owner,
        },
    )
    .await
}

// --- file write ---

use lore_vm::ops::file::write::{write as op_file_write, FileWriteArgs, FileWriteResult};

#[tauri::command]
pub async fn file_write(
    state: State<'_, AppState>,
    path: String,
    revision: String,
    output: String,
    address: String,
) -> Result<FileWriteResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_write(
        &api,
        FileWriteArgs {
            address,
            path,
            revision,
            output,
        },
    )
    .await
}

// --- file dump ---

use lore_vm::ops::file::dump::{dump as op_file_dump, FileDumpArgs, FileDumpResult};

#[tauri::command]
pub async fn file_dump(
    state: State<'_, AppState>,
    address: String,
    path: String,
) -> Result<FileDumpResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_dump(&api, FileDumpArgs { address, path }).await
}

// --- file stage ---

use lore_vm::ops::file::stage::{
    stage as op_file_stage, CaseChange, FileStageArgs, FileStageResult,
};

#[tauri::command]
pub async fn file_stage(
    state: State<'_, AppState>,
    paths: Vec<String>,
    case_change: Option<CaseChange>,
    scan: Option<bool>,
) -> Result<FileStageResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_stage(
        &api,
        FileStageArgs {
            paths,
            case_change: case_change.unwrap_or_default(),
            scan: scan.unwrap_or(false),
        },
    )
    .await
}

// --- file dirty ---

use lore_vm::ops::file::dirty::{dirty as op_file_dirty, FileDirtyArgs, FileDirtyResult};

#[tauri::command]
pub async fn file_dirty(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<FileDirtyResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_dirty(&api, FileDirtyArgs { paths }).await
}

// --- file dirty_copy ---

use lore_vm::ops::file::dirty_copy::{
    dirty_copy as op_file_dirty_copy, FileDirtyCopyArgs, FileDirtyCopyResult,
};

#[tauri::command]
pub async fn file_dirty_copy(
    state: State<'_, AppState>,
    from_path: String,
    to_path: String,
) -> Result<FileDirtyCopyResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_dirty_copy(&api, FileDirtyCopyArgs { from_path, to_path }).await
}

// --- file dirty_move ---

use lore_vm::ops::file::dirty_move::{
    dirty_move as op_file_dirty_move, FileDirtyMoveArgs, FileDirtyMoveResult,
};

#[tauri::command]
pub async fn file_dirty_move(
    state: State<'_, AppState>,
    from_path: String,
    to_path: String,
) -> Result<FileDirtyMoveResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_dirty_move(&api, FileDirtyMoveArgs { from_path, to_path }).await
}

// --- file reset_to_last_merged ---

use lore_vm::ops::file::reset_to_last_merged::{
    reset_to_last_merged as op_file_reset_to_last_merged, FileResetToLastMergedArgs,
    FileResetToLastMergedResult,
};

#[tauri::command]
pub async fn file_reset_to_last_merged(
    state: State<'_, AppState>,
    paths: Vec<String>,
    branch: String,
    purge: bool,
) -> Result<FileResetToLastMergedResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_reset_to_last_merged(
        &api,
        FileResetToLastMergedArgs {
            paths,
            branch,
            purge,
        },
    )
    .await
}

// --- file diff ---

use lore_vm::ops::file::diff::{diff as op_file_diff, DiffArgs, FileDiffEntry};

#[tauri::command]
pub async fn file_diff(
    state: State<'_, AppState>,
    paths: Vec<String>,
    source_revision: String,
    target_revision: String,
    diff3: bool,
    context_lines: u32,
    ignore_whitespace_eol: bool,
    ignore_whitespace_inline: bool,
) -> Result<Vec<FileDiffEntry>, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_diff(
        &api,
        DiffArgs {
            paths,
            source_revision,
            target_revision,
            diff3,
            context_lines,
            ignore_whitespace_eol,
            ignore_whitespace_inline,
        },
    )
    .await
}

// --- revision sync ---

use lore_vm::ops::revision::sync::{
    sync as op_revision_sync, RevisionSyncArgs, RevisionSyncResult,
};

#[tauri::command]
pub async fn revision_sync(
    state: State<'_, AppState>,
    revision: String,
    forward_changes: bool,
    reset: bool,
    root_files: Vec<String>,
    dependency_tags: Vec<String>,
    dependency_recursive: bool,
    dependency_depth_limit: u32,
) -> Result<RevisionSyncResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_sync(
        &api,
        RevisionSyncArgs {
            revision,
            forward_changes,
            reset,
            root_files,
            dependency_tags,
            dependency_recursive,
            dependency_depth_limit,
        },
    )
    .await
}

// --- revision history ---

use lore_vm::ops::revision::history::{
    history as op_revision_history, RevisionHistoryArgs, RevisionHistoryResult,
};

#[tauri::command]
pub async fn revision_history(
    state: State<'_, AppState>,
    revision: String,
    branch: String,
    date: u64,
    length: u32,
    only_branch: bool,
) -> Result<RevisionHistoryResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_history(
        &api,
        RevisionHistoryArgs {
            revision,
            branch,
            date,
            length,
            only_branch,
        },
    )
    .await
}

// --- revision info ---

use lore_vm::ops::revision::info::{
    info as op_revision_info, RevisionInfoArgs, RevisionInfoResult,
};

#[tauri::command]
pub async fn revision_info(
    state: State<'_, AppState>,
    revision: String,
    delta: bool,
    metadata: bool,
) -> Result<RevisionInfoResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_info(
        &api,
        RevisionInfoArgs {
            revision,
            delta,
            metadata,
        },
    )
    .await
}

// --- revision amend ---

use lore_vm::ops::revision::amend::{amend as op_revision_amend, AmendArgs, AmendResult};

#[tauri::command]
pub async fn revision_amend(
    state: State<'_, AppState>,
    message: String,
) -> Result<AmendResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_amend(&api, AmendArgs { message }).await
}

// --- revision commit (ops-layer) ---

use lore_vm::ops::revision::commit::{
    commit as op_revision_commit, CommitArgs as OpsCommitArgs, CommitResult,
};

#[tauri::command]
pub async fn revision_commit(
    state: State<'_, AppState>,
    message: String,
) -> Result<CommitResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_commit(&api, OpsCommitArgs { message }).await
}

// --- lock file_acquire ---

use lore_vm::ops::lock::file_acquire::{
    file_acquire as op_lock_file_acquire, FileAcquireArgs, FileAcquireResult,
};

#[tauri::command]
pub async fn lock_file_acquire(
    state: State<'_, AppState>,
    paths: Vec<String>,
    branch: String,
) -> Result<FileAcquireResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_lock_file_acquire(&api, FileAcquireArgs { paths, branch }).await
}

// --- lock file_status ---

use lore_vm::ops::lock::file_status::{
    file_status as op_lock_file_status, FileStatusArgs, FileStatusResult,
};

#[tauri::command]
pub async fn lock_file_status(
    state: State<'_, AppState>,
    paths: Vec<String>,
    branch: String,
) -> Result<FileStatusResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_lock_file_status(&api, FileStatusArgs { paths, branch }).await
}

// --- lock file_query ---

use lore_vm::ops::lock::file_query::{
    file_query as op_lock_file_query, FileQueryArgs, FileQueryResult,
};

#[tauri::command]
pub async fn lock_file_query(
    state: State<'_, AppState>,
    branch: String,
    owner: String,
    path: String,
) -> Result<FileQueryResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_lock_file_query(
        &api,
        FileQueryArgs {
            branch,
            owner,
            path,
        },
    )
    .await
}

// --- branch create (ops-layer) ---

use lore_vm::ops::branch::create::{
    create as op_branch_create, BranchCreateArgs, BranchCreateResult,
};

#[tauri::command]
pub async fn branch_create(
    state: State<'_, AppState>,
    branch: String,
    category: String,
    id: String,
) -> Result<BranchCreateResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_create(
        &api,
        BranchCreateArgs {
            branch,
            category,
            id,
        },
    )
    .await
}

// --- branch merge_start ---

use lore_vm::ops::branch::merge_start::{
    merge_start as op_branch_merge_start, BranchMergeStartArgs, BranchMergeStartResult,
};

#[tauri::command]
pub async fn branch_merge_start(
    state: State<'_, AppState>,
    branch: String,
    message: String,
    no_commit: bool,
    link: String,
    ignore_links: bool,
) -> Result<BranchMergeStartResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_merge_start(
        &api,
        BranchMergeStartArgs {
            branch,
            message,
            no_commit,
            link,
            ignore_links,
        },
    )
    .await
}

// --- branch merge_restart ---

use lore_vm::ops::branch::merge_restart::{
    merge_restart as op_branch_merge_restart, BranchMergeRestartArgs, BranchMergeRestartResult,
};

#[tauri::command]
pub async fn branch_merge_restart(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<BranchMergeRestartResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_merge_restart(&api, BranchMergeRestartArgs { paths }).await
}

// --- branch merge_resolve_theirs ---

use lore_vm::ops::branch::merge_resolve_theirs::{
    merge_resolve_theirs as op_branch_merge_resolve_theirs, BranchMergeResolveTheirsArgs,
    BranchMergeResolveTheirsResult,
};

#[tauri::command]
pub async fn branch_merge_resolve_theirs(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<BranchMergeResolveTheirsResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_merge_resolve_theirs(&api, BranchMergeResolveTheirsArgs { paths }).await
}

// --- branch merge_resolve_mine ---

use lore_vm::ops::branch::merge_resolve_mine::{
    merge_resolve_mine as op_branch_merge_resolve_mine, BranchMergeResolveMineArgs,
    BranchMergeResolveMineResult,
};

#[tauri::command]
pub async fn branch_merge_resolve_mine(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<BranchMergeResolveMineResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_merge_resolve_mine(&api, BranchMergeResolveMineArgs { paths }).await
}

// --- branch reset ---

use lore_vm::ops::branch::reset::{reset as op_branch_reset, BranchResetArgs, BranchResetResult};

#[tauri::command]
pub async fn branch_reset(
    state: State<'_, AppState>,
    revision: String,
    branch: String,
) -> Result<BranchResetResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_reset(&api, BranchResetArgs { revision, branch }).await
}

// --- branch latest_list ---

use lore_vm::ops::branch::latest_list::{
    latest_list as op_branch_latest_list, BranchLatestListArgs, BranchLatestListResult,
};

#[tauri::command]
pub async fn branch_latest_list(
    state: State<'_, AppState>,
    branch: String,
    limit: u32,
) -> Result<BranchLatestListResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_latest_list(&api, BranchLatestListArgs { branch, limit }).await
}

// --- branch list ---

use lore_vm::ops::branch::list::{list as op_branch_list, BranchListArgs, BranchListResult};

#[tauri::command]
pub async fn branch_list(
    state: State<'_, AppState>,
    archived: bool,
) -> Result<BranchListResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_list(&api, BranchListArgs { archived }).await
}

// --- branch merge_resolve ---

use lore_vm::ops::branch::merge_resolve::{
    merge_resolve as op_branch_merge_resolve, BranchMergeResolveArgs, BranchMergeResolveResult,
};

#[tauri::command]
pub async fn branch_merge_resolve(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<BranchMergeResolveResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_branch_merge_resolve(&api, BranchMergeResolveArgs { paths }).await
}

// --- repository create (ops-layer) ---

use lore_vm::ops::repository::create::{create as op_repository_create, CreateArgs, CreateResult};

#[tauri::command]
pub async fn repository_create(
    state: State<'_, AppState>,
    repository_url: String,
    description: String,
    id: String,
    use_shared_store: bool,
    shared_store_path: String,
    // Optional target path supplied by the onboarding wizard. When present the
    // working dir is pointed at it so the repository is created there. The
    // lower-level `repositoryCreateApi.create` caller omits it.
    path: Option<String>,
) -> Result<CreateResult, LoreError> {
    if let Some(p) = path.filter(|p| !p.is_empty()) {
        *state.working_dir.lock().unwrap() = PathBuf::from(p);
    }
    let api = LoreApi::new(state.dir());
    op_repository_create(
        &api,
        CreateArgs {
            repository_url,
            description,
            id,
            use_shared_store,
            shared_store_path,
        },
    )
    .await
}

// =====================================================================
// Onboarding / server-install flow commands (SBAI-3841..3848).
//
// These wrap the storage / shared_store / auth / service / repository ops
// that the onboarding wizard (frontend/src/onboarding/*) drives via api.ts.
// Thin wrappers only — all behaviour lives in lore-vm.
// =====================================================================

// --- onboarding: storage backend config ---

/// Storage backend configuration captured by the server-setup wizard.
///
/// Mirrors the `StorageBackendConfig` interface in `frontend/src/api.ts`.
/// camelCase JS keys map onto these snake_case fields via serde rename.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
// Object-storage fields (bucket/region/credentials) are part of the typed
// contract from the wizard but not yet consumed by `storage_open` (which only
// needs path/endpoint today); retained for forthcoming object-store wiring.
#[allow(dead_code)]
pub struct StorageBackendConfig {
    /// lore's two real store backends: "local" (filesystem) | "s3"
    /// (S3-compatible object store — AWS S3 / MinIO / Garage / … are the same
    /// backend, differing only by endpoint).
    pub kind: String,
    /// Local packfiles path (kind == "local").
    #[serde(default)]
    pub path: Option<String>,
    /// S3-compatible object-storage endpoint (kind == "s3").
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub bucket: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub access_key_id: Option<String>,
    #[serde(default)]
    pub secret_access_key: Option<String>,
    /// Mutable KV store location (branch pointers / bookkeeping).
    #[serde(default)]
    pub mutable_store: Option<String>,
}

// --- onboarding: user info (auth) ---

/// Minimal user identity returned to the onboarding auth screens.
///
/// Mirrors the `UserInfo` interface in `frontend/src/api.ts`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
}

// --- storage open ---

use lore_vm::ops::storage::open::{open as op_storage_open, StorageOpenArgs};

#[tauri::command]
pub async fn storage_open(
    state: State<'_, AppState>,
    config: StorageBackendConfig,
) -> Result<(), LoreError> {
    // Map the wizard config onto the storage-open args. For object-storage
    // backends the connection is supplied as a remote URL; "local" backends
    // open the on-disk store at `path`. When no path/endpoint is given we fall
    // back to an in-memory store so the connectivity test can still run.
    let repository_path = config.path.clone().unwrap_or_default();
    let remote_url = config.endpoint.clone().unwrap_or_default();
    let in_memory = repository_path.is_empty() && remote_url.is_empty();

    let api = LoreApi::new(state.dir());
    let result = op_storage_open(
        &api,
        StorageOpenArgs {
            repository_path,
            in_memory,
            remote_url,
            cache_target_bytes: 0,
            cache_target_fragments: 0,
        },
    )
    .await?;

    let mut session = state.storage_session.lock().unwrap();
    session.handle = Some(result.handle);
    session.keys.clear();
    Ok(())
}

// --- storage put ---

use lore_vm::ops::storage::put::{put as op_storage_put, PutItem, StoragePutArgs};

/// Fixed partition used for the onboarding connectivity probe. The storage
/// layer is content-addressed, so any stable partition works for the round
/// trip; we use the all-zero partition for simplicity.
const ONBOARDING_PARTITION: &str = "00000000000000000000000000000001";

#[tauri::command]
pub async fn storage_put(
    state: State<'_, AppState>,
    key: String,
    data: Vec<u8>,
) -> Result<(), LoreError> {
    let handle = {
        let session = state.storage_session.lock().unwrap();
        session.handle.ok_or_else(|| {
            LoreError::CommandFailed("storage_put called before storage_open".into())
        })?
    };

    let api = LoreApi::new(state.dir());
    let result = op_storage_put(
        &api,
        StoragePutArgs {
            handle,
            items: vec![PutItem {
                id: 0,
                partition: ONBOARDING_PARTITION.to_string(),
                context: String::new(),
                data,
                remote_write: false,
                local_cache: false,
                fixed_size_chunk: 0,
            }],
        },
    )
    .await?;

    let item = result
        .items
        .into_iter()
        .next()
        .ok_or_else(|| LoreError::CommandFailed("storage put returned no item".into()))?;
    if !item.ok {
        return Err(LoreError::CommandFailed(format!(
            "storage put failed: {}",
            item.error
        )));
    }

    // Record the produced content address so a later get/obliterate by the same
    // key can resolve it.
    state
        .storage_session
        .lock()
        .unwrap()
        .keys
        .insert(key, (ONBOARDING_PARTITION.to_string(), item.address));
    Ok(())
}

// --- storage get ---

use lore_vm::ops::storage::get::{storage_get as op_storage_get, GetItem, StorageGetArgs};

#[tauri::command]
pub async fn storage_get(state: State<'_, AppState>, key: String) -> Result<Vec<u8>, LoreError> {
    let (handle, partition, address) = {
        let session = state.storage_session.lock().unwrap();
        let handle = session.handle.ok_or_else(|| {
            LoreError::CommandFailed("storage_get called before storage_open".into())
        })?;
        let (partition, address) =
            session.keys.get(&key).cloned().ok_or_else(|| {
                LoreError::CommandFailed(format!("storage_get: unknown key {key:?}"))
            })?;
        (handle, partition, address)
    };

    let api = LoreApi::new(state.dir());
    let result = op_storage_get(
        &api,
        StorageGetArgs {
            handle,
            items: vec![GetItem {
                id: 0,
                partition,
                address,
                streaming: false,
                local_cache: false,
            }],
        },
    )
    .await?;

    let item = result
        .items
        .into_iter()
        .next()
        .ok_or_else(|| LoreError::CommandFailed("storage get returned no item".into()))?;
    if !item.ok {
        return Err(LoreError::CommandFailed(format!(
            "storage get failed: {}",
            item.error.unwrap_or_default()
        )));
    }
    Ok(item.data)
}

// --- storage obliterate ---

use lore_vm::ops::storage::obliterate::{
    obliterate as op_storage_obliterate, ObliterateItem, StorageObliterateArgs,
};

#[tauri::command]
pub async fn storage_obliterate(state: State<'_, AppState>, key: String) -> Result<(), LoreError> {
    let entry = {
        let session = state.storage_session.lock().unwrap();
        let handle = session.handle.ok_or_else(|| {
            LoreError::CommandFailed("storage_obliterate called before storage_open".into())
        })?;
        session
            .keys
            .get(&key)
            .cloned()
            .map(|(partition, address)| (handle, partition, address))
    };

    // Idempotent: an unknown key is treated as already-obliterated.
    let (handle, partition, address) = match entry {
        Some(v) => v,
        None => return Ok(()),
    };

    let api = LoreApi::new(state.dir());
    op_storage_obliterate(
        &api,
        StorageObliterateArgs {
            handle,
            items: vec![ObliterateItem {
                id: 0,
                partition,
                address,
            }],
        },
    )
    .await?;

    state.storage_session.lock().unwrap().keys.remove(&key);
    Ok(())
}

// =====================================================================
// Full storage-domain ops (SBAI-4024, storage template).
//
// The onboarding `storage_open`/`storage_put`/`storage_get`/`storage_obliterate`
// commands above speak in opaque string keys and a nested backend config — the
// shape the first-run wizard needs. The commands below expose the *full*
// content-addressed storage ops with flat, palette-friendly arguments
// (handle + partition + address …), so every op is reachable from the command
// palette via a generated form. All are thin wrappers over the lore-vm op.
// =====================================================================

// --- storage open_handle (flat; returns the handle for later ops) ---

/// Open a content-addressed store with flat args and return its handle id.
///
/// Unlike the onboarding `storage_open` (which takes a nested backend config and
/// stashes the handle in the session), this returns the handle directly so
/// palette users can thread it into `storage_close`/`storage_flush`/etc. The
/// handle is also recorded in the session so the Storage panel can reuse it.
#[tauri::command]
pub async fn storage_open_handle(
    state: State<'_, AppState>,
    repository_path: String,
    remote_url: String,
    in_memory: bool,
) -> Result<u64, LoreError> {
    let api = LoreApi::new(state.dir());
    let result = op_storage_open(
        &api,
        StorageOpenArgs {
            repository_path,
            in_memory,
            remote_url,
            cache_target_bytes: 0,
            cache_target_fragments: 0,
        },
    )
    .await?;
    let mut session = state.storage_session.lock().unwrap();
    session.handle = Some(result.handle);
    Ok(result.handle)
}

// --- storage close ---

use lore_vm::ops::storage::close::{
    close as op_storage_close, StorageCloseArgs, StorageCloseResult,
};

#[tauri::command]
pub async fn storage_close(
    state: State<'_, AppState>,
    handle: u64,
) -> Result<StorageCloseResult, LoreError> {
    let api = LoreApi::new(state.dir());
    let result = op_storage_close(&api, StorageCloseArgs { handle }).await?;
    // If we just closed the session handle, drop it so the panel reflects reality.
    let mut session = state.storage_session.lock().unwrap();
    if session.handle == Some(handle) {
        session.handle = None;
        session.keys.clear();
    }
    Ok(result)
}

// --- storage flush ---

use lore_vm::ops::storage::flush::{
    flush as op_storage_flush, StorageFlushArgs, StorageFlushResult,
};

#[tauri::command]
pub async fn storage_flush(
    state: State<'_, AppState>,
    handle: u64,
) -> Result<StorageFlushResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_storage_flush(&api, StorageFlushArgs { handle }).await
}

// --- storage get_metadata ---

use lore_vm::ops::storage::get_metadata::{
    storage_get_metadata as op_storage_get_metadata, GetMetadataItem, StorageGetMetadataArgs,
    StorageGetMetadataResult,
};

#[tauri::command]
pub async fn storage_get_metadata(
    state: State<'_, AppState>,
    handle: u64,
    partition: String,
    address: String,
) -> Result<StorageGetMetadataResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_storage_get_metadata(
        &api,
        StorageGetMetadataArgs {
            handle,
            items: vec![GetMetadataItem {
                id: 0,
                partition,
                address,
            }],
        },
    )
    .await
}

// --- storage put_file ---

use lore_vm::ops::storage::put_file::{
    put_file as op_storage_put_file, PutFileItem, StoragePutFileArgs, StoragePutFileResult,
};

#[tauri::command]
pub async fn storage_put_file(
    state: State<'_, AppState>,
    handle: u64,
    partition: String,
    path: String,
    context: String,
    remote_write: bool,
    local_cache: bool,
) -> Result<StoragePutFileResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_storage_put_file(
        &api,
        StoragePutFileArgs {
            handle,
            items: vec![PutFileItem {
                id: 0,
                partition,
                context,
                path,
                remote_write,
                local_cache,
                fixed_size_chunk: 0,
            }],
        },
    )
    .await
}

// --- storage copy ---

use lore_vm::ops::storage::copy::{
    copy as op_storage_copy, CopyItem, StorageCopyArgs, StorageCopyResult,
};

#[tauri::command]
pub async fn storage_copy(
    state: State<'_, AppState>,
    handle: u64,
    source_partition: String,
    target_partition: String,
    source_address: String,
    target_context: String,
) -> Result<StorageCopyResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_storage_copy(
        &api,
        StorageCopyArgs {
            handle,
            items: vec![CopyItem {
                id: 0,
                source_partition,
                target_partition,
                source_address,
                target_context,
            }],
        },
    )
    .await
}

// --- storage upload ---

use lore_vm::ops::storage::upload::{
    upload as op_storage_upload, StorageUploadArgs, StorageUploadResult, UploadItem,
};

#[tauri::command]
pub async fn storage_upload(
    state: State<'_, AppState>,
    handle: u64,
    partition: String,
    address: String,
) -> Result<StorageUploadResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_storage_upload(
        &api,
        StorageUploadArgs {
            handle,
            items: vec![UploadItem {
                id: 0,
                partition,
                address,
            }],
        },
    )
    .await
}

// --- shared_store info ---

use lore_vm::ops::shared_store::info::{
    info as op_shared_store_info, SharedStoreInfoArgs, SharedStoreInfoResult,
};

#[tauri::command]
pub async fn shared_store_info(
    state: State<'_, AppState>,
) -> Result<SharedStoreInfoResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_shared_store_info(&api, SharedStoreInfoArgs).await
}

// --- shared_store set_use_automatically ---

use lore_vm::ops::shared_store::set_use_automatically::{
    set_use_automatically as op_shared_store_set_use_automatically, SetUseAutomaticallyArgs,
    SetUseAutomaticallyResult,
};

#[tauri::command]
pub async fn shared_store_set_use_automatically(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<SetUseAutomaticallyResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_shared_store_set_use_automatically(&api, SetUseAutomaticallyArgs { enabled }).await
}

// --- shared_store create ---

use lore_vm::ops::shared_store::create::{create as op_shared_store_create, SharedStoreCreateArgs};

#[tauri::command]
pub async fn shared_store_create(
    state: State<'_, AppState>,
    path: String,
) -> Result<String, LoreError> {
    let api = LoreApi::new(state.dir());
    // The wizard supplies only a filesystem path; the remote URL is left empty
    // so the store defaults to a local backing, and it is not made the global
    // default automatically.
    let result = op_shared_store_create(
        &api,
        SharedStoreCreateArgs {
            remote_url: String::new(),
            path: Some(path),
            make_default: false,
        },
    )
    .await?;
    Ok(result.path)
}

// --- repository clone ---

use lore_vm::ops::repository::clone::{clone as op_repository_clone, CloneArgs};

#[tauri::command]
pub async fn repository_clone(
    state: State<'_, AppState>,
    url: String,
    dest: String,
) -> Result<(), LoreError> {
    // Clone into `dest`: point the working dir at it so the local path used by
    // the op (globals.repository_path) is the requested destination.
    let dest_path = PathBuf::from(&dest);
    let api = LoreApi::new(dest_path.clone());
    op_repository_clone(
        &api,
        CloneArgs {
            repository_url: url,
            ..Default::default()
        },
    )
    .await?;
    *state.working_dir.lock().unwrap() = dest_path;
    Ok(())
}

// --- auth login_interactive ---

use lore_vm::ops::auth::login_interactive::{
    login_interactive as op_auth_login_interactive, LoginInteractiveArgs,
};

#[tauri::command]
pub async fn auth_login_interactive(
    state: State<'_, AppState>,
    remote_url: String,
) -> Result<UserInfo, LoreError> {
    let api = LoreApi::new(state.dir());
    let result = op_auth_login_interactive(
        &api,
        LoginInteractiveArgs {
            remote_url,
            no_browser: false,
        },
    )
    .await?;
    Ok(UserInfo {
        id: result.user_id,
        name: result.display_name,
    })
}

// --- auth login_with_token ---

use lore_vm::ops::auth::login_with_token::{
    login_with_token as op_auth_login_with_token, LoginWithTokenArgs,
};

#[tauri::command]
pub async fn auth_login_with_token(
    state: State<'_, AppState>,
    remote_url: String,
    token: String,
) -> Result<UserInfo, LoreError> {
    let api = LoreApi::new(state.dir());
    let result = op_auth_login_with_token(
        &api,
        LoginWithTokenArgs {
            remote_url,
            token,
            token_type: "Bearer".into(),
            auth_url: String::new(),
        },
    )
    .await?;
    Ok(UserInfo {
        id: result.user_id,
        name: result.display_name,
    })
}

// --- auth user_info (current user) ---

use lore_vm::ops::auth::resolve_user_info::{
    resolve_user_info as op_auth_resolve_user_info, ResolveUserInfoArgs,
};

#[tauri::command]
pub async fn auth_user_info(state: State<'_, AppState>) -> Result<Option<UserInfo>, LoreError> {
    let api = LoreApi::new(state.dir());
    // Empty user_ids resolves the current user locally.
    let result = op_auth_resolve_user_info(
        &api,
        ResolveUserInfoArgs {
            user_ids: Vec::new(),
        },
    )
    .await?;
    Ok(result.users.into_iter().next().map(|u| UserInfo {
        id: u.user_id,
        name: u.display_name,
    }))
}

// --- tray sync_state ---

/// Push the current repository status into the system tray (icon badge color,
/// tooltip, and title). Called from the frontend whenever branch/dirty/sync
/// state changes so the tray reflects live status.
#[tauri::command]
pub fn tray_sync_state(app: AppHandle, snapshot: crate::tray::TraySnapshot) -> Result<(), String> {
    crate::tray::apply_snapshot(&app, &snapshot).map_err(|e| e.to_string())
}

// --- dependency add ---

use lore_vm::ops::dependency::dependency_add::{
    dependency_add as op_dependency_add, DependencyAddArgs, DependencyAddResult,
    DependencyAddSource,
};

#[tauri::command]
pub async fn dependency_add(
    state: State<'_, AppState>,
    sources: Vec<DependencyAddSource>,
    force: bool,
) -> Result<DependencyAddResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_dependency_add(&api, DependencyAddArgs { sources, force }).await
}

// --- dependency list ---

use lore_vm::ops::dependency::dependency_list::{
    dependency_list as op_dependency_list, DependencyListArgs, DependencyListResult,
};

#[tauri::command]
pub async fn dependency_list(
    state: State<'_, AppState>,
    paths: Vec<String>,
    revision: String,
    recursive: bool,
    reverse: bool,
    tags: Vec<String>,
    depth_limit: u32,
) -> Result<DependencyListResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_dependency_list(
        &api,
        DependencyListArgs {
            paths,
            revision,
            recursive,
            reverse,
            tags,
            depth_limit,
        },
    )
    .await
}

// --- dependency remove ---

use lore_vm::ops::dependency::dependency_remove::{
    dependency_remove as op_dependency_remove, DependencyRemoveArgs, DependencyRemoveResult,
    DependencyRemoveSource,
};

#[tauri::command]
pub async fn dependency_remove(
    state: State<'_, AppState>,
    sources: Vec<DependencyRemoveSource>,
) -> Result<DependencyRemoveResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_dependency_remove(&api, DependencyRemoveArgs { sources }).await
}

// --- revision cherry_pick_restart ---

use lore_vm::ops::revision::cherry_pick_restart::{
    cherry_pick_restart as op_cherry_pick_restart, CherryPickRestartArgs, CherryPickRestartResult,
};

#[tauri::command]
pub async fn revision_cherry_pick_restart(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<CherryPickRestartResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_cherry_pick_restart(&api, CherryPickRestartArgs { paths }).await
}

// --- service start ---

use lore_vm::ops::service::start::start as op_service_start;

/// DEPRECATED for hosting: `lore::service::start` is an upstream **stub** that
/// returns 1 and hosts no server. The real "Host a server" path is
/// `host_server_start` (see `server_host.rs`, SBAI-4065). Kept registered so
/// nothing that already calls it breaks, but do NOT use it to host.
#[tauri::command]
pub async fn service_start(
    state: State<'_, AppState>,
    install_autorun: bool,
) -> Result<(), LoreError> {
    // NOTE: the upstream `lore::service::start` op takes no arguments, so the
    // `install_autorun` toggle from the wizard is accepted but not yet acted on
    // (no autorun-install op exists in lore-vm). Wired for forward-compat.
    let _ = install_autorun;
    let api = LoreApi::new(state.dir());
    op_service_start(&api).await?;
    Ok(())
}

// --- service stop ---

use lore_vm::ops::service::stop::{stop as op_service_stop, ServiceStopArgs, ServiceStopResult};

#[tauri::command]
pub async fn service_stop(
    state: State<'_, AppState>,
    all: bool,
) -> Result<ServiceStopResult, LoreError> {
    let api = LoreApi::new(state.dir());
    let result = op_service_stop(&api, ServiceStopArgs { all }).await?;
    Ok(result)
}

// --- host server (SBAI-4065): launch + manage a real loreserver ---

use crate::server_host::{
    self, HostAdvancedOptions, HostServerOptions, HostStatus, S3StoreOptions,
};

/// Launch a real `loreserver` process serving the host flow's local stores.
///
/// Idempotent: refuses (errors) if a server is already running — call
/// `host_server_stop` first. Returns the status incl. the advertised
/// `lore://host:port/<repo>` URL clients connect to.
///
/// Accepts flat fields (rather than a nested `opts` object) so the command is
/// directly drivable from the command palette's generated form — the palette
/// passes a flat `Record<string,unknown>` straight to `invoke`. The typed
/// `api.hostServerStart(opts)` wrapper spreads `HostServerOptions` onto these.
///
/// When `s3_bucket` is supplied, the hosted server's **immutable** store is an
/// S3-compatible object store (lore's `aws` mode); otherwise it is a local
/// filesystem store under `store_dir`. The mutable (branch-pointer) store is
/// always local — see [`server_host::S3StoreOptions`].
///
/// `server_host` is synchronous (it spawns + manages a `std::process::Child`).
/// The work here is fast — write a config, resolve the binary, spawn — so we
/// run it inline under the std `Mutex` (the guard is never held across an
/// `await`). The slow one-time dev build of `loreserver` happens only on a
/// developer machine with no bundled sidecar.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn host_server_start(
    state: State<'_, AppState>,
    store_dir: String,
    port: Option<u16>,
    repository_name: Option<String>,
    auth: Option<bool>,
    bind_host: Option<String>,
    s3_bucket: Option<String>,
    s3_endpoint: Option<String>,
    s3_region: Option<String>,
    s3_access_key_id: Option<String>,
    s3_secret_access_key: Option<String>,
    s3_force_path_style: Option<bool>,
    s3_dynamodb_endpoint: Option<String>,
    advanced: Option<HostAdvancedOptions>,
) -> Result<HostStatus, LoreError> {
    // An S3 backend is requested iff a (non-blank) bucket is supplied.
    let s3 = s3_bucket
        .filter(|b| !b.trim().is_empty())
        .map(|bucket| S3StoreOptions {
            endpoint: s3_endpoint,
            bucket,
            region: s3_region,
            access_key_id: s3_access_key_id,
            secret_access_key: s3_secret_access_key,
            force_path_style: s3_force_path_style.unwrap_or(false),
            dynamodb_endpoint: s3_dynamodb_endpoint,
        });

    let opts = HostServerOptions {
        store_dir,
        port: port.filter(|p| *p != 0),
        repository_name: repository_name.filter(|n| !n.trim().is_empty()),
        auth: auth.unwrap_or(false),
        bind_host: bind_host.filter(|h| !h.trim().is_empty()),
        s3,
        // The flat palette-driven `host_server_start` omits `advanced` (basic
        // surface only). The host UI's Expert mode sends the full advanced bag
        // (SBAI-4075); whatever is unset falls through to lore's own defaults.
        advanced,
    };
    let mut slot = state.hosted_server.lock().unwrap();
    server_host::start(&mut slot, &opts)
}

/// Render the `loreserver` config TOML for the given options **without** writing
/// anything to disk or starting a server (SBAI-4075). Backs the host flow's
/// "View generated config" affordance. Accepts the full advanced-config bag as a
/// single typed `opts` object (unlike `host_server_start`'s flat args, the
/// preview is only ever driven from the host UI, not the command palette, so a
/// nested object is the cleaner surface). Returns the generated TOML string, or
/// a validation error (bad enum / out-of-range / required-when-mode).
#[tauri::command]
pub fn host_server_render_config(opts: HostServerOptions) -> Result<String, LoreError> {
    server_host::render_config(&opts)
}

/// Stop the hosted `loreserver` (kill + reap). Idempotent.
#[tauri::command]
pub fn host_server_stop(state: State<'_, AppState>) -> Result<HostStatus, LoreError> {
    let mut slot = state.hosted_server.lock().unwrap();
    server_host::stop(&mut slot)
}

/// Current hosted-server status (running? + URL/port/pid). Reaps if it died.
#[tauri::command]
pub fn host_server_status(state: State<'_, AppState>) -> Result<HostStatus, LoreError> {
    let mut slot = state.hosted_server.lock().unwrap();
    Ok(server_host::status(&mut slot))
}

// --- auth logout + clear (ops-layer) ---

use lore_vm::ops::auth::clear::{clear as op_auth_clear, ClearArgs};
use lore_vm::ops::auth::logout::{logout as op_auth_logout, LogoutArgs};

#[tauri::command]
pub async fn auth_logout(
    state: State<'_, AppState>,
    auth_url: String,
    resource: String,
    user_id: String,
) -> Result<(), LoreError> {
    let api = LoreApi::new(state.dir());
    op_auth_logout(
        &api,
        LogoutArgs {
            auth_url,
            resource,
            user_id,
        },
    )
    .await
}

#[tauri::command]
pub async fn auth_clear(state: State<'_, AppState>) -> Result<(), LoreError> {
    let api = LoreApi::new(state.dir());
    op_auth_clear(&api, ClearArgs {}).await
}

// --- repository info (SBAI-4033) ---

use lore_vm::ops::repository::info::{
    info as op_repository_info, RepositoryInfoArgs, RepositoryInfoResult,
};

#[tauri::command]
pub async fn repository_info(
    state: State<'_, AppState>,
    repository_url: String,
) -> Result<RepositoryInfoResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_info(&api, RepositoryInfoArgs { repository_url }).await
}

// --- repository release (SBAI-4033) ---

use lore_vm::ops::repository::release::{release as op_repository_release, ReleaseResult};

#[tauri::command]
pub async fn repository_release(state: State<'_, AppState>) -> Result<ReleaseResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_release(&api).await
}

// --- repository config_get (SBAI-4033) ---

use lore_vm::ops::repository::config_get::{
    config_get as op_repository_config_get, RepositoryConfigGetArgs, RepositoryConfigGetResult,
};

#[tauri::command]
pub async fn repository_config_get(
    state: State<'_, AppState>,
    key: String,
) -> Result<RepositoryConfigGetResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_config_get(&api, RepositoryConfigGetArgs { key }).await
}

// --- repository metadata_clear (SBAI-4033) ---

use lore_vm::ops::repository::metadata_clear::{
    metadata_clear as op_repository_metadata_clear, RepositoryMetadataClearArgs,
    RepositoryMetadataClearResult,
};

#[tauri::command]
pub async fn repository_metadata_clear(
    state: State<'_, AppState>,
    keys: Vec<String>,
) -> Result<RepositoryMetadataClearResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_metadata_clear(&api, RepositoryMetadataClearArgs { keys }).await
}

// --- repository create_with_metadata (SBAI-4033) ---

use lore_vm::ops::repository::create_with_metadata::{
    create_with_metadata as op_repository_create_with_metadata, CreateWithMetadataArgs,
    CreateWithMetadataResult,
};

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn repository_create_with_metadata(
    state: State<'_, AppState>,
    repository_url: String,
    creator: String,
    created: u64,
    description: String,
    id: String,
    use_shared_store: bool,
    shared_store_path: String,
) -> Result<CreateWithMetadataResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_create_with_metadata(
        &api,
        CreateWithMetadataArgs {
            repository_url,
            description,
            id,
            use_shared_store,
            shared_store_path,
            creator,
            created,
        },
    )
    .await
}

// --- repository store_immutable_query (SBAI-4033) ---

use lore_vm::ops::repository::store_immutable_query::{
    store_immutable_query as op_repository_store_immutable_query, StoreImmutableQueryArgs,
    StoreImmutableQueryResult,
};

#[tauri::command]
pub async fn repository_store_immutable_query(
    state: State<'_, AppState>,
    address: String,
    recurse: bool,
) -> Result<StoreImmutableQueryResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_store_immutable_query(&api, StoreImmutableQueryArgs { address, recurse }).await
}

// --- repository verify_fragment (SBAI-4033) ---

use lore_vm::ops::repository::verify_fragment::{
    verify_fragment as op_repository_verify_fragment, VerifyFragmentArgs, VerifyFragmentResult,
};

#[tauri::command]
pub async fn repository_verify_fragment(
    state: State<'_, AppState>,
    hash: String,
    context: String,
    heal: bool,
) -> Result<VerifyFragmentResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_verify_fragment(
        &api,
        VerifyFragmentArgs {
            hash,
            context,
            heal,
        },
    )
    .await
}

// --- repository update_path (SBAI-4033) ---

use lore_vm::ops::repository::repository_update_path::{
    repository_update_path as op_repository_update_path, RepositoryUpdatePathResult,
};

#[tauri::command]
pub async fn repository_update_path(
    state: State<'_, AppState>,
) -> Result<RepositoryUpdatePathResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_repository_update_path(&api).await
}

// --- file hash (SBAI-4034) ---

use lore_vm::ops::file::hash::{hash as op_file_hash, FileHashArgs, FileHashResult};

#[tauri::command]
pub async fn file_hash(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<FileHashResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_hash(&api, FileHashArgs { paths }).await
}

// --- file metadata_list (SBAI-4034) ---

use lore_vm::ops::file::metadata_list::{
    metadata_list as op_file_metadata_list, MetadataListArgs, MetadataListResult,
};

#[tauri::command]
pub async fn file_metadata_list(
    state: State<'_, AppState>,
    path: String,
    revision: String,
) -> Result<MetadataListResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_file_metadata_list(&api, MetadataListArgs { path, revision }).await
}

// --- revision revert_abort (SBAI-4036) ---

use lore_vm::ops::revision::revert_abort::{
    revert_abort as op_revision_revert_abort, RevertAbortArgs, RevertAbortResult,
};

#[tauri::command]
pub async fn revision_revert_abort(
    state: State<'_, AppState>,
) -> Result<RevertAbortResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_revert_abort(&api, RevertAbortArgs {}).await
}

// --- revision revert_resolve_mine (SBAI-4036) ---

use lore_vm::ops::revision::revert_resolve_mine::{
    revert_resolve_mine as op_revision_revert_resolve_mine, RevertResolveMineArgs,
    RevertResolveMineResult,
};

#[tauri::command]
pub async fn revision_revert_resolve_mine(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<RevertResolveMineResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_revert_resolve_mine(&api, RevertResolveMineArgs { paths }).await
}

// --- revision commit_with_metadata (SBAI-4036) ---

use lore_vm::ops::revision::commit_with_metadata::{
    commit_with_metadata as op_revision_commit_with_metadata, CommitWithMetadataArgs,
    CommitWithMetadataResult, MetadataFormat as CommitMetadataFormat,
};

#[tauri::command]
pub async fn revision_commit_with_metadata(
    state: State<'_, AppState>,
    message: String,
    keys: Vec<String>,
    values: Vec<String>,
    formats: Vec<CommitMetadataFormat>,
) -> Result<CommitWithMetadataResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_commit_with_metadata(
        &api,
        CommitWithMetadataArgs {
            message,
            keys,
            values,
            formats,
        },
    )
    .await
}

// --- revision metadata_clear (SBAI-4037) ---

use lore_vm::ops::revision::metadata_clear::{
    metadata_clear as op_revision_metadata_clear, MetadataClearArgs as RevisionMetadataClearArgs,
    MetadataClearResult as RevisionMetadataClearResult,
};

#[tauri::command]
pub async fn revision_metadata_clear(
    state: State<'_, AppState>,
) -> Result<RevisionMetadataClearResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_metadata_clear(&api, RevisionMetadataClearArgs {}).await
}

// --- layer add (SBAI-4038) ---

use lore_vm::ops::layer::layer_add::{layer_add as op_layer_add, LayerAddArgs, LayerAddResult};

#[tauri::command]
pub async fn layer_add(
    state: State<'_, AppState>,
    target_path: String,
    source_repository: String,
    source_path: String,
    metadata: String,
) -> Result<LayerAddResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_layer_add(
        &api,
        LayerAddArgs {
            target_path,
            source_repository,
            source_path,
            metadata,
        },
    )
    .await
}

// --- revision activity_report (SBAI-4061) ---
//
// Aggregated "who did what when" rollup over a revision chain — the data op
// behind the commercial Reporting & Insights add-on (SBAI-4068). The command
// is a thin wrapper; entitlement gating lives in the frontend (the panel is
// hidden/locked unless `isEntitled("reporting")`). See docs/COMMERCIAL-ADDONS.md.

use lore_vm::ops::revision::activity_report::{
    activity_report as op_revision_activity_report, ActivityReportArgs, ActivityReportResult,
};

/// Read an on-disk `license.key` file for the commercial entitlement gate
/// (SBAI-4068), returning its trimmed contents or `None` if absent/unreadable.
///
/// This is purely a *file reader* — it does not verify the token. Signature
/// verification happens in the frontend (`license.ts`) against the embedded
/// Ed25519 public key. The lookup order is:
///
///   1. `LOREGUI_LICENSE_FILE` env var — an explicit path to the license file.
///   2. `license.key` in the app config directory (next to `settings.json`).
///
/// Any failure (missing file, no config dir, read error) yields `Ok(None)` so an
/// absent license never breaks startup — the open core stays fully functional.
#[tauri::command]
pub async fn read_license_file(app: AppHandle) -> Result<Option<String>, LoreError> {
    use tauri::Manager;

    let path: Option<PathBuf> = std::env::var_os("LOREGUI_LICENSE_FILE")
        .map(PathBuf::from)
        .or_else(|| {
            app.path()
                .app_config_dir()
                .ok()
                .map(|d| d.join("license.key"))
        });

    let Some(path) = path else {
        return Ok(None);
    };
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let trimmed = contents.trim();
            Ok(if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            })
        }
        Err(_) => Ok(None),
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn revision_activity_report(
    state: State<'_, AppState>,
    revision: String,
    branch: String,
    length: u32,
    author: String,
    date_from: u64,
    date_to: u64,
    file_path: String,
) -> Result<ActivityReportResult, LoreError> {
    let api = LoreApi::new(state.dir());
    op_revision_activity_report(
        &api,
        ActivityReportArgs {
            revision,
            branch,
            length,
            author,
            date_from,
            date_to,
            file_path,
        },
    )
    .await
}
