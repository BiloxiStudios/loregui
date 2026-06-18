//! `notification`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::notification::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod subscribe;
pub mod unsubscribe;
