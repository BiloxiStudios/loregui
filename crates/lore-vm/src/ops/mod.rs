//! Operation modules — one file per op, one subdirectory per domain.
//!
//! Each operation is a standalone async fn that binds the upstream `lore`
//! crate directly. See IMPLEMENTATION-PLAN.md §4 for the uniform pattern.

pub mod auth;
