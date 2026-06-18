//! `branch`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::branch::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod archive;
pub mod create;
pub mod diff;
pub mod info;
pub mod latest_list;
pub mod list;
pub mod merge_abort;
pub mod merge_into;
pub mod merge_resolve;
pub mod merge_resolve_mine;
pub mod merge_resolve_theirs;
pub mod merge_restart;
pub mod merge_start;
pub mod merge_unresolve;
pub mod metadata_clear;
pub mod metadata_get;
pub mod metadata_set;
pub mod protect;
pub mod push;
pub mod reset;
pub mod switch;
pub mod unprotect;
