//! `file`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::file::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod stage;
pub mod stage_move;
pub mod stage_merge;
pub mod unstage;
pub mod dirty;
pub mod dirty_move;
pub mod dirty_copy;
pub mod reset;
pub mod reset_to_last_merged;
pub mod obliterate;
pub mod info;
pub mod history;
pub mod diff;
pub mod write;
pub mod hash;
pub mod dump;
pub mod metadata_get;
pub mod metadata_set;
pub mod metadata_list;
pub mod metadata_clear;
