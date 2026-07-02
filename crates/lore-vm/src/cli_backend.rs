//! Adapter that drives the `lore` CLI as a subprocess.
//!
//! This works the day you install Lore — no knowledge of the `lore-client` API
//! required. Mutating verbs (stage/commit/push/sync/branch) are wired fully.
//! Inspection verbs (status/log/branches) parse CLI JSON output.

use crate::backend::LoreBackend;
use crate::error::{LoreError, Result};
use crate::model::{Branch, RepoStatus, Revision};
use std::path::PathBuf;
use tokio::process::Command;

pub struct CliBackend {
    working_dir: PathBuf,
    program: String,
}

impl CliBackend {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            // Allow override for dev installs / non-PATH binaries.
            program: std::env::var("LORE_BIN").unwrap_or_else(|_| "lore".into()),
        }
    }

    /// Run `lore <args...>` in the working dir, returning stdout on success.
    async fn run(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.program)
            .args(args)
            .current_dir(&self.working_dir)
            .output()
            .await
            .map_err(|e| LoreError::CliUnavailable(format!("{}: {e}", self.program)))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(LoreError::CommandFailed(format!(
                "`{} {}` exited {}: {}",
                self.program,
                args.join(" "),
                output.status.code().unwrap_or(-1),
                stderr.trim()
            )))
        }
    }
}

#[async_trait::async_trait]
impl LoreBackend for CliBackend {
    async fn status(&self) -> Result<RepoStatus> {
        let raw = self.run(&["status", "--json"]).await?;
        serde_json::from_str(&raw).map_err(|e| {
            LoreError::Parse(format!(
                "failed to parse `lore status --json` output: {e}\nRaw: {raw}"
            ))
        })
    }

    async fn log(&self, limit: usize) -> Result<Vec<Revision>> {
        let n = limit.to_string();
        let raw = self.run(&["log", "--limit", &n, "--json"]).await?;
        serde_json::from_str(&raw).map_err(|e| {
            LoreError::Parse(format!(
                "failed to parse `lore log --json` output: {e}\nRaw: {raw}"
            ))
        })
    }

    async fn branches(&self) -> Result<Vec<Branch>> {
        let raw = self.run(&["branch", "list", "--json"]).await?;
        serde_json::from_str(&raw).map_err(|e| {
            LoreError::Parse(format!(
                "failed to parse `lore branch list --json` output: {e}\nRaw: {raw}"
            ))
        })
    }

    async fn stage(&self, paths: &[String]) -> Result<()> {
        let mut args = vec!["stage"];
        args.extend(paths.iter().map(String::as_str));
        self.run(&args).await.map(drop)
    }

    async fn unstage(&self, paths: &[String]) -> Result<()> {
        let mut args = vec!["unstage"];
        args.extend(paths.iter().map(String::as_str));
        self.run(&args).await.map(drop)
    }

    async fn commit(&self, message: &str) -> Result<String> {
        let out = self.run(&["commit", "--message", message]).await?;
        // Surface the new revision hash if the CLI prints it.
        Ok(extract_revision(&out).unwrap_or_default())
    }

    async fn create_branch(&self, name: &str) -> Result<()> {
        self.run(&["branch", "create", name]).await.map(drop)
    }

    async fn switch_branch(&self, name: &str) -> Result<()> {
        self.run(&["branch", "switch", name]).await.map(drop)
    }

    async fn merge_branch(&self, name: &str) -> Result<()> {
        self.run(&["branch", "merge", name]).await.map(drop)
    }

    async fn push(&self) -> Result<()> {
        self.run(&["push"]).await.map(drop)
    }

    async fn sync(&self) -> Result<()> {
        self.run(&["sync"]).await.map(drop)
    }

    async fn create_repository(&self, path: PathBuf, name: &str) -> Result<String> {
        let path_str = path.to_string_lossy().into_owned();
        let out = self
            .run(&["repository", "create", name, "--path", &path_str])
            .await?;
        Ok(extract_repo_id(&out).unwrap_or_default())
    }

    async fn clone(&self, url: &str, dest: PathBuf) -> Result<()> {
        let dest_str = dest.to_string_lossy().into_owned();
        self.run(&["clone", url, &dest_str]).await.map(drop)
    }
}

fn extract_revision(out: &str) -> Option<String> {
    out.lines().find_map(|l| {
        l.trim()
            .strip_prefix("revision ")
            .map(|s| s.trim().to_string())
    })
}

fn extract_repo_id(out: &str) -> Option<String> {
    out.lines().find_map(|l| {
        l.trim()
            .rsplit_once("ID")
            .map(|(_, id)| id.trim().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ChangeKind;

    #[test]
    fn test_parse_status_json() {
        let json = r#"{
            "repo_id": "test-repo",
            "branch": "main",
            "revision": "abc123",
            "changes": [
                { "path": "file1.txt", "kind": "modified", "staged": true },
                { "path": "file2.txt", "kind": "added", "staged": false }
            ],
            "ahead": 1,
            "behind": 2
        }"#;
        let status: RepoStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.repo_id, "test-repo");
        assert_eq!(status.branch, "main");
        assert_eq!(status.revision, "abc123");
        assert_eq!(status.changes.len(), 2);
        assert_eq!(status.changes[0].path, "file1.txt");
        assert_eq!(status.changes[0].kind, ChangeKind::Modified);
        assert!(status.changes[0].staged);
        assert_eq!(status.ahead, 1);
        assert_eq!(status.behind, 2);
    }

    #[test]
    fn test_parse_log_json() {
        let json = r#"[
            {
                "hash": "abc123",
                "message": "Initial commit",
                "author": "Alice",
                "timestamp": "2026-07-01T23:00:00Z",
                "parent": null
            },
            {
                "hash": "def456",
                "message": "Update README",
                "author": "Bob",
                "timestamp": "2026-07-01T23:05:00Z",
                "parent": "abc123"
            }
        ]"#;
        let log: Vec<Revision> = serde_json::from_str(json).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].hash, "abc123");
        assert_eq!(log[0].message, "Initial commit");
        assert_eq!(log[1].hash, "def456");
        assert_eq!(log[1].parent, Some("abc123".to_string()));
    }

    #[test]
    fn test_parse_branches_json() {
        let json = r#"[
            {
                "name": "main",
                "id": "b1",
                "latest_revision": "abc123",
                "is_current": true
            },
            {
                "name": "feature-x",
                "id": "b2",
                "latest_revision": "def456",
                "is_current": false
            }
        ]"#;
        let branches: Vec<Branch> = serde_json::from_str(json).unwrap();
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].name, "main");
        assert!(branches[0].is_current);
        assert_eq!(branches[1].name, "feature-x");
        assert!(!branches[1].is_current);
    }
}
