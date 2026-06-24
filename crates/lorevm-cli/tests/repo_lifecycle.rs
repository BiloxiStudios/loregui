//! Repository / revision / file lifecycle over the built `lorevm` binary.
//!
//! Each op here runs as its own subprocess against a shared on-disk repo,
//! mirroring exactly how the VS Code extension shells out one `lorevm` process
//! per op. The cross-process `stage` → `commit` case is the regression guard for
//! the SBAI-4080 flush bug, where a staged anchor written by one process was
//! lost before the next process could commit it.

mod harness;

use harness::Repo;
use serde_json::Value;

/// `repository.create` (run by the harness) followed by `repository.status` on
/// the fresh, empty repo: status reports the `main` branch, revision 0, and no
/// tracked files.
#[test]
fn create_then_status_on_empty_repo() {
    let repo = Repo::create("alice");
    let v = repo.run_noargs("repository.status").ok_value().clone();

    let rev = v.get("revision").expect("status has a revision block");
    assert_eq!(
        rev.get("branch_name").and_then(Value::as_str),
        Some("main"),
        "fresh repo should be on `main`: {v}"
    );
    assert_eq!(
        rev.get("revision_number").and_then(Value::as_u64),
        Some(0),
        "fresh repo should be at revision 0: {v}"
    );
    assert_eq!(
        v.get("files").and_then(Value::as_array).map(Vec::len),
        Some(0),
        "fresh repo should have no tracked files: {v}"
    );
}

/// THE cross-process flush regression guard (SBAI-4080).
///
/// `file.stage` and `revision.commit` run as two SEPARATE `lorevm` processes.
/// The first must durably flush the staged-revision anchor to the on-disk store
/// before exiting; otherwise the second process sees "Nothing staged for
/// commit". A green run here proves `finalize()`'s synchronous flush is wired.
#[test]
fn cross_process_stage_then_commit() {
    let repo = Repo::create("alice");
    repo.write_file("note.txt", "hello world\n");

    // --- process 1: stage ---
    let staged = repo.stage("note.txt");
    let files = staged
        .get("files")
        .and_then(Value::as_array)
        .expect("stage files[]");
    assert!(
        files.iter().any(|f| f
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|p| p.ends_with("note.txt"))
            && f.get("action").and_then(Value::as_str) == Some("add")),
        "stage should report note.txt as an `add`: {staged}"
    );

    // --- process 2 (fresh process, no shared in-memory state): commit ---
    let rev = repo.commit("add note");

    // --- process 3: history sees exactly the one commit we just made ---
    let hist = repo.run_noargs("revision.history").ok_value().clone();
    let entries = hist
        .get("entries")
        .and_then(Value::as_array)
        .expect("history entries[]");
    assert_eq!(entries.len(), 1, "expected exactly one revision: {hist}");
    assert_eq!(
        entries[0].get("revision").and_then(Value::as_str),
        Some(rev.as_str()),
        "history head must be the commit we made: {hist}"
    );
    assert_eq!(
        entries[0].get("revision_number").and_then(Value::as_u64),
        Some(1),
        "first commit should be revision_number 1: {hist}"
    );
}

/// `revision.info` (delta + metadata) on a committed revision exposes the
/// message, author, and the touched file's delta — the shape `lore-mcp` renders.
#[test]
fn revision_info_exposes_message_author_and_delta() {
    let repo = Repo::create("alice");
    repo.write_file("note.txt", "abc\n");
    repo.stage("note.txt");
    let rev = repo.commit("add note");

    let args = serde_json::json!({ "revision": rev, "delta": true, "metadata": true });
    let v = repo
        .run("revision.info", &args.to_string())
        .ok_value()
        .clone();

    // Delta records note.txt as an Add.
    let deltas = v
        .get("deltas")
        .and_then(Value::as_array)
        .expect("info deltas[]");
    assert!(
        deltas.iter().any(|d| d
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|p| p.ends_with("note.txt"))
            && d.get("action").and_then(Value::as_str) == Some("Add")),
        "info delta should record note.txt as Add: {v}"
    );

    // Metadata carries the message + author attribution.
    let meta = v
        .get("metadata")
        .and_then(Value::as_array)
        .expect("info metadata[]");
    let kv = |key: &str| {
        meta.iter()
            .find(|m| m.get("key").and_then(Value::as_str) == Some(key))
            .and_then(|m| m.get("value").and_then(Value::as_str))
            .map(str::to_string)
    };
    assert_eq!(
        kv("message").as_deref(),
        Some("add note"),
        "message metadata: {v}"
    );
    assert_eq!(
        kv("created-by").as_deref(),
        Some("alice"),
        "author metadata: {v}"
    );
}

/// `revision.info` on a revision that does not exist is a clean structured
/// engine error, not a crash.
#[test]
fn revision_info_unknown_revision_errors_cleanly() {
    let repo = Repo::create("alice");
    let args = serde_json::json!({ "revision": "deadbeef", "delta": true, "metadata": true });
    let (kind, message) = repo.run("revision.info", &args.to_string()).err_envelope();
    assert_ne!(
        kind, "cli",
        "should reach dispatch, not fail at the CLI layer"
    );
    assert!(
        message.to_lowercase().contains("not found"),
        "expected a 'not found' engine error: {message}"
    );
}

/// `revision.commit` with nothing staged is a clean structured error.
#[test]
fn commit_with_nothing_staged_errors_cleanly() {
    let repo = Repo::create("alice");
    let args = serde_json::json!({ "message": "empty" });
    let (kind, _message) = repo
        .run("revision.commit", &args.to_string())
        .err_envelope();
    assert_ne!(kind, "cli");
}

/// `revision.history` on a fresh repo returns an empty `entries` list (exit 0,
/// well-formed) rather than erroring.
#[test]
fn history_on_empty_repo_is_empty_list() {
    let repo = Repo::create("alice");
    let v = repo.run_noargs("revision.history").ok_value().clone();
    assert_eq!(
        v.get("entries").and_then(Value::as_array).map(Vec::len),
        Some(0),
        "empty repo history should be []: {v}"
    );
}

/// Stage a file, then `file.unstage` it (separate processes). The unstage result
/// reports the file with its counts — proving the staged anchor written by one
/// process is visible to the next, and is reversible.
#[test]
fn cross_process_stage_then_unstage() {
    let repo = Repo::create("alice");
    repo.write_file("scratch.txt", "temp\n");
    repo.stage("scratch.txt");

    let abs = repo.path("scratch.txt");
    let args = serde_json::json!({ "paths": [abs.to_string_lossy()] });
    let v = repo
        .run("file.unstage", &args.to_string())
        .ok_value()
        .clone();

    let files = v
        .get("files")
        .and_then(Value::as_array)
        .expect("unstage files[]");
    assert!(
        files.iter().any(|f| f
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|p| p.ends_with("scratch.txt"))),
        "unstage should report scratch.txt: {v}"
    );
    assert!(
        v.get("counts")
            .and_then(|c| c.get("total_count"))
            .and_then(Value::as_u64)
            .is_some_and(|n| n >= 1),
        "unstage counts.total_count should be >= 1: {v}"
    );
}

/// A multi-commit history is ordered newest-first with linked parents — the
/// audit shape `revision.history` guarantees, driven entirely cross-process.
#[test]
fn multi_commit_history_is_ordered_newest_first() {
    let repo = Repo::create("alice");

    repo.write_file("a.txt", "one\n");
    repo.stage("a.txt");
    let rev1 = repo.commit("add a");

    repo.write_file("b.txt", "two\n");
    repo.stage("b.txt");
    let rev2 = repo.commit("add b");
    assert_ne!(rev1, rev2, "second commit must be a distinct revision");

    let v = repo.run_noargs("revision.history").ok_value().clone();
    let entries = v
        .get("entries")
        .and_then(Value::as_array)
        .expect("entries[]");
    assert_eq!(entries.len(), 2, "expected 2 revisions: {v}");
    assert_eq!(
        entries[0].get("revision").and_then(Value::as_str),
        Some(rev2.as_str()),
        "history[0] should be the newest commit: {v}"
    );
    assert_eq!(
        entries[1].get("revision").and_then(Value::as_str),
        Some(rev1.as_str()),
        "history[1] should be the older commit: {v}"
    );
    // The newer revision links back to the older one.
    let parents = entries[0]
        .get("parents")
        .and_then(Value::as_array)
        .expect("parents[]");
    assert!(
        parents.iter().any(|p| p.as_str() == Some(rev1.as_str())),
        "newest revision's parent should be the first commit: {v}"
    );
}

/// `repository.create` missing its one required field (`repository_url`) is a
/// structured `Parse` error from dispatch's arg deserialisation.
#[test]
fn create_missing_required_arg_is_parse_error() {
    // Build a bare repo dir but invoke create with empty args.
    let tmp = tempfile::tempdir().expect("tempdir");
    let run = harness::run_cli(&[
        "repository.create",
        "--dir",
        &tmp.path().to_string_lossy(),
        "--offline",
        "--args",
        "{}",
    ]);
    let (kind, message) = run.err_envelope();
    assert_eq!(
        kind, "Parse",
        "missing required arg should be a dispatch Parse error"
    );
    assert!(
        message.contains("repository_url"),
        "message should name the missing field: {message}"
    );
}
