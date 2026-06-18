//! `storage`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::storage::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod open;
pub mod close;
pub mod flush;
pub mod put;
pub mod put_file;
pub mod get;
pub mod get_file;
pub mod get_metadata;
pub mod copy;
pub mod obliterate;
pub mod upload;
