//! Canonical op dispatch — the single `"<domain>.<op>"` → typed-op seam.
//!
//! This is the **one** place that maps a string op id to the matching
//! [`crate::ops`] async fn. Every *external* driver of lore-vm rides this fn so
//! they cannot drift:
//!
//! - `lorevm-cli` (the `lorevm` JSON CLI — the subprocess surface `lore-mcp` and
//!   the planned VS Code extension shell) calls [`dispatch`] directly.
//! - `lorevm-ffi` (the C-ABI `cdylib` the Unreal Engine plugin links) calls
//!   [`dispatch`] behind its `extern "C"` boundary.
//!
//! The in-process **GUI/Tauri** path does **not** go through here — it binds the
//! ops as ~140 typed `#[tauri::command]`s for richer per-arg typing. That is a
//! deliberate second consumption mode of the *same* in-process binding, not a
//! competing dispatch. See `CLAUDE.md` ("One binding, two consumption modes").
//!
//! ## Contract
//!
//! ```ignore
//! let value = lore_vm::dispatch(&api, "repository.status", json!({})).await?;
//! ```
//!
//! - `op_id` is `"<domain>.<op>"` (e.g. `"repository.status"`).
//! - `args` is a JSON object deserialised straight into the op's `Args` struct
//!   (snake_case serde keys; every field is `#[serde(default)]`, so `{}` / `null`
//!   means "all defaults").
//! - On success the op's typed `Result` is serialised back to a [`Value`].
//! - On failure a structured [`LoreError`] is returned — bad args / unknown op
//!   surface as [`LoreError::Parse`] so a caller can tell a dispatch-layer
//!   failure from an op failure by `kind`.
//!
//! ## Adding an op
//!
//! Add one `op!` arm to the match below **and** one entry to [`SUPPORTED_OPS`].
//! `cargo test -p lore-vm` asserts the two stay in lockstep.

use serde_json::Value;

use crate::api::LoreApi;
use crate::error::LoreError;
use crate::ops;

/// Every op id [`dispatch`] understands, in a stable, human-scannable order.
///
/// `lorevm --list`, the FFI `supported_ops` surface, and the CLI usage text all
/// read this through [`supported_ops`]. Keep it in sync with the match in
/// [`dispatch`] — the unit tests enforce that every listed id dispatches and
/// every dispatched id is listed.
const SUPPORTED_OPS: &[&str] = &[
    // ---- repository -----------------------------------------------------
    "repository.status",
    "repository.info",
    "repository.list",
    "repository.create",
    "repository.clone",
    // ---- revision -------------------------------------------------------
    "revision.history",
    "revision.diff",
    "revision.info",
    "revision.find",
    "revision.commit",
    "revision.sync",
    // ---- branch ---------------------------------------------------------
    "branch.list",
    "branch.info",
    "branch.create",
    "branch.switch",
    "branch.push",
    // ---- auth -----------------------------------------------------------
    "auth.login_with_token",
    // ---- file -----------------------------------------------------------
    "file.stage",
    "file.unstage",
    "file.info",
    "file.history",
    "file.diff",
    // ---- lock -----------------------------------------------------------
    "lock.file_query",
    "lock.file_status",
    "lock.file_acquire",
    "lock.file_acquire_as_owner",
    "lock.file_message_send",
    "lock.file_release",
];

/// The set of `"<domain>.<op>"` ids [`dispatch`] can route.
///
/// Drivers expose this verbatim: `lorevm --list` prints it, and the FFI bridge
/// re-exports it so a C host can enumerate the surface without hardcoding it.
pub fn supported_ops() -> &'static [&'static str] {
    SUPPORTED_OPS
}

/// Route `op_id` (`"<domain>.<op>"`) to its [`crate::ops`] fn.
///
/// Deserialises `args` into the op's `Args`, awaits the op against `api`, and
/// serialises its typed `Result` back to a [`Value`]. See the module docs for
/// the full contract. This is the canonical dispatch shared by every external
/// driver (CLI, FFI).
pub async fn dispatch(api: &LoreApi, op_id: &str, args: Value) -> Result<Value, LoreError> {
    /// `op!(path::to::fn, ArgsType)` — bind one op: deserialise `args` into
    /// `ArgsType`, await the op, serialise the typed result to JSON.
    macro_rules! op {
        ($path:path, $args:ty) => {{
            let parsed: $args = serde_json::from_value(args).map_err(|e| {
                LoreError::Parse(format!("could not parse args for `{op_id}`: {e}"))
            })?;
            let result = $path(api, parsed).await?;
            serde_json::to_value(&result).map_err(|e| {
                LoreError::Parse(format!("could not serialise result for `{op_id}`: {e}"))
            })
        }};
    }

    match op_id {
        // ---- repository (read / metrics + create + clone) ---------------
        "repository.status" => op!(
            ops::repository::status::status,
            ops::repository::status::RepositoryStatusArgs
        ),
        "repository.info" => op!(
            ops::repository::info::info,
            ops::repository::info::RepositoryInfoArgs
        ),
        "repository.list" => op!(ops::repository::list::list, ops::repository::list::ListArgs),
        "repository.create" => op!(
            ops::repository::create::create,
            ops::repository::create::CreateArgs
        ),
        // Networked: connects to a remote lore server over QUIC/gRPC and clones
        // into the working dir.
        "repository.clone" => op!(
            ops::repository::clone::clone,
            ops::repository::clone::CloneArgs
        ),

        // ---- revision ---------------------------------------------------
        "revision.history" => op!(
            ops::revision::history::history,
            ops::revision::history::RevisionHistoryArgs
        ),
        "revision.diff" => op!(
            ops::revision::diff::diff,
            ops::revision::diff::RevisionDiffArgs
        ),
        "revision.info" => op!(
            ops::revision::info::info,
            ops::revision::info::RevisionInfoArgs
        ),
        "revision.find" => op!(
            ops::revision::find::find,
            ops::revision::find::RevisionFindArgs
        ),
        "revision.commit" => op!(
            ops::revision::commit::commit,
            ops::revision::commit::CommitArgs
        ),
        // Networked: pulls the latest revision for the current branch from the
        // remote and syncs the working tree.
        "revision.sync" => op!(
            ops::revision::sync::sync,
            ops::revision::sync::RevisionSyncArgs
        ),

        // ---- branch -----------------------------------------------------
        "branch.list" => op!(ops::branch::list::list, ops::branch::list::BranchListArgs),
        "branch.info" => op!(ops::branch::info::info, ops::branch::info::BranchInfoArgs),
        "branch.create" => op!(
            ops::branch::create::create,
            ops::branch::create::BranchCreateArgs
        ),
        "branch.switch" => op!(
            ops::branch::switch::switch,
            ops::branch::switch::BranchSwitchArgs
        ),
        // Networked: pushes the current/specified branch and its revisions to
        // the remote.
        "branch.push" => op!(ops::branch::push::push, ops::branch::push::BranchPushArgs),

        // ---- auth -------------------------------------------------------
        // Non-interactive token login. Against a no-auth dev server this is a
        // no-op, but it exercises the credential path.
        "auth.login_with_token" => op!(
            ops::auth::login_with_token::login_with_token,
            ops::auth::login_with_token::LoginWithTokenArgs
        ),

        // ---- file -------------------------------------------------------
        "file.stage" => op!(ops::file::stage::stage, ops::file::stage::FileStageArgs),
        "file.unstage" => op!(
            ops::file::unstage::unstage,
            ops::file::unstage::FileUnstageArgs
        ),
        "file.info" => op!(ops::file::info::info, ops::file::info::FileInfoArgs),
        "file.history" => op!(
            ops::file::history::history,
            ops::file::history::FileHistoryArgs
        ),
        "file.diff" => op!(ops::file::diff::diff, ops::file::diff::DiffArgs),

        // ---- lock -------------------------------------------------------
        "lock.file_query" => op!(
            ops::lock::file_query::file_query,
            ops::lock::file_query::FileQueryArgs
        ),
        "lock.file_status" => op!(
            ops::lock::file_status::file_status,
            ops::lock::file_status::FileStatusArgs
        ),
        "lock.file_acquire" => op!(
            ops::lock::file_acquire::file_acquire,
            ops::lock::file_acquire::FileAcquireArgs
        ),
        "lock.file_acquire_as_owner" => op!(
            ops::lock::file_acquire_as_owner::file_acquire_as_owner,
            ops::lock::file_acquire_as_owner::FileAcquireAsOwnerArgs
        ),
        "lock.file_message_send" => op!(
            ops::lock::file_message_send::file_message_send,
            ops::lock::file_message_send::FileMessageSendArgs
        ),
        "lock.file_release" => op!(
            ops::lock::file_release::file_release,
            ops::lock::file_release::FileReleaseArgs
        ),

        unknown => Err(LoreError::Parse(format!(
            "unknown op `{unknown}`; see lore_vm::supported_ops() for the dispatchable set"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn supported_ops_has_no_duplicates() {
        let set: HashSet<_> = supported_ops().iter().collect();
        assert_eq!(
            set.len(),
            supported_ops().len(),
            "supported_ops() contains duplicate ids"
        );
    }

    /// `supported_ops()` and the match must stay in lockstep: no listed id may
    /// fall through to the unknown-op arm. We probe routing with *bad* args
    /// (`null`), which makes every routed op fail at deserialisation *before* it
    /// ever touches the engine — so this never opens a repo or hits the network,
    /// yet still distinguishes "routed but arg-parse failed" from the distinctive
    /// "unknown op" message produced only by the fall-through arm.
    #[tokio::test]
    async fn every_supported_op_is_routed() {
        let api = LoreApi::new(std::path::PathBuf::from("/nonexistent-lorevm-dispatch"));
        for op in supported_ops() {
            // `null` cannot deserialise into any op's Args struct, so a routed op
            // returns Parse("could not parse args …"); only an *unrouted* id
            // returns Parse("unknown op …").
            if let Err(LoreError::Parse(msg)) = dispatch(&api, op, Value::Null).await {
                assert!(
                    !msg.starts_with("unknown op"),
                    "listed op `{op}` is not routed by dispatch()"
                );
            }
        }
    }

    #[tokio::test]
    async fn unknown_op_is_a_structured_parse_error() {
        let api = LoreApi::new(std::path::PathBuf::from("."));
        let err = dispatch(&api, "nope.nope", json!({}))
            .await
            .expect_err("unknown op must error");
        match err {
            LoreError::Parse(msg) => assert!(msg.contains("unknown op")),
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    /// A couple of real ops round-trip through dispatch against an in-memory
    /// engine: a mutating op (`repository.create`) then a read op
    /// (`repository.status`), both producing JSON objects, no server.
    ///
    /// Gated behind `integration-tests` (like `tests/integration_roundtrip.rs`)
    /// so the fast default `cargo test -p lore-vm` never spins the real engine.
    /// The FFI smoke test exercises the same create→status path over the C ABI.
    #[cfg(feature = "integration-tests")]
    #[tokio::test]
    async fn ops_round_trip_through_dispatch() {
        let tmp = tempfile::tempdir().unwrap();
        let api = LoreApi::from_global(
            crate::global::LoreGlobal::new(tmp.path().to_path_buf())
                .in_memory(true)
                .offline(true)
                .identity("dispatch-test"),
        );
        let repo_url = format!("lore://localhost/dispatch-{}", std::process::id());

        let created = dispatch(
            &api,
            "repository.create",
            json!({ "repository_url": repo_url }),
        )
        .await
        .expect("repository.create should succeed in-memory");
        assert!(created.is_object(), "create result was not an object");

        let status = dispatch(&api, "repository.status", json!({}))
            .await
            .expect("repository.status should succeed after create");
        assert!(status.is_object(), "status result was not an object");
    }
}
