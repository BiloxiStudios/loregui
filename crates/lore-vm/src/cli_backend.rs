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
use crate::model::{
    Branch, ChangeKind, ConfigValue, FileChange, InstanceInfo, InstanceList,
    InstancePruneResult, ImmutableMatch, ImmutableQueryResult, MetadataEntry,
    RepoCreateResult, RepoDump, RepoInfo, RepoListing, RepoStatus, Revision,
    VerifyFragmentResult, VerifyStateResult,
};
use std::collections::HashMap;
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
            program: std::env::var("LORE_BIN").unwrap_or_else(|_| "lore".into()),
        }
    }

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
        parse_status(&raw)
    }

    async fn log(&self, limit: usize) -> Result<Vec<Revision>> {
        let n = limit.to_string();
        let raw = self.run(&["log", "--limit", &n]).await?;
        Ok(parse_log(&raw))
    }

    async fn branches(&self) -> Result<Vec<Branch>> {
        let raw = self.run(&["branch", "list"]).await?;
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

    // ===== Repository domain (21 ops) =====

    async fn repo_info(&self) -> Result<RepoInfo> {
        let raw = self.run(&["repository", "info"]).await?;
        parse_repo_info(&raw)
    }

    async fn repo_dump(&self, format: Option<&str>) -> Result<RepoDump> {
        let fmt = format.unwrap_or("json");
        let raw = self.run(&["repository", "dump", "--format", fmt]).await?;
        Ok(RepoDump {
            format: fmt.to_string(),
            data: raw,
        })
    }

    async fn repo_create_with_metadata(
        &self,
        path: PathBuf,
        name: &str,
        metadata: HashMap<String, String>,
    ) -> Result<RepoCreateResult> {
        let path_str = path.to_string_lossy().into_owned();
        let mut args: Vec<&str> = vec!["repository", "create", name, "--path", &path_str];
        let meta_strings: Vec<String> = metadata
            .iter()
            .flat_map(|(k, v)| vec![format!("--metadata"), format!("{k}={v}")])
            .collect();
        let meta_refs: Vec<&str> = meta_strings.iter().map(|s| s.as_str()).collect();
        args.extend(meta_refs);
        let out = self.run(&args).await?;
        let repo_id = extract_repo_id(&out).unwrap_or_default();
        Ok(RepoCreateResult {
            repo_id,
            path: path_str,
        })
    }

    async fn repo_delete(&self, path: PathBuf) -> Result<()> {
        let path_str = path.to_string_lossy().into_owned();
        self.run(&["repository", "delete", "--path", &path_str])
            .await
            .map(drop)
    }

    async fn repo_release(&self) -> Result<()> {
        self.run(&["repository", "release"]).await.map(drop)
    }

    async fn repo_flush(&self) -> Result<()> {
        self.run(&["repository", "flush"]).await.map(drop)
    }

    async fn repo_gc(&self, aggressive: bool) -> Result<u64> {
        let args = if aggressive {
            vec!["repository", "gc", "--aggressive"]
        } else {
            vec!["repository", "gc"]
        };
        let out = self.run(&args).await?;
        Ok(parse_bytes_freed(&out))
    }

    async fn repo_list(&self) -> Result<Vec<RepoListing>> {
        let raw = self.run(&["repository", "list"]).await?;
        Ok(parse_repo_list(&raw))
    }

    async fn repo_verify_state(&self) -> Result<VerifyStateResult> {
        let raw = self.run(&["repository", "verify-state"]).await?;
        Ok(parse_verify_state(&raw))
    }

    async fn repo_verify_fragment(
        &self,
        fragment_hash: &str,
    ) -> Result<VerifyFragmentResult> {
        let raw = self
            .run(&["repository", "verify-fragment", fragment_hash])
            .await?;
        Ok(parse_verify_fragment(&raw, fragment_hash))
    }

    async fn repo_store_immutable_query(
        &self,
        query: &str,
    ) -> Result<ImmutableQueryResult> {
        let raw = self
            .run(&["repository", "store-immutable-query", query])
            .await?;
        Ok(parse_immutable_query(&raw))
    }

    async fn repo_metadata_get(&self, key: &str) -> Result<Option<MetadataEntry>> {
        let raw = self.run(&["repository", "metadata-get", key]).await?;
        Ok(parse_metadata_entry(&raw))
    }

    async fn repo_metadata_set(&self, key: &str, value: &str) -> Result<()> {
        self.run(&["repository", "metadata-set", key, value])
            .await
            .map(drop)
    }

    async fn repo_metadata_clear(&self) -> Result<()> {
        self.run(&["repository", "metadata-clear"]).await.map(drop)
    }

    async fn repo_instance_list(&self) -> Result<InstanceList> {
        let raw = self.run(&["repository", "instance-list"]).await?;
        Ok(parse_instance_list(&raw))
    }

    async fn repo_instance_prune(&self) -> Result<InstancePruneResult> {
        let raw = self.run(&["repository", "instance-prune"]).await?;
        Ok(parse_instance_prune(&raw))
    }

    async fn repo_update_path(&self, new_path: PathBuf) -> Result<()> {
        let path_str = new_path.to_string_lossy().into_owned();
        self.run(&["repository", "update-path", &path_str])
            .await
            .map(drop)
    }

    async fn repo_config_get(&self, key: &str) -> Result<ConfigValue> {
        let raw = self.run(&["repository", "config-get", key]).await?;
        Ok(parse_config_value(&raw, key))
    }
}

// --- text parsers ---

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
        } else if t.eq_ignore_ascii_case("unstaged changes:") || t.eq_ignore_ascii_case("changes:") {
            in_staged = false;
        } else if let Some(change) = parse_change_line(t, in_staged) {
            status.changes.push(change);
        }
    }
    if status.branch.is_empty() && status.changes.is_empty() {
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
                if let Some(v) = t.strip_prefix("revision ").or_else(|| t.strip_prefix("commit ")) {
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
    out.lines()
        .find_map(|l| l.trim().strip_prefix("revision ").map(|s| s.trim().to_string()))
}

fn extract_repo_id(out: &str) -> Option<String> {
    out.lines()
        .find_map(|l| l.trim().rsplit_once("ID").map(|(_, id)| id.trim().to_string()))
}

// --- Repository domain parsers ---

fn parse_repo_info(raw: &str) -> Result<RepoInfo> {
    let mut info = RepoInfo::default();
    for line in raw.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("ID:") {
            info.repo_id = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("Name:") {
            info.name = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("Path:") {
            info.path = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("Created:") {
            info.created_at = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("Branch:") {
            info.current_branch = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("Revision:") {
            info.current_revision = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("Shared Store:") {
            let val = v.trim();
            if !val.is_empty() && val != "none" {
                info.shared_store_url = Some(val.to_string());
            }
        }
    }
    Ok(info)
}

fn parse_repo_list(raw: &str) -> Vec<RepoListing> {
    raw.lines()
        .filter_map(|line| {
            let t = line.trim();
            if t.is_empty() {
                return None;
            }
            let is_current = t.starts_with('*');
            let name = t.trim_start_matches('*').trim();
            let parts: Vec<&str> = name.splitn(3, "  ").collect();
            if parts.len() >= 2 {
                Some(RepoListing {
                    repo_id: parts.get(2).map(|s| s.strip_prefix("ID: ").unwrap_or(s).trim()).unwrap_or("").to_string(),
                    name: parts[0].trim().to_string(),
                    path: parts[1].trim().to_string(),
                    is_current,
                })
            } else {
                Some(RepoListing {
                    repo_id: String::new(),
                    name: t.to_string(),
                    path: String::new(),
                    is_current,
                })
            }
        })
        .collect()
}

fn parse_verify_state(raw: &str) -> VerifyStateResult {
    let mut issues = Vec::new();
    let is_valid = !raw.contains("INVALID") && !raw.contains("ERROR") && !raw.contains("corrupt");
    if !is_valid {
        for line in raw.lines() {
            let t = line.trim();
            if t.contains("ERROR") || t.contains("INVALID") || t.contains("corrupt") {
                issues.push(t.to_string());
            }
        }
    }
    VerifyStateResult { is_valid, issues }
}

fn parse_verify_fragment(raw: &str, fragment_hash: &str) -> VerifyFragmentResult {
    let is_valid = raw.contains("valid") || raw.contains("OK") || raw.contains("verified");
    let expected_size = parse_number_field(raw, "expected")
        .or_else(|| parse_number_field(raw, "size"))
        .unwrap_or(0);
    let actual_size = if is_valid {
        Some(expected_size)
    } else {
        parse_number_field(raw, "actual")
    };
    VerifyFragmentResult {
        fragment_hash: fragment_hash.to_string(),
        is_valid,
        expected_size,
        actual_size,
    }
}

fn parse_immutable_query(raw: &str) -> ImmutableQueryResult {
    let mut matches = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = t.splitn(3, ' ').collect();
        if parts.len() >= 2 {
            let size: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            matches.push(ImmutableMatch {
                hash: parts[0].to_string(),
                size,
                path: parts.get(2).unwrap_or(&"").to_string(),
            });
        }
    }
    ImmutableQueryResult { matches }
}

fn parse_metadata_entry(raw: &str) -> Option<MetadataEntry> {
    for line in raw.lines() {
        let t = line.trim();
        if let Some((key, value)) = t.split_once('=') {
            return Some(MetadataEntry {
                key: key.trim().to_string(),
                value: value.trim().to_string(),
            });
        }
    }
    None
}

fn parse_instance_list(raw: &str) -> InstanceList {
    let mut instances = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let is_active = t.starts_with('*');
        let name = t.trim_start_matches('*').trim();
        let parts: Vec<&str> = name.splitn(3, "  ").collect();
        if parts.len() >= 2 {
            instances.push(InstanceInfo {
                instance_id: String::new(),
                name: parts[0].trim().to_string(),
                path: parts[1].trim().to_string(),
                is_active,
            });
        } else if !name.is_empty() {
            instances.push(InstanceInfo {
                instance_id: String::new(),
                name: name.to_string(),
                path: String::new(),
                is_active,
            });
        }
    }
    InstanceList { instances }
}

fn parse_instance_prune(raw: &str) -> InstancePruneResult {
    let pruned_count = parse_number_field(raw, "pruned")
        .or_else(|| parse_number_field(raw, "removed"))
        .unwrap_or(0) as u32;
    let freed_bytes = parse_number_field(raw, "freed")
        .or_else(|| parse_number_field(raw, "bytes"))
        .unwrap_or(0);
    InstancePruneResult {
        pruned_count,
        freed_bytes,
    }
}

fn parse_config_value(raw: &str, key: &str) -> ConfigValue {
    let mut value = String::new();
    let mut source = String::new();
    for line in raw.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("Value:") {
            value = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("Source:") {
            source = v.trim().to_string();
        } else if !t.contains(':') && !t.is_empty() {
            if value.is_empty() {
                value = t.to_string();
            }
        }
    }
    ConfigValue {
        key: key.to_string(),
        value,
        source: if source.is_empty() {
            "default".to_string()
        } else {
            source
        },
    }
}

fn parse_bytes_freed(raw: &str) -> u64 {
    parse_number_field(raw, "freed")
        .or_else(|| parse_number_field(raw, "bytes"))
        .unwrap_or(0)
}

fn parse_number_field(raw: &str, keyword: &str) -> Option<u64> {
    for line in raw.lines() {
        let lower = line.to_lowercase();
        if lower.contains(keyword) {
            for word in line.split_whitespace() {
                let cleaned: String = word.chars().filter(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = cleaned.parse::<u64>() {
                    if n > 0 {
                        return Some(n);
                    }
                }
            }
        }
    }
    None
}
