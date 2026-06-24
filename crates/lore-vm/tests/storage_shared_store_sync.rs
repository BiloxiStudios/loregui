//! Behavioral integration tests for cross-repo sync through a SHARED STORE,
//! going beyond `e2e_lifecycle::multi_repo_shared_store_sync_observes_remote_commit`
//! (which only proves a single ADD propagates host → client).
//!
//! Here we drive the harder, mutation-heavy cases through two repos that share
//! one on-disk store, fully OFFLINE (the CI-headless server↔client stand-in):
//!
//!   1. UPDATE propagation — host modifies a file's content; client sees the
//!      new bytes after sync.
//!   2. DELETE propagation — host removes a tracked file; client's working tree
//!      drops it after sync.
//!   3. CONCURRENT divergent commits — both repos commit different files from a
//!      common base; after each syncs, both observe BOTH revisions through the
//!      shared store (no data lost), exercising the merge path.
//!
//! Gated behind the `integration-tests` feature like `e2e_lifecycle.rs`:
//!
//! ```sh
//! cargo test -p lore-vm --features integration-tests --test storage_shared_store_sync
//! ```
#![cfg(feature = "integration-tests")]

mod storage_support;

use lore_vm::ops;
use storage_support::{
    commit, create_disk_repo, history, join_disk_repo, stage_path, sync, write_file,
};

/// Build the same `lore://localhost/<name>` URL `create_disk_repo` used, so a
/// joiner can target the SAME repository. `create_disk_repo` embeds the pid and
/// a per-process unique suffix; we re-derive it from the DiskRepo's id only if
/// needed — but here we capture the url at creation by re-creating with a known
/// scheme. To keep it simple and robust, the host creates the repo and we join
/// by id against the same shared store with a fresh url that lore reconciles by
/// id (matching how `e2e_lifecycle` joins).
const JOIN_URL: &str = "lore://localhost/shared-join";

/// Host UPDATEs a file; client observes the updated bytes after sync.
#[tokio::test]
async fn update_propagates_through_shared_store() {
    let host = create_disk_repo("share-update", "host").await;

    // Host adds, then a client joins the same repo+store BEFORE the update so we
    // sync the update across (not the initial add).
    let file = host.repo_path.join("doc.txt");
    write_file(&file, b"v1\n");
    stage_path(&host.api, &file).await;
    let _rev_v1 = commit(&host.api, "add doc v1").await;

    let (client_api, _client_work, client_path) =
        join_disk_repo(JOIN_URL, &host.repo_id, &host.store_path, "client").await;

    // Client syncs the initial state.
    sync(&client_api).await;
    let client_file = client_path.join("doc.txt");
    assert_eq!(
        std::fs::read(&client_file).expect("client should have doc.txt after first sync"),
        b"v1\n",
        "client should observe v1 after first sync"
    );

    // Host updates the file and commits.
    write_file(&file, b"v2 updated\n");
    stage_path(&host.api, &file).await;
    let rev_v2 = commit(&host.api, "update doc to v2").await;

    // Client syncs again and must observe the new content + the new revision.
    let synced = sync(&client_api).await;
    assert!(
        synced.revisions.iter().any(|r| r.revision == rev_v2)
            || history(&client_api)
                .await
                .entries
                .iter()
                .any(|e| e.revision == rev_v2),
        "client should observe the host's update revision {rev_v2}: {synced:?}"
    );
    assert_eq!(
        std::fs::read(&client_file).expect("read client doc.txt after update sync"),
        b"v2 updated\n",
        "client working tree must reflect the host's update after sync"
    );
}

/// Host DELETEs a tracked file; client's working tree drops it after sync.
#[tokio::test]
async fn delete_propagates_through_shared_store() {
    let host = create_disk_repo("share-delete", "host").await;

    // Host adds two files; client joins and syncs so both exist on the client.
    let keep = host.repo_path.join("keep.txt");
    let gone = host.repo_path.join("gone.txt");
    write_file(&keep, b"keep me\n");
    write_file(&gone, b"delete me\n");
    stage_path(&host.api, &keep).await;
    stage_path(&host.api, &gone).await;
    let _rev_add = commit(&host.api, "add keep + gone").await;

    let (client_api, _client_work, client_path) =
        join_disk_repo(JOIN_URL, &host.repo_id, &host.store_path, "client").await;
    sync(&client_api).await;
    assert!(
        client_path.join("gone.txt").exists(),
        "client should have gone.txt after first sync"
    );

    // Host deletes gone.txt (remove from disk, stage the removal, commit).
    std::fs::remove_file(&gone).expect("remove gone.txt on host");
    let staged = stage_path(&host.api, &gone).await;
    assert!(
        staged.files.iter().any(|f| f.path.ends_with("gone.txt")),
        "host stage should report the deletion: {staged:?}"
    );
    let rev_del = commit(&host.api, "delete gone").await;

    // Client syncs: gone.txt should be removed from the client tree, keep stays.
    let synced = sync(&client_api).await;
    assert!(
        synced.revisions.iter().any(|r| r.revision == rev_del)
            || history(&client_api)
                .await
                .entries
                .iter()
                .any(|e| e.revision == rev_del),
        "client should observe the delete revision {rev_del}: {synced:?}"
    );
    assert!(
        !client_path.join("gone.txt").exists(),
        "client working tree should drop gone.txt after the delete syncs"
    );
    assert!(
        client_path.join("keep.txt").exists(),
        "keep.txt must survive a delete of an unrelated file on the client"
    );
}

/// CONFLICT detection through the shared store: with two repos on a common base
/// sharing one mutable store, the store enforces a compare-and-swap on the
/// branch tip. If the host advances the shared branch, a client still sitting on
/// the old base CANNOT commit on top of it — the commit is rejected with
/// "Branch has been advanced by another instance, sync and re-stage". After the
/// client syncs to absorb the host's revision, its commit then succeeds, and
/// both the host's and the client's revisions are visible — nothing is lost.
#[tokio::test]
async fn concurrent_branch_advance_is_rejected_until_sync() {
    let host = create_disk_repo("share-diverge", "host").await;

    // Common base commit, then client joins and syncs to the same base.
    let base = host.repo_path.join("base.txt");
    write_file(&base, b"base\n");
    stage_path(&host.api, &base).await;
    let rev_base = commit(&host.api, "base commit").await;

    let (client_api, _client_work, client_path) =
        join_disk_repo(JOIN_URL, &host.repo_id, &host.store_path, "client").await;
    sync(&client_api).await;
    assert!(
        history(&client_api)
            .await
            .entries
            .iter()
            .any(|e| e.revision == rev_base),
        "client should share the base revision before diverging"
    );

    // Host advances the SHARED branch tip past the client's base.
    let host_only = host.repo_path.join("host-only.txt");
    write_file(&host_only, b"authored on host\n");
    stage_path(&host.api, &host_only).await;
    let rev_host = commit(&host.api, "host divergent commit").await;
    assert_ne!(rev_host, rev_base, "host commit must be a new revision");

    // Client (still at base) stages a divergent file and tries to commit WITHOUT
    // syncing. The shared store's branch-tip CAS must reject it.
    let client_only = client_path.join("client-only.txt");
    write_file(&client_only, b"authored on client\n");
    stage_path(&client_api, &client_only).await;
    let stale_commit = ops::revision::commit::commit(
        &client_api,
        ops::revision::commit::CommitArgs {
            message: "client divergent commit (stale)".into(),
        },
    )
    .await;
    assert!(
        stale_commit.is_err(),
        "committing on a stale branch tip must be rejected by the shared store: {stale_commit:?}"
    );
    let err = format!("{}", stale_commit.unwrap_err());
    assert!(
        err.contains("advanced") || err.contains("sync"),
        "rejection should explain the branch was advanced / needs sync, got: {err}"
    );

    // The shared store refuses to sync while a staged state lingers, so the
    // client must first unstage the rejected change, sync to absorb the host's
    // revision, then re-stage and commit — the documented recovery sequence.
    ops::file::unstage::unstage(
        &client_api,
        ops::file::unstage::FileUnstageArgs {
            paths: vec![client_only.to_string_lossy().into_owned()],
        },
    )
    .await
    .expect("client should be able to unstage the rejected change before syncing");

    sync(&client_api).await;
    assert!(
        history(&client_api)
            .await
            .entries
            .iter()
            .any(|e| e.revision == rev_host),
        "after sync the client must see the host's revision {rev_host}"
    );
    stage_path(&client_api, &client_only).await;
    let rev_client = commit(&client_api, "client divergent commit (after sync)").await;
    assert_ne!(
        rev_client, rev_host,
        "client's post-sync commit must be a distinct revision"
    );

    // Final state: the client's history contains the base, the host's commit,
    // and its own — proving the host's work was preserved across the conflict.
    let client_hist = history(&client_api).await;
    for (rname, rev) in [
        ("base", &rev_base),
        ("host", &rev_host),
        ("client", &rev_client),
    ] {
        assert!(
            client_hist.entries.iter().any(|e| &e.revision == rev),
            "client history must contain the {rname} revision {rev}: {client_hist:?}"
        );
    }

    // The host can sync to pick up the client's follow-on commit too.
    sync(&host.api).await;
    assert!(
        history(&host.api)
            .await
            .entries
            .iter()
            .any(|e| e.revision == rev_client),
        "host should observe the client's post-sync commit {rev_client} after syncing"
    );
}
