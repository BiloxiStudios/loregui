//! Strict DCO validation operation backed by the shared governance evaluator.

use super::contract::{DcoValidateRequest, DcoValidateResult};
use super::evaluator::{evaluate_dco, GovernanceAdapter, ProductionLoreAdapter};
use crate::api::LoreApi;
use crate::error::Result;

pub async fn dco_validate_with_adapter<A: GovernanceAdapter + Sync>(
    adapter: &A,
    request: &DcoValidateRequest,
) -> DcoValidateResult {
    evaluate_dco(adapter, request).await
}

pub async fn dco_validate(api: &LoreApi, request: DcoValidateRequest) -> Result<DcoValidateResult> {
    Ok(dco_validate_with_adapter(&ProductionLoreAdapter::new(api, ""), &request).await)
}
