//! Integration tests for dependency operations.
//!
//! Tests add, remove, and list operations in the dependency domain.

use std::fs;
use tempfile::TempDir;

use lore_vm::api::LoreApi;
use lore_vm::ops::dependency::dependency_add::{
    dependency_add, DependencyAddArgs, DependencyAddEntry, DependencyAddSource,
};
use lore_vm::ops::dependency::dependency_list::{dependency_list, DependencyListArgs};
use lore_vm::ops::dependency::dependency_remove::{
    dependency_remove, DependencyRemoveArgs, DependencyRemoveEntry, DependencyRemoveSource,
};
use lore_vm::ops::file::stage::{stage, CaseChange, FileStageArgs};
use lore_vm::ops::repository::create::{create, CreateArgs};

#[tokio::test]
async fn test_dependency_lifecycle() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let repo_path = fs::canonicalize(temp_dir.path())
        .expect("canonicalize temp dir")
        .join("repo");
    fs::create_dir_all(&repo_path).unwrap();

    let api = LoreApi::new(repo_path.clone());

    // 1. Create a repository with a unique name
    let repo_name = format!("dep-test-{}", std::process::id());
    let _ = create(
        &api,
        CreateArgs {
            repository_url: format!("lore://localhost/{}", repo_name),
            description: "Dependency test repo".into(),
            id: String::new(),
            use_shared_store: false,
            shared_store_path: String::new(),
        },
    )
    .await
    .expect("repository creation failed");

    // 2. Create some files and stage them
    let a_path = repo_path.join("a.txt");
    let b_path = repo_path.join("b.txt");
    fs::write(&a_path, "source").unwrap();
    fs::write(&b_path, "dependency").unwrap();

    stage(
        &api,
        FileStageArgs {
            paths: vec!["a.txt".into(), "b.txt".into()],
            case_change: CaseChange::Error,
            scan: true,
        },
    )
    .await
    .expect("staging failed");

    // 3. Add a dependency: a.txt -> b.txt with "test-tag"
    let add_result = dependency_add(
        &api,
        DependencyAddArgs {
            sources: vec![DependencyAddSource {
                path: "a.txt".into(),
                dependencies: vec![DependencyAddEntry {
                    dependency: "b.txt".into(),
                    tags: vec!["test-tag".into()],
                }],
            }],
            force: false,
        },
    )
    .await
    .expect("dependency_add failed");
    assert_eq!(add_result.added_count, 1);

    // 4. List dependencies for a.txt
    let list_result = dependency_list(
        &api,
        DependencyListArgs {
            paths: vec!["a.txt".into()],
            revision: String::new(),
            recursive: false,
            reverse: false,
            tags: vec![],
            depth_limit: 0,
        },
    )
    .await
    .expect("dependency_list failed");

    assert_eq!(list_result.file_count, 1);
    assert_eq!(list_result.files.len(), 1);
    // The engine returns absolute paths
    assert!(list_result.files[0].path.ends_with("a.txt"));
    assert_eq!(list_result.files[0].entries.len(), 1);
    assert!(list_result.files[0].entries[0].path.ends_with("b.txt"));
    assert_eq!(list_result.files[0].entries[0].tags, vec!["test-tag"]);

    // 5. List dependents for b.txt (reverse)
    let reverse_result = dependency_list(
        &api,
        DependencyListArgs {
            paths: vec!["b.txt".into()],
            revision: String::new(),
            recursive: false,
            reverse: true,
            tags: vec![],
            depth_limit: 0,
        },
    )
    .await
    .expect("reverse dependency_list failed");

    assert_eq!(reverse_result.files.len(), 1);
    assert!(reverse_result.files[0].path.ends_with("b.txt"));
    assert_eq!(reverse_result.files[0].entries.len(), 1);
    assert!(reverse_result.files[0].entries[0].path.ends_with("a.txt"));

    // 6. Remove the dependency
    let remove_result = dependency_remove(
        &api,
        DependencyRemoveArgs {
            sources: vec![DependencyRemoveSource {
                path: "a.txt".into(),
                dependencies: vec![DependencyRemoveEntry {
                    dependency: "b.txt".into(),
                    tags: vec![], // remove entire edge
                }],
            }],
        },
    )
    .await
    .expect("dependency_remove failed");
    assert_eq!(remove_result.removed_count, 1);

    // 7. Verify removal
    let final_list = dependency_list(
        &api,
        DependencyListArgs {
            paths: vec!["a.txt".into()],
            revision: String::new(),
            recursive: false,
            reverse: false,
            tags: vec![],
            depth_limit: 0,
        },
    )
    .await
    .expect("final dependency_list failed");

    assert_eq!(final_list.files[0].entries.len(), 0);
}
