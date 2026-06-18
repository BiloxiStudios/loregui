//! `layer`-domain operations — one sub-module per op.
//!
//! Each op binds `lore::layer::<op>` directly. Reference: ops/auth/login_with_token.rs.

pub mod layer_add;
pub mod layer_remove;
pub mod layer_list;
pub mod layer_list_staged;
