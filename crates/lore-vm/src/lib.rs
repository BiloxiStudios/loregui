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
pub use model::{
    Branch, ChangeKind, ConfigValue, FileChange, InstanceInfo, InstanceList,
    InstancePruneResult, ImmutableMatch, ImmutableQueryResult, MetadataEntry,
    RepoCreateResult, RepoDump, RepoInfo, RepoListing, RepoMetadata, RepoStatus,
    Revision, VerifyFragmentResult, VerifyStateResult,
    LinkAddResult, LinkEntry, LinkListResult, LinkListStagedResult,
    LinkRemoveResult, LinkUpdateResult,
};
