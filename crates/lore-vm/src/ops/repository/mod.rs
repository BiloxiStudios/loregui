//! `repository`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::repository::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod clone;
pub mod config_get;
pub mod create;
pub mod create_with_metadata;
pub mod delete;
pub mod dump;
pub mod flush;
pub mod gc;
pub mod info;
pub mod instance_list;
pub mod instance_prune;
pub mod list;
pub mod metadata_clear;
pub mod metadata_get;
pub mod metadata_set;
pub mod release;
pub mod repository_update_path;
pub mod status;
pub mod store_immutable_query;
pub mod verify_fragment;
pub mod verify_state;
