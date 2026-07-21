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
    /// An externally-supplied "advertised URL" override for the hosted server
    /// (SBAI-4072). The open core hosts on `127.0.0.1` only; this slot lets an
    /// external module (e.g. the proprietary cross-network *relay* overlay)
    /// register a publicly-reachable URL that the host UI displays *instead of*
    /// the loopback `url` — without the core knowing anything about how that URL
    /// is produced (no tunnel/bore logic lives here). `None` = no override, so
    /// the UI shows the real loopback URL. Cleared on stop.
    pub(crate) advertised_url: Mutex<Option<String>>,
    /// Local lock-coordination inbox (SBAI-4044). Holds incoming check-in
    /// requests addressed to *this* user. In the core/MIT build the sender and
    /// receiver share one process (single machine), so a request is delivered by
    /// pushing it here + firing a tray notification. Cross-*network* delivery
    /// rides the premium relay (SBAI-4072) — see `docs/lock-messaging-spike.md`.
    pub(crate) lock_inbox: Mutex<Vec<LockRequest>>,
    /// Monotonic id source for lock requests.
    pub(crate) lock_request_counter: AtomicU64,
    /// Live mDNS advertisement of the hosted server (SBAI-4073). `Some` only
    /// while a server is hosted; set on `host_server_start`, dropped (which
    /// unregisters the mDNS service) on `host_server_stop`. The open core
    /// advertises a `lore://host:port/<repo>` URL so LAN peers can find it.
    pub(crate) lan_announcer: Mutex<Option<crate::lan_discovery::Announcer>>,
    /// Live LAN browse session (SBAI-4073). `Some` while the connect/onboarding
    /// flow is discovering servers; lazily started by `lan_discover_browse` and
    /// torn down by `lan_discover_stop`. Pushes `lan/discovered` events.
    pub(crate) lan_browser: Mutex<Option<crate::lan_discovery::Browser>>,
}

/// A lock-coordination request: "please check in / release this file".
///
/// In the core build this is delivered to the local inbox. The same shape is
/// what the premium relay (SBAI-4072) would carry across the network — see
/// `lore_vm::ops::lock::file_message_send::FileMessageSendArgs` for the wire
/// contract the relay would use.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockRequest {
    /// Stable id for inbox dedupe / dismissal.
    pub id: String,
    /// File the lock applies to.
    pub path: String,
    /// Branch the lock is on (empty = current).
    pub branch: String,
    /// Display name of the requester (who is asking).
    pub from: String,
    /// User id of the holder this request is addressed to.
    pub to_user_id: String,
    /// Display name of the holder (for the inbox row).
    pub holder: String,
    /// Optional free-text note from the requester.
    #[serde(default)]
    pub note: String,
    /// Unix epoch millis when the request was created.
    pub created_at: u64,
}

impl AppState {
    pub(crate) fn dir(&self) -> PathBuf {
        self.working_dir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Allocate a new subscription ID and register it.
    pub(crate) fn next_subscription_id(&self) -> SubscriptionId {
        let id = self.subscription_counter.fetch_add(1, Ordering::SeqCst) + 1;
        self.subscriptions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id);
        id
    }

    /// Remove a subscription. Returns true if it existed, false if it was
    /// already gone (idempotent).
    pub(crate) fn remove_subscription(&self, id: SubscriptionId) -> bool {
        self.subscriptions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id)
    }
}

/// Drain the lore engine's deferred store writes to disk after a mutating op.
///
/// **Why (SBAI-4080 in the GUI).** The upstream `lore` engine flushes its
/// mutable store via a fire-and-forget background task spawned at the end of a
/// command. The CLI and FFI drivers await that flush by calling
/// [`lore_vm::finalize`] after every op, before tearing down their runtime — so
/// the staged-anchor pointer and friends actually land on disk. The Tauri GUI
/// is a *long-lived* driver: nothing here awaited that flush, so a stage /
/// commit / branch / lock mutation could return success to the UI while the
/// on-disk write was still racing — and was lost if the app was closed (or a
/// separate `lorevm` process, e.g. the VS Code extension, read the repo) before
/// the spawned task finished. That is silent data loss.
///
/// Every mutating command now calls this (via [`finalized`] for the ops-layer
/// commands, or directly for the `default_backend` ones) before returning, so
/// the GUI matches the CLI/FFI durability contract.
///
/// [`lore_vm::finalize`] now returns the flush [`Result`] (SBAI-4080): a flush
/// failure after a mutating op means the durable write may have been lost, so we
/// must NOT silently discard it. We can't fail the command on it here (the op's
/// own result is the contract the UI awaits, and this runs after mutating ops
/// only), but we surface it: a *benign* "no store open at this path" (read-only
/// op / non-repo dir) is logged at debug and tolerated, exactly as the CLI/FFI
/// treat it via [`lore_vm::is_benign_flush_failure`]; any other flush failure is
/// a possibly-lost write and is logged at `error` so it is never hidden.
async fn flush_working_tree(dir: PathBuf) {
    let api = LoreApi::new(dir);
    flush_api(&api).await;
}

/// Await [`lore_vm::finalize`] for an already-built `api` and consume the flush
/// [`Result`] (SBAI-4080) the same way [`flush_working_tree`] does: a benign
/// "no store open" is logged at debug and tolerated; any other flush failure is
/// a possibly-lost durable write and is logged at `error` so it is never hidden.
/// Used by the `default_backend`-style commands that hold a local `api` directly.
async fn flush_api(api: &LoreApi) {
    if let Err(e) = lore_vm::finalize(api).await {
        if lore_vm::is_benign_flush_failure(&e) {
            tracing::debug!(error = %e, "flush: benign flush failure (no store open); tolerated");
        } else {
            tracing::error!(
                error = %e,
                "flush: durable store flush FAILED after a mutating op — \
                 the on-disk write may have been lost (SBAI-4080 durability gap)"
            );
        }
    }
}

/// Shutdown backstop: flush the currently-open working tree's deferred store
/// writes to disk.
///
/// Called from the window-close / app-exit hooks in `lib.rs` so any staged or
/// committed work whose background flush had not yet landed (see
/// [`flush_working_tree`]) is made durable before the process goes away. The
/// per-command flushes cover the normal case; this catches anything a long-lived
/// session left in flight at quit time.
pub(crate) async fn flush_app_state(state: &AppState) {
    flush_working_tree(state.dir()).await;
}

/// Await [`lore_vm::finalize`] for `api`, then return the op's `result`.
///
/// Wrap a mutating ops-layer call as `finalized(&api, op(&api, args).await).await`
/// so its deferred store writes are durable before the command returns. The
/// flush runs on **both** the `Ok` and `Err` paths: a partially-applied mutation
/// must still be flushed so on-disk state is consistent. See
/// [`flush_working_tree`] for the full rationale.
///
/// `finalize` now returns the flush [`Result`] (SBAI-4080). We keep returning the
/// op's own `result` to the UI (that's the command contract), but we do **not**
/// discard a flush failure: a benign "no store open" is logged at debug and
/// tolerated, any other failure is a possibly-lost durable write and is logged at
/// `error` — matching the CLI/FFI treatment via
/// [`lore_vm::is_benign_flush_failure`].
async fn finalized<T>(api: &LoreApi, result: Result<T, LoreError>) -> Result<T, LoreError> {
    flush_api(api).await;
    result
}

/// Point the app at a different working tree (e.g. after a folder picker).
#[tauri::command]
pub fn open_repository(state: State<'_, AppState>, path: String) -> Result<(), LoreError> {
    *state.working_dir.lock().unwrap_or_else(|e| e.into_inner()) = PathBuf::from(path);
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
    let dir = state.dir();
    let result = default_backend(dir.clone()).stage(&paths).await;
    flush_working_tree(dir).await;
    result
}

#[tauri::command]
pub async fn unstage(state: State<'_, AppState>, paths: Vec<String>) -> Result<(), LoreError> {
    let dir = state.dir();
    let result = default_backend(dir.clone()).unstage(&paths).await;
    flush_working_tree(dir).await;
    result
}

#[tauri::command]
pub async fn commit(state: State<'_, AppState>, message: String) -> Result<String, LoreError> {
    let dir = state.dir();
    let result = default_backend(dir.clone()).commit(&message).await;
    flush_working_tree(dir).await;
    result
}

#[tauri::command]
pub async fn create_branch(state: State<'_, AppState>, name: String) -> Result<(), LoreError> {
    let dir = state.dir();
    let result = default_backend(dir.clone()).create_branch(&name).await;
    flush_working_tree(dir).await;
    result
}

#[tauri::command]
pub async fn switch_branch(state: State<'_, AppState>, name: String) -> Result<(), LoreError> {
    let dir = state.dir();
    let result = default_backend(dir.clone()).switch_branch(&name).await;
    flush_working_tree(dir).await;
    result
}

#[tauri::command]
pub async fn merge_branch(state: State<'_, AppState>, name: String) -> Result<(), LoreError> {
    let dir = state.dir();
    let result = default_backend(dir.clone()).merge_branch(&name).await;
    flush_working_tree(dir).await;
    result
}

#[tauri::command]
pub async fn push(state: State<'_, AppState>) -> Result<(), LoreError> {
    let dir = state.dir();
    let result = default_backend(dir.clone()).push().await;
    flush_working_tree(dir).await;
    result
}

#[tauri::command]
pub async fn sync(state: State<'_, AppState>) -> Result<(), LoreError> {
    let dir = state.dir();
    let result = default_backend(dir.clone()).sync().await;
    flush_working_tree(dir).await;
    result
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
    *state.working_dir.lock().unwrap_or_else(|e| e.into_inner()) = p.clone();
    flush_working_tree(p).await;
    Ok(id)
}

#[tauri::command]
pub async fn clone(state: State<'_, AppState>, url: String, dest: String) -> Result<(), LoreError> {
    let d = PathBuf::from(&dest);
    default_backend(state.dir()).clone(&url, d.clone()).await?;
    *state.working_dir.lock().unwrap_or_else(|e| e.into_inner()) = d.clone();
    flush_working_tree(d).await;
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
    finalized(
        &api,
        op_branch_protect(&api, BranchProtectArgs { branch }).await,
    )
    .await
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
    finalized(
        &api,
        op_branch_unprotect(&api, BranchUnprotectArgs { branch }).await,
    )
    .await
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
    finalized(
        &api,
        op_branch_archive(&api, BranchArchiveArgs { branch }).await,
    )
    .await
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
    finalized(
        &api,
        op_branch_merge_abort(&api, BranchMergeAbortArgs { link, ignore_links }).await,
    )
    .await
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
    finalized(
        &api,
        op_branch_merge_unresolve(&api, BranchMergeUnresolveArgs { paths }).await,
    )
    .await
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
    finalized(
        &api,
        op_file_obliterate(&api, FileObliterateArgs { address, path }).await,
    )
    .await
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
    finalized(
        &api,
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
        .await,
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
    finalized(
        &api,
        op_verify_state(&api, VerifyStateArgs { path, heal }).await,
    )
    .await
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
    finalized(
        &api,
        op_repository_delete(&api, DeleteArgs { repository_url }).await,
    )
    .await
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
    finalized(
        &api,
        op_repository_metadata_set(
            &api,
            RepositoryMetadataSetArgs {
                keys,
                values,
                formats,
            },
        )
        .await,
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
    finalized(&api, op_repository_gc(&api).await).await
}

// --- repository instance_prune ---

use lore_vm::ops::repository::instance_prune::{
    instance_prune as op_repository_instance_prune, InstancePruneResult,
};

#[tauri::command]
pub async fn repository_instance_prune(
    state: State<'_, AppState>,
) -> Result<InstancePruneResult, LoreError> {
    let api = LoreApi::new(state.dir());
    finalized(&api, op_repository_instance_prune(&api).await).await
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
    finalized(
        &api,
        op_revision_revert_local(
            &api,
            RevertLocalArgs {
                revision,
                message,
                no_commit,
            },
        )
        .await,
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
    finalized(
        &api,
        op_revision_revert_resolve(&api, RevertResolveArgs { paths }).await,
    )
    .await
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
    finalized(
        &api,
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
        .await,
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
    finalized(&api, op_link_remove(&api, RemoveArgs { link_path }).await).await
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
    finalized(
        &api,
        op_lock_file_release(
            &api,
            FileReleaseArgs {
                paths,
                branch,
                owner,
                owner_id,
            },
        )
        .await,
    )
    .await
}

// --- lock messaging (SBAI-4044): request check-in from a lock holder ---
//
// Transport verdict (see `docs/lock-messaging-spike.md`): lore's high-level
// `notification` crate carries only lock/branch lifecycle events — it has no
// `publish` and drops `ExtensionEvent`, so it cannot relay an arbitrary
// user→user message in-band. Cross-*network* delivery therefore needs the
// premium relay (SBAI-4072). The core/MIT build delivers **locally**: the sender
// and the holder share one process (single machine), so a request is delivered
// by pushing it onto `AppState.lock_inbox`, firing an OS tray notification, and
// emitting a `lock/request` event the UI listens for. This is a real, working
// end-to-end loop on one host; the one network seam is marked `TODO(relay)`.

use tauri::Emitter;

/// Frontend event channel for a newly-arrived lock request.
pub const LOCK_REQUEST_EVENT: &str = "lock/request";

fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Ask the holder of a file's lock to check it in / release it.
///
/// `from` is the requester's display name, `holder`/`to_user_id` identify the
/// current lock holder (already known to the caller from `lock_file_status` /
/// `lock_file_query`). Returns the created [`LockRequest`] (with its assigned
/// `id`) so the sender can show "request sent".
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn lock_request_checkin(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    branch: String,
    from: String,
    to_user_id: String,
    holder: String,
    note: String,
) -> Result<LockRequest, LoreError> {
    let id = state.lock_request_counter.fetch_add(1, Ordering::SeqCst) + 1;
    let request = LockRequest {
        id: format!("lockreq-{id}"),
        path,
        branch,
        from,
        to_user_id,
        holder,
        note,
        created_at: now_millis(),
    };

    // Local delivery: push onto the holder's inbox (same process in the core
    // build). TODO(SBAI-4072 relay): when the sender and holder are on different
    // machines, POST this same `LockRequest` to the relay side-channel instead
    // of (or in addition to) the local inbox — see docs/lock-messaging-spike.md.
    state
        .lock_inbox
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(request.clone());

    // Fire the OS tray notification + nudge the window so the holder sees it.
    crate::tray::notify_lock_request(&app, &request.from, &request.path);

    // Tell the frontend so the inbox badge / drawer updates live.
    let _ = app.emit(LOCK_REQUEST_EVENT, request.clone());

    Ok(request)
}

/// Return the current lock-request inbox (incoming check-in requests).
#[tauri::command]
pub fn lock_inbox_list(state: State<'_, AppState>) -> Vec<LockRequest> {
    state
        .lock_inbox
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Remove a lock request from the inbox (Dismiss, or after Release). Returns
/// `true` if it was present.
#[tauri::command]
pub fn lock_inbox_dismiss(state: State<'_, AppState>, id: String) -> bool {
    let mut inbox = state.lock_inbox.lock().unwrap_or_else(|e| e.into_inner());
    let before = inbox.len();
    inbox.retain(|r| r.id != id);
    inbox.len() != before
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
    finalized(
        &api,
        op_lock_file_acquire_as_owner(
            &api,
            FileAcquireAsOwnerArgs {
                paths,
                branch,
                owner,
            },
        )
        .await,
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
    finalized(
        &api,
        op_file_stage(
            &api,
            FileStageArgs {
                paths,
                case_change: case_change.unwrap_or_default(),
                scan: scan.unwrap_or(false),
            },
        )
        .await,
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
    finalized(&api, op_file_dirty(&api, FileDirtyArgs { paths }).await).await
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
    finalized(
        &api,
        op_file_dirty_copy(&api, FileDirtyCopyArgs { from_path, to_path }).await,
    )
    .await
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
    finalized(
        &api,
        op_file_dirty_move(&api, FileDirtyMoveArgs { from_path, to_path }).await,
    )
    .await
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
    finalized(
        &api,
        op_file_reset_to_last_merged(
            &api,
            FileResetToLastMergedArgs {
                paths,
                branch,
                purge,
            },
        )
        .await,
    )
    .await
}

// --- file diff ---

use lore_vm::ops::file::diff::{diff as op_file_diff, DiffArgs, FileDiffEntry};

// Arg count is dictated by the frontend IPC contract (one named field per
// diff option), not freely reducible into a struct here.
#[allow(clippy::too_many_arguments)]
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

// Arg count is dictated by the frontend IPC contract (one named field per sync
// option), not freely reducible into a struct here.
#[allow(clippy::too_many_arguments)]
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
    finalized(
        &api,
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
        .await,
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
    finalized(&api, op_revision_amend(&api, AmendArgs { message }).await).await
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
    finalized(
        &api,
        op_revision_commit(&api, OpsCommitArgs { message }).await,
    )
    .await
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
    finalized(
        &api,
        op_lock_file_acquire(&api, FileAcquireArgs { paths, branch }).await,
    )
    .await
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
    finalized(
        &api,
        op_branch_create(
            &api,
            BranchCreateArgs {
                branch,
                category,
                id,
            },
        )
        .await,
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
    finalized(
        &api,
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
        .await,
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
    finalized(
        &api,
        op_branch_merge_restart(&api, BranchMergeRestartArgs { paths }).await,
    )
    .await
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
    finalized(
        &api,
        op_branch_merge_resolve_theirs(&api, BranchMergeResolveTheirsArgs { paths }).await,
    )
    .await
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
    finalized(
        &api,
        op_branch_merge_resolve_mine(&api, BranchMergeResolveMineArgs { paths }).await,
    )
    .await
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
    finalized(
        &api,
        op_branch_reset(&api, BranchResetArgs { revision, branch }).await,
    )
    .await
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
    finalized(
        &api,
        op_branch_merge_resolve(&api, BranchMergeResolveArgs { paths }).await,
    )
    .await
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
        *state.working_dir.lock().unwrap_or_else(|e| e.into_inner()) = PathBuf::from(p);
    }
    let api = LoreApi::new(state.dir());
    finalized(
        &api,
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
        .await,
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

    let mut session = state
        .storage_session
        .lock()
        .unwrap_or_else(|e| e.into_inner());
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
        let session = state
            .storage_session
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
    .await;
    flush_api(&api).await;
    let result = result?;

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
        let session = state
            .storage_session
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let session = state
            .storage_session
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
    let result = op_storage_obliterate(
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
    .await;
    flush_api(&api).await;
    result?;

    state
        .storage_session
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .keys
        .remove(&key);
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
    let mut session = state
        .storage_session
        .lock()
        .unwrap_or_else(|e| e.into_inner());
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
    let mut session = state
        .storage_session
        .lock()
        .unwrap_or_else(|e| e.into_inner());
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
    finalized(
        &api,
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
        .await,
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
    finalized(
        &api,
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
        .await,
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
    finalized(
        &api,
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
        .await,
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
    finalized(
        &api,
        op_shared_store_set_use_automatically(&api, SetUseAutomaticallyArgs { enabled }).await,
    )
    .await
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
    .await;
    flush_api(&api).await;
    let result = result?;
    Ok(result.path)
}

// --- onboarding: local host-store prepare / probe ---
//
// The first-run "Host a server" flow with a LOCAL filesystem backend does NOT
// use the lore repository/remote storage abstraction. Its store is a plain
// directory the standalone `loreserver` fills with its content-addressed
// `immutable/` + `mutable/` layout at launch (see `server_host`). These commands
// give the wizard a local-FS-native "prepare store" (steps 1 + 3) and "validate
// connectivity" (step 2) that never require an existing `.lore` repository or a
// remote URL — the bug that made `storage_open` ("missing .lore") and
// `shared_store_create` ("no remote URL") fail for a brand-new local host.

/// Ensure a local host-store directory (and an optional separate mutable-store
/// directory) exists; returns the resolved store path. Idempotent.
#[tauri::command]
pub fn host_store_prepare(
    path: String,
    mutable_store: Option<String>,
) -> Result<String, LoreError> {
    server_host::prepare_local_store(&path, mutable_store.as_deref())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Round-trip writability probe for a local host-store directory (write → read →
/// delete a probe file). The local-FS equivalent of the connectivity check.
#[tauri::command]
pub fn host_store_probe(path: String) -> Result<(), LoreError> {
    server_host::probe_local_store(&path)
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
    let result = op_repository_clone(
        &api,
        CloneArgs {
            repository_url: url,
            ..Default::default()
        },
    )
    .await;
    flush_api(&api).await;
    result?;
    *state.working_dir.lock().unwrap_or_else(|e| e.into_inner()) = dest_path;
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
    .await;
    flush_api(&api).await;
    let result = result?;
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
    .await;
    flush_api(&api).await;
    let result = result?;
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
    finalized(
        &api,
        op_dependency_add(&api, DependencyAddArgs { sources, force }).await,
    )
    .await
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
    finalized(
        &api,
        op_dependency_remove(&api, DependencyRemoveArgs { sources }).await,
    )
    .await
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
    finalized(
        &api,
        op_cherry_pick_restart(&api, CherryPickRestartArgs { paths }).await,
    )
    .await
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
    app: AppHandle,
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

    // Build cert context from Tauri path resolver (SBAI-4087). Best-effort:
    // a failure to resolve app_data_dir degrades to the bundled fallback.
    let cert_ctx = {
        use tauri::Manager;
        server_host::CertContext {
            app_data_dir: app.path().app_data_dir().ok(),
            resource_dir: app.path().resource_dir().ok(),
        }
    };

    let mut slot = state
        .hosted_server
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let status = server_host::start(&mut slot, &opts, &cert_ctx)?;

    // Advertise the freshly-started server on the LAN (SBAI-4073) so peers can
    // discover it without copying a URL by hand. The connect URL clients dial is
    // the loopback `lore://` the server reports; we re-point its host at the
    // primary LAN IPv4 so a *remote* peer can actually reach it (the loopback
    // 127.0.0.1 in `status.url` is only valid on the host machine). Best-effort:
    // a failure to announce (e.g. firewall-blocked multicast) never fails the
    // host start — the manual connect path always remains.
    if let Some(url) = status.url.as_deref() {
        let lan_ip = crate::lan_discovery::primary_lan_ipv4();
        let connect_url = lan_advertise_url(url, lan_ip);
        let repo = opts.repository_name.clone().unwrap_or_default();
        let friendly = lan_friendly_name(&repo);
        let port = status.port.unwrap_or(0);
        match crate::lan_discovery::Announcer::start(&connect_url, &repo, &friendly, port, lan_ip) {
            Ok(announcer) => {
                *state
                    .lan_announcer
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(announcer);
            }
            Err(e) => {
                tracing::warn!(error = %e, "LAN announce failed; server still hosted");
            }
        }
    }

    Ok(status)
}

/// Rewrite a hosted server's loopback `lore://127.0.0.1:port[/repo]` URL so its
/// host is the machine's primary LAN IPv4, producing the URL a *remote* peer
/// dials. If we couldn't determine a LAN IP, the original (loopback) URL is left
/// as-is — discovery still surfaces the server, and a same-machine client can
/// still use it. Pure string substitution on the host portion; the rest (scheme,
/// port, path) is preserved verbatim.
fn lan_advertise_url(loopback_url: &str, lan_ip: Option<std::net::Ipv4Addr>) -> String {
    let Some(ip) = lan_ip else {
        return loopback_url.to_string();
    };
    // lore://<host>:<port>/<path> — replace only the <host> between "://" and ":".
    if let Some(rest) = loopback_url.strip_prefix("lore://") {
        if let Some((hostport, tail)) = rest.split_once(':') {
            let _ = hostport; // host we are replacing
            return format!("lore://{ip}:{tail}");
        }
    }
    loopback_url.to_string()
}

/// A human-facing label for this host's LAN advertisement. Uses the OS hostname
/// (what a teammate recognises), optionally suffixed with the repo so two repos
/// on one machine are distinguishable. Falls back to "LoreGUI server".
fn lan_friendly_name(repo: &str) -> String {
    let host = hostname_label();
    match repo.trim() {
        "" => host,
        r => format!("{host} · {r}"),
    }
}

/// Best-effort OS hostname, trimmed; "LoreGUI server" if unavailable.
fn hostname_label() -> String {
    if_addrs_hostname().unwrap_or_else(|| "LoreGUI server".to_string())
}

/// Resolve the machine hostname without pulling an extra crate: read the
/// platform's hostname via std where available, else an env fallback.
fn if_addrs_hostname() -> Option<String> {
    // `HOSTNAME` (most Linux shells) / `COMPUTERNAME` (Windows) are the most
    // portable no-dep sources. Trim and reject empties.
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
}

/// Render the `loreserver` config TOML for the given options **without** writing
/// anything to disk or starting a server (SBAI-4075). Backs the host flow's
/// "View generated config" affordance. Accepts the full advanced-config bag as a
/// single typed `opts` object (unlike `host_server_start`'s flat args, the
/// preview is only ever driven from the host UI, not the command palette, so a
/// nested object is the cleaner surface). Returns the generated TOML string, or
/// a validation error (bad enum / out-of-range / required-when-mode).
#[tauri::command]
pub fn host_server_render_config(
    app: AppHandle,
    opts: HostServerOptions,
) -> Result<String, LoreError> {
    // For a config preview we try to resolve the cert (so the preview shows real
    // paths), but fall back to placeholders if no cert exists yet (allow_missing=true
    // is set inside render_config). Best-effort: a path resolver error just means
    // the preview shows placeholder cert paths, which is fine.
    use tauri::Manager;
    let ctx = server_host::CertContext {
        app_data_dir: app.path().app_data_dir().ok(),
        resource_dir: app.path().resource_dir().ok(),
    };
    server_host::render_config(&opts, &ctx)
}

/// Stop the hosted `loreserver` (kill + reap). Idempotent.
///
/// Also clears any externally-registered advertised-URL override (SBAI-4072):
/// once the server is down, a stale public URL would point clients at nothing,
/// and the relay overlay tears its own tunnel down on stop. The core owns this
/// lifecycle invariant so the override can never outlive the server.
#[tauri::command]
pub fn host_server_stop(state: State<'_, AppState>) -> Result<HostStatus, LoreError> {
    let mut slot = state
        .hosted_server
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let status = server_host::stop(&mut slot)?;
    *state
        .advertised_url
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    // Drop the LAN advertisement (SBAI-4073): dropping the Announcer unregisters
    // the mDNS service so peers stop seeing a server that is no longer running.
    *state
        .lan_announcer
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    Ok(status)
}

/// Current hosted-server status (running? + URL/port/pid). Reaps if it died.
///
/// Overlays any externally-registered advertised URL (SBAI-4072) onto the
/// returned status' `advertisedUrl` field. The real loopback `url` is always
/// preserved; the override is purely additive display metadata. If the server is
/// not running, no override is surfaced (and any stale one is dropped).
#[tauri::command]
pub fn host_server_status(state: State<'_, AppState>) -> Result<HostStatus, LoreError> {
    let mut slot = state
        .hosted_server
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let status = server_host::status(&mut slot);
    if !status.running {
        // Server is down — a stale advertised URL is meaningless; drop it.
        *state
            .advertised_url
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        return Ok(status);
    }
    let advertised = state
        .advertised_url
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Ok(status.with_advertised_url(advertised.as_deref()))
}

/// Register an externally-supplied "advertised URL" for the hosted server
/// (SBAI-4072) — the open-core seam that lets a premium cross-network *relay*
/// overlay surface a publicly-reachable URL in the host UI.
///
/// This is deliberately generic: the core stores the string and echoes it back
/// through [`host_server_status`]. It contains NO tunnel/bore/relay logic — how
/// the URL is produced is entirely the caller's concern. A blank string clears
/// the override (equivalent to [`host_server_clear_advertised_url`]). Setting it
/// while no server is running is allowed but inert: `host_server_status` only
/// surfaces the override while the server is up.
#[tauri::command]
pub fn host_server_set_advertised_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<(), LoreError> {
    let trimmed = url.trim();
    *state
        .advertised_url
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    };
    Ok(())
}

/// Clear any externally-registered advertised-URL override (SBAI-4072). After
/// this, `host_server_status` reports only the real loopback `url`. The relay
/// overlay calls this when it tears its tunnel down.
#[tauri::command]
pub fn host_server_clear_advertised_url(state: State<'_, AppState>) -> Result<(), LoreError> {
    *state
        .advertised_url
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
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
    finalized(
        &api,
        op_auth_logout(
            &api,
            LogoutArgs {
                auth_url,
                resource,
                user_id,
            },
        )
        .await,
    )
    .await
}

#[tauri::command]
pub async fn auth_clear(state: State<'_, AppState>) -> Result<(), LoreError> {
    let api = LoreApi::new(state.dir());
    finalized(&api, op_auth_clear(&api, ClearArgs {}).await).await
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
    finalized(&api, op_repository_release(&api).await).await
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
    finalized(
        &api,
        op_repository_metadata_clear(&api, RepositoryMetadataClearArgs { keys }).await,
    )
    .await
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
    finalized(
        &api,
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
        .await,
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
    finalized(&api, op_repository_update_path(&api).await).await
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
    finalized(
        &api,
        op_revision_revert_abort(&api, RevertAbortArgs {}).await,
    )
    .await
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
    finalized(
        &api,
        op_revision_revert_resolve_mine(&api, RevertResolveMineArgs { paths }).await,
    )
    .await
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
    finalized(
        &api,
        op_revision_commit_with_metadata(
            &api,
            CommitWithMetadataArgs {
                message,
                keys,
                values,
                formats,
            },
        )
        .await,
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
    finalized(
        &api,
        op_revision_metadata_clear(&api, RevisionMetadataClearArgs {}).await,
    )
    .await
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
    finalized(
        &api,
        op_layer_add(
            &api,
            LayerAddArgs {
                target_path,
                source_repository,
                source_path,
                metadata,
            },
        )
        .await,
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

// ---------------------------------------------------------------------------
// Working-tree file I/O for the content workspace (SBAI-4083 / 4084 / 4085).
//
// The Preview / Diff / Edit surface needs to read and write the *bytes on
// disk* in the currently-open working tree — distinct from `file_write`, which
// materialises lore *content* (a revision/address) to a path. These three
// commands are plain filesystem helpers scoped to the open repo's working
// directory; they never touch the lore store. They are UI-only helpers (no
// lore op), so they are `excluded` from the palette parity gate like
// `read_license_file` / `tray_sync_state`.
// ---------------------------------------------------------------------------

/// Maximum bytes a single read returns to the frontend (16 MiB). Larger files
/// fall back to metadata-only preview / read-only editor on the UI side.
pub const MAX_READ_BYTES: u64 = 16 * 1024 * 1024;

/// Resolve a repo-relative `path` against the open working tree and reject any
/// path that escapes it.
///
/// SECURITY (path-traversal / arbitrary host file read+write -> RCE): this is
/// the *sole* containment check for the working-tree IPC commands
/// (`read_text_file` / `read_file_bytes` / `write_text_file`), which are
/// reachable from the webview. It MUST treat every input as untrusted and never
/// resolve outside `root`. The hardening here is:
///
///  1. **Absolute inputs are rejected.** The UI only ever passes repo-relative
///     paths; accepting an absolute path verbatim (the old behaviour) let the
///     webview read/write anywhere on the host (`/etc/passwd`, `~/.ssh/...`).
///  2. **Containment is enforced unconditionally.** The old guard was gated on
///     `&& !candidate.is_absolute()`, and since a re-rooted relative path is
///     always absolute, the guard never fired — so `../../../etc/passwd`
///     normalised straight to `/etc/passwd`. We now always verify the result
///     stays within `root`.
///  3. **Symlink escapes are defeated** by canonicalising the deepest existing
///     ancestor and re-checking containment against the canonical root. Done
///     safely so a not-yet-existing target (a fresh write) still resolves.
fn resolve_in_working_tree(root: &std::path::Path, path: &str) -> Result<PathBuf, LoreError> {
    let escapes = || LoreError::CommandFailed(format!("path escapes the working tree: {path}"));

    let requested = PathBuf::from(path);
    // (1) Reject absolute inputs outright. A working-tree path is always
    // repo-relative; an absolute path is either an attack or a UI bug, and we
    // must never resolve it verbatim.
    if requested.is_absolute() {
        return Err(escapes());
    }
    // Reject Windows path prefixes / drive-relative roots too (e.g. `C:foo`,
    // `\\server\share`). On non-Windows these never appear; on Windows they
    // would otherwise sneak past the `is_absolute()` check for some forms.
    if requested.components().any(|c| {
        matches!(
            c,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        )
    }) {
        return Err(escapes());
    }

    let candidate = root.join(&requested);

    // Normalise `.`/`..` lexically (the file may not exist yet for writes, so we
    // can't rely on `canonicalize` for the whole path). Reject any component
    // that climbs above root.
    let mut normalised = PathBuf::new();
    for comp in candidate.components() {
        use std::path::Component::*;
        match comp {
            ParentDir => {
                if !normalised.pop() {
                    return Err(escapes());
                }
            }
            CurDir => {}
            other => normalised.push(other.as_os_str()),
        }
    }

    // The lexically-normalised path must stay within the lexically-normalised
    // root. This now runs UNCONDITIONALLY (the old `&& !candidate.is_absolute()`
    // clause made it dead code).
    let root_norm: PathBuf = {
        let mut r = PathBuf::new();
        for comp in root.components() {
            r.push(comp.as_os_str());
        }
        r
    };
    if !normalised.starts_with(&root_norm) {
        return Err(escapes());
    }

    // (3) Defeat symlink-based escapes: canonicalise the root and the deepest
    // existing ancestor of the target, then re-verify containment. The target
    // itself may not exist yet (fresh write), so we canonicalise the nearest
    // existing ancestor and append the remaining (not-yet-created) tail.
    if let Ok(canon_root) = root.canonicalize() {
        let mut existing = normalised.as_path();
        let mut tail: Vec<std::ffi::OsString> = Vec::new();
        let canon_existing = loop {
            match existing.canonicalize() {
                Ok(c) => break c,
                Err(_) => match existing.parent() {
                    Some(parent) => {
                        if let Some(name) = existing.file_name() {
                            tail.push(name.to_os_string());
                        }
                        existing = parent;
                    }
                    // Walked off the top without finding an existing ancestor;
                    // the lexical check above already vouches for this path.
                    None => return Ok(normalised),
                },
            }
        };
        // The deepest existing real directory must itself be inside the real
        // root (this is what catches a symlinked subdir pointing outside).
        if !canon_existing.starts_with(&canon_root) {
            return Err(escapes());
        }
        let mut resolved = canon_existing;
        for name in tail.into_iter().rev() {
            resolved.push(name);
        }
        return Ok(resolved);
    }

    Ok(normalised)
}

/// Metadata-only view of a working-tree file (used when content is too large or
/// binary to embed). Mirrors the fields the content workspace renders.
#[derive(serde::Serialize)]
pub struct WorkingFileMeta {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub exists: bool,
}

/// Result of a working-tree text read.
#[derive(serde::Serialize)]
pub struct ReadTextFileResult {
    pub path: String,
    pub content: String,
    pub size: u64,
    /// True when the file exceeded `MAX_READ_BYTES` and `content` is empty.
    pub too_large: bool,
}

/// Result of a working-tree byte read (base64-encoded for transport).
#[derive(serde::Serialize)]
pub struct ReadFileBytesResult {
    pub path: String,
    /// Standard base64 of the file bytes; empty when `too_large`.
    pub base64: String,
    pub size: u64,
    /// Best-effort MIME type derived from the extension.
    pub mime: String,
    pub too_large: bool,
}

/// Read a working-tree file as UTF-8 text. Lossy-decodes so binary files still
/// return *something* the editor/preview can show (callers gate on MIME first).
#[tauri::command]
pub async fn read_text_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<ReadTextFileResult, LoreError> {
    let abs = resolve_in_working_tree(&state.dir(), &path)?;
    let meta = std::fs::metadata(&abs)
        .map_err(|e| LoreError::CommandFailed(format!("stat {path}: {e}")))?;
    let size = meta.len();
    if size > MAX_READ_BYTES {
        return Ok(ReadTextFileResult {
            path,
            content: String::new(),
            size,
            too_large: true,
        });
    }
    let bytes =
        std::fs::read(&abs).map_err(|e| LoreError::CommandFailed(format!("read {path}: {e}")))?;
    Ok(ReadTextFileResult {
        path,
        content: String::from_utf8_lossy(&bytes).into_owned(),
        size,
        too_large: false,
    })
}

/// Read a working-tree file as raw bytes, base64-encoded for the frontend
/// (images / audio / 3D models load this into a Blob URL).
#[tauri::command]
pub async fn read_file_bytes(
    state: State<'_, AppState>,
    path: String,
) -> Result<ReadFileBytesResult, LoreError> {
    use base64::Engine;
    let abs = resolve_in_working_tree(&state.dir(), &path)?;
    let meta = std::fs::metadata(&abs)
        .map_err(|e| LoreError::CommandFailed(format!("stat {path}: {e}")))?;
    let size = meta.len();
    let mime = mime_for_path(&path);
    if size > MAX_READ_BYTES {
        return Ok(ReadFileBytesResult {
            path,
            base64: String::new(),
            size,
            mime,
            too_large: true,
        });
    }
    let bytes =
        std::fs::read(&abs).map_err(|e| LoreError::CommandFailed(format!("read {path}: {e}")))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(ReadFileBytesResult {
        path,
        base64: b64,
        size,
        mime,
        too_large: false,
    })
}

/// Write UTF-8 `content` to a working-tree file (the editor's Save). Creates
/// parent directories as needed. Returns the resulting metadata.
#[tauri::command]
pub async fn write_text_file(
    state: State<'_, AppState>,
    path: String,
    content: String,
) -> Result<WorkingFileMeta, LoreError> {
    let abs = resolve_in_working_tree(&state.dir(), &path)?;
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| LoreError::CommandFailed(format!("mkdir for {path}: {e}")))?;
    }
    std::fs::write(&abs, content.as_bytes())
        .map_err(|e| LoreError::CommandFailed(format!("write {path}: {e}")))?;
    let meta = std::fs::metadata(&abs)
        .map_err(|e| LoreError::CommandFailed(format!("stat {path}: {e}")))?;
    Ok(WorkingFileMeta {
        path,
        size: meta.len(),
        is_dir: meta.is_dir(),
        exists: true,
    })
}

/// Best-effort MIME type from a file extension — enough for the content
/// workspace to pick a renderer (image / audio / model / text / unknown).
fn mime_for_path(path: &str) -> String {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let m = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "usdz" => "model/vnd.usdz+zip",
        "usd" | "usda" => "text/plain",
        "usdc" => "application/octet-stream",
        "fbx" => "application/octet-stream",
        "obj" => "text/plain",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    };
    m.to_string()
}

// --- LAN auto-discovery of lore servers (SBAI-4073) -----------------------
//
// Open-core, MIT, NOT gated: dynamic mDNS discovery, the same pattern
// studiobrain-model-manager uses for gateway clustering. The host half lives in
// `host_server_start` / `host_server_stop` above (it owns the `Announcer`); these
// commands are the client/browse half consumed by the connect/onboarding flow.

use crate::lan_discovery::{Browser, DiscoveredServer, LAN_DISCOVERED_EVENT};

/// Start (or reuse) a LAN browse session and return the servers seen so far.
///
/// Idempotent: the first call spawns a background mDNS browse that keeps running
/// and pushes `lan/discovered` events as servers appear/leave; subsequent calls
/// just return the current snapshot (so the UI can render immediately on mount
/// while the event stream keeps it live). Call `lan_discover_stop` to tear the
/// browse down when leaving the connect flow.
///
/// Returns the discovered servers sorted by name. An empty list is normal — the
/// UI shows a "no servers found yet" empty state and refreshes as events arrive.
#[tauri::command]
pub fn lan_discover_browse(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<DiscoveredServer>, LoreError> {
    let mut guard = state.lan_browser.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let app_for_events = app.clone();
        let browser = Browser::start(move |servers| {
            // Push the live list to the frontend; failure to emit is non-fatal
            // (the next poll/refresh still reflects state).
            use tauri::Emitter;
            let _ = app_for_events.emit(LAN_DISCOVERED_EVENT, servers);
        })
        .map_err(LoreError::Client)?;
        *guard = Some(browser);
    }
    Ok(guard.as_ref().map(Browser::snapshot).unwrap_or_default())
}

/// Return the current LAN discovery snapshot without (re)starting a browse.
///
/// Backs the connect flow's manual "Refresh" affordance: it returns whatever the
/// running browse has accumulated. If no browse is active it returns an empty
/// list (the UI should call `lan_discover_browse` to start one).
#[tauri::command]
pub fn lan_discover_refresh(
    state: State<'_, AppState>,
) -> Result<Vec<DiscoveredServer>, LoreError> {
    let guard = state.lan_browser.lock().unwrap_or_else(|e| e.into_inner());
    Ok(guard.as_ref().map(Browser::snapshot).unwrap_or_default())
}

/// Stop the LAN browse session and release its mDNS daemon.
///
/// Called when the connect/onboarding flow unmounts so we are not browsing in
/// the background for the whole app lifetime. Idempotent — a no-op if no browse
/// is running.
#[tauri::command]
pub fn lan_discover_stop(state: State<'_, AppState>) -> Result<(), LoreError> {
    *state.lan_browser.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

#[cfg(test)]
mod working_tree_path_tests {
    //! Regression tests for the working-tree path-traversal fix.
    //!
    //! The IPC commands `read_text_file` / `read_file_bytes` / `write_text_file`
    //! are reachable from the webview and all funnel through
    //! `resolve_in_working_tree`. These tests lock down its containment guard so
    //! the path-traversal -> arbitrary host read/write -> RCE class of bug can
    //! never regress.

    use super::resolve_in_working_tree;
    use std::path::Path;

    /// Inputs that MUST be rejected as escaping the working tree.
    #[test]
    fn rejects_escaping_inputs() {
        let root = Path::new("/srv/repo");
        let escaping = [
            "/etc/passwd",              // absolute, taken verbatim by the old code
            "../../../etc/passwd",      // classic relative traversal
            "sub/../../repo2/secret",   // climbs out via an interior `..`
            "/srv/repo-sibling/secret", // absolute path to a sibling dir
            "..",                       // bare parent
            "../",                      // parent with trailing sep
            "a/../../b",                // net-negative traversal
            "/srv/repo/../escape",      // absolute, but resolves outside
        ];
        for p in escaping {
            assert!(
                resolve_in_working_tree(root, p).is_err(),
                "expected `{p}` to be rejected as escaping the working tree, but it resolved Ok"
            );
        }
    }

    /// Legitimate in-repo relative paths must still resolve, rooted under `root`.
    #[test]
    fn accepts_in_repo_relative_paths() {
        let root = Path::new("/srv/repo");
        let ok_cases = [
            ("file.md", "/srv/repo/file.md"),
            ("sub/dir/file.png", "/srv/repo/sub/dir/file.png"),
            ("./file.md", "/srv/repo/file.md"),
            ("sub/../file.md", "/srv/repo/file.md"), // `..` that stays inside is fine
            ("a/b/../c/file.txt", "/srv/repo/a/c/file.txt"),
        ];
        for (input, expected) in ok_cases {
            let resolved = resolve_in_working_tree(root, input)
                .unwrap_or_else(|_| panic!("expected `{input}` to resolve Ok"));
            assert_eq!(
                resolved,
                Path::new(expected),
                "in-repo path `{input}` resolved to the wrong location"
            );
        }
    }

    /// An absolute path that happens to point *inside* the root is still
    /// rejected: working-tree paths are repo-relative by contract, and accepting
    /// any absolute form re-opens the verbatim-absolute hole.
    #[test]
    fn rejects_absolute_even_when_inside_root() {
        let root = Path::new("/srv/repo");
        assert!(resolve_in_working_tree(root, "/srv/repo/file.md").is_err());
    }

    /// End-to-end against a real temp dir: a relative path resolves into the
    /// real root, while traversal out of it is rejected.
    #[test]
    fn resolves_against_real_directory() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/file.md"), b"hi").unwrap();

        let resolved = resolve_in_working_tree(root, "sub/file.md").expect("resolves in-repo");
        // Canonicalisation may resolve `/tmp` symlinks etc; just assert the
        // result lives under the canonical root and points at our file.
        let canon_root = root.canonicalize().unwrap();
        assert!(resolved.starts_with(&canon_root));
        assert_eq!(std::fs::read(&resolved).unwrap(), b"hi");

        assert!(resolve_in_working_tree(root, "../../../etc/passwd").is_err());
    }

    /// Symlink escape: a symlink *inside* the root that points outside must not
    /// allow reads/writes to the symlink target's real location.
    #[cfg(unix)]
    #[test]
    fn defeats_symlink_escape() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path().join("repo");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret"), b"top secret").unwrap();

        // `repo/link` -> `../outside` (a directory symlink escaping the root).
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();

        // Reading through the symlink to the outside secret must be rejected.
        assert!(
            resolve_in_working_tree(&root, "link/secret").is_err(),
            "symlinked path escaping the root must be rejected"
        );
    }
}
