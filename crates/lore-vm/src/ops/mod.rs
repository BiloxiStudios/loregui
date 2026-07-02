//! Operation modules — one file per op, one subdirectory per domain.
//!
//! Each op is a standalone async fn binding the upstream `lore` crate. See
//! IMPLEMENTATION-PLAN.md §4 for the uniform pattern.

pub(crate) mod paths;

pub mod auth;
pub mod branch;
pub mod dependency;
pub mod file;
pub mod layer;
pub mod link;
pub mod lock;
pub mod notification;
pub mod repository;
pub mod revision;
pub mod service;
pub mod shared_store;
pub mod storage;
