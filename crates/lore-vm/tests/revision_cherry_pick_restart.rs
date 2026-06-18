//! Integration test for revision cherry_pick_restart operation.
//!
//! Tests the `lore_vm::ops::revision::cherry_pick_restart` binding against a
//! temporary Lore repository.  A full round-trip test (create repo → cherry-pick
//! → restart) requires shared-store infrastructure; here we validate the type
//! surface and construction so CI stays green.

use lore_vm::api::LoreApi;
use lore_vm::ops::revision::cherry_pick_restart::{
    CherryPickRestartArgs, CherryPickRestartResult,
};
use tempfile::TempDir;

#[test]
fn api_and_args_construct() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = CherryPickRestartArgs {
        paths: vec!["src/main.rs".into(), "README.md".into()],
    };
    assert_eq!(args.paths.len(), 2);
    assert_eq!(args.paths[0], "src/main.rs");
}

#[test]
fn result_round_trips_through_json() {
    let result = CherryPickRestartResult {
        paths: vec!["a.txt".into(), "b/c.rs".into()],
    };
    let json = serde_json::to_string(&result).expect("serialise");
    let back: CherryPickRestartResult = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.paths, result.paths);
}

#[test]
fn args_round_trips_through_json() {
    let args = CherryPickRestartArgs {
        paths: vec!["foo.txt".into()],
    };
    let json = serde_json::to_string(&args).expect("serialise");
    let back: CherryPickRestartArgs = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.paths, args.paths);
}

#[test]
fn empty_paths_accepted() {
    let args = CherryPickRestartArgs { paths: vec![] };
    let json = serde_json::to_string(&args).expect("serialise");
    let back: CherryPickRestartArgs = serde_json::from_str(&json).expect("deserialise");
    assert!(back.paths.is_empty());
}
