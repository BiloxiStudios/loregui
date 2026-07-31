//! SBAI-5840: CI contract test for the release workflow's triggers.
//!
//! PR #445 proved that every merge to `main` armed the legacy `release.yml`
//! main-push trigger, letting a matrix leg clobber nightly assets before
//! cancellation. The binding trigger contract is therefore:
//!
//!   1. `release.yml` must NOT run on branch pushes (no `branches:` filter
//!      under `on:` at all, and no reference to `main` there);
//!   2. stable tag releases (`push.tags: v*`) must KEEP working;
//!   3. manual `workflow_dispatch` must KEEP working.
//!
//! The test is a line-structural parse of the top-level `on:` block — no
//! YAML dependency, so it cannot add Cargo manifest/lock churn. It runs
//! under `cargo test -p loregui` (the tauri-e2e ipc-harness CI job), so a
//! future edit that re-adds a main-push trigger fails CI, not review.

use std::path::Path;

fn release_workflow() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows/release.yml");
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "release workflow must exist at {} (contract test guards its triggers): {error}",
            path.display()
        )
    })
}

/// The top-level `on:` block: every subsequent line that is indented, blank,
/// or a comment, up to the next top-level key. Panics if the block is absent
/// or empty so the assertions below can never pass vacuously.
fn on_block(yaml: &str) -> String {
    let mut block = String::new();
    let mut in_on = false;
    for line in yaml.lines() {
        if line.trim_end() == "on:" {
            in_on = true;
            continue;
        }
        if in_on {
            let is_top_level_key = !line.starts_with([' ', '\t'])
                && !line.trim().is_empty()
                && !line.trim_start().starts_with('#');
            if is_top_level_key {
                break;
            }
            block.push_str(line);
            block.push('\n');
        }
    }
    assert!(
        in_on && !block.trim().is_empty(),
        "release.yml must have a non-empty top-level `on:` block"
    );
    block
}

#[test]
fn release_workflow_never_triggers_on_branch_pushes() {
    let yaml = release_workflow();
    assert!(
        yaml.contains("name: release"),
        "contract test must be reading the release workflow"
    );
    let on = on_block(&yaml);
    assert!(
        !on.contains("branches"),
        "release.yml must not carry ANY branch-push trigger — a main merge \
         must never arm a release run (SBAI-5840 / PR #445 incident). on-block:\n{on}"
    );
    assert!(
        !on.contains("main"),
        "no reference to main may remain in the on-block:\n{on}"
    );
}

#[test]
fn release_workflow_keeps_stable_tag_and_manual_dispatch_triggers() {
    let on = on_block(&release_workflow());
    assert!(
        on.contains("push:"),
        "tag pushes still ride the push event:\n{on}"
    );
    let tags_line = on
        .lines()
        .find(|line| line.trim_start().starts_with("tags:"))
        .unwrap_or_else(|| panic!("push.tags filter must be present:\n{on}"));
    assert!(
        tags_line.contains("v*"),
        "stable v* tag releases must keep working: {tags_line}"
    );
    assert!(
        on.contains("workflow_dispatch"),
        "manual dispatch must keep working:\n{on}"
    );
}
