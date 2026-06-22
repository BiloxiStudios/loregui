//! `lorevm` — a thin JSON CLI over [`lore_vm`]'s in-process op bindings.
//!
//! This is **not** the legacy upstream `lore` CLI. It links the `lore-vm` crate
//! (which binds the upstream `lore` engine in-process via its `ops/` layer) and
//! exposes a tiny, uniform JSON interface so an out-of-process driver — notably
//! the `lore-mcp` MCP server and the planned VS Code extension — can invoke any
//! supported op without knowing Rust:
//!
//! ```sh
//! lorevm <domain>.<op> --dir <repo> --args '<json>'
//! ```
//!
//! It is one of the deliberate **external-driver** consumers of the canonical
//! [`lore_vm::dispatch`] seam (alongside `lorevm-ffi`). The CLI owns only
//! argument parsing and JSON I/O; the actual `"<domain>.<op>"` → op routing lives
//! once in `lore-vm` so the CLI, the FFI bridge, and any future driver cannot
//! drift. See `CLAUDE.md` ("One binding, two consumption modes").
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
//!   when the failure came from dispatch (op failure, bad args, unknown op), or
//!   a `{kind:"cli", message}` shape for argument/usage problems caught before
//!   dispatch.
//!
//! Adding an op is a one-line arm in [`lore_vm::dispatch`] plus its entry in
//! `lore_vm::supported_ops()` — nothing changes here.

use std::path::PathBuf;
use std::process::ExitCode;

use lore_vm::global::LoreGlobal;
use lore_vm::{dispatch, supported_ops, LoreApi};
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

/// A small CLI-level error (bad usage / bad JSON), kept distinct from a
/// [`lore_vm::LoreError`] so the JSON `error.kind` tells callers which layer
/// failed. Op-level failures, unknown ops, and bad args are reported by
/// `dispatch` itself as a `LoreError`.
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
        supported_ops().join("\n  ")
    )
}

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
        for id in supported_ops() {
            println!("{id}");
        }
        return ExitCode::SUCCESS;
    }

    let api = build_api(&cli);
    // The single shared seam: every external driver (this CLI, lorevm-ffi) routes
    // `<domain>.<op>` through `lore_vm::dispatch`.
    match dispatch(&api, &cli.op_id, cli.args).await {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            ExitCode::SUCCESS
        }
        Err(e) => {
            // Structured op / dispatch error → {"error": {kind, message}}.
            let v = serde_json::to_value(&e)
                .unwrap_or_else(|_| json!({ "kind": "client", "message": e.to_string() }));
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "error": v })).unwrap()
            );
            ExitCode::FAILURE
        }
    }
}
