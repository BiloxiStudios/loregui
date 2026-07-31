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
    // Exact positive enforcement (review finding on 1a194d3): the stable
    // pattern must be the literal list member `v*`. A substring check passed
    // decoys like `["not-v*"]` — a glob that never matches a normal v1.2.3
    // tag and silently kills the stable release path.
    let patterns = tag_patterns(tags)?;
    if patterns != ["v*"] {
        return Err(format!(
            "push.tags patterns must be exactly [\"v*\"], got {patterns:?}"
        ));
    }

    Ok(())
}

/// Cut a YAML fragment at its first UNQUOTED `#` (review finding on
/// 0267091: comment stripping must happen BEFORE any bracket/scalar
/// parsing, or `tags: # ["v*"]` — whose real value is null — passes the
/// bracket search inside the comment).
fn strip_unquoted_comment(fragment: &str) -> &str {
    let mut in_double = false;
    let mut in_single = false;
    for (i, c) in fragment.char_indices() {
        match c {
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            '#' if !in_double && !in_single => return &fragment[..i],
            _ => {}
        }
    }
    fragment
}

/// Parse the tags filter's pattern list: inline `["a", "b"]`, scalar, or a
/// `- "a"` sub-list. The first unquoted comment is stripped BEFORE any
/// parsing; empty sequence members are violations, not ignorable noise.
/// Fails closed on anything else.
fn tag_patterns(tags: &Child) -> Result<Vec<String>, String> {
    let unquote = |s: &str| s.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
    let inline = strip_unquoted_comment(&tags.inline).trim();
    // An inline list must consume the ENTIRE trimmed value (review finding
    // on a5d3820: bracket-searching anywhere let the valid scalar
    // `release-only ["v*"]` pass on the bracket while its real YAML value
    // is the whole scalar).
    if inline.starts_with('[') || inline.ends_with(']') {
        let body = inline
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .ok_or_else(|| {
                format!("tags value must be a complete [..] list or a scalar, got {inline:?}")
            })?;
        {
            let mut patterns = Vec::new();
            for member in body.split(',') {
                let member = unquote(member);
                if member.is_empty() {
                    return Err(format!("tags list has an empty pattern member: {inline:?}"));
                }
                patterns.push(member);
            }
            return Ok(patterns);
        }
    }
    if !inline.is_empty() {
        // Scalar form `tags: v*` (comment already stripped above).
        let scalar = unquote(inline);
        if scalar.is_empty() {
            return Err(format!("unparseable tags value {:?}", tags.inline));
        }
        return Ok(vec![scalar]);
    }
    let mut patterns = Vec::new();
    for line in &tags.sub_lines {
        let t = strip_unquoted_comment(line).trim();
        if t.is_empty() {
            continue;
        }
        let item = t
            .strip_prefix('-')
            .ok_or_else(|| format!("unexpected tags sub-line {t:?}"))?;
        let item = unquote(item);
        if item.is_empty() {
            return Err(format!(
                "tags sub-list has an empty pattern member: {line:?}"
            ));
        }
        patterns.push(item);
    }
    if patterns.is_empty() {
        return Err("tags filter has no patterns".into());
    }
    Ok(patterns)
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
    assert!(error.contains("empty pattern member"), "{error}");
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
