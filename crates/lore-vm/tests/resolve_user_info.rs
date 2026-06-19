//! Integration test for auth resolve_user_info operation.

use lore_vm::api::LoreApi;
use lore_vm::ops::auth::resolve_user_info::{ResolveUserInfoArgs, ResolveUserInfoResult, ResolvedUserInfo};
use tempfile::TempDir;

#[test]
fn test_resolve_user_info_args_construction() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = temp_dir.path().join("test_repo");

    let api = LoreApi::new(repo_path.clone());
    assert_eq!(api.global().repository_path, repo_path);

    let args = ResolveUserInfoArgs {
        user_ids: vec!["user1".to_string(), "user2".to_string()],
    };
    assert_eq!(args.user_ids.len(), 2);
    assert_eq!(args.user_ids[0], "user1");
}

#[test]
fn test_resolve_user_info_args_serialization() {
    let args = ResolveUserInfoArgs {
        user_ids: vec!["user1".to_string()],
    };
    let json = serde_json::to_string(&args).unwrap();
    let deserialized: ResolveUserInfoArgs = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.user_ids, args.user_ids);
}

#[test]
fn test_resolve_user_info_result_serialization() {
    let result = ResolveUserInfoResult {
        users: vec![
            ResolvedUserInfo {
                user_id: "user1".into(),
                display_name: "User One".into(),
            },
        ],
    };
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: ResolveUserInfoResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.users.len(), 1);
    assert_eq!(deserialized.users[0].user_id, "user1");
    assert_eq!(deserialized.users[0].display_name, "User One");
}
