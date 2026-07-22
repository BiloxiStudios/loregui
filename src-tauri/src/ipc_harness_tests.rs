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
//!   * the core VCS **read** commands (`status` / `log` / `branches`) fail closed
//!     through IPC with the exact structured `NoRepository` error at startup.
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

use lore_vm::api::LoreApi;
use lore_vm::global::LoreGlobal;
use lore_vm::ops;
use serde_json::json;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewWindowBuilder};

use crate::commands::{self, AppState};
use crate::settings::SettingsManager;
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
fn build_app_with_config(config_dir: &std::path::Path) -> App<tauri::test::MockRuntime> {
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
        .manage(SettingsManager::new(config_dir.to_path_buf()))
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
            commands::create_repository,
            commands::clone,
            commands::repository_create,
            commands::repository_clone,
            commands::repository_list,
            commands::host_store_prepare,
            commands::host_store_probe,
            commands::auth_local_user_info,
            commands::auth_login_interactive,
            commands::auth_login_with_token,
            commands::auth_user_info,
            commands::auth_logout,
            commands::auth_clear,
            commands::shared_store_create,
            commands::service_start,
            commands::service_stop,
            commands::host_server_restart,
            commands::lock_inbox_list,
        ])
        .build(mock_context(noop_assets()))
        .expect("failed to build mock loregui app")
}

fn build_app() -> App<tauri::test::MockRuntime> {
    let config_dir = tempfile::tempdir().expect("temp settings directory").keep();
    build_app_with_config(&config_dir)
}

async fn create_offline_fixture_repository(
    client_path: &std::path::Path,
    store_path: &std::path::Path,
) {
    let api = LoreApi::from_global(
        LoreGlobal::new(client_path.to_path_buf())
            .in_memory(false)
            .offline(true)
            .identity("ipc-fixture"),
    );
    ops::shared_store::create::create(
        &api,
        ops::shared_store::create::SharedStoreCreateArgs {
            remote_url: String::new(),
            path: Some(store_path.to_string_lossy().into_owned()),
            make_default: false,
        },
    )
    .await
    .expect("create fixture shared store");
    ops::repository::create::create(
        &api,
        ops::repository::create::CreateArgs {
            repository_url: "lore://localhost/restart-fixture".into(),
            description: "restart persistence fixture".into(),
            id: String::new(),
            use_shared_store: true,
            shared_store_path: store_path.to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("create offline fixture repository");
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
    let settings = app.state::<SettingsManager>();

    let error = tauri::async_runtime::block_on(commands::open_repository(
        state.clone(),
        settings,
        tmp.path().to_string_lossy().into_owned(),
    ))
    .unwrap_err();

    assert!(
        matches!(error, lore_vm::LoreError::NoRepository(ref message) if message == "no repository is open"),
        "invalid repository should return NoRepository, got {error:?}"
    );
    assert_eq!(commands::current_repository(state), None);
}

#[test]
fn repository_activation_publishes_runtime_only_after_settings_persist() {
    let tmp = tempfile::tempdir().expect("temp fixture root");
    let config_dir = tmp.path().join("blocked-config");
    let client_path = tmp.path().join("client-working-tree");
    let shared_store = tmp.path().join("client-shared-store");
    tauri::async_runtime::block_on(create_offline_fixture_repository(
        &client_path,
        &shared_store,
    ));
    std::fs::write(&config_dir, "not-a-directory").expect("blocking config file");

    let app = build_app_with_config(&config_dir);
    let state = app.state::<AppState>();
    let settings = app.state::<SettingsManager>();
    let error = tauri::async_runtime::block_on(commands::open_repository(
        state.clone(),
        settings.clone(),
        client_path.to_string_lossy().into_owned(),
    ))
    .unwrap_err();

    assert!(matches!(
        error,
        lore_vm::LoreError::CommandFailed(ref message)
            if message == "failed to persist active repository context"
    ));
    assert_eq!(commands::current_repository(state), None);
    assert_eq!(settings.get().active_repository, None);
    assert_eq!(
        std::fs::read_to_string(config_dir).expect("blocking file remains"),
        "not-a-directory"
    );
}

#[test]
fn validated_repository_path_survives_rebuild_and_stale_path_fails_closed() {
    let tmp = tempfile::tempdir().expect("temp fixture root");
    let config_dir = tmp.path().join("config");
    let server_store = tmp.path().join("server-store");
    let shared_store = tmp.path().join("client-shared-store");
    let client_path = tmp.path().join("client-working-tree");

    tauri::async_runtime::block_on(create_offline_fixture_repository(
        &client_path,
        &shared_store,
    ));

    {
        let app = build_app_with_config(&config_dir);
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build webview");
        let prepared = invoke(
            &webview,
            "host_store_prepare",
            json!({
                "path": server_store.to_string_lossy(),
                "mutableStore": null,
            }),
        )
        .expect("fixture host store preparation must succeed");
        assert_eq!(prepared, json!(server_store.to_string_lossy()));
        invoke(
            &webview,
            "host_store_probe",
            json!({ "path": server_store.to_string_lossy() }),
        )
        .expect("fixture host store probe must succeed");
        assert_ne!(server_store, client_path);
        assert_eq!(
            invoke(&webview, "current_repository", json!({})),
            Ok(serde_json::Value::Null),
            "preparing/probing a server store must not activate a client repository"
        );
        assert_eq!(
            app.state::<SettingsManager>().get().active_repository,
            None,
            "server storage must never be persisted as active repository context"
        );

        invoke(
            &webview,
            "open_repository",
            json!({ "path": client_path.to_string_lossy() }),
        )
        .expect("fixture repository open must succeed");
        assert_eq!(
            invoke(&webview, "current_repository", json!({})),
            Ok(json!(client_path.to_string_lossy()))
        );
        assert_eq!(
            app.state::<SettingsManager>().get().active_repository,
            Some(client_path.clone())
        );
    }

    {
        let app = build_app_with_config(&config_dir);
        tauri::async_runtime::block_on(commands::restore_active_repository(
            &app.state::<AppState>(),
            &app.state::<SettingsManager>(),
        ));
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build restarted webview");
        assert_eq!(
            invoke(&webview, "current_repository", json!({})),
            Ok(json!(client_path.to_string_lossy())),
            "validated local path must restore after rebuilding app state"
        );
    }

    std::fs::remove_dir_all(&client_path).expect("remove fixture repository");
    let app = build_app_with_config(&config_dir);
    tauri::async_runtime::block_on(commands::restore_active_repository(
        &app.state::<AppState>(),
        &app.state::<SettingsManager>(),
    ));
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build stale-candidate webview");
    assert_eq!(
        invoke(&webview, "current_repository", json!({})),
        Ok(serde_json::Value::Null)
    );
    assert_eq!(
        app.state::<SettingsManager>().get().active_repository,
        None,
        "stale candidate must be removed from persistence"
    );
}

#[test]
fn failed_open_preserves_the_last_validated_runtime_and_persisted_path() {
    let tmp = tempfile::tempdir().expect("temp fixture root");
    let config_dir = tmp.path().join("config");
    let shared_store = tmp.path().join("client-shared-store");
    let client_path = tmp.path().join("client-working-tree");
    tauri::async_runtime::block_on(create_offline_fixture_repository(
        &client_path,
        &shared_store,
    ));
    let app = build_app_with_config(&config_dir);
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");
    invoke(
        &webview,
        "open_repository",
        json!({ "path": client_path.to_string_lossy() }),
    )
    .expect("fixture repository open must succeed");

    let missing = tmp.path().join("not-a-repository");
    std::fs::create_dir_all(&missing).expect("create non-repository directory");
    assert_eq!(
        invoke(
            &webview,
            "open_repository",
            json!({ "path": missing.to_string_lossy() }),
        ),
        Err(json!({
            "kind": "NoRepository",
            "message": "no repository is open",
        }))
    );

    for (command, args) in [
        (
            "create_repository",
            json!({
                "path": tmp.path().join("legacy-create-failure").to_string_lossy(),
                "name": "must-not-activate",
            }),
        ),
        (
            "clone",
            json!({
                "url": "not-a-lore-url",
                "dest": tmp.path().join("legacy-clone-failure").to_string_lossy(),
            }),
        ),
        (
            "repository_create",
            json!({
                "repositoryUrl": "lore://127.0.0.1:1/must-not-activate",
                "description": "failure preservation",
                "id": "",
                "useSharedStore": false,
                "sharedStorePath": "",
                "path": tmp.path().join("ops-create-failure").to_string_lossy(),
            }),
        ),
        (
            "repository_clone",
            json!({
                "url": "lore://127.0.0.1:1/must-not-activate",
                "dest": tmp.path().join("ops-clone-failure").to_string_lossy(),
            }),
        ),
    ] {
        assert!(
            invoke(&webview, command, args).is_err(),
            "{command} fixture failure must propagate"
        );
        assert_eq!(
            invoke(&webview, "current_repository", json!({})),
            Ok(json!(client_path.to_string_lossy())),
            "{command} failure must preserve the prior runtime path"
        );
        assert_eq!(
            app.state::<SettingsManager>().get().active_repository,
            Some(client_path.clone()),
            "{command} failure must preserve the prior persisted path"
        );
    }
    assert_eq!(
        invoke(&webview, "current_repository", json!({})),
        Ok(json!(client_path.to_string_lossy()))
    );
    assert_eq!(
        app.state::<SettingsManager>().get().active_repository,
        Some(client_path)
    );
}

#[test]
fn storage_onboarding_round_trips_without_repository() {
    let app = build_app();
    let state = app.state::<AppState>();

    tauri::async_runtime::block_on(async {
        commands::storage_open(
            state.clone(),
            commands::StorageBackendConfig {
                kind: "s3".into(),
                path: None,
                endpoint: None,
                bucket: None,
                region: None,
                access_key_id: None,
                secret_access_key: None,
                mutable_store: None,
            },
        )
        .await
        .expect("storage_open must not require an active client repository");

        let key = "no-repository-storage-round-trip".to_string();
        let expected = b"storage session".to_vec();
        commands::storage_put(state.clone(), key.clone(), expected.clone())
            .await
            .expect("storage_put must use the open storage session");
        let actual = commands::storage_get(state.clone(), key.clone())
            .await
            .expect("storage_get must use the open storage session");
        assert_eq!(actual, expected);
        commands::storage_obliterate(state, key)
            .await
            .expect("storage_obliterate must use the open storage session");
    });
}

#[test]
fn auth_token_login_without_repository_reaches_auth_backend() {
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    let error = invoke(
        &webview,
        "auth_login_with_token",
        json!({
            "remoteUrl": "lore://127.0.0.1:1/unreachable",
            "token": "test-token",
        }),
    )
    .expect_err("the deliberately unreachable auth endpoint should fail");

    assert_eq!(
        error,
        json!({
            "kind": "CommandFailed",
            "message": "Disconnected from server",
        })
    );
}

#[test]
fn auth_clear_without_repository_uses_local_auth_lifecycle() {
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    invoke(&webview, "auth_clear", json!({}))
        .expect("auth_clear must use local auth lifecycle without a repository");
}

#[test]
fn host_restart_rejects_without_backend_session_through_real_ipc() {
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");
    let error = invoke(
        &webview,
        "host_server_restart",
        json!({ "expectedGeneration": 41 }),
    )
    .expect_err("restart without a backend-owned session must fail closed");
    assert_eq!(error["kind"], "CommandFailed");
    assert!(error["message"]
        .as_str()
        .unwrap_or_default()
        .contains("no backend-owned hosted server session"));
}

#[test]
fn onboarding_lifecycle_commands_do_not_require_repository() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    assert_eq!(
        invoke(
            &webview,
            "shared_store_create",
            json!({ "path": tmp.path().join("shared-store").to_string_lossy() }),
        ),
        Err(json!({
            "kind": "CommandFailed",
            "message": "Failed to connect to remote URL : no remote URL",
        }))
    );
    let stub_error = Err(json!({
        "kind": "CommandFailed",
        "message": "event stream cancelled: channel closed",
    }));
    assert_eq!(
        invoke(
            &webview,
            "service_start",
            json!({ "installAutorun": false }),
        ),
        stub_error.clone()
    );
    assert_eq!(
        invoke(&webview, "service_stop", json!({ "all": true })),
        stub_error
    );
}

#[test]
fn repository_create_reaches_unavailable_backend_without_active_repository() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let work = tmp.path().join("created-repository");
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    let error = invoke(
        &webview,
        "repository_create",
        json!({
            "repositoryUrl": "lore://127.0.0.1:1/unreachable-create",
            "description": "unreachable create regression",
            "id": "",
            "useSharedStore": false,
            "sharedStorePath": "",
            "path": work.to_string_lossy(),
        }),
    )
    .expect_err("repository_create should fail against the unreachable backend");

    assert_eq!(
        error,
        json!({ "kind": "CommandFailed", "message": "Disconnected from server" })
    );
    assert_eq!(commands::current_repository(app.state()), None);
}

#[test]
fn repository_clone_reaches_unavailable_backend_without_active_repository() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let dest = tmp.path().join("cloned-repository");
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    let error = invoke(
        &webview,
        "repository_clone",
        json!({
            "url": "lore://127.0.0.1:1/unreachable-clone",
            "dest": dest.to_string_lossy(),
        }),
    )
    .expect_err("repository_clone should fail against the unreachable backend");

    assert_eq!(
        error,
        json!({ "kind": "CommandFailed", "message": "Disconnected from server" })
    );
    assert_eq!(commands::current_repository(app.state()), None);
}

#[test]
fn repository_list_reaches_unavailable_backend_without_active_repository() {
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    let error = invoke(
        &webview,
        "repository_list",
        json!({ "url": "lore://127.0.0.1:1/unreachable-list" }),
    )
    .expect_err("repository_list should reach the deliberately unreachable backend");

    assert_eq!(
        error,
        json!({ "kind": "CommandFailed", "message": "Disconnected from server" })
    );
    assert_eq!(commands::current_repository(app.state()), None);
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
fn auth_local_user_info_without_repository_uses_auth_lifecycle() {
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    let result = invoke(
        &webview,
        "auth_local_user_info",
        json!({ "authEndpoint": "", "userIds": [], "withToken": false }),
    );
    match result {
        Ok(value) => {
            assert!(value.get("users").is_some_and(serde_json::Value::is_array));
            assert!(value.get("tokens").is_some_and(serde_json::Value::is_array));
        }
        Err(error) => assert_eq!(
            error,
            json!({ "kind": "CommandFailed", "message": "No auth endpoint available" }),
            "local identity lookup may only use its documented empty-state error"
        ),
    }
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
/// IPC boundary against the **real in-process lore engine** and reject with the
/// exact structured `NoRepository` startup error. This deterministic,
/// network-free fail-closed contract runs by default.
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
fn vcs_read_commands_fail_closed_without_repository() {
    let app = build_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    let expected = json!({ "kind": "NoRepository", "message": "no repository is open" });
    assert_eq!(invoke(&webview, "status", json!({})), Err(expected.clone()));
    assert_eq!(
        invoke(&webview, "log", json!({ "limit": 5 })),
        Err(expected.clone())
    );
    assert_eq!(invoke(&webview, "branches", json!({})), Err(expected));
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
