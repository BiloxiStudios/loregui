//! The seam every GUI/host talks to. One async trait covering the full Lore
//! verb set the CLI exposes (`repository create`, `stage`, `status`, `commit`,
//! `push`, `clone`, `branch create/switch/merge`, `sync`, `shared-store create`).
//!
//! Two implementations ship:
//!   * [`crate::cli_backend::CliBackend`] — shells to `lore`. Works today.
//!   * [`crate::client_backend::ClientBackend`] — links `lore-client` in-process.
//!     This is the destination; it's stubbed until the pre-1.0 API is pinned.

use crate::error::Result;
use crate::model::{Branch, RepoStatus, Revision};
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
}

/// Pick a backend by enabled feature. The frontend never knows which is live.
///
/// NOTE: this legacy CLI/feature-gated adapter is slated for removal — the ops/
/// layer (LoreApi) is the real API-first path. Tracked as a follow-up; the
/// `return`s are required here because each arm is a separate `#[cfg]` block.
#[allow(clippy::needless_return)]
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
