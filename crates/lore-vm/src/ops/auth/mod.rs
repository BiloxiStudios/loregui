//! `auth`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::auth::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod clear;
pub mod list;
pub mod local_user_info;
pub mod login_interactive;
pub mod login_with_token;
pub mod logout;
pub mod resolve_user_info;
