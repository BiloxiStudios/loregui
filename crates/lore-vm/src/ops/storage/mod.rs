//! `storage`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::storage::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod close;
pub mod copy;
pub mod flush;
pub mod get;
pub mod get_file;
pub mod get_metadata;
pub mod obliterate;
pub mod open;
pub mod put;
pub mod put_file;
pub mod upload;
