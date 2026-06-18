//! Adapter that drives the `lore` CLI as a subprocess.
//!
//! This works the day you install Lore — no knowledge of the `lore-client` API
//! required. Mutating verbs (stage/commit/push/sync/branch) are wired fully.
//! Inspection verbs (status/log/branches) parse CLI text output; Lore is pre-1.0
//! and its human output format isn't contractual, so those parsers are marked
//! `TODO(parse)` — switch them to `--format json` / `--porcelain` as soon as the
//! CLI offers a stable machine format, or move to the in-process ClientBackend.

use crate::backend::LoreBackend;
use crate::error::{LoreError, Result};
use crate::model::{Branch, ChangeKind, FileChange, RepoStatus, Revision};
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
        let raw = self.run(&["status"]).await?;
        // TODO(parse): replace with `lore status --format json` once stable.
        // Best-effort line parser against the human output for now.
        parse_status(&raw)
    }

    async fn log(&self, limit: usize) -> Result<Vec<Revision>> {
        let n = limit.to_string();
        let raw = self.run(&["log", "--limit", &n]).await?;
        // TODO(parse): prefer a JSON log format when available.
        Ok(parse_log(&raw))
    }

    async fn branches(&self) -> Result<Vec<Branch>> {
        let raw = self.run(&["branch", "list"]).await?;
        // TODO(parse): prefer a JSON branch list when available.
        Ok(parse_branches(&raw))
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
        // Surface the new revision hash if the CLI prints it; fall back to status.
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

// --- text parsers (best-effort; swap for JSON when the CLI exposes it) ---

fn parse_status(raw: &str) -> Result<RepoStatus> {
    let mut status = RepoStatus::default();
    let mut in_staged = false;
    for line in raw.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("branch:") {
            status.branch = rest.trim().to_string();
        } else if let Some(rest) = t.strip_prefix("revision:") {
            status.revision = rest.trim().to_string();
        } else if t.eq_ignore_ascii_case("staged changes:") {
            in_staged = true;
        } else if t.eq_ignore_ascii_case("unstaged changes:") || t.eq_ignore_ascii_case("changes:")
        {
            in_staged = false;
        } else if let Some(change) = parse_change_line(t, in_staged) {
            status.changes.push(change);
        }
    }
    if status.branch.is_empty() && status.changes.is_empty() {
        // Couldn't make sense of it — surface raw so the UI shows *something*.
        return Err(LoreError::Parse(format!(
            "unrecognized `lore status` output:\n{raw}"
        )));
    }
    Ok(status)
}

fn parse_change_line(line: &str, staged: bool) -> Option<FileChange> {
    let (marker, path) = line.split_once(char::is_whitespace)?;
    let kind = match marker {
        "A" | "added" | "+" => ChangeKind::Added,
        "M" | "modified" | "~" => ChangeKind::Modified,
        "D" | "deleted" | "-" => ChangeKind::Deleted,
        "R" | "renamed" => ChangeKind::Renamed,
        "?" | "untracked" => ChangeKind::Untracked,
        _ => return None,
    };
    Some(FileChange {
        path: path.trim().to_string(),
        kind,
        staged,
    })
}

fn parse_log(raw: &str) -> Vec<Revision> {
    // Expects blocks separated by blank lines; tolerant of missing fields.
    raw.split("\n\n")
        .filter_map(|block| {
            let mut rev = Revision {
                hash: String::new(),
                message: String::new(),
                author: String::new(),
                timestamp: String::new(),
                parent: None,
            };
            for line in block.lines() {
                let t = line.trim();
                if let Some(v) = t
                    .strip_prefix("revision ")
                    .or_else(|| t.strip_prefix("commit "))
                {
                    rev.hash = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("Author:") {
                    rev.author = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("Date:") {
                    rev.timestamp = v.trim().to_string();
                } else if let Some(v) = t.strip_prefix("Parent:") {
                    rev.parent = Some(v.trim().to_string());
                } else if !t.is_empty() && rev.message.is_empty() && !rev.hash.is_empty() {
                    rev.message = t.to_string();
                }
            }
            (!rev.hash.is_empty()).then_some(rev)
        })
        .collect()
}

fn parse_branches(raw: &str) -> Vec<Branch> {
    raw.lines()
        .filter_map(|line| {
            let t = line.trim();
            if t.is_empty() {
                return None;
            }
            let is_current = t.starts_with('*');
            let name = t.trim_start_matches('*').trim().to_string();
            (!name.is_empty()).then_some(Branch {
                name,
                id: String::new(),
                latest_revision: String::new(),
                is_current,
            })
        })
        .collect()
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
