//! SBAI-5910: fail-closed contract for the pinned upstream `lore` source.
//!
//! `lore` and its vendored `quinn-proto` patch MUST reference the same git
//! host AND the same exact rev, in the workspace manifest and in every
//! `Cargo.lock` source line. A mixed-host state (one dep still on
//! `EpicGames/lore` while the other moved to the `BiloxiStudios/lore`
//! maintenance fork) or a mixed-rev state would silently mix trees: the
//! `quinn-proto` patch is only ABI-compatible with the `lore-transport` from
//! the same commit, and a security backport applied to one but not the other
//! would ship a half-patched client.
//!
//! Structural, byte-exact checks — no TOML parser dependency, so this adds no
//! manifest/lock churn. Runs under `cargo test -p loregui`.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("contract test must read {}: {e}", path.display()))
}

/// `(host_url, rev)` for a `name = { git = "...", rev = "..." }` line whose
/// key is exactly `name` at the start of a line.
fn pinned_dep(manifest: &str, name: &str) -> (String, String) {
    let line = manifest
        .lines()
        .find(|l| l.starts_with(&format!("{name} = ")))
        .unwrap_or_else(|| panic!("workspace manifest must pin `{name}` on its own line"));
    let grab = |key: &str| -> String {
        let needle = format!("{key} = \"");
        let start = line
            .find(&needle)
            .unwrap_or_else(|| panic!("`{name}` pin must carry {key}: {line}"))
            + needle.len();
        let rest = &line[start..];
        let end = rest
            .find('"')
            .unwrap_or_else(|| panic!("unterminated {key} in `{name}` pin: {line}"));
        rest[..end].to_string()
    };
    (grab("git"), grab("rev"))
}

#[test]
fn lore_and_quinn_proto_pin_the_same_host_and_rev() {
    let manifest = read(&repo_root().join("Cargo.toml"));
    let (lore_host, lore_rev) = pinned_dep(&manifest, "lore");
    let (quinn_host, quinn_rev) = pinned_dep(&manifest, "quinn-proto");

    assert_eq!(
        lore_host, quinn_host,
        "mixed-host pin: lore={lore_host} quinn-proto={quinn_host} — the vendored \
         quinn-proto patch must come from the same tree as lore-transport"
    );
    assert_eq!(
        lore_rev, quinn_rev,
        "mixed-rev pin: lore={lore_rev} quinn-proto={quinn_rev}"
    );
    assert_eq!(
        lore_rev.len(),
        40,
        "pins must be full 40-hex revs, never branches or tags: {lore_rev}"
    );
    assert!(
        lore_rev.chars().all(|c| c.is_ascii_hexdigit()),
        "pin rev must be hex: {lore_rev}"
    );
}

#[test]
fn lockfile_sources_match_the_manifest_pin_exactly() {
    let root = repo_root();
    let manifest = read(&root.join("Cargo.toml"));
    let lock = read(&root.join("Cargo.lock"));
    let (host, rev) = pinned_dep(&manifest, "lore");

    // Every lore-tree source line in the lock must be this exact host+rev.
    let expected = format!("source = \"git+{host}?rev={rev}#{rev}\"");
    let lore_sources: Vec<&str> = lock
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("source = \"git+") && l.contains("/lore.git?rev="))
        .collect();
    assert!(
        !lore_sources.is_empty(),
        "lock must carry lore-tree git sources"
    );
    for line in &lore_sources {
        assert_eq!(
            *line, expected,
            "lock source does not match the manifest pin (mixed host or rev)"
        );
    }
}
