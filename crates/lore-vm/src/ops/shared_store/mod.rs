//! `shared_store`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::shared_store::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod create;
pub mod info;
pub mod set_use_automatically;
