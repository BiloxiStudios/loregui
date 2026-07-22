//! `service`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::service::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod restart;
pub mod start;
pub mod stop;
