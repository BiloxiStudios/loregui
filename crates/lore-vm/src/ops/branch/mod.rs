//! `branch`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::branch::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod create;
pub mod info;
pub mod diff;
pub mod list;
pub mod latest_list;
pub mod switch;
pub mod push;
pub mod reset;
pub mod archive;
pub mod protect;
pub mod unprotect;
pub mod merge_start;
pub mod merge_into;
pub mod merge_resolve;
pub mod merge_resolve_mine;
pub mod merge_resolve_theirs;
pub mod merge_unresolve;
pub mod merge_restart;
pub mod merge_abort;
pub mod metadata_get;
pub mod metadata_set;
pub mod metadata_clear;
