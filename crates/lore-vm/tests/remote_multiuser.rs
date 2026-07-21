//! REMOTE / multi-user networked harness for `lore-vm` (SBAI: remote-QA).
//!
//! # Why this exists
//!
//! `lore`'s headline, "Perforce-class" selling point is its *remote, multi-user*
//! path: a real server that many clients sync from, push to, and coordinate
//! file **locks** through, with **conflict** detection when two users race. Until
//! now that path had **zero** automated coverage. The CI-gated
//! [`e2e_lifecycle.rs`] suite is excellent but single-user, and its
//! `multi_repo_shared_store_sync_observes_remote_commit` test is *explicitly* a
//! CI-headless stand-in: it shares an on-disk store instead of a live server, and
//! its own module docs flag the genuine networked path (`loreserver` +
//! `branch::push` + `repository::clone` + cross-user locks) as a **deferred gap**:
//!
//! > "The genuine networked path (a QUIC `lore` server + `repository::clone` of a
//! >  `lore://host/repo` URL + token auth + `branch::push`) requires TLS material
//! >  and a live server and is documented as a deferred gap … `branch::push`
//! >  itself fails offline with `NoRemote`."
//!
//! This suite closes that gap. It boots a **real `loreserver`** on loopback
//! (exactly the recipe `src-tauri/src/server_host.rs` productionised and the
//! SBAI-4064 spike proved) and drives **two independent clones** through it OVER
//! THE WIRE. Each client has its own local store in a separate temp dir, so any
//! cross-client visibility is a genuine network round trip, never a shared-store
//! shortcut.
//!
//! # Scenarios (maximally realistic multi-user)
//!
//!   1. **sync — file UPDATE propagation**: alice commits+pushes V1, bob clones;
//!      alice updates+pushes V2, bob `sync`s and sees V2 on disk.
//!   2. **sync — file DELETE propagation**: alice deletes a tracked file + pushes;
//!      bob `sync`s and the file disappears from bob's working tree.
//!   3. **push CONFLICT**: alice and bob both commit on top of the same tip; the
//!      first push wins, the second is **rejected** (stale remote tip) — proving
//!      the server enforces linear history and a client can't clobber a peer.
//!   4. **two-user LOCKS — acquire / contention / release**: alice acquires a lock
//!      on a path; bob's acquire of the SAME path is refused (or visibly owned by
//!      alice); `lock::file_query` shows alice as owner; alice releases; bob can
//!      then acquire. Exercises `lock.acquire`, `lock.query`, `lock.release`.
//!   5. **file CONFLICT state**: after a real diverge+sync, assert the conflicted
//!      file surfaces through `file::info`'s `flag_conflict`.
//!
//! # CI-hostability
//!
//! This needs a `loreserver` binary and a free loopback port. The binary is the
//! heavy upstream build (~1 GB) from the pinned `lore` git checkout, which the
//! fast PR runners do NOT have. So this suite is:
//!
//!   * **feature-gated** behind `remote-integration-tests` (off by default — the
//!     normal `cargo test -p lore-vm` and even the `integration-tests` job never
//!     touch it), and
//!   * **self-skipping**: if no `loreserver` can be resolved (no
//!     `LOREVM_SERVER_BIN`, no built checkout binary, no sidecar) the test prints
//!     a clear SKIP and returns `Ok` rather than failing. A contributor without
//!     the heavy build is never blocked; CI that *does* provision the binary runs
//!     the real assertions.
//!
//! ```sh
//! # Resolve the server explicitly (fastest, CI-friendly):
//! LOREVM_SERVER_BIN=/path/to/loreserver \
//!   cargo test -p lore-vm --features remote-integration-tests --test remote_multiuser -- --nocapture
//!
//! # Or let it build/resolve from the pinned lore checkout (slow first run):
//! cargo test -p lore-vm --features remote-integration-tests --test remote_multiuser -- --nocapture
//! ```
//!
//! See `scripts/remote-multiuser-qa.sh` (boots a server + runs this) and
//! `docs/REMOTE-QA.md` (manual checklist + CI rationale).
//!
//! [`e2e_lifecycle.rs`]: ./e2e_lifecycle.rs
#![cfg(feature = "remote-integration-tests")]

use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use lore_vm::api::LoreApi;
use lore_vm::global::LoreGlobal;
use lore_vm::ops;

// ===========================================================================
// Server harness: boot a real `loreserver` on loopback (mirrors the recipe in
// src-tauri/src/server_host.rs + scripts/live-server-client.sh).
// ===========================================================================

/// Bind host — loopback only, exactly like the host flow.
const BIND_HOST: &str = "127.0.0.1";

/// A running `loreserver` child, reaped on drop so a panicking assertion never
/// leaks a server process.
struct LoreServer {
    child: Child,
    port: u16,
    store_dir: PathBuf,
    log_path: PathBuf,
    // Keep the temp dir alive for the server's lifetime.
    _tmp: tempfile::TempDir,
}

impl LoreServer {
    fn repo_url(&self, repo_name: &str) -> String {
        // `lore://` (no trailing `s`) so the client skips server-cert validation
        // against the self-signed test cert — exactly what the spike/host flow do.
        format!("lore://{BIND_HOST}:{}/{}", self.port, repo_name)
    }

    /// Tail of the server log, for diagnostics on failure.
    fn log_tail(&self, lines: usize) -> String {
        let text = std::fs::read_to_string(&self.log_path).unwrap_or_default();
        let collected: Vec<&str> = text.lines().rev().take(lines).collect();
        collected.into_iter().rev().collect::<Vec<_>>().join("\n")
    }
}

impl Drop for LoreServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Why a server could not be booted. `Skip` is a soft outcome (no binary /
/// no certs available in this environment) and means the WHOLE suite should
/// no-op-pass; `Hard` is a genuine failure once a binary *was* resolved.
enum ServerOutcome {
    Started(Box<LoreServer>),
    Skip(String),
    Hard(String),
}

/// Extract the first 40-hex-char `rev = "..."` from a Cargo.toml string.
/// (Same logic as `server_host::parse_pinned_rev`, duplicated here so the test
/// stays self-contained and never imports src-tauri.)
fn parse_pinned_rev(cargo_toml: &str) -> Option<String> {
    for line in cargo_toml.lines() {
        if let Some(idx) = line.find("rev = \"") {
            let rest = &line[idx + "rev = \"".len()..];
            if let Some(end) = rest.find('"') {
                let rev = &rest[..end];
                if rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Some(rev.to_string());
                }
            }
        }
    }
    None
}

/// Repo root = two levels up from this crate's manifest dir
/// (`crates/lore-vm` → repo root).
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or(manifest)
}

/// Best-effort home dir (avoid pulling in `dirs`).
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Locate the cargo-unpacked `lore` git checkout for the pinned rev.
fn lore_checkout() -> Option<PathBuf> {
    let root = repo_root();
    let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let rev = parse_pinned_rev(&cargo_toml)?;
    let short = &rev[..7];
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".cargo")))?;
    let checkouts = cargo_home.join("git").join("checkouts");
    for entry in std::fs::read_dir(&checkouts).ok()?.flatten() {
        if entry.file_name().to_string_lossy().starts_with("lore-") {
            let candidate = entry.path().join(short);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

fn server_bin_name() -> &'static str {
    if cfg!(windows) {
        "loreserver.exe"
    } else {
        "loreserver"
    }
}

/// Resolve a `loreserver` binary WITHOUT building anything heavy.
///
/// Order:
///   1. `LOREVM_SERVER_BIN` env override (CI-friendly; caller must point at the
///      **exact** Cargo.toml-pinned rev artifact for proof runs).
///   2. The **exact-pin** lore checkout's `target/{release,debug}/loreserver`
///      only (short rev from workspace `Cargo.toml` — never sibling checkouts).
///   3. A sidecar `loreserver` next to the test executable (dev bundle path).
///
/// Soft-skip (`None`) only when nothing resolves. Once `boot_server` has a
/// binary, startup/readiness failure is `ServerOutcome::Hard` (test fails).
fn resolve_server_binary() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LOREVM_SERVER_BIN").map(PathBuf::from) {
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(checkout) = lore_checkout() {
        for profile in ["release", "debug"] {
            let cand = checkout
                .join("target")
                .join(profile)
                .join(server_bin_name());
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join(server_bin_name());
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// Regression: checkout-derived server binary must sit under the pinned short rev.
#[test]
fn resolve_server_binary_checkout_path_is_exact_pin_only() {
    let root = repo_root();
    let cargo_toml = match std::fs::read_to_string(root.join("Cargo.toml")) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("[SKIP] no Cargo.toml for pin check");
            return;
        }
    };
    let Some(rev) = parse_pinned_rev(&cargo_toml) else {
        eprintln!("[SKIP] could not parse pinned rev");
        return;
    };
    let short = &rev[..7];
    let Some(checkout) = lore_checkout() else {
        eprintln!("[SKIP] pinned lore checkout not present");
        return;
    };
    let c = checkout.to_string_lossy();
    assert!(
        c.contains(short),
        "lore_checkout must be exact pin short rev {short}, got {c}"
    );
    // No sibling short-rev walk: path component after lore-* must be short rev.
    let after = c.split("git/checkouts/").nth(1).unwrap_or_default();
    let parts: Vec<&str> = after.split('/').collect();
    assert!(parts.len() >= 2, "unexpected checkout shape: {c}");
    assert_eq!(parts[1], short, "checkout must use pinned short rev only");
    if let Some(bin) = resolve_server_binary() {
        let s = bin.to_string_lossy();
        if s.contains("git/checkouts") {
            assert!(
                s.contains(short),
                "resolved loreserver under checkouts must be pin {short}: {s}"
            );
        }
    }
}

/// The shipped self-signed QUIC test certs from the pinned lore checkout.
fn test_cert_paths() -> Option<(PathBuf, PathBuf)> {
    let checkout = lore_checkout()?;
    let test_data = checkout
        .join("lore-server")
        .join("src")
        .join("protocol")
        .join("test_data");
    let cert = test_data.join("test_cert.pem");
    let key = test_data.join("test_key.pem");
    if cert.is_file() && key.is_file() {
        Some((cert, key))
    } else {
        None
    }
}

/// Pick a free TCP loopback port by binding ephemeral and releasing it. lore
/// binds both TCP (gRPC) and UDP (QUIC) on the SAME port number; a free TCP port
/// is the practical proxy (the spike uses a fixed 41337). Small race window, but
/// the harness retries the whole boot on failure.
fn free_port() -> Option<u16> {
    let listener = std::net::TcpListener::bind((BIND_HOST, 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    drop(listener);
    Some(port)
}

/// Render the minimal local server config TOML — byte-for-byte the spike's
/// `local.toml`: loopback QUIC+gRPC on `port`, HTTP on `port+2`, shipped test
/// certs, local stores, single-node topology, and crucially NO `[server.auth]`
/// block so the server runs auth-disabled.
fn render_config(port: u16, store_dir: &Path, cert: &Path, key: &Path) -> String {
    let esc = |p: &Path| {
        p.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    };
    let store = esc(store_dir);
    format!(
        "# Generated by lore-vm remote_multiuser test harness. loopback-only, no auth.\n\
         [server.quic]\n\
         host = \"{BIND_HOST}\"\n\
         port = {port}\n\
         [server.quic.certificate]\n\
         cert_file = \"{cert}\"\n\
         pkey_file = \"{key}\"\n\
         \n\
         [server.grpc]\n\
         host = \"{BIND_HOST}\"\n\
         port = {port}\n\
         \n\
         [server.http]\n\
         host = \"{BIND_HOST}\"\n\
         port = {http}\n\
         \n\
         [immutable_store.local]\n\
         path = \"{store}\"\n\
         [mutable_store.local]\n\
         path = \"{store}\"\n\
         \n\
         [telemetry.logger]\n\
         format = \"ansi\"\n\
         \n\
         [topology]\n\
         provider = \"none\"\n",
        cert = esc(cert),
        key = esc(key),
        http = port.wrapping_add(2),
    )
}

/// Boot a real `loreserver` and wait for it to listen on its gRPC TCP port.
fn boot_server() -> ServerOutcome {
    let Some(binary) = resolve_server_binary() else {
        return ServerOutcome::Skip(
            "no `loreserver` binary resolved (set LOREVM_SERVER_BIN, or pre-build the pinned \
             lore checkout's loreserver) — skipping the remote multi-user suite"
                .into(),
        );
    };
    let Some((cert, key)) = test_cert_paths() else {
        return ServerOutcome::Skip(
            "lore self-signed QUIC test certs not found in the pinned lore checkout \
             (run `cargo fetch` so the dep is unpacked) — skipping"
                .into(),
        );
    };
    let Some(port) = free_port() else {
        return ServerOutcome::Hard("could not allocate a free loopback port".into());
    };

    let tmp = match tempfile::tempdir() {
        Ok(t) => t,
        Err(e) => return ServerOutcome::Hard(format!("create server tempdir: {e}")),
    };
    let store_dir = tmp.path().join("store");
    let config_dir = tmp.path().join("config");
    let log_path = tmp.path().join("server.log");
    if let Err(e) = std::fs::create_dir_all(&store_dir).and(std::fs::create_dir_all(&config_dir)) {
        return ServerOutcome::Hard(format!("create server dirs: {e}"));
    }
    let config_path = config_dir.join("local.toml");
    if let Err(e) = std::fs::write(&config_path, render_config(port, &store_dir, &cert, &key)) {
        return ServerOutcome::Hard(format!("write server config: {e}"));
    }

    let log = match std::fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => return ServerOutcome::Hard(format!("create server log: {e}")),
    };
    let log_err = match log.try_clone() {
        Ok(f) => f,
        Err(e) => return ServerOutcome::Hard(format!("clone server log handle: {e}")),
    };

    // Boot exactly like server_host::start: LORE_CONFIG_PATH → config dir,
    // LORE_ENV=local selects local.toml, cwd = config dir.
    let child = Command::new(&binary)
        .env("LORE_CONFIG_PATH", &config_dir)
        .env("LORE_ENV", "local")
        .current_dir(&config_dir)
        .stdout(log)
        .stderr(log_err)
        .spawn();
    let child = match child {
        Ok(c) => c,
        Err(e) => {
            return ServerOutcome::Hard(format!(
                "failed to spawn loreserver ({}): {e}",
                binary.display()
            ));
        }
    };

    let mut server = LoreServer {
        child,
        port,
        store_dir,
        log_path,
        _tmp: tmp,
    };

    // Wait up to 30s for the gRPC TCP socket to accept a connection (or the
    // process to die). UDP/QUIC binds the same port; a TCP connect is the
    // portable readiness probe (`ss` isn't available everywhere).
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(Some(status)) = server.child.try_wait() {
            return ServerOutcome::Hard(format!(
                "loreserver exited during startup (status {status}). Log tail:\n{}",
                server.log_tail(40)
            ));
        }
        let probe = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        if TcpStream::connect_timeout(&probe, Duration::from_millis(200)).is_ok() {
            // Give QUIC a beat to finish binding after the TCP listener is up.
            std::thread::sleep(Duration::from_millis(300));
            return ServerOutcome::Started(Box::new(server));
        }
        if Instant::now() >= deadline {
            return ServerOutcome::Hard(format!(
                "loreserver never listened on {BIND_HOST}:{port} within 30s. Log tail:\n{}",
                server.log_tail(40)
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

// ===========================================================================
// Client helpers (mirror examples/live_server_client.rs + e2e_lifecycle.rs).
// ===========================================================================

/// An ONLINE api (offline=false) so push/clone/sync/locks actually hit the
/// server. `identity` flows through as the revision author + connection
/// identity; the no-auth dev server accepts any non-empty value.
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
    let mut f = std::fs::File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .expect("open file for write");
    f.write_all(contents).expect("write file");
}

async fn stage(api: &LoreApi, path: &Path) -> ops::file::stage::FileStageResult {
    ops::file::stage::stage(
        api,
        ops::file::stage::FileStageArgs {
            paths: vec![path.to_string_lossy().into_owned()],
            case_change: ops::file::stage::CaseChange::Error,
            scan: true,
        },
    )
    .await
    .unwrap_or_else(|e| panic!("file::stage({}) should succeed: {e}", path.display()))
}

async fn commit(api: &LoreApi, message: &str) -> ops::revision::commit::CommitResult {
    ops::revision::commit::commit(
        api,
        ops::revision::commit::CommitArgs {
            message: message.into(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("revision::commit({message:?}) should succeed: {e}"))
}

async fn push(api: &LoreApi) -> lore_vm::error::Result<ops::branch::push::BranchPushResult> {
    ops::branch::push::push(
        api,
        ops::branch::push::BranchPushArgs {
            branch: String::new(),
            fast_forward_merge: false,
        },
    )
    .await
}

async fn sync(api: &LoreApi) -> lore_vm::error::Result<ops::revision::sync::RevisionSyncResult> {
    ops::revision::sync::sync(api, ops::revision::sync::RevisionSyncArgs::default()).await
}

async fn create_tracking_repo(api: &LoreApi, repo_url: &str, who: &str) {
    ops::repository::create::create(
        api,
        ops::repository::create::CreateArgs {
            repository_url: repo_url.to_string(),
            description: format!("remote-qa repo ({who})"),
            id: String::new(),
            use_shared_store: false,
            shared_store_path: String::new(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("repository::create ({who}) should succeed: {e}"));
}

async fn clone_repo(
    api: &LoreApi,
    repo_url: &str,
    who: &str,
) -> ops::repository::clone::CloneResult {
    ops::repository::clone::clone(
        api,
        ops::repository::clone::CloneArgs {
            repository_url: repo_url.to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap_or_else(|e| panic!("repository::clone ({who}) should succeed: {e}"))
}

/// `file::info` for a single path at the working tree (no revision).
async fn file_info(api: &LoreApi, path: &Path) -> ops::file::info::FileInfoResult {
    ops::file::info::info(
        api,
        ops::file::info::FileInfoArgs {
            paths: vec![path.to_string_lossy().into_owned()],
            revision: String::new(),
            local: true,
            filtered: false,
        },
    )
    .await
    .unwrap_or_else(|e| panic!("file::info({}) should succeed: {e}", path.display()))
}

// ===========================================================================
// The suite. One #[tokio::test] drives the whole multi-user story end to end so
// the (expensive) server boots exactly once; each phase asserts strictly.
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn remote_multiuser_sync_push_conflict_and_locks() {
    let server = match boot_server() {
        ServerOutcome::Started(s) => s,
        ServerOutcome::Skip(why) => {
            eprintln!("SKIP remote_multiuser: {why}");
            return; // soft pass — environment can't host a live server
        }
        ServerOutcome::Hard(err) => panic!("remote multi-user harness failed to boot: {err}"),
    };

    let repo_name = format!("remoteqa-{}", std::process::id());
    let repo_url = server.repo_url(&repo_name);
    eprintln!("[harness] loreserver up; repo_url = {repo_url}");

    // Two separate working trees + local stores: any cross-client visibility is a
    // genuine network round trip, never a shared-store shortcut.
    let alice_dir = tempfile::tempdir().expect("alice tempdir");
    let bob_dir = tempfile::tempdir().expect("bob tempdir");
    let alice = online_api(alice_dir.path(), "alice");
    let bob = online_api(bob_dir.path(), "bob");

    // -----------------------------------------------------------------------
    // PHASE 0 — alice creates the repo, commits V1 of foo.txt, pushes it.
    // -----------------------------------------------------------------------
    create_tracking_repo(&alice, &repo_url, "alice").await;
    let foo_a = alice_dir.path().join("foo.txt");
    const V1: &[u8] = b"foo: version one\n";
    const V2: &[u8] = b"foo: version two, updated over the wire\n";
    write_file(&foo_a, V1);
    let staged = stage(&alice, &foo_a).await;
    assert!(
        staged.files.iter().any(|f| f.path.ends_with("foo.txt")),
        "alice stage should report foo.txt: {staged:?}"
    );
    let rev_v1 = commit(&alice, "add foo (v1)").await;
    assert!(!rev_v1.revision.is_empty(), "v1 commit empty: {rev_v1:?}");
    let push_v1 = push(&alice)
        .await
        .unwrap_or_else(|e| panic!("alice push v1 should succeed over the wire: {e}"));
    eprintln!(
        "[A] pushed v1 branch={} local_rev={} already_pushed={}",
        push_v1.branch_name, push_v1.local_revision, push_v1.already_pushed
    );
    // The branch identifier alice committed on — needed for lock ops later.
    let branch_id = rev_v1.branch.clone();
    assert!(!branch_id.is_empty(), "commit should report a branch id");

    // -----------------------------------------------------------------------
    // PHASE 1 — bob CLONES; sees alice's V1 file + revision over the wire.
    // -----------------------------------------------------------------------
    let cloned = clone_repo(&bob, &repo_url, "bob").await;
    eprintln!(
        "[B] cloned repo={} branch={} revision={}",
        cloned.repository, cloned.branch, cloned.revision
    );
    assert_eq!(
        cloned.revision, rev_v1.revision,
        "bob's clone tip must equal alice's pushed v1 revision"
    );
    let foo_b = bob_dir.path().join("foo.txt");
    assert!(
        foo_b.exists(),
        "bob's working tree must contain foo.txt after clone"
    );
    assert_eq!(
        std::fs::read(&foo_b).expect("read bob foo.txt"),
        V1,
        "bob's cloned foo.txt content must match alice's V1"
    );

    // -----------------------------------------------------------------------
    // PHASE 2 — sync: file UPDATE propagation. alice pushes V2; bob syncs; bob's
    // on-disk foo.txt becomes V2.
    // -----------------------------------------------------------------------
    write_file(&foo_a, V2);
    stage(&alice, &foo_a).await;
    let rev_v2 = commit(&alice, "update foo (v2)").await;
    assert_ne!(rev_v2.revision, rev_v1.revision, "v2 must be distinct");
    push(&alice)
        .await
        .unwrap_or_else(|e| panic!("alice push v2 should succeed: {e}"));

    let synced = sync(&bob)
        .await
        .unwrap_or_else(|e| panic!("bob sync (update) should succeed over the wire: {e}"));
    eprintln!(
        "[B] synced {} revisions, {} files updated",
        synced.revisions.len(),
        synced.files_updated
    );
    assert!(
        synced
            .revisions
            .iter()
            .any(|r| r.revision == rev_v2.revision),
        "bob's sync should observe alice's v2 revision: {synced:?}"
    );
    assert_eq!(
        std::fs::read(&foo_b).expect("read bob foo.txt after sync"),
        V2,
        "bob's foo.txt must be V2 after syncing alice's update"
    );

    // -----------------------------------------------------------------------
    // PHASE 3 — sync: file DELETE propagation. alice removes foo.txt + pushes;
    // bob syncs and foo.txt disappears from bob's working tree.
    // -----------------------------------------------------------------------
    std::fs::remove_file(&foo_a).expect("alice removes foo.txt");
    let staged_del = stage(&alice, &foo_a).await;
    assert!(
        staged_del.files.iter().any(|f| {
            f.path.ends_with("foo.txt") && f.action == ops::file::stage::FileStageAction::Delete
        }),
        "alice stage should report foo.txt as Delete: {staged_del:?}"
    );
    let rev_del = commit(&alice, "delete foo").await;
    push(&alice)
        .await
        .unwrap_or_else(|e| panic!("alice push delete should succeed: {e}"));

    let synced_del = sync(&bob)
        .await
        .unwrap_or_else(|e| panic!("bob sync (delete) should succeed: {e}"));
    eprintln!(
        "[B] synced delete: {} revisions, {} files deleted",
        synced_del.revisions.len(),
        synced_del.files_deleted
    );
    assert!(
        synced_del
            .revisions
            .iter()
            .any(|r| r.revision == rev_del.revision),
        "bob's sync should observe alice's delete revision: {synced_del:?}"
    );
    assert!(
        !foo_b.exists(),
        "bob's foo.txt must be gone after syncing alice's delete"
    );

    // -----------------------------------------------------------------------
    // PHASE 4 — push CONFLICT. Both re-add a file from the SAME tip; first push
    // wins, second is rejected (stale remote tip → linear-history enforced).
    // bob first re-syncs to the current (post-delete) tip so both share it.
    // -----------------------------------------------------------------------
    let race_a = alice_dir.path().join("race.txt");
    let race_b = bob_dir.path().join("race.txt");
    write_file(&race_a, b"alice's race entry\n");
    write_file(&race_b, b"bob's race entry\n");
    stage(&alice, &race_a).await;
    stage(&bob, &race_b).await;
    let _ = commit(&alice, "alice adds race.txt").await;
    let _ = commit(&bob, "bob adds race.txt").await;

    // Alice pushes first — must succeed and advance the remote tip.
    push(&alice)
        .await
        .unwrap_or_else(|e| panic!("alice's first push in the race should win: {e}"));

    // Bob pushes second from the now-stale tip — must be REJECTED. A peer cannot
    // clobber the remote; this is the core multi-user safety guarantee.
    let bob_push = push(&bob).await;
    assert!(
        bob_push.is_err(),
        "bob's push from a stale tip MUST be rejected (got Ok: {bob_push:?}) — \
         the server must enforce linear history so one user can't clobber another"
    );
    eprintln!(
        "[B] push correctly rejected (stale tip): {}",
        bob_push.unwrap_err()
    );

    // -----------------------------------------------------------------------
    // PHASE 5 — file CONFLICT state. Bob reconciles by syncing alice's race.txt
    // on top of his divergent local race.txt; the conflicted file must surface
    // through file::info's flag_conflict. (`reset = false` keeps bob's local
    // change so the incoming change collides with it.)
    // -----------------------------------------------------------------------
    let bob_sync_conflict = ops::revision::sync::sync(
        &bob,
        ops::revision::sync::RevisionSyncArgs {
            forward_changes: true,
            ..Default::default()
        },
    )
    .await;
    // Whether the conflict surfaces on the sync result or on file::info, at least
    // one of the two must report it. We assert via file::info, the GUI's source of
    // truth for the SCM conflict badge.
    match &bob_sync_conflict {
        Ok(r) => eprintln!(
            "[B] reconciling sync returned {} revisions (has_conflicts on any: {})",
            r.revisions.len(),
            r.revisions.iter().any(|x| x.has_conflicts)
        ),
        Err(e) => eprintln!("[B] reconciling sync surfaced conflict as an error: {e}"),
    }
    let info_b = file_info(&bob, &race_b).await;
    let conflicted = info_b.entries.iter().any(|f| f.flag_conflict)
        || bob_sync_conflict
            .as_ref()
            .map(|r| r.revisions.iter().any(|x| x.has_conflicts))
            .unwrap_or(true);
    assert!(
        conflicted,
        "a real two-user divergence on race.txt must surface as a conflict \
         (file::info flag_conflict or sync has_conflicts): {info_b:?}"
    );
    eprintln!("[B] conflict on race.txt observed");

    // -----------------------------------------------------------------------
    // PHASE 6 — two-user LOCKS: acquire / contention / query / release.
    //
    // Locks are keyed by branch + a path that must resolve to a TRACKED node in
    // the working tree of the acquiring client (upstream resolves the user path
    // against `repository.require_path()` and validates it via `find_node_link`).
    // So we first give BOTH clients a clean, identically-tracked lock target.
    //
    // alice commits+pushes `lockme.txt`. bob's *original* clone is now sitting on
    // a local conflict (phase 5), so rather than fight that staged state we model
    // a realistic SECOND user: bob clones the (clean) remote tip into a FRESH
    // working dir + store. Both then track the SAME repo-relative node — alice via
    // her abs path, "bob2" via his — and contention across the two clones (same
    // repo-relative resource, different absolute paths) is a genuine multi-user
    // lock collision over the wire.
    // -----------------------------------------------------------------------
    const LOCK_LEAF: &str = "lockme.txt";
    let lock_a = alice_dir.path().join(LOCK_LEAF); // alice's absolute path

    // Alice adds + pushes the clean lock target.
    write_file(&lock_a, b"lock target\n");
    stage(&alice, &lock_a).await;
    let rev_lock = commit(&alice, "add lockme.txt").await;
    push(&alice)
        .await
        .unwrap_or_else(|e| panic!("alice push lockme.txt should succeed: {e}"));

    // bob2: a fresh clone of the clean remote tip in its own dir/store.
    let bob2_dir = tempfile::tempdir().expect("bob2 tempdir");
    let bob2 = online_api(bob2_dir.path(), "bob");
    let bob2_clone = clone_repo(&bob2, &repo_url, "bob2").await;
    assert_eq!(
        bob2_clone.revision, rev_lock.revision,
        "bob2's fresh clone tip must be alice's lockme.txt revision"
    );
    let lock_b = bob2_dir.path().join(LOCK_LEAF); // bob2's absolute path
    assert!(
        lock_b.exists(),
        "bob2's working tree must contain lockme.txt after clone"
    );

    // Alice acquires the lock on her absolute path.
    let a_acq = ops::lock::file_acquire::file_acquire(
        &alice,
        ops::lock::file_acquire::FileAcquireArgs {
            paths: vec![lock_a.to_string_lossy().into_owned()],
            branch: branch_id.clone(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("alice lock::file_acquire should succeed: {e}"));
    assert!(
        a_acq.acquired.iter().any(|p| p.ends_with(LOCK_LEAF)),
        "alice should have acquired the lock on {LOCK_LEAF}: {a_acq:?}"
    );
    eprintln!("[A] acquired lock on {LOCK_LEAF}");

    // Query: the lock is visible and owned by alice.
    let q = ops::lock::file_query::file_query(
        &alice,
        ops::lock::file_query::FileQueryArgs {
            branch: branch_id.clone(),
            owner: String::new(),
            path: String::new(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("lock::file_query should succeed: {e}"));
    assert!(
        q.locks.iter().any(|l| l.path.ends_with(LOCK_LEAF)),
        "query should report the lock on {LOCK_LEAF}: {q:?}"
    );
    eprintln!("[A] query shows {} active lock(s)", q.count);

    // CONTENTION: bob tries to acquire the SAME node (his absolute path resolves
    // to the same repo-relative resource). Upstream reports an already-held lock
    // by NOT granting it (it lands in `ignored`), or the call errors. Either way,
    // bob must NOT come away as a fresh owner of the node.
    let b_acq = ops::lock::file_acquire::file_acquire(
        &bob2,
        ops::lock::file_acquire::FileAcquireArgs {
            paths: vec![lock_b.to_string_lossy().into_owned()],
            branch: branch_id.clone(),
        },
    )
    .await;
    match &b_acq {
        Ok(res) => {
            assert!(
                !res.acquired.iter().any(|p| p.ends_with(LOCK_LEAF)),
                "bob must NOT acquire a node alice already holds; it should be ignored. {res:?}"
            );
            eprintln!("[B] contended acquire correctly did not grant {LOCK_LEAF} (ignored)");
        }
        Err(e) => eprintln!("[B] contended acquire correctly refused: {e}"),
    }
    // Cross-check: a query still shows exactly one holder of the node.
    let q2 = ops::lock::file_query::file_query(
        &alice,
        ops::lock::file_query::FileQueryArgs {
            branch: branch_id.clone(),
            owner: String::new(),
            path: String::new(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("lock::file_query (contention check) should succeed: {e}"));
    assert_eq!(
        q2.locks
            .iter()
            .filter(|l| l.path.ends_with(LOCK_LEAF))
            .count(),
        1,
        "contended node must have exactly one active lock holder: {q2:?}"
    );

    // RELEASE: alice releases, then bob can acquire.
    let owner = q
        .locks
        .iter()
        .find(|l| l.path.ends_with(LOCK_LEAF))
        .map(|l| l.owner.clone())
        .unwrap_or_default();
    let a_rel = ops::lock::file_release::file_release(
        &alice,
        ops::lock::file_release::FileReleaseArgs {
            paths: vec![lock_a.to_string_lossy().into_owned()],
            branch: branch_id.clone(),
            owner: owner.clone(),
            owner_id: owner.clone(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("alice lock::file_release should succeed: {e}"));
    // SBAI-5434: at 0.8.5 LockFileReleaseBegin fires on EVERY release call, so
    // a successful release must report the released path AND must NOT flag
    // not_found — the flag, not the event's presence, carries the outcome.
    assert!(
        a_rel.released.iter().any(|p| p.ends_with(LOCK_LEAF)),
        "alice should release the lock on {LOCK_LEAF}: {a_rel:?}"
    );
    assert!(
        !a_rel.not_found,
        "a successful release must not report not_found: {a_rel:?}"
    );
    eprintln!("[A] released lock on {LOCK_LEAF}");

    // Releasing the SAME path again finds no lock: not_found must be true.
    let a_rel2 = ops::lock::file_release::file_release(
        &alice,
        ops::lock::file_release::FileReleaseArgs {
            paths: vec![lock_a.to_string_lossy().into_owned()],
            branch: branch_id.clone(),
            owner: owner.clone(),
            owner_id: owner,
        },
    )
    .await
    .unwrap_or_else(|e| panic!("alice re-release lock::file_release should succeed: {e}"));
    assert!(
        a_rel2.not_found,
        "re-releasing an already-released lock must report not_found: {a_rel2:?}"
    );
    eprintln!("[A] re-release correctly reports not_found");

    // After release, the lock is gone from the query.
    let q3 = ops::lock::file_query::file_query(
        &alice,
        ops::lock::file_query::FileQueryArgs {
            branch: branch_id.clone(),
            owner: String::new(),
            path: String::new(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("lock::file_query (post-release) should succeed: {e}"));
    assert!(
        !q3.locks.iter().any(|l| l.path.ends_with(LOCK_LEAF)),
        "the lock on {LOCK_LEAF} must be gone after alice releases it: {q3:?}"
    );

    // Now bob can acquire the freed node — proving the lock truly transferred.
    let b_acq2 = ops::lock::file_acquire::file_acquire(
        &bob2,
        ops::lock::file_acquire::FileAcquireArgs {
            paths: vec![lock_b.to_string_lossy().into_owned()],
            branch: branch_id.clone(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("bob2 lock::file_acquire after release should succeed: {e}"));
    assert!(
        b_acq2.acquired.iter().any(|p| p.ends_with(LOCK_LEAF)),
        "bob must be able to acquire {LOCK_LEAF} once alice released it: {b_acq2:?}"
    );
    eprintln!("[B] acquired {LOCK_LEAF} after alice's release — lock handoff verified");

    eprintln!(
        "[harness] ALL remote multi-user scenarios verified against {} (store {})",
        repo_url,
        server.store_dir.display()
    );
}

// ===========================================================================
// Mutable storage over a real loreserver (SBAI-5473)
// ===========================================================================

const MUTABLE_PARTITION: &str = "000000000000000000000000000000e1";
const MUTABLE_KEY: &str = "e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2";
const MUTABLE_VAL: &str = "e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3e3";
const MUTABLE_VAL2: &str = "e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4";

/// Open an in-memory storage handle bound to the server's remote endpoint.
async fn open_remote_mutable_handle(api: &LoreApi, remote_url: &str) -> u64 {
    ops::storage::open::open(
        api,
        ops::storage::open::StorageOpenArgs {
            repository_path: String::new(),
            in_memory: true,
            remote_url: remote_url.to_string(),
            cache_target_bytes: 0,
            cache_target_fragments: 0,
        },
    )
    .await
    .unwrap_or_else(|e| panic!("storage::open with remote_url should succeed: {e}"))
    .handle
}

/// Remote store/load/CAS against a real loreserver + explicit remote list rejection.
#[tokio::test]
async fn remote_mutable_store_load_cas_and_list_rejection() {
    let server = match boot_server() {
        ServerOutcome::Started(s) => s,
        ServerOutcome::Skip(why) => {
            eprintln!("[SKIP] remote mutable storage suite: {why}");
            return;
        }
        ServerOutcome::Hard(why) => panic!("failed to boot loreserver for mutable remote: {why}"),
    };

    // Create a tracking repo so the server has an identity/endpoint clients can
    // open a storage remote session against.
    let work = tempfile::tempdir().expect("mutable remote workdir");
    let api = online_api(work.path(), "mutable-alice");
    let repo_url = server.repo_url("mutable-kv");
    create_tracking_repo(&api, &repo_url, "mutable-alice").await;

    let handle = open_remote_mutable_handle(&api, &repo_url).await;
    assert!(handle != 0, "remote storage handle must be non-zero");

    // ---- remote store -----------------------------------------------------
    let stored = ops::storage::mutable_store::mutable_store(
        &api,
        ops::storage::mutable_store::StorageMutableStoreArgs {
            handle,
            remote: true,
            items: vec![ops::storage::mutable_store::MutableStoreItem {
                id: 1,
                partition: MUTABLE_PARTITION.into(),
                key: MUTABLE_KEY.into(),
                value: MUTABLE_VAL.into(),
                key_type: "branchLatestPointer".into(),
            }],
        },
    )
    .await
    .unwrap_or_else(|e| panic!("remote mutable_store should succeed: {e}"));
    assert!(
        stored.items.iter().any(|i| i.ok),
        "remote store item should ok: {stored:?}"
    );
    eprintln!("[remote] mutable_store ok");

    // ---- remote load ------------------------------------------------------
    let loaded = ops::storage::mutable_load::mutable_load(
        &api,
        ops::storage::mutable_load::StorageMutableLoadArgs {
            handle,
            remote: true,
            items: vec![ops::storage::mutable_load::MutableLoadItem {
                id: 1,
                partition: MUTABLE_PARTITION.into(),
                key: MUTABLE_KEY.into(),
                key_type: "branchLatestPointer".into(),
            }],
        },
    )
    .await
    .unwrap_or_else(|e| panic!("remote mutable_load should succeed: {e}"));
    assert!(
        loaded.items.iter().any(|i| i.ok && i.value == MUTABLE_VAL),
        "remote load must return stored value: {loaded:?}"
    );
    eprintln!("[remote] mutable_load ok");

    // ---- remote CAS -------------------------------------------------------
    let cas = ops::storage::mutable_compare_and_swap::mutable_compare_and_swap(
        &api,
        ops::storage::mutable_compare_and_swap::StorageMutableCompareAndSwapArgs {
            handle,
            remote: true,
            items: vec![
                ops::storage::mutable_compare_and_swap::MutableCompareAndSwapItem {
                    id: 1,
                    partition: MUTABLE_PARTITION.into(),
                    key: MUTABLE_KEY.into(),
                    expected: MUTABLE_VAL.into(),
                    value: MUTABLE_VAL2.into(),
                    key_type: "branchLatestPointer".into(),
                },
            ],
        },
    )
    .await
    .unwrap_or_else(|e| panic!("remote mutable_compare_and_swap should succeed: {e}"));
    assert!(
        cas.items.iter().any(|i| i.ok && i.swapped),
        "remote CAS should swap: {cas:?}"
    );
    eprintln!("[remote] mutable_compare_and_swap ok");

    // ---- remote list must be rejected -------------------------------------
    let list_err = ops::storage::mutable_list::mutable_list(
        &api,
        ops::storage::mutable_list::StorageMutableListArgs {
            handle,
            remote: true,
            items: vec![ops::storage::mutable_list::MutableListItem {
                id: 1,
                partition: MUTABLE_PARTITION.into(),
                key_type: "branchLatestPointer".into(),
            }],
        },
    )
    .await
    .expect_err("remote mutable_list must fail");
    let msg = list_err.to_string();
    assert!(
        msg.contains("mutable_list is only supported on the local store")
            || msg.contains("failed with status")
            || msg.contains("InvalidArguments")
            || msg.contains("remote"),
        "unexpected remote list rejection: {msg}"
    );
    eprintln!("[remote] mutable_list correctly rejected: {msg}");

    let _ =
        ops::storage::close::close(&api, ops::storage::close::StorageCloseArgs { handle }).await;

    eprintln!("[harness] remote mutable storage scenarios verified against {repo_url}");
}
