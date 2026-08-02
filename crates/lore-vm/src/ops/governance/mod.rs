//! `governance`-domain operations — one sub-module per op.
//!
//! Governance/security ops that operate on lore worktree state. Unlike other
//! domains, these are lore-vm native (no upstream `lore::governance` module)
//! and compose existing lore primitives (revision info, metadata, status) to
//! implement security-governance workflows.

pub mod artifact_mark_superseded;
pub mod dco_validate;
pub mod evidence_preserve;
pub mod submission_gate_check;
