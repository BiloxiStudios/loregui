//! `dependency`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::dependency::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod dependency_add;
pub mod dependency_list;
pub mod dependency_remove;
