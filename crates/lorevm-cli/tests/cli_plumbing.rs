//! CLI plumbing: argument parsing, `--dir` resolution, the op listing, and the
//! `{error:{kind,message}}` envelope on every malformed invocation.
//!
//! These tests deliberately do NOT touch a repo — they exercise the binary's
//! own front matter (parse_cli, usage, dispatch's unknown-op arm) which is the
//! first thing every downstream driver hits.

mod harness;

use harness::{run_cli, run_cli_in};
use serde_json::Value;

/// `--list` prints one dispatchable op id per line, exit 0, no JSON wrapper.
#[test]
fn list_prints_every_op_id() {
    let run = run_cli(&["--list"]);
    assert!(run.success, "--list should exit 0:\n{}", run.stderr);
    let ids: Vec<&str> = run.stdout.lines().filter(|l| !l.is_empty()).collect();
    // A representative slice of the documented surface must be present.
    for expected in [
        "repository.create",
        "repository.status",
        "revision.history",
        "revision.info",
        "revision.commit",
        "branch.list",
        "branch.create",
        "branch.switch",
        "file.stage",
        "file.unstage",
        "lock.file_query",
        "lock.file_release",
    ] {
        assert!(
            ids.contains(&expected),
            "--list output missing `{expected}`. got: {ids:?}"
        );
    }
    // Every id is a `<domain>.<op>` — no stray blank/garbage lines.
    assert!(
        ids.iter().all(|id| id.contains('.')),
        "--list emitted a non-op line: {ids:?}"
    );
}

/// `list-ops` is an accepted alias for `--list`.
#[test]
fn list_ops_alias_matches_list() {
    let a = run_cli(&["--list"]);
    let b = run_cli(&["list-ops"]);
    assert!(a.success && b.success);
    assert_eq!(a.stdout, b.stdout, "`list-ops` must match `--list`");
}

/// `--help` / `-h` / `help` / no-args all print the usage banner to stdout and
/// exit 0 (usage is raw text, intentionally NOT a JSON error).
#[test]
fn help_and_bare_invocation_print_usage() {
    for argv in [vec!["--help"], vec!["-h"], vec!["help"], vec![]] {
        let run = run_cli(&argv);
        assert!(
            run.success,
            "`{argv:?}` should exit 0 with usage, got failure:\n{}",
            run.stdout
        );
        assert!(
            run.stdout.contains("USAGE:") && run.stdout.contains("lorevm"),
            "`{argv:?}` did not print the usage banner:\n{}",
            run.stdout
        );
        // Usage text is not JSON — confirm it isn't masquerading as an error.
        assert!(
            serde_json::from_str::<Value>(&run.stdout).is_err(),
            "usage text should be raw, not JSON: {}",
            run.stdout
        );
    }
}

/// An unknown `<domain>.<op>` is reported by `dispatch` as a structured
/// `LoreError` envelope (kind `Parse`), exit 1 — NOT a `cli` error.
#[test]
fn unknown_op_yields_parse_error_envelope() {
    let run = run_cli(&["bogus.op", "--dir", ".", "--offline"]);
    let (kind, message) = run.err_envelope();
    assert_eq!(kind, "Parse", "unknown op should be a dispatch Parse error");
    assert!(
        message.contains("unknown op") && message.contains("bogus.op"),
        "message should name the unknown op: {message}"
    );
}

/// Invalid JSON in `--args` is caught at the CLI layer (kind `cli`) before
/// dispatch, exit 1.
#[test]
fn bad_args_json_yields_cli_error_envelope() {
    let run = run_cli(&[
        "repository.status",
        "--dir",
        ".",
        "--args",
        "{not json}",
        "--offline",
    ]);
    let (kind, message) = run.err_envelope();
    assert_eq!(kind, "cli", "bad JSON is a CLI-layer error");
    assert!(
        message.contains("not valid JSON"),
        "message should explain the JSON failure: {message}"
    );
}

/// An unrecognised flag is a CLI-layer usage error.
#[test]
fn unknown_flag_yields_cli_error_envelope() {
    let run = run_cli(&[
        "repository.status",
        "--dir",
        ".",
        "--frobnicate",
        "--offline",
    ]);
    let (kind, message) = run.err_envelope();
    assert_eq!(kind, "cli");
    assert!(
        message.contains("unknown flag") && message.contains("--frobnicate"),
        "message should name the bad flag: {message}"
    );
}

/// Each value-taking flag with no following value is a clean CLI error, not a
/// panic. Covers `--dir`, `--args`, `--identity`.
#[test]
fn value_flags_require_a_value() {
    for (flag, needle) in [
        ("--dir", "--dir requires a value"),
        ("--args", "--args requires a JSON value"),
        ("--identity", "--identity requires a value"),
    ] {
        let run = run_cli(&["repository.status", flag]);
        let (kind, message) = run.err_envelope();
        assert_eq!(kind, "cli", "`{flag}` with no value should be a cli error");
        assert_eq!(message, needle, "unexpected message for `{flag}`");
    }
}

/// `--dir` defaults to `.` — proven by running a repo op from inside a non-repo
/// cwd with NO `--dir`: the op reaches the engine (not a CLI parse error) and
/// fails because the cwd is not a repository, with a structured dispatch error.
/// This nails the default-resolution behaviour downstream drivers rely on.
#[test]
fn dir_defaults_to_cwd_and_reaches_dispatch() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let run = run_cli_in(Some(tmp.path()), &["repository.status", "--offline"]);
    let (kind, message) = run.err_envelope();
    // Reached the engine (not the CLI's `cli` layer): the cwd isn't a repo.
    assert_ne!(
        kind, "cli",
        "should have resolved --dir to cwd and dispatched: {message}"
    );
    assert!(
        message.to_lowercase().contains("repository not found")
            || message.to_lowercase().contains("not found"),
        "expected a 'repository not found' style engine error, got: {message}"
    );
}

/// Explicit `--dir <path>` selecting a non-repo directory produces the same
/// structured engine error (and exit 1) — the path is honoured, not ignored.
#[test]
fn explicit_dir_to_non_repo_is_a_clean_engine_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let run = run_cli(&[
        "repository.status",
        "--dir",
        &tmp.path().to_string_lossy(),
        "--offline",
    ]);
    let (kind, _message) = run.err_envelope();
    assert_ne!(kind, "cli");
}

/// Every failure path prints valid JSON to stdout AND exits non-zero — the
/// invariant `lore-mcp` depends on to distinguish success from failure without
/// parsing prose. Spot-checks several distinct failure classes at once.
#[test]
fn all_failure_paths_are_json_and_nonzero() {
    let cases: &[&[&str]] = &[
        &["bogus.op", "--dir", ".", "--offline"],
        &[
            "repository.status",
            "--dir",
            ".",
            "--args",
            "{bad",
            "--offline",
        ],
        &["repository.status", "--bad-flag"],
        &["repository.status", "--dir"],
    ];
    for argv in cases {
        let run = run_cli(argv);
        assert!(!run.success, "`{argv:?}` unexpectedly succeeded");
        let v: Value = serde_json::from_str(&run.stdout)
            .unwrap_or_else(|_| panic!("`{argv:?}` failure stdout was not JSON:\n{}", run.stdout));
        assert!(
            v.get("error").is_some(),
            "`{argv:?}` failure JSON missing `error`: {v}"
        );
    }
}
