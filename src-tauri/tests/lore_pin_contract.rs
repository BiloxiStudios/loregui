//! SBAI-5910: fail-closed contract for the pinned upstream `lore` source.
//!
//! Three pins must agree with each other AND with byte-exact accepted
//! constants: the `lore` dependency, the vendored `quinn-proto` patch, and
//! the `lore-credential` dev-dependency that
//! `tests/credential_boundary_guard.rs` uses to prove the cached-token DENY
//! behavior.
//!
//! Consistency alone is NOT a guard (review finding on b19d0e3): moving all
//! pins together to another host or another 40-hex SHA would keep a
//! consistency-only check green while silently leaving the accepted
//! maintenance tree — including a move back to a tree without the SBAI-5909
//! credential fix. A dev pin pointing somewhere other than the runtime pin
//! is just as bad: the tests would exercise the safe tree while the shipped
//! binary used another. So the accepted host and rev are literals here, and
//! changing them is a deliberate, reviewable edit to this file.
//!
//! [`check_pins`] takes manifest/lock text and returns `Result`, so the
//! seeded must-fail fixtures below prove the guard actually bites.

/// The only accepted upstream source. `ba92f943` is the signed maintenance
/// merge carrying the exact-domain JWT label boundary and the legacy
/// unscoped-token fail-closed fix (SBAI-5909); it exists ONLY on this fork.
const ACCEPTED_HOST: &str = "https://github.com/BiloxiStudios/lore.git";
const ACCEPTED_REV: &str = "ba92f94305df15796283755040c0bdd9b351841e";

/// Every package the lock resolves from the lore tree at the accepted pin.
/// Binding the exact identities means a removed, added, or substituted
/// package cannot hide behind a "some sources matched" check.
const EXPECTED_LOCK_PACKAGES: [&str; 13] = [
    "lore",
    "lore-base",
    "lore-credential",
    "lore-error-set",
    "lore-error-set-macro",
    "lore-macro",
    "lore-notification",
    "lore-proto",
    "lore-revision",
    "lore-storage",
    "lore-telemetry",
    "lore-transport",
    "quinn-proto",
];

/// `(host, rev)` for a `name = { git = "...", rev = "..." }` line.
fn pinned_dep(manifest: &str, name: &str) -> Result<(String, String), String> {
    let line = manifest
        .lines()
        .map(str::trim_start)
        .find(|l| l.starts_with(&format!("{name} = ")))
        .ok_or_else(|| format!("manifest must pin `{name}`"))?;
    let grab = |key: &str| -> Result<String, String> {
        let needle = format!("{key} = \"");
        let start = line
            .find(&needle)
            .ok_or_else(|| format!("`{name}` pin missing {key}: {line}"))?
            + needle.len();
        let rest = &line[start..];
        let end = rest
            .find('"')
            .ok_or_else(|| format!("unterminated {key} in `{name}`: {line}"))?;
        Ok(rest[..end].to_string())
    };
    Ok((grab("git")?, grab("rev")?))
}

/// Full contract over supplied text. `Err(reason)` on any violation.
fn check_pins(workspace_manifest: &str, tauri_manifest: &str, lock: &str) -> Result<(), String> {
    // 1. All three pins must be byte-exactly the accepted host + rev. This
    //    also covers mixed-host/mixed-rev and non-40-hex by construction.
    for (label, manifest, dep) in [
        ("lore", workspace_manifest, "lore"),
        ("quinn-proto", workspace_manifest, "quinn-proto"),
        ("lore-credential (dev)", tauri_manifest, "lore-credential"),
    ] {
        let (host, rev) = pinned_dep(manifest, dep)?;
        if host != ACCEPTED_HOST {
            return Err(format!(
                "{label} pins host {host:?}; only {ACCEPTED_HOST:?} is accepted \
                 (the SBAI-5909 fix exists only there)"
            ));
        }
        if rev != ACCEPTED_REV {
            return Err(format!(
                "{label} pins rev {rev:?}; only {ACCEPTED_REV:?} is accepted"
            ));
        }
    }

    // 2. Every lock source line from any lore tree must be exactly the
    //    accepted source — no stale, mixed, or substituted origin.
    let expected_source =
        format!("source = \"git+{ACCEPTED_HOST}?rev={ACCEPTED_REV}#{ACCEPTED_REV}\"");
    for line in lock.lines().map(str::trim) {
        if line.starts_with("source = \"git+")
            && line.contains("/lore.git?rev=")
            && line != expected_source
        {
            return Err(format!("lock carries a non-accepted lore source: {line}"));
        }
    }

    // 3. The exact package identities must resolve from that source —
    //    neither fewer (removal/substitution) nor more (unexpected addition).
    let mut found: Vec<String> = Vec::new();
    let mut current_name: Option<String> = None;
    for line in lock.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("name = \"") {
            current_name = rest.strip_suffix('"').map(str::to_string);
        } else if line == expected_source {
            match current_name.take() {
                Some(name) => found.push(name),
                None => return Err("lock source line without a preceding name".into()),
            }
        }
    }
    found.sort();
    found.dedup();
    let mut expected: Vec<String> = EXPECTED_LOCK_PACKAGES
        .iter()
        .map(|s| s.to_string())
        .collect();
    expected.sort();
    if found != expected {
        return Err(format!(
            "lock packages at the accepted source changed: expected {expected:?}, found {found:?}"
        ));
    }
    Ok(())
}

/// SBAI-5910 (lock ruling): the PR's lock delta against the base commit must
/// be exactly the 13 lore-tree source repins plus the single direct
/// `lore-credential` edge — zero registry/resolver churn. Restoring the base
/// lock and repinning mechanically (rather than re-resolving) proved the base
/// graph is valid under `--locked`, so any additional edge movement in a
/// future bump is unexplained churn that must be justified, not absorbed.
///
/// Verified structurally here: every lore-tree source line is the accepted
/// one (checked above) AND no lore-tree package resolves from more than one
/// source, which is what a partial/stale repin would look like.
fn check_no_split_lore_sources(lock: &str) -> Result<(), String> {
    let expected_source =
        format!("source = \"git+{ACCEPTED_HOST}?rev={ACCEPTED_REV}#{ACCEPTED_REV}\"");
    let mut current_name: Option<String> = None;
    for line in lock.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("name = \"") {
            current_name = rest.strip_suffix('"').map(str::to_string);
        } else if line.starts_with("source = \"git+") && line.contains("/lore.git?rev=") {
            let name = current_name.clone().unwrap_or_default();
            if line != expected_source {
                return Err(format!(
                    "package {name} resolves from a non-accepted lore source: {line}"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn lock_carries_no_split_or_stale_lore_sources() {
    let (_, _, lock) = repo_files();
    if let Err(reason) = check_no_split_lore_sources(&lock) {
        panic!("lore lock sources are inconsistent: {reason}");
    }
    // A stale line anywhere must be caught.
    let stale = format!(
        "[[package]]\nname = \"lore-transport\"\nsource = \"git+https://github.com/EpicGames/lore.git?rev=9664606f5a4708606642a6670a57d16bd3d37596#9664606f5a4708606642a6670a57d16bd3d37596\"\n"
    );
    check_no_split_lore_sources(&stale).expect_err("a stale lore source must fail");
}

fn repo_files() -> (String, String, String) {
    let src_tauri = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = src_tauri.parent().expect("src-tauri has a parent");
    let read = |p: std::path::PathBuf| {
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };
    (
        read(root.join("Cargo.toml")),
        read(src_tauri.join("Cargo.toml")),
        read(root.join("Cargo.lock")),
    )
}

#[test]
fn repository_pins_satisfy_the_contract() {
    let (workspace, tauri, lock) = repo_files();
    if let Err(reason) = check_pins(&workspace, &tauri, &lock) {
        panic!("lore pin contract violated: {reason}");
    }
}

// ---------------------------------------------------------------------------
// Seeded must-fail fixtures — the guard has to BITE, not just pass on the
// happy tree (review finding on b19d0e3).
// ---------------------------------------------------------------------------

fn manifest_with(host: &str, rev: &str) -> String {
    format!("lore = {{ git = \"{host}\", rev = \"{rev}\" }}\nquinn-proto = {{ git = \"{host}\", rev = \"{rev}\" }}\n")
}

fn dev_manifest_with(host: &str, rev: &str) -> String {
    format!("lore-credential = {{ git = \"{host}\", rev = \"{rev}\" }}\n")
}

fn good_lock() -> String {
    let source = format!("source = \"git+{ACCEPTED_HOST}?rev={ACCEPTED_REV}#{ACCEPTED_REV}\"");
    EXPECTED_LOCK_PACKAGES
        .iter()
        .map(|name| format!("[[package]]\nname = \"{name}\"\nversion = \"0.0.0\"\n{source}\n"))
        .collect()
}

fn accepted_manifests() -> (String, String) {
    (
        manifest_with(ACCEPTED_HOST, ACCEPTED_REV),
        dev_manifest_with(ACCEPTED_HOST, ACCEPTED_REV),
    )
}

#[test]
fn fixture_baseline_passes_so_the_negatives_are_meaningful() {
    let (w, t) = accepted_manifests();
    check_pins(&w, &t, &good_lock()).expect("accepted fixture must pass");
}

#[test]
fn both_pins_moving_together_to_the_wrong_host_or_rev_fails() {
    let (_, dev) = accepted_manifests();
    let epic = manifest_with("https://github.com/EpicGames/lore.git", ACCEPTED_REV);
    let error = check_pins(&epic, &dev, &good_lock()).expect_err("wrong host must fail");
    assert!(error.contains("pins host"), "{error}");

    let other_sha = manifest_with(ACCEPTED_HOST, "0123456789abcdef0123456789abcdef01234567");
    let error = check_pins(&other_sha, &dev, &good_lock()).expect_err("wrong rev must fail");
    assert!(error.contains("pins rev"), "{error}");
}

#[test]
fn mixed_host_mixed_rev_and_non_40_hex_fail() {
    let (_, dev) = accepted_manifests();
    let mixed_host = format!(
        "lore = {{ git = \"{ACCEPTED_HOST}\", rev = \"{ACCEPTED_REV}\" }}\nquinn-proto = {{ git = \"https://github.com/EpicGames/lore.git\", rev = \"{ACCEPTED_REV}\" }}\n"
    );
    check_pins(&mixed_host, &dev, &good_lock()).expect_err("mixed host must fail");

    let mixed_rev = format!(
        "lore = {{ git = \"{ACCEPTED_HOST}\", rev = \"{ACCEPTED_REV}\" }}\nquinn-proto = {{ git = \"{ACCEPTED_HOST}\", rev = \"9664606f5a4708606642a6670a57d16bd3d37596\" }}\n"
    );
    check_pins(&mixed_rev, &dev, &good_lock()).expect_err("mixed rev must fail");

    let short = manifest_with(ACCEPTED_HOST, "ba92f94");
    check_pins(&short, &dev, &good_lock()).expect_err("non-40-hex rev must fail");
}

#[test]
fn dev_pin_pointing_elsewhere_fails() {
    let (workspace, _) = accepted_manifests();
    let wrong_host_dev = dev_manifest_with("https://github.com/EpicGames/lore.git", ACCEPTED_REV);
    let error =
        check_pins(&workspace, &wrong_host_dev, &good_lock()).expect_err("dev host must fail");
    assert!(error.contains("lore-credential (dev)"), "{error}");

    let wrong_rev_dev =
        dev_manifest_with(ACCEPTED_HOST, "9664606f5a4708606642a6670a57d16bd3d37596");
    check_pins(&workspace, &wrong_rev_dev, &good_lock()).expect_err("dev rev must fail");

    let missing = String::new();
    check_pins(&workspace, &missing, &good_lock()).expect_err("absent dev pin must fail");
}

#[test]
fn stale_extra_or_missing_lock_sources_fail() {
    let (w, t) = accepted_manifests();

    // A stale source line left behind on the old pin.
    let stale = format!(
        "{}[[package]]\nname = \"lore-transport\"\nsource = \"git+https://github.com/EpicGames/lore.git?rev=9664606f5a4708606642a6670a57d16bd3d37596#9664606f5a4708606642a6670a57d16bd3d37596\"\n",
        good_lock()
    );
    let error = check_pins(&w, &t, &stale).expect_err("stale lock source must fail");
    assert!(error.contains("non-accepted lore source"), "{error}");

    // A package silently removed from the accepted set.
    let source = format!("source = \"git+{ACCEPTED_HOST}?rev={ACCEPTED_REV}#{ACCEPTED_REV}\"");
    let missing: String = EXPECTED_LOCK_PACKAGES
        .iter()
        .filter(|name| **name != "lore-credential")
        .map(|name| format!("[[package]]\nname = \"{name}\"\nversion = \"0.0.0\"\n{source}\n"))
        .collect();
    let error = check_pins(&w, &t, &missing).expect_err("missing package must fail");
    assert!(error.contains("lock packages"), "{error}");

    // An unexpected package resolving from the lore tree.
    let extra = format!(
        "{}[[package]]\nname = \"lore-surprise\"\nversion = \"0.0.0\"\n{source}\n",
        good_lock()
    );
    check_pins(&w, &t, &extra).expect_err("unexpected package must fail");
}
