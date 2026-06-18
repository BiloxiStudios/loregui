//! Tauri command layer. Each command is a thin wrapper that builds a backend for
//! the currently-open working directory and forwards to `lore-vm`. No business
//! logic lives here — that's the whole point of the lore-vm seam.

use lore_vm::{
    default_backend, Branch, ConfigValue, InstanceList, InstancePruneResult,
    ImmutableQueryResult, LoreError, MetadataEntry, RepoCreateResult, RepoDump,
    RepoInfo, RepoListing, RepoStatus, Revision, VerifyFragmentResult,
    VerifyStateResult,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

/// The only mutable app state: which working tree we're looking at.
pub struct AppState {
    pub working_dir: Mutex<PathBuf>,
}

impl AppState {
    fn dir(&self) -> PathBuf {
        self.working_dir.lock().unwrap().clone()
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
    let id = default_backend(state.dir()).create_repository(p.clone(), &name).await?;
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

// ===== Repository domain (21 ops) =====

#[tauri::command]
pub async fn repo_info(state: State<'_, AppState>) -> Result<RepoInfo, LoreError> {
    default_backend(state.dir()).repo_info().await
}

#[tauri::command]
pub async fn repo_dump(
    state: State<'_, AppState>,
    format: Option<String>,
) -> Result<RepoDump, LoreError> {
    default_backend(state.dir())
        .repo_dump(format.as_deref())
        .await
}

#[tauri::command]
pub async fn repo_create(
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
pub async fn repo_create_with_metadata(
    state: State<'_, AppState>,
    path: String,
    name: String,
    metadata: HashMap<String, String>,
) -> Result<RepoCreateResult, LoreError> {
    let p = PathBuf::from(&path);
    let result = default_backend(state.dir())
        .repo_create_with_metadata(p.clone(), &name, metadata)
        .await?;
    *state.working_dir.lock().unwrap() = p;
    Ok(result)
}

#[tauri::command]
pub async fn repo_delete(state: State<'_, AppState>, path: String) -> Result<(), LoreError> {
    let p = PathBuf::from(&path);
    default_backend(state.dir()).repo_delete(p).await
}

#[tauri::command]
pub async fn repo_release(state: State<'_, AppState>) -> Result<(), LoreError> {
    default_backend(state.dir()).repo_release().await
}

#[tauri::command]
pub async fn repo_flush(state: State<'_, AppState>) -> Result<(), LoreError> {
    default_backend(state.dir()).repo_flush().await
}

#[tauri::command]
pub async fn repo_gc(
    state: State<'_, AppState>,
    aggressive: bool,
) -> Result<u64, LoreError> {
    default_backend(state.dir()).repo_gc(aggressive).await
}

#[tauri::command]
pub async fn repo_list(state: State<'_, AppState>) -> Result<Vec<RepoListing>, LoreError> {
    default_backend(state.dir()).repo_list().await
}

#[tauri::command]
pub async fn repo_verify_state(
    state: State<'_, AppState>,
) -> Result<VerifyStateResult, LoreError> {
    default_backend(state.dir()).repo_verify_state().await
}

#[tauri::command]
pub async fn repo_verify_fragment(
    state: State<'_, AppState>,
    fragment_hash: String,
) -> Result<VerifyFragmentResult, LoreError> {
    default_backend(state.dir())
        .repo_verify_fragment(&fragment_hash)
        .await
}

#[tauri::command]
pub async fn repo_store_immutable_query(
    state: State<'_, AppState>,
    query: String,
) -> Result<ImmutableQueryResult, LoreError> {
    default_backend(state.dir())
        .repo_store_immutable_query(&query)
        .await
}

#[tauri::command]
pub async fn repo_metadata_get(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<MetadataEntry>, LoreError> {
    default_backend(state.dir()).repo_metadata_get(&key).await
}

#[tauri::command]
pub async fn repo_metadata_set(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), LoreError> {
    default_backend(state.dir())
        .repo_metadata_set(&key, &value)
        .await
}

#[tauri::command]
pub async fn repo_metadata_clear(state: State<'_, AppState>) -> Result<(), LoreError> {
    default_backend(state.dir()).repo_metadata_clear().await
}

#[tauri::command]
pub async fn repo_instance_list(
    state: State<'_, AppState>,
) -> Result<InstanceList, LoreError> {
    default_backend(state.dir()).repo_instance_list().await
}

#[tauri::command]
pub async fn repo_instance_prune(
    state: State<'_, AppState>,
) -> Result<InstancePruneResult, LoreError> {
    default_backend(state.dir()).repo_instance_prune().await
}

#[tauri::command]
pub async fn repo_update_path(
    state: State<'_, AppState>,
    new_path: String,
) -> Result<(), LoreError> {
    let p = PathBuf::from(&new_path);
    default_backend(state.dir()).repo_update_path(p).await
}

#[tauri::command]
pub async fn repo_config_get(
    state: State<'_, AppState>,
    key: String,
) -> Result<ConfigValue, LoreError> {
    default_backend(state.dir()).repo_config_get(&key).await
}
