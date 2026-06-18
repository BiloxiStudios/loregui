//! `revision`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::revision::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod commit;
pub mod commit_with_metadata;
pub mod amend;
pub mod info;
pub mod history;
pub mod diff;
pub mod find;
pub mod find_local;
pub mod sync;
pub mod restore;
pub mod bisect;
pub mod metadata_get;
pub mod metadata_set;
pub mod metadata_list;
pub mod metadata_clear;
pub mod cherry_pick;
pub mod cherry_pick_local;
pub mod cherry_pick_abort;
pub mod cherry_pick_unresolve;
pub mod cherry_pick_restart;
pub mod cherry_pick_resolve;
pub mod cherry_pick_resolve_mine;
pub mod cherry_pick_resolve_theirs;
pub mod revert;
pub mod revert_local;
pub mod revert_abort;
pub mod revert_unresolve;
pub mod revert_restart;
pub mod revert_resolve;
pub mod revert_resolve_mine;
pub mod revert_resolve_theirs;
