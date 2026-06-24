//! Real-flow staging regression harness for `lore-vm` (SBAI-4080 — 0.2.3).
//!
//! The 0.2.1 cross-process flush fix and the 0.2.2 relative-path fix were both
//! validated against a CLEAN, CLI-created scratch repo. That missed the way a
//! REAL VS Code-extension user drives the engine: they EDIT a tracked file in
//! the editor (not via the CLI) and then stage it, with one `lorevm` process per
//! op. Two real-flow gaps fell through:
//!
//!   1. **`repository.status` only reports editor-edited working-tree changes
//!      when `scan = true`.** Without `scan` the engine reports nothing the user
//!      hasn't already staged, so a freshly edited tracked file + a new untracked
//!      file are invisible — the SCM "Changes" group looks empty. (The extension
//!      now always polls `status { scan: true }`; this asserts the engine
//!      contract that backs it.)
//!
//!   2. **A dangling staged anchor permanently breaks staging.** If a prior
//!      process wrote the staged-anchor pointer (mutable store) but its state
//!      fragment never became durable (immutable store) — the cross-process
//!      flush race — every later `file.stage` hits
//!      `Failed to deserialize staged state: Failed to read state data` (or the
//!      local-store `Not found`) because stage first deserialises the existing
//!      staged state. `file::stage` now self-heals: it drops the bad anchor and
//!      retries once, recovering a repo that was otherwise stuck forever.
//!
//! Runs against a REAL on-disk repo (`in_memory = 0`, `offline = 1`) backed by a
//! shared store OUTSIDE the working tree, so we can manipulate the on-disk
//! immutable store to reproduce the dangling-anchor corruption deterministically.
//!
//! ```sh
//! cargo test -p lore-vm --features integration-tests --test stage_real_flow
//! ```
#![cfg(feature = "integration-tests")]

use std::io::Write;
use std::path::Path;

use lore_vm::api::LoreApi;
use lore_vm::global::LoreGlobal;
use lore_vm::ops;

/// On-disk headless api as a given identity, mirroring `e2e_lifecycle.rs`.
fn on_disk_api(dir: &Path, identity: &str) -> LoreApi {
    let global = LoreGlobal::new(dir.to_path_buf())
        .in_memory(false)
        .offline(true)
        .identity(identity);
    LoreApi::from_global(global)
}

fn write_file(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    let mut f = std::fs::File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .expect("create file");
    f.write_all(contents).expect("write file");
}

/// Stage repo-RELATIVE paths exactly as the VS Code extension does
/// (`path.relative(repoRoot, uri)` + `scan: true`).
async fn stage_rel(
    api: &LoreApi,
    paths: &[&str],
) -> lore_vm::error::Result<ops::file::stage::FileStageResult> {
    ops::file::stage::stage(
        api,
        ops::file::stage::FileStageArgs {
            paths: paths.iter().map(|p| p.to_string()).collect(),
            case_change: ops::file::stage::CaseChange::Error,
            scan: true,
        },
    )
    .await
}

async fn status(
    api: &LoreApi,
    args: ops::repository::status::RepositoryStatusArgs,
) -> ops::repository::status::RepositoryStatusResult {
    ops::repository::status::status(api, args)
        .await
        .expect("repository::status should succeed")
}

/// Build a real on-disk repo with a shared store outside the tree and one
/// committed baseline file (`tracked.txt`). Returns the api.
async fn setup_repo(repo_path: &Path, store_path: &Path, identity: &str) -> LoreApi {
    let api = on_disk_api(repo_path, identity);

    let name = format!("real-flow-{}", std::process::id());
    ops::repository::create::create(
        &api,
        ops::repository::create::CreateArgs {
            repository_url: format!("lore://localhost/{name}"),
            description: "stage real-flow harness".into(),
            id: String::new(),
            use_shared_store: true,
            shared_store_path: store_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("repository::create should succeed");

    write_file(&repo_path.join("tracked.txt"), b"baseline v1\n");
    stage_rel(&api, &["tracked.txt"])
        .await
        .expect("baseline stage should succeed");
    ops::revision::commit::commit(
        &api,
        ops::revision::commit::CommitArgs {
            message: "baseline".into(),
        },
    )
    .await
    .expect("baseline commit should succeed");

    api
}

/// BUG #1 (real flow): an editor edit to a tracked file + a new untracked file
/// must BOTH show up as working-tree changes — but ONLY when status scans the
/// filesystem. This is the engine contract behind the SCM "Changes" group.
#[tokio::test]
async fn status_scan_lists_editor_edited_and_untracked_files() {
    let work = tempfile::tempdir().expect("work tempdir");
    let store = tempfile::tempdir().expect("store tempdir");
    let repo = work.path();
    let store_path = store.path().join("shared-store");

    let api = setup_repo(repo, &store_path, "alice").await;

    // Simulate the editor: modify the tracked file ON DISK (not via the CLI) and
    // drop a brand-new untracked file next to it.
    write_file(
        &repo.join("tracked.txt"),
        b"baseline v1\nEDITED IN EDITOR\n",
    );
    write_file(&repo.join("untracked.txt"), b"brand new\n");

    // Without scan the working-tree edits are invisible — this is exactly why a
    // naive poll left the SCM view empty.
    let bare = status(
        &api,
        ops::repository::status::RepositoryStatusArgs::default(),
    )
    .await;
    assert!(
        !bare.files.iter().any(|f| f.path.ends_with("tracked.txt")),
        "precondition: a non-scanning status must NOT surface the editor edit \
         (that's the bug the extension works around with scan:true): {bare:?}"
    );

    // With scan (what the extension now always sends) BOTH changes appear.
    let scanned = status(
        &api,
        ops::repository::status::RepositoryStatusArgs {
            scan: true,
            ..Default::default()
        },
    )
    .await;
    assert!(
        scanned
            .files
            .iter()
            .any(|f| f.path.ends_with("tracked.txt") && f.dirty),
        "scanning status must report the editor-edited tracked file: {scanned:?}"
    );
    assert!(
        scanned
            .files
            .iter()
            .any(|f| f.path.ends_with("untracked.txt") && f.dirty),
        "scanning status must report the new untracked file: {scanned:?}"
    );
}

/// BUG #2 (real flow): the full editor edit → stage → commit cycle, across the
/// SAME api but exercising each op independently, must persist the edit.
#[tokio::test]
async fn editor_edit_stage_commit_persists() {
    let work = tempfile::tempdir().expect("work tempdir");
    let store = tempfile::tempdir().expect("store tempdir");
    let repo = work.path();
    let store_path = store.path().join("shared-store");

    let api = setup_repo(repo, &store_path, "alice").await;

    write_file(&repo.join("tracked.txt"), b"baseline v1\nEDITED\n");
    let staged = stage_rel(&api, &["tracked.txt"])
        .await
        .expect("stage of editor-edited file should succeed");
    assert!(
        staged.files.iter().any(|f| f.path.ends_with("tracked.txt")),
        "stage should report the edited file: {staged:?}"
    );

    let committed = ops::revision::commit::commit(
        &api,
        ops::revision::commit::CommitArgs {
            message: "edit tracked".into(),
        },
    )
    .await
    .expect("commit should succeed");
    assert!(
        !committed.revision.is_empty(),
        "commit must return a revision: {committed:?}"
    );

    // The edit is durable: HEAD's delta touches the file and the tree is clean.
    let info = ops::revision::info::info(
        &api,
        ops::revision::info::RevisionInfoArgs {
            revision: committed.revision.clone(),
            delta: true,
            metadata: false,
        },
    )
    .await
    .expect("revision::info should succeed");
    assert!(
        info.deltas.iter().any(|d| d.path.ends_with("tracked.txt")),
        "committed revision must include the edit in its delta: {info:?}"
    );

    let after = status(
        &api,
        ops::repository::status::RepositoryStatusArgs {
            scan: true,
            ..Default::default()
        },
    )
    .await;
    assert!(
        !after.files.iter().any(|f| f.path.ends_with("tracked.txt")),
        "after committing the edit, tracked.txt should be clean: {after:?}"
    );
}

/// Direct unit coverage for the dangling-anchor error classifier that gates the
/// self-heal retry. The full cross-process recovery is proven in
/// `dangling_anchor_self_heals_across_processes` below (the in-process api keeps
/// the immutable fragment cached, so on-disk corruption is invisible to it —
/// only a *fresh* process actually fails the read).
#[test]
fn dangling_anchor_signatures_are_recognized() {
    use lore_vm::error::LoreError;
    use lore_vm::ops::file::stage::is_dangling_staged_state_for_test as is_dangling;

    // Recognised: the structured shared-store wrapper, and the bare local-store
    // `Not found` (the real cross-process signal — recovery depends on it).
    for msg in [
        "Failed to deserialize staged state: Failed to read state data",
        "Failed to deserialize revision state: Failed to read state data",
        "Not found",
    ] {
        assert!(
            is_dangling(&LoreError::CommandFailed(msg.into())),
            "{msg:?} should be classified as a dangling staged anchor"
        );
    }

    // NOT recognised: other `… not found` errors are a DIFFERENT failure (missing
    // path/node/link/address), not the staged-state read. The exact bare-`Not
    // found` match excludes them, so they surface immediately instead of looping
    // through the heal. (Recovery is non-destructive regardless, so this is about
    // not resetting needlessly — see `self_heal_preserves_full_staged_set`.)
    for msg in [
        "Node not found",
        "Link not found",
        "file not found: foo.txt",
        "Failed to read state data",
        "path 'nope.txt' does not exist",
    ] {
        assert!(
            !is_dangling(&LoreError::CommandFailed(msg.into())),
            "{msg:?} must NOT be classified as a dangling staged anchor"
        );
    }
}

/// BUG #2 (self-heal, REAL cross-process flow): a dangling staged anchor —
/// staged-anchor pointer present in the mutable store, its state fragment GONE
/// from the immutable store — must NOT permanently brick staging. This drives the
/// built `lorevm` CLI with ONE process per op (exactly how the VS Code extension
/// shells out) so each op opens a fresh store with no warm in-process cache —
/// the only way the missing fragment actually fails a read. Before the fix the
/// stuck repo's every `file.stage` errored "Not found" / "Failed to read state
/// data" forever; after it, the next stage drops the bad anchor, retries, and the
/// repo commits cleanly.
#[test]
fn dangling_anchor_self_heals_across_processes() {
    let lorevm = match locate_lorevm() {
        Some(p) => p,
        None => {
            eprintln!("skipping cross-process self-heal test: lorevm binary not built");
            return;
        }
    };
    let work = tempfile::tempdir().expect("work tempdir");
    let repo = work.path();

    let run = |op: &str, args: &str| -> serde_json::Value {
        let out = std::process::Command::new(&lorevm)
            .args([op, "--dir"])
            .arg(repo)
            .args(["--offline", "--args", args])
            .output()
            .expect("spawn lorevm");
        serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|_| panic!("lorevm {op} produced non-JSON: {:?}", out.stdout))
    };

    run(
        "repository.create",
        r#"{"repository_url":"lore://localhost/heal"}"#,
    );
    write_file(&repo.join("a.txt"), b"content one\n");
    run("file.stage", r#"{"paths":["a.txt"],"scan":true}"#);

    // Find the staged revision, then delete its on-disk immutable fragment bucket
    // to leave the mutable anchor dangling.
    let status = run("repository.status", "{}");
    let staged_rev = status["revision"]["revision_staged"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        staged_rev.len() >= 2,
        "expected a staged revision after stage: {status}"
    );
    let bucket = &staged_rev[..2];
    let frag_dir = repo
        .join(".lore")
        .join("immutable")
        .join("index")
        .join(bucket)
        .join("pack");
    if let Ok(rd) = std::fs::read_dir(&frag_dir) {
        for entry in rd.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }

    // A fresh process now fails to read the dangling staged state.
    let broken = run("repository.status", "{}");
    assert!(
        broken.get("error").is_some(),
        "precondition: a fresh process must fail to read the dangling staged \
         state before the self-heal can be proven: {broken}"
    );

    // The next stage must self-heal: drop the bad anchor, retry, succeed.
    let healed = run("file.stage", r#"{"paths":["a.txt"],"scan":true}"#);
    assert!(
        healed.get("error").is_none(),
        "stage must self-heal the dangling anchor, got: {healed}"
    );
    assert!(
        healed["files"]
            .as_array()
            .is_some_and(|f| f.iter().any(|e| e["path"] == "a.txt")),
        "after self-heal, stage should report a.txt: {healed}"
    );

    // The recovered repo commits cleanly in yet another fresh process.
    let committed = run("revision.commit", r#"{"message":"recovered"}"#);
    assert!(
        committed.get("error").is_none()
            && committed["revision"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
        "commit after self-heal should succeed: {committed}"
    );
}

/// Data-loss regression: when the self-heal fires, it must re-stage the FULL
/// prior staged set, not just the path the current `file.stage` call named.
/// Before the fix, the heal reset the staged set and re-staged only the current
/// call's paths — every other already-staged file was silently dropped and the
/// op still returned `Ok`. Two files are staged across processes, the anchor is
/// corrupted, and a stage of just ONE of them must heal AND keep BOTH staged.
#[test]
fn self_heal_preserves_full_staged_set() {
    let lorevm = match locate_lorevm() {
        Some(p) => p,
        None => {
            eprintln!("skipping self-heal-preserves-set test: lorevm binary not built");
            return;
        }
    };
    let work = tempfile::tempdir().expect("work tempdir");
    let repo = work.path();

    let run = |op: &str, args: &str| -> serde_json::Value {
        let out = std::process::Command::new(&lorevm)
            .args([op, "--dir"])
            .arg(repo)
            .args(["--offline", "--args", args])
            .output()
            .expect("spawn lorevm");
        serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|_| panic!("lorevm {op} produced non-JSON: {:?}", out.stdout))
    };

    run(
        "repository.create",
        r#"{"repository_url":"lore://localhost/heal-set"}"#,
    );
    write_file(&repo.join("a.txt"), b"content a\n");
    write_file(&repo.join("b.txt"), b"content b\n");
    // Stage BOTH files (one staged set with two paths).
    run("file.stage", r#"{"paths":["a.txt","b.txt"],"scan":true}"#);

    // Corrupt the staged anchor by deleting its on-disk immutable fragment.
    let status = run("repository.status", "{}");
    let staged_rev = status["revision"]["revision_staged"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        staged_rev.len() >= 2,
        "expected a staged revision after staging two files: {status}"
    );
    let bucket = &staged_rev[..2];
    let frag_dir = repo
        .join(".lore")
        .join("immutable")
        .join("index")
        .join(bucket)
        .join("pack");
    if let Ok(rd) = std::fs::read_dir(&frag_dir) {
        for entry in rd.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let broken = run("repository.status", "{}");
    assert!(
        broken.get("error").is_some(),
        "precondition: a fresh process must fail to read the dangling staged state: {broken}"
    );

    // Stage ONLY a.txt. The heal must fire AND restore b.txt too — losing b.txt
    // would be silent data loss.
    let healed = run("file.stage", r#"{"paths":["a.txt"],"scan":true}"#);
    assert!(
        healed.get("error").is_none(),
        "stage must self-heal the dangling anchor, got: {healed}"
    );
    assert_eq!(
        healed["healed"],
        serde_json::Value::Bool(true),
        "the recovery must be surfaced via healed=true, not a silent Ok: {healed}"
    );

    // Both files must be present in the recovered staged set.
    let recovered = run("repository.status", r#"{"staged":true}"#);
    let staged_paths: Vec<String> = recovered["files"]
        .as_array()
        .map(|files| {
            files
                .iter()
                .filter(|f| f["staged"] == serde_json::Value::Bool(true))
                .filter_map(|f| f["path"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        staged_paths.iter().any(|p| p.ends_with("a.txt")),
        "a.txt must remain staged after heal: {recovered}"
    );
    assert!(
        staged_paths.iter().any(|p| p.ends_with("b.txt")),
        "b.txt must NOT be lost by the heal — full staged set must be preserved: {recovered}"
    );
}

/// Locate the built `lorevm` CLI (release preferred, then debug) for the
/// cross-process self-heal test. Returns `None` when it hasn't been built, so the
/// test self-skips rather than failing on a clean checkout.
fn locate_lorevm() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("LOREVM_BIN") {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    // CARGO_MANIFEST_DIR = crates/lore-vm; the workspace target/ is two up.
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = manifest
        .ancestors()
        .nth(2)
        .map(|root| root.join("target"))?;
    for profile in ["release", "debug"] {
        let cand = target.join(profile).join("lorevm");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}
