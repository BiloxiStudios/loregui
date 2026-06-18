//! The seam every GUI/host talks to. One async trait covering the full Lore
//! verb set the CLI exposes (`repository create`, `stage`, `status`, `commit`,
//! `push`, `clone`, `branch create/switch/merge`, `sync`, `shared-store create`).
//!
//! Two implementations ship:
//!   * [`crate::cli_backend::CliBackend`] — shells to `lore`. Works today.
//!   * [`crate::client_backend::ClientBackend`] — links `lore-client` in-process.
//!     This is the destination; it's stubbed until the pre-1.0 API is pinned.

use crate::error::Result;
use crate::model::{
    Branch, ConfigValue, InstanceList, InstancePruneResult, ImmutableQueryResult,
    LinkAddResult, LinkListResult, LinkListStagedResult,
    LinkRemoveResult, LinkUpdateResult, MetadataEntry, RepoCreateResult, RepoDump,
    RepoInfo, RepoListing, RepoStatus, Revision, VerifyFragmentResult,
    VerifyStateResult,
};
use std::collections::HashMap;
use std::path::PathBuf;

/// Full-surface async interface over a Lore working tree.
#[async_trait::async_trait]
pub trait LoreBackend: Send + Sync {
    // --- inspection ---
    async fn status(&self) -> Result<RepoStatus>;
    async fn log(&self, limit: usize) -> Result<Vec<Revision>>;
    async fn branches(&self) -> Result<Vec<Branch>>;

    // --- working-tree mutations (offline-capable in Lore) ---
    async fn stage(&self, paths: &[String]) -> Result<()>;
    async fn unstage(&self, paths: &[String]) -> Result<()>;
    /// Records staged files as a new revision; returns its short hash.
    async fn commit(&self, message: &str) -> Result<String>;

    // --- branching ---
    async fn create_branch(&self, name: &str) -> Result<()>;
    async fn switch_branch(&self, name: &str) -> Result<()>;
    async fn merge_branch(&self, name: &str) -> Result<()>;

    // --- remote ---
    async fn push(&self) -> Result<()>;
    async fn sync(&self) -> Result<()>;

    // --- lifecycle (operate outside an existing working tree) ---
    async fn create_repository(&self, path: PathBuf, name: &str) -> Result<String>;
    async fn clone(&self, url: &str, dest: PathBuf) -> Result<()>;

    // ===== Repository domain (21 ops) =====

    /// Get detailed information about the current repository.
    async fn repo_info(&self) -> Result<RepoInfo>;

    /// Dump the repository state as serialized data.
    async fn repo_dump(&self, format: Option<&str>) -> Result<RepoDump>;

    /// Create a repository with metadata key-value pairs.
    async fn repo_create_with_metadata(
        &self,
        path: PathBuf,
        name: &str,
        metadata: HashMap<String, String>,
    ) -> Result<RepoCreateResult>;

    /// Delete the repository at the given path.
    async fn repo_delete(&self, path: PathBuf) -> Result<()>;

    /// Release the repository (make it read-only, preserve history).
    async fn repo_release(&self) -> Result<()>;

    /// Flush pending writes to disk.
    async fn repo_flush(&self) -> Result<()>;

    /// Run garbage collection on the repository.
    async fn repo_gc(&self, aggressive: bool) -> Result<u64>;

    /// List known repositories.
    async fn repo_list(&self) -> Result<Vec<RepoListing>>;

    /// Verify the repository state is consistent.
    async fn repo_verify_state(&self) -> Result<VerifyStateResult>;

    /// Verify a specific fragment (by hash) is intact.
    async fn repo_verify_fragment(
        &self,
        fragment_hash: &str,
    ) -> Result<VerifyFragmentResult>;

    /// Query the immutable store for matching entries.
    async fn repo_store_immutable_query(
        &self,
        query: &str,
    ) -> Result<ImmutableQueryResult>;

    /// Get a single metadata entry by key.
    async fn repo_metadata_get(&self, key: &str) -> Result<Option<MetadataEntry>>;

    /// Set a metadata key-value pair.
    async fn repo_metadata_set(&self, key: &str, value: &str) -> Result<()>;

    /// Clear all metadata from the repository.
    async fn repo_metadata_clear(&self) -> Result<()>;

    /// List repository instances.
    async fn repo_instance_list(&self) -> Result<InstanceList>;

    /// Prune unused instances.
    async fn repo_instance_prune(&self) -> Result<InstancePruneResult>;

    /// Update the repository path.
    async fn repo_update_path(&self, new_path: PathBuf) -> Result<()>;

    /// Get a configuration value.
    async fn repo_config_get(&self, key: &str) -> Result<ConfigValue>;

    // ===== Link domain (5 ops) =====

    /// Add a new link to a linked repository.
    async fn link_add(
        &self,
        link: &str,
        link_path: &str,
        source_path: &str,
        pin: &str,
        disable_branching: bool,
    ) -> Result<LinkAddResult>;

    /// Remove a link from the repository.
    async fn link_remove(&self, link_path: &str) -> Result<LinkRemoveResult>;

    /// Update the pin of an existing link.
    async fn link_update(&self, link_path: &str, pin: &str) -> Result<LinkUpdateResult>;

    /// List all linked repositories.
    async fn link_list(&self) -> Result<LinkListResult>;

    /// List staged link changes (not yet committed).
    async fn link_list_staged(&self) -> Result<LinkListStagedResult>;
}

/// Pick a backend by enabled feature. The frontend never knows which is live.
pub fn default_backend(working_dir: PathBuf) -> Box<dyn LoreBackend> {
    #[cfg(feature = "client-backend")]
    {
        return Box::new(crate::client_backend::ClientBackend::new(working_dir));
    }
    #[cfg(all(feature = "cli-backend", not(feature = "client-backend")))]
    {
        return Box::new(crate::cli_backend::CliBackend::new(working_dir));
    }
    #[cfg(not(any(feature = "cli-backend", feature = "client-backend")))]
    {
        let _ = working_dir;
        compile_error!("enable either the `cli-backend` or `client-backend` feature");
    }
}
