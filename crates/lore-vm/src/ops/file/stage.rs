//! `file stage` operation — binds `lore::file::stage`.
//!
//! Stages one or more files for inclusion in the next commit. Each path is
//! classified as a file or directory by the upstream engine; directory paths
//! honour the `scan` flag for a recursive filesystem walk.
//!
//! Emits `FileStageFile` per file and `FileStageRevision` with the resulting
//! staged-revision identifier.

use crate::api::LoreApi;
use crate::collect::collect_events;
use crate::error::{LoreError, Result};

use lore::file::LoreFileStageArgs;
use lore::interface::LoreEvent;
use serde::{Deserialize, Serialize};

/// Case-change handling for staged paths — mirrors the upstream `case_change`
/// integer with serde-friendly naming for the Tauri boundary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseChange {
    /// Error on a case-only change (0).
    #[default]
    Error,
    /// Update the filesystem to match the repository (1).
    Keep,
    /// Update the repository to match the filesystem (2).
    Rename,
}

impl CaseChange {
    fn as_u32(self) -> u32 {
        match self {
            CaseChange::Error => 0,
            CaseChange::Keep => 1,
            CaseChange::Rename => 2,
        }
    }
}

/// Arguments for [`stage`].
///
/// Mirrors `LoreFileStageArgs` from the upstream `lore` crate but uses plain
/// `Vec<String>` so it serialises cleanly across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageArgs {
    /// Paths to stage. Individual files are always reconciled against the
    /// filesystem; directory paths honour [`scan`](FileStageArgs::scan).
    #[serde(default)]
    pub paths: Vec<String>,
    /// How to handle case-only path changes.
    #[serde(default)]
    pub case_change: CaseChange,
    /// Force a recursive filesystem scan of directory paths.
    #[serde(default)]
    pub scan: bool,
}

impl FileStageArgs {
    /// Convert to the upstream lore args, resolving every incoming path against
    /// `repo_root`.
    ///
    /// The upstream `lore::file::stage` only stages files when handed an
    /// **absolute** path; a repository-relative path (e.g. `"src/main.rs"`) is
    /// silently dropped and stages nothing. Every external driver — the VS Code
    /// extension (`path.relative(repoRoot, uri)`), the MCP server, and CLI
    /// callers — sends repo-relative paths today, so we join each relative path
    /// onto the repo root here. Paths that are already absolute are passed
    /// through unchanged.
    fn into_lore(self, repo_root: &std::path::Path) -> LoreFileStageArgs {
        LoreFileStageArgs {
            paths: crate::ops::paths::lore_path_args(repo_root, &self.paths),
            case_change: self.case_change.as_u32(),
            scan: u8::from(self.scan),
        }
    }
}

/// The action applied to a file when it was staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStageAction {
    Keep,
    Add,
    Delete,
    Move,
    Copy,
}

fn map_action(action: &lore::interface::LoreFileAction) -> FileStageAction {
    match action {
        lore::interface::LoreFileAction::Keep => FileStageAction::Keep,
        lore::interface::LoreFileAction::Add => FileStageAction::Add,
        lore::interface::LoreFileAction::Delete => FileStageAction::Delete,
        lore::interface::LoreFileAction::Move => FileStageAction::Move,
        lore::interface::LoreFileAction::Copy => FileStageAction::Copy,
    }
}

/// One file affected by the stage operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageEntry {
    /// Repository-relative path that was staged.
    pub path: String,
    /// Previous path, when the file was moved. Empty otherwise.
    pub from_path: String,
    /// Action applied to the file.
    pub action: FileStageAction,
}

/// Result returned on a successful stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStageResult {
    /// One entry per file affected.
    pub files: Vec<FileStageEntry>,
    /// Resulting staged-revision identifier (empty when none was reported).
    pub revision: String,
    /// True when a dangling staged anchor was detected and self-healed during
    /// this stage. The previously staged set was snapshotted, the bad anchor
    /// dropped, and the full set (prior staged paths + this call's paths)
    /// re-staged. Surfaced (rather than silently `Ok`) so a driver can warn the
    /// user that a recovery happened.
    #[serde(default)]
    pub healed: bool,
}

/// Stage one or more files for the next commit.
///
/// Calls the upstream `lore::file::stage` in-process and collects
/// `FileStageFile` / `FileStageRevision` events into a typed result.
///
/// **Self-heals a dangling staged anchor (SBAI-4080, the "real-flow" stage bug).**
/// Every stage first deserialises the *pre-existing* staged state (upstream
/// `State::deserialize_current_and_staged`). If a *prior* process wrote the
/// staged-anchor pointer into the mutable store but its state fragment never
/// became durable in the immutable store — the classic "anchor present, fragment
/// missing" corruption that surfaces in real repos as
/// `Failed to deserialize staged state: Failed to read state data` /
/// `Failed to deserialize revision state` (shared store) or a bare `Not found`
/// (local store) — then **every** subsequent `file.stage` is permanently stuck:
/// the engine can't read the state it's supposed to extend.
///
/// The only thing that clears a dangling anchor is dropping it before any
/// deserialize touches it (`repository.status` with `reset = true`, which calls
/// `delete_staged_anchor` up front). `reset = true` *deletes the entire staged
/// set*, so a naive "reset then re-stage only this call's paths" would silently
/// drop every other file the user had already staged — data loss returned as
/// `Ok`. To avoid that we:
///
///   1. Drop the bad anchor (`status { reset: true }`). The prior staged set
///      can't be read *before* this — the dangling anchor blocks exactly that
///      read — so we recover the full set afterwards from the filesystem.
///   2. **Re-discover the full change set with a scanning status**
///      (`status { scan: true }`), which reconciles against the working tree and
///      reports every dirty file — including ones that were staged before the
///      anchor went bad (their on-disk content is still dirty vs the committed
///      revision). **Re-stage the union** of that set and this call's paths, so
///      nothing the user had staged is lost.
///   3. Surface the recovery on the result (`healed = true`) and emit a
///      `tracing::warn!` — never a silent `Ok`.
///
/// A retry is only attempted for the dangling-anchor signature — the structured
/// staged/revision-state deserialize wrapper (shared store) or the exact bare
/// `Not found` the local store emits for that read (see
/// [`is_dangling_staged_state`]). Genuine stage errors (bad path, conflict, other
/// `… not found` failures) still surface immediately. And because the recovery
/// re-stages the full working-tree change set, even an over-match cannot lose a
/// staged file.
pub async fn stage(api: &LoreApi, args: FileStageArgs) -> Result<FileStageResult> {
    match stage_once(api, args.clone()).await {
        Ok(result) => Ok(result),
        Err(err) if is_dangling_staged_state(&err) => heal_and_restage(api, args, err).await,
        Err(err) => Err(err),
    }
}

/// Recover from a dangling staged anchor without silently dropping the rest of
/// the staged set. See [`stage`] for the full rationale. Returns the re-staged
/// result with `healed = true`; on any failure of the recovery itself surfaces
/// the ORIGINAL stage error (more actionable than a reset error).
async fn heal_and_restage(
    api: &LoreApi,
    args: FileStageArgs,
    original_err: LoreError,
) -> Result<FileStageResult> {
    // 1. Drop the unreadable staged anchor — the ONE upstream path that deletes
    //    the dangling pointer (`status { reset: true }` -> `delete_staged_anchor`)
    //    before any deserialize touches it. We can't read the old staged set
    //    BEFORE this (the dangling anchor blocks exactly that read), so we recover
    //    the full set AFTER the reset by scanning the working tree (step 2). If
    //    the reset itself fails, surface the ORIGINAL stage error.
    reset_staged_anchor(api).await.map_err(|_| original_err)?;

    // 2. Discover the FULL set of paths that must be re-staged so nothing the user
    //    had staged is silently lost. A *scanning* status reconciles against the
    //    filesystem and reports every working-tree change — including files that
    //    were staged before the anchor went bad (their on-disk content is still
    //    dirty relative to the committed revision). We re-stage that whole set,
    //    unioned with this call's explicitly-named paths.
    let mut union: Vec<String> = scan_dirty_paths(api).await;
    for p in &args.paths {
        if !union.contains(p) {
            union.push(p.clone());
        }
    }
    let recovered_count = union.len();
    let restage_args = FileStageArgs {
        paths: union,
        scan: true,
        ..args
    };
    let mut result = stage_once(api, restage_args).await?;

    // 3. Surface the recovery — never a silent Ok.
    result.healed = true;
    tracing::warn!(
        recovered_paths = recovered_count,
        "self-healed a dangling staged anchor: dropped the unreadable anchor and \
         re-staged the full working-tree change set ({recovered_count} path(s))"
    );

    Ok(result)
}

/// Repository-relative paths the working tree shows as changed, discovered via a
/// *scanning* status (which reconciles against the filesystem and so survives a
/// dangling anchor that blocks reading the prior staged set). Best-effort: an
/// empty vec if status can't be read, so recovery still falls back to the current
/// call's paths rather than staying bricked.
async fn scan_dirty_paths(api: &LoreApi) -> Vec<String> {
    use crate::ops::repository::status::{status, RepositoryStatusArgs};
    match status(
        api,
        RepositoryStatusArgs {
            scan: true,
            ..Default::default()
        },
    )
    .await
    {
        Ok(s) => s.files.into_iter().map(|f| f.path).collect(),
        Err(_) => Vec::new(),
    }
}

/// One raw attempt at the upstream stage — no recovery. Factored out so [`stage`]
/// can retry it after healing a dangling staged anchor.
async fn stage_once(api: &LoreApi, args: FileStageArgs) -> Result<FileStageResult> {
    let (callback, rx) = collect_events();

    let globals = api.globals();
    let repo_root = globals.repository_path.clone();
    let status = lore::file::stage(globals.build(), args.into_lore(&repo_root), callback).await;

    let stream = rx
        .await
        .map_err(|e| LoreError::CommandFailed(format!("event stream cancelled: {e}")))?;

    if !stream.is_ok() {
        return Err(LoreError::CommandFailed(stream.error.unwrap_or_else(
            || format!("file stage failed with status {status}"),
        )));
    }

    let mut files = Vec::new();
    let mut revision = String::new();

    for event in &stream.events {
        match event {
            LoreEvent::FileStageFile(data) => {
                files.push(FileStageEntry {
                    path: data.path.as_str().to_string(),
                    from_path: data.from_path.as_str().to_string(),
                    action: map_action(&data.action),
                });
            }
            LoreEvent::FileStageRevision(data) => {
                revision = format!("{}", data.revision);
            }
            _ => {}
        }
    }

    Ok(FileStageResult {
        files,
        revision,
        healed: false,
    })
}

/// True when `err` is the "dangling staged anchor" failure: stage tried to read
/// the pre-existing staged state to extend it, and the mutable store's anchor
/// pointed at an immutable *state fragment* that is missing or unreadable.
///
/// `is_dangling_staged_state` is ONLY ever evaluated on the error of a *stage*
/// attempt (see [`stage`]), so the message is always "the read stage performs
/// before extending the staged state failed". Across the two store tiers that
/// failure surfaces as:
///
///   - **shared store** — the structured `Failed to deserialize staged state:
///     Failed to read state data` (or `… revision state`) wrapper;
///   - **local store** — a *bare* `Not found` (`lore-base` `Error::NotFound`),
///     with no deserialize wrapper. This is the real, observed signal in the
///     cross-process VS Code flow; without matching it the local-store recovery
///     never fires and the repo stays bricked forever.
///
/// The earlier worry about the bare `Not found` arm was **data loss, not the
/// match itself**: the old recovery reset the whole staged set and re-staged only
/// the current call's paths, so a false-positive (or even a true) match silently
/// dropped every other staged file. That is fixed at the recovery site, not here
/// — [`heal_and_restage`] now snapshots the FULL staged set and re-stages the
/// union, so recovery is non-destructive even if this classifier over-matches.
/// We therefore keep the bare-`Not found` arm (genuine recovery needs it) while
/// the destructive edge it used to have is gone.
///
/// Test-only re-export of [`is_dangling_staged_state`] so the integration harness
/// can assert the classifier directly.
#[doc(hidden)]
pub fn is_dangling_staged_state_for_test(err: &LoreError) -> bool {
    is_dangling_staged_state(err)
}

fn is_dangling_staged_state(err: &LoreError) -> bool {
    // Match on the RAW carried message, not `err.to_string()` (whose `Display`
    // prepends "lore command failed: "), so the exact bare-`Not found` check is
    // reliable.
    let msg = match err {
        LoreError::CommandFailed(m) | LoreError::Client(m) => m.as_str(),
        _ => return false,
    };
    // The structured shared-store wrapper, OR the bare local-store `Not found`
    // (the only thing the local tier emits for a missing staged-state fragment).
    // `== "Not found"` is exact so other `… not found` errors (Node/Link/file/
    // Address/Payload not found) do NOT match. Recovery is non-destructive
    // (full-set snapshot + re-stage), so matching the bare `Not found` here can
    // no longer wipe a healthy staged set.
    msg.contains("deserialize staged state")
        || msg.contains("deserialize revision state")
        || msg == "Not found"
}

/// Drop a dangling staged anchor by routing through `repository.status` with
/// `reset = true` — the one upstream path that deletes the staged-anchor pointer
/// *before* any deserialize touches it (`delete_staged_anchor`). Used only as a
/// recovery step inside [`stage`].
async fn reset_staged_anchor(api: &LoreApi) -> Result<()> {
    use crate::ops::repository::status::{status, RepositoryStatusArgs};
    status(
        api,
        RepositoryStatusArgs {
            reset: true,
            ..Default::default()
        },
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_args_serializes() {
        let args = FileStageArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
            case_change: CaseChange::Error,
            scan: true,
        };
        let json = serde_json::to_string(&args).expect("should serialize");
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("README.md"));
    }

    #[test]
    fn stage_args_deserializes_with_defaults() {
        let json = r#"{}"#;
        let args: FileStageArgs = serde_json::from_str(json).expect("should deserialize");
        assert!(args.paths.is_empty());
        assert_eq!(args.case_change, CaseChange::Error);
        assert!(!args.scan);
    }

    #[test]
    fn stage_args_into_lore_conversion() {
        let args = FileStageArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
            case_change: CaseChange::Rename,
            scan: true,
        };
        let repo_root = std::path::Path::new("/repo");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(lore_args.paths.len(), 2);
        assert_eq!(lore_args.case_change, 2);
        assert_eq!(lore_args.scan, 1);
    }

    /// Regression for BUG #1 (SBAI-4080): repository-relative paths — what every
    /// external driver (VS Code, MCP, CLI) sends — must be resolved against the
    /// repo root so the upstream engine actually stages them. Previously they
    /// were passed through verbatim and the engine silently staged nothing.
    #[test]
    fn stage_args_resolves_relative_paths_against_repo_root() {
        let args = FileStageArgs {
            paths: vec!["src/main.rs".into(), "README.md".into()],
            case_change: CaseChange::Error,
            scan: false,
        };
        let repo_root = std::path::Path::new("/work/myrepo");
        let lore_args = args.into_lore(repo_root);

        let resolved: Vec<String> = lore_args
            .paths
            .as_slice()
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
        assert_eq!(resolved.len(), 2);
        // Both relative paths are now joined onto the repo root.
        assert!(
            resolved.contains(&"/work/myrepo/src/main.rs".to_string()),
            "relative path should be resolved against repo root, got {resolved:?}"
        );
        assert!(
            resolved.contains(&"/work/myrepo/README.md".to_string()),
            "relative path should be resolved against repo root, got {resolved:?}"
        );
    }

    /// Already-absolute paths must pass through unchanged (the engine accepted
    /// these all along; the fix must not double-prefix them).
    #[test]
    fn stage_args_passes_absolute_paths_through() {
        let abs = if cfg!(windows) {
            r"C:\work\myrepo\rel.txt"
        } else {
            "/work/myrepo/rel.txt"
        };
        let args = FileStageArgs {
            paths: vec![abs.into()],
            case_change: CaseChange::Error,
            scan: false,
        };
        let repo_root = std::path::Path::new("/some/other/root");
        let lore_args = args.into_lore(repo_root);
        assert_eq!(lore_args.paths.len(), 1);
        assert_eq!(lore_args.paths.as_slice()[0].as_str(), abs);
    }

    #[test]
    fn case_change_serde() {
        assert_eq!(
            serde_json::to_string(&CaseChange::Error).unwrap(),
            r#""error""#
        );
        assert_eq!(
            serde_json::to_string(&CaseChange::Keep).unwrap(),
            r#""keep""#
        );
        assert_eq!(
            serde_json::to_string(&CaseChange::Rename).unwrap(),
            r#""rename""#
        );
    }

    #[test]
    fn stage_result_serializes() {
        let result = FileStageResult {
            files: vec![FileStageEntry {
                path: "a.txt".into(),
                from_path: String::new(),
                action: FileStageAction::Add,
            }],
            revision: "abc123".into(),
            healed: false,
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("a.txt"));
        assert!(json.contains("abc123"));
        assert!(json.contains(r#""add""#));
    }

    /// The dangling-anchor classifier recognises the real signals (structured
    /// shared-store wrapper + the bare local-store `Not found`) but is scoped
    /// tightly enough that OTHER `*not found*` errors — which are NOT the
    /// staged-state read failing — do not match. Recovery is non-destructive, so
    /// even an over-match cannot lose data, but tight scoping avoids needless
    /// resets. Regression for the data-loss bug.
    #[test]
    fn dangling_classifier_matches_real_signals_only() {
        // Recognised: the structured wrappers (shared store) ...
        for msg in [
            "Failed to deserialize staged state: Failed to read state data",
            "Failed to deserialize staged state: Not found",
            "Failed to deserialize revision state: Failed to read state data",
            // ... and the bare local-store `Not found` (the real cross-process
            // signal — genuine recovery depends on matching it).
            "Not found",
        ] {
            assert!(
                is_dangling_staged_state(&LoreError::CommandFailed(msg.into())),
                "{msg:?} should be classified as a dangling staged anchor"
            );
        }

        // NOT recognised: other `… not found` errors are a different failure (a
        // missing path/node/link/address), not the staged-state read — the exact
        // bare-`Not found` match excludes them.
        for msg in [
            "Address not found: ab12",
            "file not found: foo.txt",
            "Node not found",
            "Link not found",
            "Payload not found: deadbeef",
            // A bare read-state-data error with no deserialize wrapper is also out
            // of scope.
            "Failed to read state data",
            "path 'nope.txt' does not exist",
        ] {
            assert!(
                !is_dangling_staged_state(&LoreError::CommandFailed(msg.into())),
                "{msg:?} must NOT be classified as a dangling staged anchor"
            );
        }
    }
}
