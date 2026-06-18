//! # lore-vm
//!
//! GUI-agnostic view-model core over the [Lore](https://github.com/EpicGames/lore)
//! version-control system. This crate is the reusable foundation: the standalone
//! `loregui` Tauri app consumes it today, and StudioBrain's desktop app can embed
//! the same crate later (the model-manager pattern — standalone, but also wraps in).
//!
//! Everything funnels through one trait, [`backend::LoreBackend`], with two
//! implementations selected by feature flag:
//! - `cli-backend` (default): shells to the `lore` CLI. Works immediately.
//! - `client-backend`: links `lore-client` in-process. The destination; stubbed.

pub mod backend;
pub mod error;
pub mod model;

#[cfg(feature = "cli-backend")]
pub mod cli_backend;

#[cfg(feature = "client-backend")]
pub mod client_backend;

pub use backend::{default_backend, LoreBackend};
pub use error::{LoreError, Result};
pub use model::{Branch, ChangeKind, FileChange, RepoStatus, Revision};

/// The pinned upstream Lore library version we bind against. Re-exported so the
/// GUI can display it and so the build exercises the `lore` crate dependency.
/// (Foundation, SBAI-3685: this is the in-process binding seam — the CLI/FFI are
/// other consumers of the same `lore` crate.)
pub use lore::LORE_LIBRARY_VERSION;

/// Convenience accessor for the bound upstream Lore version.
pub fn upstream_lore_version() -> &'static str {
    LORE_LIBRARY_VERSION
}
