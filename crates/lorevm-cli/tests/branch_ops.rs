//! Branch domain over the built `lorevm` binary: list / create / switch, all
//! cross-process against one shared on-disk repo.

mod harness;

use harness::Repo;
use serde_json::Value;

/// Helper: names of the branches `branch.list` reports.
fn branch_names(repo: &Repo) -> Vec<String> {
    let v = repo.run_noargs("branch.list").ok_value().clone();
    v.get("entries")
        .and_then(Value::as_array)
        .expect("branch.list entries[]")
        .iter()
        .filter_map(|e| e.get("name").and_then(Value::as_str).map(str::to_string))
        .collect()
}

/// A fresh repo (after its first commit) lists exactly the current `main`
/// branch, flagged `is_current`.
#[test]
fn branch_list_reports_main_as_current() {
    let repo = Repo::create("alice");
    repo.write_file("seed.txt", "seed\n");
    repo.stage("seed.txt");
    repo.commit("seed");

    let v = repo.run_noargs("branch.list").ok_value().clone();
    let entries = v
        .get("entries")
        .and_then(Value::as_array)
        .expect("entries[]");
    let main = entries
        .iter()
        .find(|e| e.get("name").and_then(Value::as_str) == Some("main"))
        .unwrap_or_else(|| panic!("no `main` branch in list: {v}"));
    assert_eq!(
        main.get("is_current").and_then(Value::as_bool),
        Some(true),
        "main should be the current branch: {v}"
    );
    assert_eq!(
        v.get("count").and_then(Value::as_u64),
        Some(entries.len() as u64),
        "count must match entries length: {v}"
    );
}

/// `branch.create` adds a new branch (visible to a later `branch.list`
/// process), and `branch.switch` moves onto it and back — the full
/// create → switch → switch-back loop, each step its own process.
#[test]
fn branch_create_switch_roundtrip() {
    let repo = Repo::create("alice");
    repo.write_file("seed.txt", "seed\n");
    repo.stage("seed.txt");
    repo.commit("seed");

    // create
    let created = repo
        .run(
            "branch.create",
            &serde_json::json!({ "branch": "feature-x" }).to_string(),
        )
        .ok_value()
        .clone();
    assert_eq!(
        created.get("name").and_then(Value::as_str),
        Some("feature-x"),
        "create result should echo the branch name: {created}"
    );

    // a later, separate process sees it
    assert!(
        branch_names(&repo).iter().any(|n| n == "feature-x"),
        "feature-x should appear in branch.list after create"
    );

    // switch onto it
    let sw = repo
        .run(
            "branch.switch",
            &serde_json::json!({ "branch": "feature-x" }).to_string(),
        )
        .ok_value()
        .clone();
    assert_eq!(
        sw.get("branch").and_then(Value::as_str),
        Some("feature-x"),
        "switch result should report the new branch: {sw}"
    );

    // switch back to main
    let back = repo
        .run(
            "branch.switch",
            &serde_json::json!({ "branch": "main" }).to_string(),
        )
        .ok_value()
        .clone();
    assert_eq!(
        back.get("branch").and_then(Value::as_str),
        Some("main"),
        "switch back should report main: {back}"
    );
}

/// `branch.create` missing its required `branch` field is a structured `Parse`
/// error.
#[test]
fn branch_create_missing_name_is_parse_error() {
    let repo = Repo::create("alice");
    let (kind, message) = repo.run("branch.create", "{}").err_envelope();
    assert_eq!(kind, "Parse");
    assert!(
        message.contains("branch"),
        "should name the missing field: {message}"
    );
}

/// `branch.switch` to a non-existent branch is a clean structured engine error.
#[test]
fn branch_switch_unknown_branch_errors_cleanly() {
    let repo = Repo::create("alice");
    repo.write_file("seed.txt", "seed\n");
    repo.stage("seed.txt");
    repo.commit("seed");

    let (kind, message) = repo
        .run(
            "branch.switch",
            &serde_json::json!({ "branch": "does-not-exist" }).to_string(),
        )
        .err_envelope();
    assert_ne!(kind, "cli", "should reach dispatch: {message}");
    assert!(
        message.to_lowercase().contains("not found"),
        "expected a 'branch not found' engine error: {message}"
    );
}
