#![allow(clippy::doc_lazy_continuation)]
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
//!
//! Additionally, the `lore` crate is bound directly for the `ops/` layer (API-first
//! per IMPLEMENTATION-PLAN.md §4). New operations go in `ops/<domain>/<name>.rs`.

pub mod api;
pub mod backend;
pub mod collect;
pub mod dispatch;
pub mod error;
pub mod global;
pub mod model;
pub mod ops;

#[cfg(feature = "cli-backend")]
pub mod cli_backend;

#[cfg(feature = "client-backend")]
pub mod client_backend;

pub use api::LoreApi;
pub use backend::{default_backend, LoreBackend};
pub use dispatch::{dispatch, finalize, supported_ops};
pub use error::{LoreError, Result};
pub use model::{Branch, ChangeKind, FileChange, RepoStatus, Revision};
