//! `file`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::file::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod diff;
pub mod dirty;
pub mod dirty_copy;
pub mod dirty_move;
pub mod dump;
pub mod hash;
pub mod history;
pub mod info;
pub mod metadata_clear;
pub mod metadata_get;
pub mod metadata_list;
pub mod metadata_set;
pub mod obliterate;
pub mod reset;
pub mod reset_to_last_merged;
pub mod stage;
pub mod stage_merge;
pub mod stage_move;
pub mod unstage;
pub mod write;
