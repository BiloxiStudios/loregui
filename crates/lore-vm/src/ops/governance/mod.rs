//! Shared strict Option-A governance contract and evaluator.
//!
//! Operation-specific implementations live beside this module.  They must use
//! the schemas in [`contract`] and the injected evaluator in [`evaluator`]; no
//! operation is permitted to create a weaker private variant.

pub mod contract;
pub mod dco_validate;
pub mod evaluator;
pub mod evidence_preserve;
pub mod submission_gate_check;

use contract::{
    GovernanceRole, ImmutableGetItem, ImmutablePutItem, MutationObservation, ReadObservation,
};

/// Narrow, injectable boundary for the one governed metadata write and the
/// immutable-store proof round trip. Implementations retain raw per-item
/// results so operation code can reject missing, duplicate, or foreign events.
#[async_trait::async_trait]
pub trait GovernanceIo {
    fn role(&self) -> GovernanceRole;

    async fn revision_metadata_set(&self, key: &str, value: &str) -> MutationObservation<()>;

    async fn storage_open(&self) -> MutationObservation<Vec<u64>>;

    async fn storage_put(
        &self,
        handle: u64,
        bytes: &[u8],
    ) -> MutationObservation<Vec<ImmutablePutItem>>;

    async fn storage_get(
        &self,
        handle: u64,
        address: &str,
    ) -> ReadObservation<Vec<ImmutableGetItem>>;

    async fn storage_close(&self, handle: u64) -> MutationObservation<()>;
}
