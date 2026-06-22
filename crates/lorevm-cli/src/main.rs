//! `lorevm` — a thin JSON CLI over [`lore_vm`]'s in-process op bindings.
//!
//! This is **not** the legacy upstream `lore` CLI. It links the `lore-vm` crate
//! (which binds the upstream `lore` engine in-process via its `ops/` layer) and
//! exposes a tiny, uniform JSON interface so an out-of-process driver — notably
//! the `lore-mcp` MCP server — can invoke any supported op without knowing Rust:
//!
//! ```sh
//! lorevm <domain>.<op> --dir <repo> --args '<json>'
//! ```
//!
//! Behaviour:
//! - `--dir <path>` selects the repository working directory. Defaults to `.`.
//! - `--args '<json>'` is a JSON object deserialised straight into the op's
//!   `Args` struct (snake_case keys, matching the serde field names). Omit it
//!   (or pass `{}`) for no-arg ops — every `Args` field is `#[serde(default)]`
//!   where the op allows it.
//! - `--in-memory` / `--offline` / `--identity <id>` tweak the global args, so
//!   the same headless in-memory mode the integration-test harness uses is
//!   reachable for smoke tests (`lorevm repository.create --in-memory --offline ...`).
//! - On success it prints the op's typed `Result` as pretty JSON to stdout and
//!   exits 0.
//! - On any error it prints `{"error": {...}}` to stdout and exits 1. The
//!   `error` value is the structured [`lore_vm::LoreError`] (`{kind, message}`)
//!   when the failure came from an op, or a `{kind:"cli", message}` shape for
//!   argument/dispatch problems.
//!
//! Adding an op is one line in the [`dispatch`] match — see the `op!` macro.

use std::path::PathBuf;
use std::process::ExitCode;

use lore_vm::api::LoreApi;
use lore_vm::global::LoreGlobal;
use lore_vm::ops;
use serde::Serialize;
use serde_json::{json, Value};

/// Parsed command line.
struct Cli {
    /// `"<domain>.<op>"` dispatch key.
    op_id: String,
    /// Repository working directory.
    dir: PathBuf,
    /// Raw JSON args object (defaults to `{}`).
    args: Value,
    in_memory: bool,
    offline: bool,
    identity: Option<String>,
}

/// A small CLI-level error (bad usage / unknown op / bad JSON), kept distinct
/// from a [`lore_vm::LoreError`] so the JSON `error.kind` tells callers which
/// layer failed.
#[derive(Debug)]
struct CliError(String);

impl CliError {
    fn new(msg: impl Into<String>) -> Self {
        CliError(msg.into())
    }
}

fn print_error_json(kind: &str, message: &str) {
    let body = json!({ "error": { "kind": kind, "message": message } });
    println!("{}", serde_json::to_string_pretty(&body).unwrap());
}

/// Print a successfully-typed op result as pretty JSON.
fn print_ok<T: Serialize>(value: &T) -> Result<(), CliError> {
    let v = serde_json::to_value(value)
        .map_err(|e| CliError::new(format!("failed to serialise result: {e}")))?;
    println!("{}", serde_json::to_string_pretty(&v).unwrap());
    Ok(())
}

fn parse_cli() -> Result<Cli, CliError> {
    let mut args = std::env::args().skip(1);
    let op_id = args.next().ok_or_else(|| CliError::new(usage()))?;

    if op_id == "--help" || op_id == "-h" || op_id == "help" {
        return Err(CliError::new(usage()));
    }
    if op_id == "--list" || op_id == "list-ops" {
        // Sentinel: handled by caller before building an API.
        return Ok(Cli {
            op_id: "__list__".into(),
            dir: PathBuf::from("."),
            args: json!({}),
            in_memory: false,
            offline: false,
            identity: None,
        });
    }

    let mut dir = PathBuf::from(".");
    let mut args_json: Option<String> = None;
    let mut in_memory = false;
    let mut offline = false;
    let mut identity: Option<String> = None;

    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--dir" => {
                dir = PathBuf::from(
                    args.next()
                        .ok_or_else(|| CliError::new("--dir requires a value"))?,
                );
            }
            "--args" => {
                args_json = Some(
                    args.next()
                        .ok_or_else(|| CliError::new("--args requires a JSON value"))?,
                );
            }
            "--identity" => {
                identity = Some(
                    args.next()
                        .ok_or_else(|| CliError::new("--identity requires a value"))?,
                );
            }
            "--in-memory" => in_memory = true,
            "--offline" => offline = true,
            other => {
                return Err(CliError::new(format!("unknown flag: {other}")));
            }
        }
    }

    let args_value: Value = match args_json {
        None => json!({}),
        Some(s) if s.trim().is_empty() => json!({}),
        Some(s) => serde_json::from_str(&s)
            .map_err(|e| CliError::new(format!("--args is not valid JSON: {e}")))?,
    };

    Ok(Cli {
        op_id,
        dir,
        args: args_value,
        in_memory,
        offline,
        identity,
    })
}

fn usage() -> String {
    format!(
        "lorevm — JSON CLI over lore-vm ops\n\n\
         USAGE:\n  \
         lorevm <domain>.<op> --dir <repo> [--args '<json>'] [--in-memory] [--offline] [--identity <id>]\n  \
         lorevm --list        # print every dispatchable op id (one per line)\n\n\
         Supported ops:\n  {}\n",
        SUPPORTED_OPS.join("\n  ")
    )
}

/// The set of ops `dispatch` knows about. Keep in sync with the match below;
/// the integration smoke test and `--list` both read this.
const SUPPORTED_OPS: &[&str] = &[
    "repository.status",
    "repository.info",
    "repository.list",
    "repository.create",
    "repository.clone",
    "revision.history",
    "revision.diff",
    "revision.info",
    "revision.find",
    "revision.commit",
    "revision.sync",
    "branch.list",
    "branch.info",
    "branch.create",
    "branch.switch",
    "branch.push",
    "auth.login_with_token",
    "file.stage",
    "file.unstage",
    "file.info",
    "file.history",
    "file.diff",
    "lock.file_query",
    "lock.file_status",
    "lock.file_acquire",
    "lock.file_release",
];

/// Build the headless [`LoreApi`] for `cli`.
fn build_api(cli: &Cli) -> LoreApi {
    let mut global = LoreGlobal::new(cli.dir.clone())
        .in_memory(cli.in_memory)
        .offline(cli.offline);
    if let Some(id) = &cli.identity {
        global = global.identity(id.clone());
    }
    LoreApi::from_global(global)
}

/// Dispatch `<domain>.<op>` to the matching `lore_vm::ops` fn.
///
/// The `op!` macro deserialises `cli.args` into the op's `Args` type, awaits the
/// op, and prints its typed `Result` as JSON. Adding a new op is one `op!` arm.
async fn dispatch(cli: &Cli, api: &LoreApi) -> Result<(), CliError> {
    /// `op!(id => path::to::fn, ArgsType)` — bind one op.
    macro_rules! op {
        ($path:path, $args:ty) => {{
            let parsed: $args = serde_json::from_value(cli.args.clone()).map_err(|e| {
                CliError::new(format!(
                    "could not parse --args into {} for `{}`: {e}",
                    stringify!($args),
                    cli.op_id
                ))
            })?;
            match $path(api, parsed).await {
                Ok(result) => print_ok(&result),
                Err(e) => {
                    // Structured op error → {"error": {kind, message}}.
                    let v = serde_json::to_value(&e).unwrap_or_else(|_| {
                        json!({ "kind": "client", "message": e.to_string() })
                    });
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({ "error": v })).unwrap()
                    );
                    Err(CliError::new("__already_reported__"))
                }
            }
        }};
    }

    match cli.op_id.as_str() {
        // ---- repository (read / metrics + create) ---------------------------
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
        // into `--dir`. Added for the live-server spike (SBAI-4064).
        "repository.clone" => op!(
            ops::repository::clone::clone,
            ops::repository::clone::CloneArgs
        ),

        // ---- revision -------------------------------------------------------
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
        // remote and syncs the working tree. Added for the live-server spike.
        "revision.sync" => op!(
            ops::revision::sync::sync,
            ops::revision::sync::RevisionSyncArgs
        ),

        // ---- branch ---------------------------------------------------------
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
        // the remote. Added for the live-server spike (SBAI-4064).
        "branch.push" => op!(ops::branch::push::push, ops::branch::push::BranchPushArgs),

        // ---- auth -----------------------------------------------------------
        // Non-interactive token login. Against a no-auth dev server this is a
        // no-op, but it exercises the credential path. Added for the spike.
        "auth.login_with_token" => op!(
            ops::auth::login_with_token::login_with_token,
            ops::auth::login_with_token::LoginWithTokenArgs
        ),

        // ---- file -----------------------------------------------------------
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

        // ---- lock -----------------------------------------------------------
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
        "lock.file_release" => op!(
            ops::lock::file_release::file_release,
            ops::lock::file_release::FileReleaseArgs
        ),

        unknown => Err(CliError::new(format!(
            "unknown op `{unknown}`. Run `lorevm --list` to see supported ops."
        ))),
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let cli = match parse_cli() {
        Ok(c) => c,
        Err(e) => {
            // Usage / help text is multi-line; print it raw rather than as a
            // JSON error so `--help` reads cleanly.
            if e.0.starts_with("lorevm —") {
                println!("{}", e.0);
                return ExitCode::SUCCESS;
            }
            print_error_json("cli", &e.0);
            return ExitCode::FAILURE;
        }
    };

    if cli.op_id == "__list__" {
        for id in SUPPORTED_OPS {
            println!("{id}");
        }
        return ExitCode::SUCCESS;
    }

    let api = build_api(&cli);
    match dispatch(&cli, &api).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.0 == "__already_reported__" => ExitCode::FAILURE,
        Err(e) => {
            print_error_json("cli", &e.0);
            ExitCode::FAILURE
        }
    }
}
