//! Tauri command layer. Each command is a thin wrapper that builds a backend fo
//! the currently-open working directory and forwards to `lore-vm`. No business
//! logic lives here — that's the whole point of the lore-vm seam.

use lore_vm::{default_backend, Branch, LoreApi, LoreError, RepoStatus, Revision};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::State;

use crate::operations::SubscriptionId;

/// The only mutable app state: which working tree we're looking at, and
/// notification subscription tracking.
pub struct AppState {
    pub working_dir: Mutex<PathBuf>,
    /// Monotonically increasing counter for subscription IDs.
    pub(crate) subscription_counter: AtomicU64,
    /// Currently active subscription IDs.
    pub(crate) subscriptions: Mutex<HashSet<SubscriptionId>>,
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
