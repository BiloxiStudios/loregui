//! Operation modules — one file per op, one subdirectory per domain.
//!
//! Each op is a standalone async fn binding the upstream `lore` crate. See
//! IMPLEMENTATION-PLAN.md §4 for the uniform pattern.

pub mod auth;
pub mod repository;
pub mod branch;
pub mod revision;
pub mod file;
pub mod lock;
pub mod link;
pub mod layer;
pub mod storage;
pub mod shared_store;
pub mod service;
pub mod notification;
pub mod dependency;
