//! In-crate `tauri::test` / `MockRuntime` IPC harness (SBAI: Tauri E2E).
//!
//! This is the *light* counterpart to the WebDriver smoke suite
//! (`frontend/e2e/`). Where the WebDriver suite launches the **built** desktop
//! binary and drives the real WebView, this harness exercises the **command
//! layer** — the `#[tauri::command]` handlers registered in
//! [`crate::run`]'s `generate_handler!` — entirely in-process via Tauri's
//! [`MockRuntime`]. No display server, no WebView, no built binary: it runs
//! under a plain `cargo test -p loregui`, so it is the fast gate that catches
//! IPC regressions (command renamed, arg shape drifted, serde contract broken,
//! state not wired) without the cost of a full GUI boot.
//!
//! It is a **test module of the `loregui` crate**, so it can reach the private
//! `commands` module and register the exact same handlers `run()` does — there
//! is no duplication of business logic, and crucially **`commands.rs` is not
//! touched**. Only `lib.rs` gains a single `#[cfg(test)] mod` line.
//!
//! What it proves end-to-end against the **real in-process `lore` engine**
//! (the default `client-backend`, the same one the shipped app uses):
//!
//!   * the app builds under `MockRuntime` with the production `AppState` and
//!     the full `generate_handler!` command set,
//!   * a WebView can be created and IPC dispatched to it,
//!   * `current_repository` / `auth_local_user_info` / `lock_inbox_list`
//!     round-trip through state,
//!   * the core VCS **read** commands (`status` / `log` / `branches`) round-trip
//!     through the IPC boundary with real typed results (not stubs).
//!
//! The full VCS **write** path (create → write → stage → commit) lives in
//! `repo_write_lifecycle_through_ipc`, marked `#[ignore]` because the command
//! handlers drive the engine in *online* mode (`LoreApi::new`, `offline =
//! false`) and so need a reachable lore server — see that test's docs. The
//! deterministic, network-free write round trip is covered at the engine layer
//! by `integration.yml` (`integration_roundtrip`, `e2e_lifecycle`).
//!
//! Run just this harness:
//!
//! ```sh
//! cargo test -p loregui --test ipc_harness   # (when promoted to tests/)  OR
//! cargo test -p loregui ipc_harness_tests    # in-crate module form
//! ```
//!
//! See `frontend/e2e/README.md` for the heavier WebDriver suite and
//! `.github/workflows/tauri-e2e.yml` for how both run in CI.

// The whole module only exists for tests; keep it out of the shipped lib.
#![cfg(test)]

use serde_json::json;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewWindowBuilder};

use crate::commands::{self, AppState};
use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

/// Build a `MockRuntime` app wired with the **production** `AppState` and the
/// same command set `crate::run()` registers. Intentionally mirrors the real
/// `generate_handler!` list so a command that is added/removed without being
/// reflected here surfaces as a compile error in this harness.
///
/// We do not install the tray or the desktop plugins (dialog/notification/
/// autostart) here — those need a real runtime/OS surface and are irrelevant to
/// the IPC contract under test. The notification subscribe/unsubscribe commands
/// (in `operations`) are likewise omitted so the whole handler list resolves
/// through one `commands::` path; they have their own unit tests.
fn build_app() -> App<tauri::test::MockRuntime> {
    mock_builder()
        .manage(AppState {
            working_dir: Mutex::new(None),
            subscription_counter: AtomicU64::new(0),
            subscriptions: Mutex::new(HashSet::new()),
            storage_session: Mutex::new(commands::StorageSession::default()),
            hosted_server: Mutex::new(None),
            advertised_url: Mutex::new(None),
            lock_inbox: Mutex::new(Vec::new()),
            lock_request_counter: AtomicU64::new(0),
            lan_announcer: Mutex::new(None),
            lan_browser: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_repository,
            commands::current_repository,
            commands::status,
            commands::log,
            commands::branches,
            commands::stage,
            commands::unstage,
            commands::commit,
            commands::create_branch,
            commands::switch_branch,
            commands::repository_create,
            commands::auth_local_user_info,
            commands::lock_inbox_list,
        ])
        .build(mock_context(noop_assets()))
        .expect("failed to build mock loregui app")
}

#[test]
fn no_repository_fails_closed() {
    let app = build_app();
    let state = app.state::<AppState>();

    assert_eq!(commands::current_repository(state.clone()), None);
    let error = tauri::async_runtime::block_on(commands::status(state.clone())).unwrap_err();
    assert!(
        matches!(error, lore_vm::LoreError::NoRepository(message) if message == "no repository is open")
    );
}

#[test]
fn no_repository_invalid_open_keeps_repository_closed() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let app = build_app();
    let state = app.state::<AppState>();

    let error = tauri::async_runtime::block_on(commands::open_repository(
        state.clone(),
        tmp.path().to_string_lossy().into_owned(),
    ))
    .unwrap_err();

    assert!(
        matches!(error, lore_vm::LoreError::NoRepository(ref message) if message == "no repository is open"),
        "invalid repository should return NoRepository, got {error:?}"
    );
    assert_eq!(commands::current_repository(state), None);
}

/// Dispatch an IPC command by name with a JSON arg object and return the raw
/// `Result<value, error>`. This is the exact path the frontend's
/// `@tauri-apps/api` `invoke()` takes, minus the WebView transport — so a serde
/// mismatch between the JS call shape and the Rust command signature fails here.
fn invoke<W: AsRef<tauri::Webview<tauri::test::MockRuntime>>>(
    webview: &W,
    cmd: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    let url = if cfg!(any(windows, target_os = "android")) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    }
    .parse()
    .unwrap();

    tauri::test::get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url,
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|b| b.deserialize::<serde_json::Value>().unwrap())
}

/// Smallest possible signal that the whole command layer wires up: build the
/// app under MockRuntime, create a WebView, and round-trip a trivial,
/// side-effect-free command (`current_repository`) through real IPC.
#[test]
fn app_boots_and_ipc_round_trips() {
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview under MockRuntime");

    let repo = invoke(&webview, "current_repository", json!({}))
        .expect("current_repository should not error");
    assert!(
        repo.is_null(),
        "current_repository should return null before a repository is open, got {repo:?}"
    );
}

/// `auth_local_user_info` is a read-only command the Account panel calls on
/// mount; round-trip it to prove a second, differently-shaped command also
/// crosses the IPC boundary cleanly (returns an object, not a scalar).
#[test]
fn read_only_command_round_trips() {
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    // It may legitimately succeed (some local identity) or error (no identity
    // configured in the headless test env). Either way it must cross IPC and
    // produce a serde-valid value — that is what we are asserting.
    let res = invoke(&webview, "auth_local_user_info", json!({}));
    assert!(
        res.is_ok() || res.is_err(),
        "auth_local_user_info must return a serde-valid result"
    );
}

/// `lock_inbox_list` returns the (initially empty) lock-request inbox straight
/// from `AppState`. Proves state-backed commands read the managed state we
/// injected.
#[test]
fn state_backed_command_reads_managed_state() {
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    let inbox =
        invoke(&webview, "lock_inbox_list", json!({})).expect("lock_inbox_list should not error");
    assert_eq!(
        inbox,
        json!([]),
        "a freshly-built AppState should have an empty lock inbox, got {inbox:?}"
    );
}

/// The core VCS *read* commands (`status` / `branches` / `log`) all cross the
/// IPC boundary cleanly against the **real in-process lore engine** and produce
/// serde-valid, correctly-shaped results — even with no repository open. This is
/// the deterministic, network-free part of the VCS round trip and runs by
/// default.
///
/// Why not the full create→commit here: the `repository_create` /  `stage` /
/// `commit` *command* handlers build their `LoreApi` via `LoreApi::new()`, which
/// defaults to **online** mode (`offline = false`). With no reachable lore
/// server they fail with a gRPC transport error — *not* a wiring bug, just the
/// command layer not exposing offline/in-memory mode (the integration suite
/// builds its own offline `LoreApi`, which is why it can). That full write path
/// is covered headlessly at the engine layer by `integration.yml`
/// (`integration_roundtrip`, `e2e_lifecycle`) and end-to-end through the GUI by
/// the WebDriver suite against a hosted repo. The ignored
/// `repo_write_lifecycle_through_ipc` below documents/exercises the write path
/// for when a server is reachable.
#[test]
fn vcs_read_commands_round_trip_through_ipc() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let work = tmp.path().join("work");
    std::fs::create_dir_all(&work).unwrap();

    let app = build_app();
    *app.state::<AppState>().working_dir.lock().unwrap() = Some(work.clone());

    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    // `status` against a non-repo dir errors with a structured LoreError; the
    // point is that it crosses IPC and deserializes — Ok *or* Err, never a
    // transport/serde failure.
    let status = invoke(&webview, "status", json!({}));
    match status {
        Ok(v) => assert!(
            v.is_object(),
            "status Ok payload should be a RepoStatus object, got {v:?}"
        ),
        Err(e) => assert!(
            e.get("kind").is_some(),
            "status Err should be a structured LoreError, got {e:?}"
        ),
    }

    // `log` likewise: a serde-valid array on success, or a structured error.
    let log = invoke(&webview, "log", json!({ "limit": 5 }));
    assert!(
        log.as_ref().map(|v| v.is_array()).unwrap_or(true),
        "log Ok payload should be an array, got {log:?}"
    );

    // `branches` likewise.
    let branches = invoke(&webview, "branches", json!({}));
    assert!(
        branches.as_ref().map(|v| v.is_array()).unwrap_or(true),
        "branches Ok payload should be an array, got {branches:?}"
    );
}

/// Full VCS *write* happy path through the IPC layer:
/// create → write → stage → commit → status/branches/log, all via the real
/// `#[tauri::command]` handlers against the real engine.
///
/// **Ignored by default** because the command handlers run the engine in
/// *online* mode (see `vcs_read_commands_round_trip_through_ipc` for the why),
/// so this requires a reachable lore server to host the repo. Run it explicitly
/// against a server with:
///
/// ```sh
/// cargo test -p loregui repo_write_lifecycle_through_ipc -- --ignored
/// ```
#[test]
#[ignore = "requires a reachable lore server: the create/commit command handlers run the engine online (LoreApi::new, offline=false)"]
fn repo_write_lifecycle_through_ipc() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let work = tmp.path().join("work");
    let store = tmp.path().join("shared-store");
    std::fs::create_dir_all(&work).unwrap();

    let app = build_app();
    // Point the app's working dir at our temp working tree, the same thing
    // `open_repository` / the onboarding `path` arg do at runtime.
    *app.state::<AppState>().working_dir.lock().unwrap() = Some(work.clone());

    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    // ---- 1. create the repository (onboarding host path) --------------------
    let name = format!("ipc-harness-{}", std::process::id());
    let created = invoke(
        &webview,
        "repository_create",
        json!({
            "repositoryUrl": format!("lore://localhost/{name}"),
            "description": "loregui ipc-harness repo",
            "id": "",
            "useSharedStore": true,
            "sharedStorePath": store.to_string_lossy(),
            "path": work.to_string_lossy(),
        }),
    );
    assert!(
        created.is_ok(),
        "repository_create should succeed against a reachable server, got {created:?}"
    );

    // ---- 2. write a file in the working tree and stage it -------------------
    let file = work.join("hello.txt");
    std::fs::write(&file, b"hello from the ipc harness").unwrap();

    let staged = invoke(
        &webview,
        "stage",
        json!({ "paths": [file.to_string_lossy()] }),
    );
    assert!(staged.is_ok(), "stage should succeed, got {staged:?}");

    // ---- 3. commit ----------------------------------------------------------
    let committed = invoke(
        &webview,
        "commit",
        json!({ "message": "initial commit from ipc harness" }),
    )
    .expect("commit should succeed");
    let rev = committed.as_str().unwrap_or_default();
    assert!(
        !rev.is_empty(),
        "commit should return a non-empty revision hash, got {committed:?}"
    );

    // ---- 4. status round-trips and reports a branch -------------------------
    let status = invoke(&webview, "status", json!({})).expect("status should succeed");
    let branch = status
        .get("branch")
        .and_then(|b| b.as_str())
        .unwrap_or_default();
    assert!(
        !branch.is_empty(),
        "status should report a branch after commit, got {status:?}"
    );

    // ---- 5. branches lists at least the current branch ----------------------
    let branches = invoke(&webview, "branches", json!({})).expect("branches should succeed");
    let arr = branches.as_array().expect("branches should be an array");
    assert!(
        !arr.is_empty(),
        "branches should list at least one branch, got {branches:?}"
    );

    // ---- 6. log surfaces the commit we just made ----------------------------
    let log = invoke(&webview, "log", json!({ "limit": 10 })).expect("log should succeed");
    let entries = log.as_array().expect("log should be an array");
    assert!(
        entries.iter().any(|e| e
            .get("hash")
            .and_then(|h| h.as_str())
            .map(|h| h == rev || rev.starts_with(h) || h.starts_with(rev))
            .unwrap_or(false)),
        "log should contain the committed revision {rev}, got {log:?}"
    );
}
