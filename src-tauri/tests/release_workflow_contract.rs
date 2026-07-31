//! SBAI-5840: CI contract test for the release workflow's triggers.
//!
//! PR #445 proved that every merge to `main` armed the legacy `release.yml`
//! main-push trigger, letting a matrix leg clobber nightly assets before
//! cancellation. The binding trigger contract:
//!
//!   1. `release.yml` must NOT run on branch pushes — no `branches:` filter,
//!      no `main` reference, and no EMPTY/unfiltered `push:` (an empty push
//!      trigger re-arms every branch);
//!   2. stable tag releases must keep working: `push` carries a DIRECT
//!      `tags:` child matching `v*`, and nothing else;
//!   3. manual `workflow_dispatch` must keep working as a DIRECT child of
//!      `on:` — a `tags`-shaped string elsewhere (e.g. a dispatch input)
//!      must never satisfy requirement 2.
//!
//! The checks parse the DIRECT-CHILD structure of the `on:` block (review
//! finding on 3a1d7b3: independent substring searches were fail-open — an
//! empty `push:` plus `workflow_dispatch.inputs.tags: v*` passed). The
//! seeded bypass fixtures below must FAIL the contract forever. No YAML
//! dependency, so the test adds zero Cargo manifest/lock churn. Runs under
//! `cargo test -p loregui` (the tauri-e2e ipc-harness CI job).

use std::path::Path;

/// One parsed direct child of a block: key, inline value (after the colon),
/// and its indented sub-lines.
struct Child {
    key: String,
    inline: String,
    sub_lines: Vec<String>,
}

fn content_indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn is_noise(line: &str) -> bool {
    let t = line.trim_start();
    t.is_empty() || t.starts_with('#')
}

/// Split a block's lines into direct children (all content at the block's
/// first indent level) with their sub-lines. Fails closed on any structure
/// it does not recognize.
fn direct_children(lines: &[String]) -> Result<Vec<Child>, String> {
    let first = lines
        .iter()
        .find(|l| !is_noise(l))
        .ok_or("block has no content")?;
    let child_indent = content_indent(first);
    let mut children: Vec<Child> = Vec::new();
    for line in lines {
        if is_noise(line) {
            continue;
        }
        let indent = content_indent(line);
        if indent == child_indent {
            let t = line.trim();
            let (key, inline) = t
                .split_once(':')
                .ok_or_else(|| format!("unexpected non-key line in block: {t:?}"))?;
            children.push(Child {
                key: key.trim().to_string(),
                inline: inline.trim().to_string(),
                sub_lines: Vec::new(),
            });
        } else if indent > child_indent {
            children
                .last_mut()
                .ok_or_else(|| format!("indented line before any child key: {line:?}"))?
                .sub_lines
                .push(line.clone());
        } else {
            return Err(format!("inconsistent indentation in block: {line:?}"));
        }
    }
    Ok(children)
}

/// The full trigger contract, returning `Err(reason)` instead of panicking
/// so the seeded bypass fixtures can assert failure in-process.
fn check_trigger_contract(yaml: &str) -> Result<(), String> {
    if !yaml.contains("name: release") {
        return Err("not the release workflow".into());
    }

    // Collect the top-level `on:` block.
    let mut in_on = false;
    let mut on_lines: Vec<String> = Vec::new();
    for line in yaml.lines() {
        if line.trim_end() == "on:" {
            in_on = true;
            continue;
        }
        if in_on {
            let top_level = !line.starts_with([' ', '\t']) && !is_noise(line);
            if top_level {
                break;
            }
            on_lines.push(line.to_string());
        }
    }
    if !in_on {
        return Err("missing top-level on: block".into());
    }

    // Belt: nothing branch-shaped or main-shaped anywhere in the block, not
    // even commented out.
    let joined = on_lines.join("\n");
    if joined.contains("branches") {
        return Err(format!("branches filter present in on-block:\n{joined}"));
    }
    if joined.contains("main") {
        return Err(format!("`main` referenced in on-block:\n{joined}"));
    }

    // Structural core: the on-block's DIRECT children must be exactly
    // `push` and `workflow_dispatch` — any third trigger is a contract
    // change that needs a new ruling, not a silent pass.
    let triggers = direct_children(&on_lines).map_err(|e| format!("on-block: {e}"))?;
    let mut keys: Vec<&str> = triggers.iter().map(|c| c.key.as_str()).collect();
    keys.sort_unstable();
    if keys != ["push", "workflow_dispatch"] {
        return Err(format!(
            "on-block triggers must be exactly push + workflow_dispatch, got {keys:?}"
        ));
    }

    // `push` must be NONEMPTY (empty = unfiltered = every branch) and its
    // direct children must be exactly one `tags` filter carrying v*.
    let push = triggers.iter().find(|c| c.key == "push").expect("checked");
    if !push.inline.is_empty() {
        return Err(format!(
            "push must be a mapping with a tags filter, got inline value {:?}",
            push.inline
        ));
    }
    if push.sub_lines.iter().all(|l| is_noise(l)) {
        return Err("push: is EMPTY — an unfiltered push trigger re-arms every branch".into());
    }
    let filters = direct_children(&push.sub_lines).map_err(|e| format!("push block: {e}"))?;
    if filters.len() != 1 || filters[0].key != "tags" {
        let keys: Vec<&str> = filters.iter().map(|c| c.key.as_str()).collect();
        return Err(format!("push filters must be exactly [tags], got {keys:?}"));
    }
    let tags = &filters[0];
    // Canonical-form enforcement (review addendum on a5d3820): the
    // generalized quote/list parser kept yielding exact-value bypasses —
    // recursive quote-trimming turned ["'v*'"] (a pattern whose real text
    // is 'v*', quotes included) into v*. The workflow uses exactly ONE
    // syntax; anything else — alternate quoting, scalars, sub-lists, extra
    // members, extra bytes — fails closed until a reviewed contract change.
    // Byte-exact: no comment mini-parser either — a trailing comment on the
    // tags line is itself non-canonical and fails closed.
    let value = tags.inline.trim();
    if value != "[\"v*\"]" {
        return Err(format!(
            "push.tags must be exactly the canonical [\"v*\"] — got {value:?}; \
             alternate formatting requires a reviewed contract change"
        ));
    }
    if !tags.sub_lines.iter().all(|l| is_noise(l)) {
        return Err("push.tags must be the inline canonical [\"v*\"] with no sub-lines".into());
    }
    Ok(())
}

fn release_workflow() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows/release.yml");
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "release workflow must exist at {} (contract test guards its triggers): {error}",
            path.display()
        )
    })
}

#[test]
fn release_workflow_satisfies_the_trigger_contract() {
    if let Err(reason) = check_trigger_contract(&release_workflow()) {
        panic!("release.yml violates the SBAI-5840 trigger contract: {reason}");
    }
}

/// Review finding on 3a1d7b3, kept forever as a seeded bypass: an EMPTY
/// `push:` (which re-arms every branch) combined with a `tags: v*` string
/// hidden under `workflow_dispatch.inputs` fooled substring checks. The
/// structural contract must reject it.
#[test]
fn empty_push_with_dispatch_input_tags_decoy_fails() {
    let fixture = "name: release\non:\n  push:\n  workflow_dispatch:\n    inputs:\n      tags:\n        description: \"release v* tag\"\njobs: {}\n";
    let error = check_trigger_contract(fixture).expect_err("bypass fixture must fail");
    assert!(
        error.contains("EMPTY"),
        "names the empty-push defect: {error}"
    );
}

/// The legacy pre-SBAI-5840 shape (the PR #445 incident) must keep failing.
#[test]
fn legacy_main_branch_trigger_fails() {
    let fixture =
        "name: release\non:\n  push:\n    branches: [main]\n    tags: [\"v*\"]\n  workflow_dispatch:\njobs: {}\n";
    let error = check_trigger_contract(fixture).expect_err("legacy shape must fail");
    assert!(error.contains("branches"), "{error}");
}

/// A tags filter that is not a DIRECT child of push satisfies nothing.
#[test]
fn tags_outside_push_fails() {
    let fixture =
        "name: release\non:\n  push:\n    paths: [\"src/**\"]\n  tags: [\"v*\"]\n  workflow_dispatch:\njobs: {}\n";
    let error = check_trigger_contract(fixture).expect_err("misplaced tags must fail");
    assert!(
        error.contains("exactly push + workflow_dispatch") || error.contains("exactly [tags]"),
        "{error}"
    );
}

/// Any third trigger, or a missing manual dispatch, is a contract change
/// that must fail CI rather than pass silently.
#[test]
fn extra_or_missing_triggers_fail() {
    let extra =
        "name: release\non:\n  push:\n    tags: [\"v*\"]\n  workflow_dispatch:\n  schedule:\n    - cron: \"0 0 * * *\"\njobs: {}\n";
    check_trigger_contract(extra).expect_err("a third trigger must fail");

    let missing_dispatch = "name: release\non:\n  push:\n    tags: [\"v*\"]\njobs: {}\n";
    check_trigger_contract(missing_dispatch).expect_err("missing workflow_dispatch must fail");
}

/// Review finding on 1a194d3: `contains("v*")` passed the decoy pattern
/// `["not-v*"]`, which never matches a real stable tag — the pattern list
/// must be exactly `["v*"]`, and a comment cannot stand in for it.
#[test]
fn not_v_star_and_comment_decoy_tag_patterns_fail() {
    let not_v_star =
        "name: release\non:\n  push:\n    tags: [\"not-v*\"]\n  workflow_dispatch:\njobs: {}\n";
    let error = check_trigger_contract(not_v_star).expect_err("not-v* decoy must fail");
    assert!(error.contains("exactly"), "{error}");

    let comment_decoy =
        "name: release\non:\n  push:\n    tags: [\"release-only\"] # v*\n  workflow_dispatch:\njobs: {}\n";
    let error = check_trigger_contract(comment_decoy).expect_err("comment decoy must fail");
    assert!(error.contains("exactly"), "{error}");
}

/// Review finding on 0267091, all three proven fail-open by compiling the
/// prior parser against them: comments hid the real (null/scalar) tags
/// value from the bracket search, and empty members were silently dropped.
#[test]
fn comment_hidden_and_empty_member_tag_values_fail() {
    // Real value is NULL — the ["v*"] lives entirely inside a comment.
    let null_value =
        "name: release\non:\n  push:\n    tags: # [\"v*\"]\n  workflow_dispatch:\njobs: {}\n";
    check_trigger_contract(null_value).expect_err("comment-only tags value must fail");

    // Real value is the scalar `release-only`; the comment carries the decoy.
    let scalar_decoy = "name: release\non:\n  push:\n    tags: release-only # [\"v*\"]\n  workflow_dispatch:\njobs: {}\n";
    let error = check_trigger_contract(scalar_decoy).expect_err("scalar decoy must fail");
    assert!(error.contains("release-only"), "{error}");

    // Empty sequence member violates the exact-list contract.
    let empty_member =
        "name: release\non:\n  push:\n    tags: [\"v*\", \"\"]\n  workflow_dispatch:\njobs: {}\n";
    let error = check_trigger_contract(empty_member).expect_err("empty member must fail");
    assert!(error.contains("canonical"), "{error}");
}

/// Review finding on a5d3820: `tags: release-only ["v*"]` is VALID YAML
/// whose real value is the whole scalar — the bracket content must not be
/// parsed out of the middle of it.
#[test]
fn scalar_prefixed_bracket_decoy_fails() {
    let fixture = "name: release\non:\n  push:\n    tags: release-only [\"v*\"]\n  workflow_dispatch:\njobs: {}\n";
    let error = check_trigger_contract(fixture).expect_err("scalar-prefixed bracket must fail");
    assert!(
        error.contains("complete") || error.contains("exactly"),
        "{error}"
    );
}

/// Review addendum on a5d3820: valid YAML whose real pattern text includes
/// literal quote characters (so it is NOT v*) must fail — recursive
/// quote-trimming used to normalize it into a false pass.
#[test]
fn nested_quote_tag_patterns_fail() {
    let quoted_list =
        "name: release\non:\n  push:\n    tags: [\"'v*'\"]\n  workflow_dispatch:\njobs: {}\n";
    let error = check_trigger_contract(quoted_list).expect_err("nested-quote list must fail");
    assert!(error.contains("canonical"), "{error}");

    let quoted_scalar =
        "name: release\non:\n  push:\n    tags: \"'v*'\"\n  workflow_dispatch:\njobs: {}\n";
    let error = check_trigger_contract(quoted_scalar).expect_err("nested-quote scalar must fail");
    assert!(error.contains("canonical"), "{error}");
}
