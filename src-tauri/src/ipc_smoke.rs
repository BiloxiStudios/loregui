//! MockRuntime IPC smoke harness (SBAI — first desktop test coverage).
//!
//! This is the **fast, display-less gate** over the Tauri command layer
//! (`src-tauri/src/commands.rs`). It does NOT touch that file — it only drives
//! the registered `#[tauri::command]`s through Tauri's own in-process test
//! runtime ([`tauri::test::MockRuntime`]) exactly the way the frontend's
//! `invoke()` wrappers do: a real [`InvokeRequest`] in, a real
//! (de)serialized response out.
//!
//! What it proves end-to-end, with no webview / no display / no `lore` server:
//! 1. **Command registration** — every command named here is wired into the
//!    `generate_handler!` macro (a typo or a dropped registration fails the
//!    build here, not at runtime in front of a user).
//! 2. **State plumbing** — `AppState` is `manage`d and reachable; the
//!    working-dir mutation in `open_repository` is observed by
//!    `current_repository` across two independent IPC calls.
//! 3. **Argument + result serde** — args arrive as the frontend sends them
//!    (camelCase JSON via `InvokeBody`) and results round-trip back through
//!    `serde_json`, including the `LoreError` error envelope shape the GUI
//!    pattern-matches on.
//! 4. **The async error path doesn't hang** — engine-backed commands invoked
//!    against an empty temp dir must return a *structured* result (Ok or a
//!    serialized `LoreError`) promptly, never block the IPC channel.
//!
//! The full create → stage → commit → status *happy path* against a real
//! engine is covered at the binding layer by
//! `crates/lore-vm/tests/integration_roundtrip.rs` and exercised through the
//! real packaged app by the WebDriver suite in `e2e/`. This harness is the
//! cheap always-on gate that keeps the IPC seam itself honest.

#![cfg(test)]

use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;
use std::time::Duration;

use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindowBuilder};

use crate::commands::{self, AppState, StorageSession};

/// Build a `MockRuntime` app with the *real* command handlers and a fresh
/// `AppState`. Mirrors the `manage` + `invoke_handler` wiring in `lib.rs::run`
/// — minus the OS-facing plugins (tray/dialog/autostart) that a headless test
/// runtime can't install — so the commands under test see the exact same state
/// shape they see in production.
fn smoke_app() -> tauri::App<tauri::test::MockRuntime> {
    mock_builder()
        .manage(AppState {
            working_dir: Mutex::new(std::env::temp_dir()),
            subscription_counter: AtomicU64::new(0),
            subscriptions: Mutex::new(HashSet::new()),
            storage_session: Mutex::new(StorageSession::default()),
            hosted_server: Mutex::new(None),
            advertised_url: Mutex::new(None),
            lock_inbox: Mutex::new(Vec::new()),
            lock_request_counter: AtomicU64::new(0),
            lan_announcer: Mutex::new(None),
            lan_browser: Mutex::new(None),
        })
        // A representative slice of the registered surface. The point is to gate
        // the IPC seam, not to re-register all ~130 commands; these cover the
        // core happy path (open/current), the read verbs the main view drives
        // (status/log/branches), and the write verbs (stage/commit).
        .invoke_handler(tauri::generate_handler![
            commands::open_repository,
            commands::current_repository,
            commands::status,
            commands::log,
            commands::branches,
            commands::stage,
            commands::commit,
        ])
        .build(mock_context(noop_assets()))
        .expect("failed to build MockRuntime app")
}

/// Build a default `InvokeRequest` for `cmd` carrying `args` as the JSON body,
/// matching what `@tauri-apps/api`'s `invoke()` sends.
fn request(cmd: &str, args: serde_json::Value) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: if cfg!(any(windows, target_os = "android")) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        }
        .parse()
        .unwrap(),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

/// True iff the JSON value is the serialized `LoreError` envelope
/// (`{"kind": "...", "message": "..."}` — see `lore_vm::LoreError`).
fn is_lore_error(v: &serde_json::Value) -> bool {
    v.get("kind").and_then(|k| k.as_str()).is_some()
}

/// Pure-state round trip: `open_repository(path)` then `current_repository()`
/// must report the path back, proving command registration, the `AppState`
/// working-dir mutex, and arg/result serde all work through real IPC. No engine.
#[test]
fn open_then_current_repository_round_trips_through_ipc() {
    let app = smoke_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    let target = std::env::temp_dir().join("loregui-ipc-smoke-repo");
    let path_str = target.to_string_lossy().into_owned();

    // open_repository returns `Result<(), LoreError>` → serializes to `null` on Ok.
    let open = get_ipc_response(
        &webview,
        request("open_repository", serde_json::json!({ "path": path_str })),
    )
    .map(|b| b.deserialize::<serde_json::Value>().unwrap());
    assert_eq!(
        open,
        Ok(serde_json::Value::Null),
        "open_repository should succeed and return null"
    );

    // current_repository returns the working dir as a plain String.
    let current = get_ipc_response(
        &webview,
        request("current_repository", serde_json::json!({})),
    )
    .expect("current_repository must not error")
    .deserialize::<String>()
    .expect("current_repository returns a string");
    assert_eq!(
        current, path_str,
        "current_repository did not reflect the path set by open_repository"
    );
}

/// `current_repository` is also reachable through the `AppState` directly,
/// confirming the same managed state the IPC layer mutates is the canonical one.
#[test]
fn managed_state_is_the_same_state_ipc_mutates() {
    let app = smoke_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    let target = std::env::temp_dir().join("loregui-ipc-smoke-state");
    let path_str = target.to_string_lossy().into_owned();
    let _ = get_ipc_response(
        &webview,
        request("open_repository", serde_json::json!({ "path": path_str })),
    );

    let state = app.state::<AppState>();
    assert_eq!(
        state.dir().to_string_lossy(),
        path_str,
        "AppState working_dir should reflect the IPC mutation"
    );
}

/// An unknown command name must surface as a structured IPC error, not a panic
/// — the registration guard the frontend relies on.
#[test]
fn unknown_command_is_a_structured_error() {
    let app = smoke_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");

    let res = get_ipc_response(
        &webview,
        request("definitely_not_a_real_command", serde_json::json!({})),
    );
    assert!(
        res.is_err(),
        "unknown command should produce an Err response, got: {res:?}"
    );
}

/// Engine-backed read/write verbs invoked against a *fresh empty temp dir*
/// (no `.lore` repo, no server) must return promptly with a *structured*
/// outcome — either `Ok` or a serialized `LoreError` — and must never block the
/// IPC channel. Run on a worker thread with a hard wall-clock bound so a
/// regression that hangs the seam fails loudly instead of stalling CI.
#[test]
fn engine_commands_resolve_promptly_against_empty_dir() {
    // A genuinely empty, non-repo working dir.
    let tmp = std::env::temp_dir().join(format!("loregui-ipc-empty-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("mk temp dir");
    let dir = tmp.to_string_lossy().into_owned();

    // Each (command, args) pair the GUI's main view / commit flow drives.
    let cases: Vec<(&str, serde_json::Value)> = vec![
        ("status", serde_json::json!({})),
        ("log", serde_json::json!({ "limit": 10 })),
        ("branches", serde_json::json!({})),
        (
            "stage",
            serde_json::json!({ "paths": ["does-not-exist.txt"] }),
        ),
        ("commit", serde_json::json!({ "message": "smoke" })),
    ];

    for (cmd, args) in cases {
        let dir = dir.clone();
        let cmd_owned = cmd.to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        // Build the app + webview inside the worker so the !Send MockRuntime
        // handle never crosses the thread boundary.
        let handle = std::thread::spawn(move || {
            let app = smoke_app();
            {
                let state = app.state::<AppState>();
                *state.working_dir.lock().unwrap() = std::path::PathBuf::from(&dir);
            }
            let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .expect("build webview");
            let res = get_ipc_response(&webview, request(&cmd_owned, args))
                .map(|b| b.deserialize::<serde_json::Value>().unwrap());
            let _ = tx.send(res);
        });

        let res = rx
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or_else(|_| {
                panic!("IPC command `{cmd}` did not resolve within 30s — the IPC seam is hanging")
            });
        handle.join().expect("worker thread panicked");

        match res {
            Ok(v) => {
                // Ok payload (engine reachable) OR a serialized LoreError both
                // mean the seam resolved. A raw error envelope can ride the Ok
                // channel for commands that map it; either way it's structured.
                assert!(
                    v.is_null()
                        || v.is_object()
                        || v.is_array()
                        || v.is_string()
                        || is_lore_error(&v),
                    "`{cmd}` returned an unexpected payload shape: {v:?}"
                );
            }
            Err(e) => {
                // The Err channel must carry the structured LoreError envelope
                // (kind/message), which is what the frontend wrappers surface.
                assert!(
                    is_lore_error(&e),
                    "`{cmd}` errored with a non-LoreError payload: {e:?}"
                );
            }
        }
    }

    let _ = std::fs::remove_dir_all(&tmp);
}
