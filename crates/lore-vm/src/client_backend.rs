//! In-process adapter over Lore's own `lore-client` crate — no subprocess.
//!
//! This is the architectural destination: linking `lore-client` directly is what
//! lets `lore-vm` become the shared foundation StudioBrain's desktop app embeds
//! (the same way model-manager links in). It's stubbed because the lore-client
//! API is pre-1.0 and must be pinned to an exact rev before wiring.
//!
//! To activate:
//!   1. Uncomment `lore-client` in the workspace Cargo.toml and pin `rev`.
//!   2. Build with `--features client-backend` (drop `cli-backend`).
//!   3. Replace each `todo!()` with the corresponding lore-client call. The trait
//!      method names mirror the CLI verbs, so the mapping is mechanical.

#![cfg(feature = "client-backend")]

use crate::backend::LoreBackend;
use crate::error::{LoreError, Result};
use crate::model::{
    Branch, ConfigValue, InstanceList, InstancePruneResult, ImmutableQueryResult,
    MetadataEntry, RepoCreateResult, RepoDump, RepoInfo, RepoListing,
    RepoStatus, Revision, VerifyFragmentResult, VerifyStateResult,
};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct ClientBackend {
    #[allow(dead_code)]
    working_dir: PathBuf,
    // client: lore_client::Client,   // <- hold the real handle here
}

impl ClientBackend {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    #[allow(dead_code)]
    fn unimplemented(verb: &str) -> LoreError {
        LoreError::Client(format!(
            "client-backend `{verb}` not wired yet — pin lore-client and replace the todo!()"
        ))
    }
}

#[async_trait::async_trait]
impl LoreBackend for ClientBackend {
    async fn status(&self) -> Result<RepoStatus> {
        todo!("map lore_client status -> RepoStatus")
    }
    async fn log(&self, _limit: usize) -> Result<Vec<Revision>> {
        todo!("walk the revision chain -> Vec<Revision>")
    }
    async fn branches(&self) -> Result<Vec<Branch>> {
        todo!("read branch pointers from the mutable KV store")
    }
    async fn stage(&self, _paths: &[String]) -> Result<()> {
        todo!()
    }
    async fn unstage(&self, _paths: &[String]) -> Result<()> {
        todo!()
    }
    async fn commit(&self, _message: &str) -> Result<String> {
        todo!()
    }
    async fn create_branch(&self, _name: &str) -> Result<()> {
        todo!()
    }
    async fn switch_branch(&self, _name: &str) -> Result<()> {
        todo!()
    }
    async fn merge_branch(&self, _name: &str) -> Result<()> {
        todo!()
    }
    async fn push(&self) -> Result<()> {
        todo!()
    }
    async fn sync(&self) -> Result<()> {
        todo!()
    }
    async fn create_repository(&self, _path: PathBuf, _name: &str) -> Result<String> {
        todo!()
    }
    async fn clone(&self, _url: &str, _dest: PathBuf) -> Result<()> {
        todo!()
    }

    // ===== Repository domain (21 ops) =====
    async fn repo_info(&self) -> Result<RepoInfo> {
        todo!("map lore_client repository info -> RepoInfo")
    }
    async fn repo_dump(&self, _format: Option<&str>) -> Result<RepoDump> {
        todo!("map lore_client repository dump -> RepoDump")
    }
    async fn repo_create_with_metadata(
        &self,
        _path: PathBuf,
        _name: &str,
        _metadata: HashMap<String, String>,
    ) -> Result<RepoCreateResult> {
        todo!("map lore_client repository create_with_metadata -> RepoCreateResult")
    }
    async fn repo_delete(&self, _path: PathBuf) -> Result<()> {
        todo!("map lore_client repository delete")
    }
    async fn repo_release(&self) -> Result<()> {
        todo!("map lore_client repository release")
    }
    async fn repo_flush(&self) -> Result<()> {
        todo!("map lore_client repository flush")
    }
    async fn repo_gc(&self, _aggressive: bool) -> Result<u64> {
        todo!("map lore_client repository gc -> freed bytes")
    }
    async fn repo_list(&self) -> Result<Vec<RepoListing>> {
        todo!("map lore_client repository list -> Vec<RepoListing>")
    }
    async fn repo_verify_state(&self) -> Result<VerifyStateResult> {
        todo!("map lore_client repository verify_state -> VerifyStateResult")
    }
    async fn repo_verify_fragment(&self, _fragment_hash: &str) -> Result<VerifyFragmentResult> {
        todo!("map lore_client repository verify_fragment -> VerifyFragmentResult")
    }
    async fn repo_store_immutable_query(&self, _query: &str) -> Result<ImmutableQueryResult> {
        todo!("map lore_client repository store_immutable_query -> ImmutableQueryResult")
    }
    async fn repo_metadata_get(&self, _key: &str) -> Result<Option<MetadataEntry>> {
        todo!("map lore_client repository metadata_get -> Option<MetadataEntry>")
    }
    async fn repo_metadata_set(&self, _key: &str, _value: &str) -> Result<()> {
        todo!("map lore_client repository metadata_set")
    }
    async fn repo_metadata_clear(&self) -> Result<()> {
        todo!("map lore_client repository metadata_clear")
    }
    async fn repo_instance_list(&self) -> Result<InstanceList> {
        todo!("map lore_client repository instance_list -> InstanceList")
    }
    async fn repo_instance_prune(&self) -> Result<InstancePruneResult> {
        todo!("map lore_client repository instance_prune -> InstancePruneResult")
    }
    async fn repo_update_path(&self, _new_path: PathBuf) -> Result<()> {
        todo!("map lore_client repository update_path")
    }
    async fn repo_config_get(&self, _key: &str) -> Result<ConfigValue> {
        todo!("map lore_client repository config_get -> ConfigValue")
    }
}
