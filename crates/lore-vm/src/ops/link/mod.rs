//! `link`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::link::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod add;
pub mod remove;
pub mod update;
pub mod list;
pub mod list_staged;
