//! In-process adapter over Lore's own engine — no subprocess.
//!
//! This is the architectural destination: `lore-vm` already binds the upstream
//! `lore` crate in-process through its `ops/` layer (`LoreApi`), the same path
//! the GUI panels use. `ClientBackend` is a thin adapter that maps the
//! [`LoreBackend`] verb set onto those ops, so the shipped app is fully
//! self-contained and never shells out to a `lore` CLI binary.
//!
//! Each method builds a [`LoreApi`] pointed at the working directory (or, for
//! lifecycle verbs that operate outside an existing tree, at the supplied path),
//! calls the matching op under [`crate::ops`], and maps the op's typed result
//! to the UI-agnostic view-model types in [`crate::model`].

#![cfg(feature = "client-backend")]

use crate::api::LoreApi;
use crate::backend::LoreBackend;
use crate::error::Result;
use crate::model::{Branch, ChangeKind, FileChange, RepoStatus, Revision};
use crate::ops;
use std::path::PathBuf;

pub struct ClientBackend {
    working_dir: PathBuf,
}

impl ClientBackend {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    /// Build a fresh `LoreApi` rooted at this backend's working directory.
    fn api(&self) -> LoreApi {
        LoreApi::new(self.working_dir.clone())
    }
}

/// Map an upstream status file action to the view-model [`ChangeKind`].
fn map_change_kind(action: ops::repository::status::StatusFileAction) -> ChangeKind {
    use ops::repository::status::StatusFileAction;
    match action {
        StatusFileAction::Add => ChangeKind::Added,
        StatusFileAction::Delete => ChangeKind::Deleted,
        StatusFileAction::Move | StatusFileAction::Copy => ChangeKind::Renamed,
        // "Keep" means the file is present with content changed in place.
        StatusFileAction::Keep => ChangeKind::Modified,
    }
}

#[async_trait::async_trait]
impl LoreBackend for ClientBackend {
    async fn status(&self) -> Result<RepoStatus> {
        let api = self.api();
        let result = ops::repository::status::status(
            &api,
            ops::repository::status::RepositoryStatusArgs {
                staged: true,
                // Reconcile against the filesystem so dirty/untracked files show.
                scan: true,
                ..Default::default()
            },
        )
        .await?;

        let changes = result
            .files
            .into_iter()
            .map(|f| FileChange {
                kind: map_change_kind(f.action),
                path: f.path,
                staged: f.staged,
            })
            .collect();

        // `StatusRevision` does not derive Default, so destructure the Option
        // and supply empty strings when no revision context is reported.
        let (repo_id, branch, revision) = match result.revision {
            Some(rev) => (
                rev.repository,
                // Prefer the human branch name; fall back to the branch id.
                if rev.branch_name.is_empty() {
                    rev.branch
                } else {
                    rev.branch_name
                },
                rev.revision,
            ),
            None => (String::new(), String::new(), String::new()),
        };

        Ok(RepoStatus {
            repo_id,
            branch,
            revision,
            changes,
            // Status does not report ahead/behind counts; the remote-state
            // delta is surfaced by push/sync, not by a local status scan.
            ahead: 0,
            behind: 0,
        })
    }

    async fn log(&self, limit: usize) -> Result<Vec<Revision>> {
        let api = self.api();
        let result = ops::revision::history::history(
            &api,
            ops::revision::history::RevisionHistoryArgs {
                length: limit as u32,
                ..Default::default()
            },
        )
        .await?;

        // History entries carry the revision chain (hash + parents). Author,
        // message, and timestamp are not part of the history op's payload, so
        // they are left empty here; a richer view fetches per-revision info.
        Ok(result
            .entries
            .into_iter()
            .map(|e| Revision {
                hash: e.revision,
                message: String::new(),
                author: String::new(),
                timestamp: String::new(),
                parent: e.parents.into_iter().next(),
            })
            .collect())
    }

    async fn branches(&self) -> Result<Vec<Branch>> {
        let api = self.api();
        // `BranchListArgs` does not derive Default; list active (non-archived).
        let result =
            ops::branch::list::list(&api, ops::branch::list::BranchListArgs { archived: false })
                .await?;

        Ok(result
            .entries
            .into_iter()
            .map(|b| Branch {
                name: b.name,
                id: b.id,
                latest_revision: b.latest,
                is_current: b.is_current,
            })
            .collect())
    }

    async fn stage(&self, paths: &[String]) -> Result<()> {
        let api = self.api();
        ops::file::stage::stage(
            &api,
            ops::file::stage::FileStageArgs {
                paths: paths.to_vec(),
                case_change: ops::file::stage::CaseChange::default(),
                scan: true,
            },
        )
        .await
        .map(drop)
    }

    async fn unstage(&self, paths: &[String]) -> Result<()> {
        let api = self.api();
        ops::file::unstage::unstage(
            &api,
            ops::file::unstage::FileUnstageArgs {
                paths: paths.to_vec(),
            },
        )
        .await
        .map(drop)
    }

    async fn commit(&self, message: &str) -> Result<String> {
        let api = self.api();
        let result = ops::revision::commit::commit(
            &api,
            ops::revision::commit::CommitArgs {
                message: message.to_string(),
            },
        )
        .await?;
        Ok(result.revision)
    }

    async fn create_branch(&self, name: &str) -> Result<()> {
        let api = self.api();
        ops::branch::create::create(
            &api,
            ops::branch::create::BranchCreateArgs {
                branch: name.to_string(),
                category: String::new(),
                id: String::new(),
            },
        )
        .await
        .map(drop)
    }

    async fn switch_branch(&self, name: &str) -> Result<()> {
        let api = self.api();
        ops::branch::switch::switch(
            &api,
            ops::branch::switch::BranchSwitchArgs {
                branch: name.to_string(),
                revision: String::new(),
                reset: false,
                bare: false,
            },
        )
        .await
        .map(drop)
    }

    async fn merge_branch(&self, name: &str) -> Result<()> {
        // `merge_branch(name)` merges the named *source* branch into the
        // current branch. `merge_start` is the single-shot op for exactly that
        // (begins merging a source branch into the current branch, auto-
        // committing when conflict-free). `merge_into` is the inverse direction
        // (current -> target), so it is intentionally not used here.
        let api = self.api();
        ops::branch::merge_start::merge_start(
            &api,
            ops::branch::merge_start::BranchMergeStartArgs {
                branch: name.to_string(),
                message: String::new(),
                no_commit: false,
                link: String::new(),
                ignore_links: false,
            },
        )
        .await
        .map(drop)
    }

    async fn push(&self) -> Result<()> {
        let api = self.api();
        ops::branch::push::push(
            &api,
            ops::branch::push::BranchPushArgs {
                // Empty branch => current branch.
                branch: String::new(),
                fast_forward_merge: false,
            },
        )
        .await
        .map(drop)
    }

    async fn sync(&self) -> Result<()> {
        let api = self.api();
        // Default args sync to the current branch tip.
        ops::revision::sync::sync(&api, ops::revision::sync::RevisionSyncArgs::default())
            .await
            .map(drop)
    }

    async fn create_repository(&self, path: PathBuf, name: &str) -> Result<String> {
        // Lifecycle verb: operate at the supplied path rather than this
        // backend's working dir.
        let api = LoreApi::new(path);
        let result = ops::repository::create::create(
            &api,
            ops::repository::create::CreateArgs {
                // The op derives the repository name/location from the URL.
                repository_url: name.to_string(),
                description: String::new(),
                id: String::new(),
                use_shared_store: false,
                shared_store_path: String::new(),
            },
        )
        .await?;
        Ok(result.id)
    }

    async fn clone(&self, url: &str, dest: PathBuf) -> Result<()> {
        // Lifecycle verb: clone into the supplied destination directory.
        let api = LoreApi::new(dest);
        ops::repository::clone::clone(
            &api,
            ops::repository::clone::CloneArgs {
                repository_url: url.to_string(),
                ..Default::default()
            },
        )
        .await
        .map(drop)
    }
}
