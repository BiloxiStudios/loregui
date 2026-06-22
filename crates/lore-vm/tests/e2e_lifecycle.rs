//! NON-SKIPPABLE end-to-end revision-lifecycle harness for `lore-vm` (SBAI-4060).
//!
//! Where [`integration_roundtrip`](integration_roundtrip.rs) proves the op
//! bindings are wired correctly with a single happy-path round trip, THIS suite
//! drives the **full revision lifecycle** of a real, on-disk lore repository and
//! asserts strictly at every step so a future regression fails loudly:
//!
//!   ADD → UPDATE → DELETE → RESTORE → (individual) REVERT → final history audit
//!
//! Unlike the in-memory roundtrip, this runs against a **real on-disk repo**
//! (`in_memory = 0`, `offline = 1`) backed by a **shared store created OUTSIDE
//! the working tree** — the closest thing to how a self-hosted user's repo is
//! laid out, and a stronger guard against on-disk persistence regressions.
//!
//! Gated behind the `integration-tests` cargo feature like the roundtrip
//! harness, and wired as a required step in `.github/workflows/integration.yml`.
//! It is intentionally NOT `#[ignore]`: when the feature is on, it MUST run.
//!
//! ```sh
//! cargo test -p lore-vm --features integration-tests --test e2e_lifecycle
//! ```
#![cfg(feature = "integration-tests")]

use std::io::Write;
use std::path::Path;

use lore_vm::api::LoreApi;
use lore_vm::global::LoreGlobal;
use lore_vm::ops;

/// Build a `LoreApi` pointed at `dir`, configured for headless **on-disk**
/// operation as a given user: a real `.urc` store on disk, no server
/// (`offline = 1`), no in-memory store. `identity` flows through to the
/// revision author metadata, which lets us assert per-user attribution.
fn on_disk_api(dir: &Path, identity: &str) -> LoreApi {
    let global = LoreGlobal::new(dir.to_path_buf())
        .in_memory(false)
        .offline(true)
        .identity(identity);
    LoreApi::from_global(global)
}

/// Write `contents` to `path`, creating parent directories as needed.
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

/// Stage a single absolute path and assert the op succeeded.
async fn stage_path(api: &LoreApi, path: &Path) -> ops::file::stage::FileStageResult {
    ops::file::stage::stage(
        api,
        ops::file::stage::FileStageArgs {
            paths: vec![path.to_string_lossy().into_owned()],
            case_change: ops::file::stage::CaseChange::Error,
            scan: true,
        },
    )
    .await
    .unwrap_or_else(|e| panic!("file::stage should succeed for {}: {e}", path.display()))
}

/// Commit the staged state with `message`, asserting a non-empty revision hash
/// comes back, and return that hash.
async fn commit(api: &LoreApi, message: &str) -> String {
    let res = ops::revision::commit::commit(
        api,
        ops::revision::commit::CommitArgs {
            message: message.into(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("revision::commit({message:?}) should succeed: {e}"));
    assert!(
        !res.revision.is_empty(),
        "commit({message:?}) returned an empty revision hash: {res:?}"
    );
    res.revision
}

/// Fetch the metadata-rich info for a revision (message + author live here).
async fn rev_info(api: &LoreApi, revision: &str) -> ops::revision::info::RevisionInfoResult {
    ops::revision::info::info(
        api,
        ops::revision::info::RevisionInfoArgs {
            revision: revision.to_string(),
            delta: true,
            metadata: true,
        },
    )
    .await
    .unwrap_or_else(|e| panic!("revision::info({revision}) should succeed: {e}"))
}

/// Full revision history for the current branch, newest-first.
async fn history(api: &LoreApi) -> ops::revision::history::RevisionHistoryResult {
    ops::revision::history::history(api, ops::revision::history::RevisionHistoryArgs::default())
        .await
        .expect("revision::history should succeed")
}

/// True if `revision`'s delta reports `file_name` present (an Add/Modify, i.e.
/// the file is part of that revision's tree state for this change).
fn delta_touches(info: &ops::revision::info::RevisionInfoResult, file_name: &str) -> bool {
    info.deltas.iter().any(|d| d.path.ends_with(file_name))
}

/// The full add → update → delete → restore → revert lifecycle against a REAL
/// on-disk lore repo with a shared store created OUTSIDE the working tree.
/// Every step asserts on the typed results the op bindings return, with exact
/// messages and counts so a regression cannot pass silently.
#[tokio::test]
async fn revision_lifecycle_add_update_delete_restore() {
    // Working tree and the shared store live in SEPARATE temp dirs: the store
    // must be outside the repo it backs.
    let work = tempfile::tempdir().expect("create work tempdir");
    let store = tempfile::tempdir().expect("create store tempdir");
    let repo_path = work.path().to_path_buf();
    let store_path = store.path().join("shared-store");

    let alice = on_disk_api(&repo_path, "alice");

    // ---- 0. SETUP: shared store OUTSIDE the working tree --------------------
    let created = ops::shared_store::create::create(
        &alice,
        ops::shared_store::create::SharedStoreCreateArgs {
            remote_url: String::new(),
            path: Some(store_path.to_string_lossy().into_owned()),
            make_default: false,
        },
    )
    .await
    .expect("shared_store::create should succeed");
    assert!(
        !created.path.is_empty(),
        "shared_store::create returned an empty path: {created:?}"
    );
    assert!(
        !repo_path.starts_with(store.path()) && !store.path().starts_with(&repo_path),
        "store and working tree must be disjoint; store={store_path:?} repo={repo_path:?}"
    );

    // ---- 0b. create the repository backed by that shared store --------------
    let name = format!("e2e-{}", std::process::id());
    let create = ops::repository::create::create(
        &alice,
        ops::repository::create::CreateArgs {
            repository_url: format!("lore://localhost/{name}"),
            description: "lore-vm e2e lifecycle repo".into(),
            id: String::new(),
            use_shared_store: true,
            shared_store_path: store_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("repository::create should succeed");
    assert!(
        !create.id.is_empty(),
        "repository::create returned an empty id: {create:?}"
    );

    let file_path = repo_path.join("foo.txt");
    const V1: &[u8] = b"foo: version one\n";
    const V2: &[u8] = b"foo: version two, updated\n";

    // ======================== ADD ===========================================
    write_file(&file_path, V1);
    let staged = stage_path(&alice, &file_path).await;
    assert!(
        staged.files.iter().any(|f| f.path.ends_with("foo.txt")),
        "stage did not report foo.txt on ADD: {staged:?}"
    );
    let rev_add = commit(&alice, "add foo").await;

    // History grew to exactly one commit.
    let hist = history(&alice).await;
    assert_eq!(
        hist.entries.len(),
        1,
        "expected exactly 1 revision after ADD, got: {hist:?}"
    );
    assert_eq!(
        hist.entries[0].revision, rev_add,
        "history head should be the ADD revision: {hist:?}"
    );

    // Latest revision's message + author + tracked file.
    let info_add = rev_info(&alice, &rev_add).await;
    assert_eq!(
        info_add.message(),
        Some("add foo"),
        "ADD revision message mismatch: {info_add:?}"
    );
    assert_eq!(
        info_add.author(),
        Some("alice"),
        "ADD revision author mismatch: {info_add:?}"
    );
    assert!(
        delta_touches(&info_add, "foo.txt"),
        "ADD revision should track foo.txt in its delta: {info_add:?}"
    );

    // ======================== UPDATE ========================================
    write_file(&file_path, V2);
    let staged = stage_path(&alice, &file_path).await;
    assert!(
        staged.files.iter().any(|f| f.path.ends_with("foo.txt")),
        "stage did not report foo.txt on UPDATE: {staged:?}"
    );
    let rev_update = commit(&alice, "update foo").await;
    assert_ne!(
        rev_update, rev_add,
        "UPDATE must produce a new, distinct revision"
    );

    // On-disk content reflects the update.
    let on_disk = std::fs::read(&file_path).expect("read updated foo.txt");
    assert_eq!(
        on_disk, V2,
        "working-tree content should be V2 after UPDATE"
    );

    // History now shows exactly 2 commits, newest-first, correctly ordered.
    let hist = history(&alice).await;
    assert_eq!(
        hist.entries.len(),
        2,
        "expected exactly 2 revisions after UPDATE, got: {hist:?}"
    );
    assert_eq!(
        hist.entries[0].revision, rev_update,
        "history[0] should be the UPDATE revision"
    );
    assert_eq!(
        hist.entries[1].revision, rev_add,
        "history[1] should be the ADD revision"
    );
    // The UPDATE's parent is the ADD revision — proves the chain links up.
    assert!(
        hist.entries[0].parents.iter().any(|p| p == &rev_add),
        "UPDATE revision's parent should be the ADD revision: {hist:?}"
    );

    let info_update = rev_info(&alice, &rev_update).await;
    assert_eq!(
        info_update.message(),
        Some("update foo"),
        "UPDATE revision message mismatch: {info_update:?}"
    );

    // ======================== DELETE ========================================
    // lore tracks a deletion as: remove the file from disk, then stage the
    // removal. `file::stage` reconciles the staged path against the filesystem
    // — a tracked path that is gone from disk is staged as a Delete.
    std::fs::remove_file(&file_path).expect("remove foo.txt from disk");
    let staged = stage_path(&alice, &file_path).await;
    assert!(
        staged.files.iter().any(|f| {
            f.path.ends_with("foo.txt") && f.action == ops::file::stage::FileStageAction::Delete
        }),
        "stage should report foo.txt as a Delete on DELETE: {staged:?}"
    );
    let rev_delete = commit(&alice, "delete foo").await;
    assert_ne!(
        rev_delete, rev_update,
        "DELETE must produce a new, distinct revision"
    );

    // The delete revision records foo.txt with a Delete action in its delta.
    let info_delete = rev_info(&alice, &rev_delete).await;
    assert_eq!(
        info_delete.message(),
        Some("delete foo"),
        "DELETE revision message mismatch: {info_delete:?}"
    );
    let delete_delta = info_delete
        .deltas
        .iter()
        .find(|d| d.path.ends_with("foo.txt"));
    assert!(
        delete_delta.is_some(),
        "DELETE revision should record foo.txt in its delta: {info_delete:?}"
    );
    assert_eq!(
        delete_delta.map(|d| d.action.as_str()),
        Some("Delete"),
        "DELETE revision's foo.txt action should be Delete: {info_delete:?}"
    );

    // History grew to 3.
    let hist = history(&alice).await;
    assert_eq!(
        hist.entries.len(),
        3,
        "expected exactly 3 revisions after DELETE, got: {hist:?}"
    );

    // ======================== RESTORE (deleted file) ========================
    // Restore the DELETED file's content from the UPDATE revision back onto
    // disk via `file::write` (path + revision), then stage + commit it back.
    let restore = ops::file::write::write(
        &alice,
        ops::file::write::FileWriteArgs {
            address: String::new(),
            // Upstream resolves `path` against the working dir, so pass the
            // absolute path (matching lore's own file_write test).
            path: file_path.to_string_lossy().into_owned(),
            revision: rev_update.clone(),
            output: file_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("file::write should restore foo.txt from the UPDATE revision");
    assert!(
        restore.path.ends_with("foo.txt"),
        "file::write should report writing foo.txt: {restore:?}"
    );

    // The restored on-disk content must match the pre-delete (V2) version.
    let restored = std::fs::read(&file_path).expect("read restored foo.txt");
    assert_eq!(
        restored, V2,
        "RESTORE content must match the pre-delete (V2) version"
    );

    // Commit the restoration so history reflects it.
    let staged = stage_path(&alice, &file_path).await;
    assert!(
        staged.files.iter().any(|f| f.path.ends_with("foo.txt")),
        "stage did not report foo.txt on RESTORE: {staged:?}"
    );
    let rev_restore = commit(&alice, "restore foo").await;
    let info_restore = rev_info(&alice, &rev_restore).await;
    assert_eq!(
        info_restore.message(),
        Some("restore foo"),
        "RESTORE revision message mismatch: {info_restore:?}"
    );
    assert!(
        delta_touches(&info_restore, "foo.txt"),
        "RESTORE revision should track foo.txt (re-added): {info_restore:?}"
    );

    // ======================== FINAL HISTORY AUDIT ===========================
    // Exactly four commits, in order, with the expected messages and author.
    let hist = history(&alice).await;
    assert_eq!(
        hist.entries.len(),
        4,
        "expected exactly 4 revisions in final history, got: {hist:?}"
    );
    let expected_newest_first = [
        (&rev_restore, "restore foo"),
        (&rev_delete, "delete foo"),
        (&rev_update, "update foo"),
        (&rev_add, "add foo"),
    ];
    for (i, (expected_rev, expected_msg)) in expected_newest_first.iter().enumerate() {
        assert_eq!(
            &hist.entries[i].revision, *expected_rev,
            "history[{i}] revision mismatch: {hist:?}"
        );
        let info = rev_info(&alice, &hist.entries[i].revision).await;
        assert_eq!(
            info.message(),
            Some(*expected_msg),
            "history[{i}] message mismatch"
        );
        assert_eq!(
            info.author(),
            Some("alice"),
            "history[{i}] author should be alice"
        );
    }
    // Revision numbers strictly increase from oldest to newest.
    assert!(
        hist.entries[0].revision_number > hist.entries[3].revision_number,
        "newest revision_number must exceed oldest: {hist:?}"
    );
}

/// Server↔client equivalent (CI-headless gate): two repos sharing a single
/// on-disk store, OFFLINE. Repo A commits; repo B `sync`s and observes A's
/// revision — purely through the shared mutable+immutable store, with NO
/// server process.
///
/// This is the closest CI-runnable stand-in for the real network server↔client
/// flow. The genuine networked path (a QUIC `lore` server + `repository::clone`
/// of a `lore://host/repo` URL + token auth + `branch::push`) requires TLS
/// material and a live server and is documented as a deferred gap in the module
/// docs / final report — `branch::push` itself fails offline with `NoRemote`.
#[tokio::test]
async fn multi_repo_shared_store_sync_observes_remote_commit() {
    // One shared store, two separate working trees, all in disjoint temp dirs.
    let store = tempfile::tempdir().expect("create store tempdir");
    let work_a = tempfile::tempdir().expect("create work-a tempdir");
    let work_b = tempfile::tempdir().expect("create work-b tempdir");
    let store_path = store.path().join("shared-store");

    let repo_a = work_a.path().to_path_buf();
    let repo_b = work_b.path().to_path_buf();

    let api_a = on_disk_api(&repo_a, "host");
    let api_b = on_disk_api(&repo_b, "client");

    // Host (A) creates the shared store and a repo backed by it. Capture the
    // repo id so the client (B) joins the SAME repository.
    ops::shared_store::create::create(
        &api_a,
        ops::shared_store::create::SharedStoreCreateArgs {
            remote_url: String::new(),
            path: Some(store_path.to_string_lossy().into_owned()),
            make_default: false,
        },
    )
    .await
    .expect("shared_store::create should succeed");

    let repo_url = format!("lore://localhost/e2e-share-{}", std::process::id());
    let created_a = ops::repository::create::create(
        &api_a,
        ops::repository::create::CreateArgs {
            repository_url: repo_url.clone(),
            description: "shared-store host repo".into(),
            id: String::new(),
            use_shared_store: true,
            shared_store_path: store_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("repository::create (host) should succeed");
    assert!(
        !created_a.id.is_empty(),
        "host repo id empty: {created_a:?}"
    );

    // Client (B) joins the SAME repository (same id) on its own working tree,
    // backed by the SAME shared store.
    let created_b = ops::repository::create::create(
        &api_b,
        ops::repository::create::CreateArgs {
            repository_url: repo_url,
            description: "shared-store client repo".into(),
            id: created_a.id.clone(),
            use_shared_store: true,
            shared_store_path: store_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("repository::create (client, same id) should succeed");
    assert_eq!(
        created_b.id, created_a.id,
        "client must join the host's repository id"
    );

    // Host commits a file.
    let host_file = repo_a.join("shared.txt");
    write_file(&host_file, b"authored on the host\n");
    let staged = stage_path(&api_a, &host_file).await;
    assert!(
        staged.files.iter().any(|f| f.path.ends_with("shared.txt")),
        "host stage did not report shared.txt: {staged:?}"
    );
    let host_rev = commit(&api_a, "host commit").await;

    // Client syncs — offline, this reads the branch tip from the SHARED mutable
    // store and pulls fragments from the shared immutable store.
    let synced =
        ops::revision::sync::sync(&api_b, ops::revision::sync::RevisionSyncArgs::default())
            .await
            .expect("revision::sync (client) should succeed against the shared store");

    // The client now sees the host's revision as the synced tip.
    assert!(
        synced.revisions.iter().any(|r| r.revision == host_rev),
        "client sync should observe the host's revision {host_rev}: {synced:?}"
    );

    // And the client's history contains the host's commit, with the host as its
    // author — proving cross-repo visibility through the shared store alone.
    let hist_b = history(&api_b).await;
    assert!(
        hist_b.entries.iter().any(|e| e.revision == host_rev),
        "client history should contain the host revision {host_rev}: {hist_b:?}"
    );
    let info_b = rev_info(&api_b, &host_rev).await;
    assert_eq!(
        info_b.message(),
        Some("host commit"),
        "client should see the host commit message: {info_b:?}"
    );
    assert_eq!(
        info_b.author(),
        Some("host"),
        "client should see the host as the revision author: {info_b:?}"
    );

    // The host's file content should land in the client's working tree.
    let client_file = repo_b.join("shared.txt");
    assert!(
        client_file.exists(),
        "client working tree should contain shared.txt after sync"
    );
    assert_eq!(
        std::fs::read(&client_file).expect("read client shared.txt"),
        b"authored on the host\n",
        "client file content should match the host's commit"
    );
}

/// REVERT of an individual change: commit two distinct files, then revert only
/// the SECOND change with `revision::revert_local` (which auto-commits the
/// inverse). Asserts the reverted file is gone from the working tree while the
/// untouched file survives, and that history records the auto-commit.
#[tokio::test]
async fn revert_local_reverts_a_single_change() {
    let work = tempfile::tempdir().expect("create work tempdir");
    let store = tempfile::tempdir().expect("create store tempdir");
    let repo_path = work.path().to_path_buf();
    let store_path = store.path().join("shared-store");

    let bob = on_disk_api(&repo_path, "bob");

    ops::shared_store::create::create(
        &bob,
        ops::shared_store::create::SharedStoreCreateArgs {
            remote_url: String::new(),
            path: Some(store_path.to_string_lossy().into_owned()),
            make_default: false,
        },
    )
    .await
    .expect("shared_store::create should succeed");

    let name = format!("e2e-revert-{}", std::process::id());
    ops::repository::create::create(
        &bob,
        ops::repository::create::CreateArgs {
            repository_url: format!("lore://localhost/{name}"),
            description: "lore-vm e2e revert repo".into(),
            id: String::new(),
            use_shared_store: true,
            shared_store_path: store_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("repository::create should succeed");

    // Commit #1: keep.txt — the change we will NOT revert.
    let keep = repo_path.join("keep.txt");
    write_file(&keep, b"keep me\n");
    stage_path(&bob, &keep).await;
    let _rev_keep = commit(&bob, "add keep").await;

    // Commit #2: drop.txt — the change we WILL revert.
    let drop = repo_path.join("drop.txt");
    write_file(&drop, b"drop me\n");
    stage_path(&bob, &drop).await;
    let rev_drop = commit(&bob, "add drop").await;

    let hist_before = history(&bob).await;
    assert_eq!(
        hist_before.entries.len(),
        2,
        "expected 2 revisions before revert: {hist_before:?}"
    );

    // Revert ONLY the second change. With no conflicts, this auto-commits the
    // inverse (removing drop.txt) on top of history.
    let reverted = ops::revision::revert_local::revert_local(
        &bob,
        ops::revision::revert_local::RevertLocalArgs {
            revision: rev_drop.clone(),
            message: "revert add drop".into(),
            no_commit: false,
        },
    )
    .await
    .expect("revision::revert_local should succeed");
    assert!(
        !reverted.has_conflicts,
        "revert of an isolated add should not conflict: {reverted:?}"
    );
    assert!(
        reverted.committed_revision.is_some(),
        "revert with no_commit=false should auto-commit: {reverted:?}"
    );

    // The reverted file is gone; the untouched file remains.
    assert!(
        !drop.exists(),
        "drop.txt should be removed from the working tree after revert"
    );
    assert!(
        keep.exists(),
        "keep.txt must survive a revert of an unrelated change"
    );

    // History grew by exactly one (the auto-committed inverse).
    let hist_after = history(&bob).await;
    assert_eq!(
        hist_after.entries.len(),
        3,
        "expected 3 revisions after revert auto-commit: {hist_after:?}"
    );
    let revert_rev = reverted.committed_revision.expect("auto-commit revision");
    assert_eq!(
        hist_after.entries[0].revision, revert_rev,
        "history head should be the revert auto-commit"
    );
    let info_revert = rev_info(&bob, &revert_rev).await;
    assert_eq!(
        info_revert.author(),
        Some("bob"),
        "revert auto-commit author should be bob"
    );
}
