//! Live networked server↔client round-trip harness (SBAI-4064 spike).
//!
//! This is the genuine networked path that the CI-gated
//! `tests/e2e_lifecycle.rs::multi_repo_shared_store_sync_observes_remote_commit`
//! test deliberately stands in for with a *shared on-disk store* (because CI has
//! no live server). Here we drive the REAL loop against a running `loreserver`
//! QUIC/gRPC process:
//!
//!   auth probe: verify the server's explicit no-auth compatibility signal
//!   client A:  repository::create  (lore://host/<repo>)
//!              → write file → file::stage → revision::commit
//!              → branch::push          (uploads revision to the server OVER THE WIRE)
//!   client B:  repository::clone (lore://host/<repo>)   (pulls OVER THE WIRE)
//!              → assert the file + revision authored by A are present
//!
//! Client B has its OWN local store in a separate temp dir and never touches
//! A's store, so B observing A's revision proves a real network round trip
//! through the server, not a shared-local-store shortcut.
//!
//! Run via `scripts/live-server-client.sh`, which boots the server first. To run
//! by hand against an already-running server:
//!
//! ```sh
//! cargo run -p lore-vm --example live_server_client -- \
//!     lore://127.0.0.1:41337/spikerepo /tmp/clientA /tmp/clientB
//! ```
//!
//! Args: <repo_url> <client_a_dir> <client_b_dir>
//!
//! Exit 0 on a fully-verified round trip; non-zero with a diagnostic otherwise.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lore_vm::api::LoreApi;
use lore_vm::global::LoreGlobal;
use lore_vm::ops;

/// An ONLINE api (offline=false) so push/clone actually hit the server. The
/// identity flows through as the revision author and the connection identity;
/// against a no-auth dev server any non-empty value is accepted.
fn online_api(dir: &Path, identity: &str) -> LoreApi {
    let global = LoreGlobal::new(dir.to_path_buf())
        .in_memory(false)
        .offline(false)
        .identity(identity);
    LoreApi::from_global(global)
}

fn write_file(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    std::fs::write(path, contents).expect("write file");
}

async fn run(repo_url: &str, dir_a: &Path, dir_b: &Path) -> Result<(), String> {
    let alice = online_api(dir_a, "alice");

    // ---- compatibility probe: reachable server intentionally has no auth ---
    println!("[auth] login_interactive against no-auth server");
    let login = ops::auth::login_interactive::login_interactive(
        &alice,
        ops::auth::login_interactive::LoginInteractiveArgs {
            remote_url: repo_url.to_string(),
            no_browser: true,
        },
    )
    .await;
    match login {
        Err(lore_vm::error::LoreError::CommandFailed(message))
            if message == "No authentication configured on server" =>
        {
            println!("[auth] verified exact no-auth server response");
        }
        other => {
            return Err(format!(
                "auth compatibility probe returned {other:?}, expected exact no-auth response"
            ));
        }
    }

    // ---- client A: create a repo tracking the remote URL --------------------
    println!("[A] repository::create {repo_url}");
    let created = ops::repository::create::create(
        &alice,
        ops::repository::create::CreateArgs {
            repository_url: repo_url.to_string(),
            description: "live-server spike repo".into(),
            id: String::new(),
            use_shared_store: false,
            shared_store_path: String::new(),
        },
    )
    .await
    .map_err(|e| format!("A repository::create failed: {e}"))?;
    println!("[A] created repo id={} name={}", created.id, created.name);

    // ---- client A: write + stage + commit (one process keeps staged state) --
    let file_path = dir_a.join("hello.txt");
    const CONTENT: &[u8] = b"hello from the live spike\nround trip test\n";
    write_file(&file_path, CONTENT);

    println!("[A] file::stage hello.txt");
    let staged = ops::file::stage::stage(
        &alice,
        ops::file::stage::FileStageArgs {
            paths: vec![file_path.to_string_lossy().into_owned()],
            case_change: ops::file::stage::CaseChange::Error,
            scan: true,
        },
    )
    .await
    .map_err(|e| format!("A file::stage failed: {e}"))?;
    if !staged.files.iter().any(|f| f.path.ends_with("hello.txt")) {
        return Err(format!("A stage did not report hello.txt: {staged:?}"));
    }

    println!("[A] revision::commit");
    let commit = ops::revision::commit::commit(
        &alice,
        ops::revision::commit::CommitArgs {
            message: "initial commit from alice".into(),
        },
    )
    .await
    .map_err(|e| format!("A revision::commit failed: {e}"))?;
    if commit.revision.is_empty() {
        return Err(format!("A commit returned empty revision: {commit:?}"));
    }
    let pushed_rev = commit.revision.clone();
    println!("[A] committed revision {pushed_rev}");

    // ---- client A: push to the server OVER THE WIRE -------------------------
    println!("[A] branch::push  → {repo_url}");
    let push = ops::branch::push::push(
        &alice,
        ops::branch::push::BranchPushArgs {
            branch: String::new(),
            fast_forward_merge: false,
        },
    )
    .await
    .map_err(|e| format!("A branch::push failed (server not reachable / auth?): {e}"))?;
    println!(
        "[A] pushed branch={} local_rev={} remote_rev={} already_pushed={}",
        push.branch_name, push.local_revision, push.remote_revision, push.already_pushed
    );

    // ---- client B: clone the SAME URL into a fresh dir OVER THE WIRE --------
    let bob = online_api(dir_b, "bob");
    println!("[B] repository::clone {repo_url}  (separate local store)");
    let cloned = ops::repository::clone::clone(
        &bob,
        ops::repository::clone::CloneArgs {
            repository_url: repo_url.to_string(),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| format!("B repository::clone failed: {e}"))?;
    println!(
        "[B] cloned repo={} branch={} revision={} path={}",
        cloned.repository, cloned.branch, cloned.revision, cloned.path
    );

    // ---- PROOF: B sees A's revision + file content, pulled over the wire ----
    if cloned.revision != pushed_rev {
        return Err(format!(
            "B cloned revision {} != pushed revision {pushed_rev}",
            cloned.revision
        ));
    }

    let cloned_file = dir_b.join("hello.txt");
    if !cloned_file.exists() {
        return Err(format!(
            "B working tree missing hello.txt after clone (path {})",
            cloned_file.display()
        ));
    }
    let got = std::fs::read(&cloned_file).map_err(|e| format!("read cloned file: {e}"))?;
    if got != CONTENT {
        return Err(format!(
            "B file content mismatch: got {:?}",
            String::from_utf8_lossy(&got)
        ));
    }

    // The cloned revision's author/message must match A's commit — proves the
    // revision metadata (not just bytes) crossed the network.
    let info = ops::revision::info::info(
        &bob,
        ops::revision::info::RevisionInfoArgs {
            revision: pushed_rev.clone(),
            delta: true,
            metadata: true,
        },
    )
    .await
    .map_err(|e| format!("B revision::info failed: {e}"))?;
    if info.author() != Some("alice") {
        return Err(format!("B saw author {:?}, expected alice", info.author()));
    }
    if info.message() != Some("initial commit from alice") {
        return Err(format!("B saw message {:?}", info.message()));
    }

    println!();
    println!("ROUND TRIP VERIFIED:");
    println!("  revision {pushed_rev} authored by alice");
    println!(
        "  pushed from {} → cloned into {}",
        dir_a.display(),
        dir_b.display()
    );
    println!("  file hello.txt content matches, over the network via {repo_url}");
    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let repo_url = match args.next() {
        Some(u) => u,
        None => {
            eprintln!(
                "usage: live_server_client <repo_url> <client_a_dir> <client_b_dir>\n\
                 e.g.   live_server_client lore://127.0.0.1:41337/spikerepo /tmp/A /tmp/B"
            );
            return ExitCode::FAILURE;
        }
    };
    let dir_a = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "/tmp/lore-spike/clientA".into()),
    );
    let dir_b = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "/tmp/lore-spike/clientB".into()),
    );

    match run(&repo_url, &dir_a, &dir_b).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\nSPIKE FAILED: {e}");
            ExitCode::FAILURE
        }
    }
}
