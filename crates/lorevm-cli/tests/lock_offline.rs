//! Lock domain — graceful **offline** behaviour.
//!
//! Locks are server-coordinated: there is no local lock store. Run `--offline`
//! (no remote), the contract every lock op must honour is: fail *gracefully* —
//! exit 1 with a clean structured `{error:{kind,message}}` envelope, never hang,
//! crash, or emit non-JSON. The VS Code extension shows these as "you're
//! offline, locking is unavailable" instead of an opaque failure, so the
//! machine-readable envelope is the load-bearing part of the contract.
//!
//! Two distinct offline outcomes are valid and both are asserted as clean
//! envelopes:
//!   * arg-shape failures (a required field like `branch`/`owner` is absent) →
//!     `Parse`, caught before any network attempt;
//!   * "No remote configured" / connection failures when the op is well-formed
//!     but has nowhere to talk to → `CommandFailed`.

mod harness;

use harness::Repo;

/// Every lock op, when its args are well-formed but there is no remote, fails
/// with a clean structured envelope (exit 1, JSON, kind+message present) rather
/// than crashing. `err_envelope()` itself asserts all of that.
#[test]
fn lock_ops_fail_gracefully_offline() {
    let repo = Repo::create("alice");

    // (op, well-formed args) — args carry every required field so the failure is
    // the *offline/no-remote* path, not an arg-parse path.
    let cases: &[(&str, serde_json::Value)] = &[
        (
            "lock.file_query",
            serde_json::json!({ "branch": "main", "owner": "", "path": "" }),
        ),
        (
            "lock.file_status",
            serde_json::json!({ "branch": "main", "paths": ["note.txt"] }),
        ),
        (
            "lock.file_acquire",
            serde_json::json!({ "branch": "main", "paths": ["note.txt"] }),
        ),
        (
            "lock.file_acquire_as_owner",
            serde_json::json!({ "branch": "main", "paths": ["note.txt"], "owner": "alice" }),
        ),
        (
            "lock.file_release",
            serde_json::json!({
                "branch": "main", "paths": ["note.txt"], "owner": "alice", "owner_id": "id-1"
            }),
        ),
        (
            "lock.file_message_send",
            serde_json::json!({
                "branch": "main",
                "file_path": "note.txt",
                "to_user_id": "bob",
                "message_type": "free_text",
                "note": "please release",
            }),
        ),
    ];

    for (op, args) in cases {
        let run = repo.run(op, &args.to_string());
        // err_envelope asserts: !success, stdout is JSON, has error.{kind,message},
        // message non-empty. That is the whole "graceful offline" guarantee.
        let (kind, message) = run.err_envelope();
        assert!(
            kind == "CommandFailed" || kind == "Parse",
            "`{op}` offline should fail with CommandFailed/Parse, got `{kind}`: {message}"
        );
    }
}

/// A lock op missing a required arg fails at arg-parse time with a structured
/// `Parse` envelope — proving malformed input never reaches (or hangs on) the
/// network layer.
#[test]
fn lock_missing_required_arg_is_parse_error() {
    let repo = Repo::create("alice");
    // file_query requires `branch`; omit it entirely.
    let (kind, message) = repo.run("lock.file_query", "{}").err_envelope();
    assert_eq!(
        kind, "Parse",
        "missing lock arg should be a dispatch Parse error"
    );
    assert!(
        message.contains("branch"),
        "message should name the missing field: {message}"
    );
}

/// `lock.file_query` with a well-formed query but no remote reports the
/// "No remote configured" engine condition as a clean `CommandFailed` envelope —
/// the specific, user-facing offline signal.
#[test]
fn lock_query_no_remote_is_command_failed() {
    let repo = Repo::create("alice");
    let args = serde_json::json!({ "branch": "main", "owner": "", "path": "" });
    let (kind, message) = repo
        .run("lock.file_query", &args.to_string())
        .err_envelope();
    assert_eq!(
        kind, "CommandFailed",
        "no-remote should surface as CommandFailed: {message}"
    );
    assert!(
        message.to_lowercase().contains("no remote"),
        "expected a 'No remote configured' message offline: {message}"
    );
}
