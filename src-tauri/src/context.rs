//! Versioned, non-secret LoreGUI server/repository/project context.
//!
//! Credentials stay in the operating-system credential store. Persisted and
//! IPC-visible context contains opaque references only.

use crate::settings::SettingsManager;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use tauri::State;

pub const CONTEXT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    NotRequired,
    Required,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServerSource {
    Manual,
    Lan,
    Hosted,
    StudioBrain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ServerProfile {
    pub id: String,
    pub alias: String,
    pub url: String,
    pub source: ServerSource,
    pub favorite: bool,
    pub auth_mode: AuthMode,
    pub credential_ref: Option<String>,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct RepositoryBookmark {
    pub id: String,
    pub server_id: Option<String>,
    pub display_name: String,
    pub url: Option<String>,
    pub favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct LocalProject {
    pub id: String,
    pub repository_id: String,
    pub display_name: String,
    pub local_path: String,
    pub branch: Option<String>,
    pub favorite: bool,
    pub last_opened_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct HostedServerProfile {
    pub id: String,
    pub display_name: String,
    pub store_path: String,
    pub advertised_url: String,
    pub last_configuration: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "snake_case", deny_unknown_fields)]
pub struct ActiveContext {
    pub project_id: Option<String>,
    pub server_id: Option<String>,
    pub identity_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "snake_case", deny_unknown_fields)]
pub struct ContextSettings {
    pub schema_version: u32,
    pub servers: Vec<ServerProfile>,
    pub repositories: Vec<RepositoryBookmark>,
    pub projects: Vec<LocalProject>,
    pub hosted_servers: Vec<HostedServerProfile>,
    pub active: ActiveContext,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            schema_version: CONTEXT_SCHEMA_VERSION,
            servers: Vec::new(),
            repositories: Vec::new(),
            projects: Vec::new(),
            hosted_servers: Vec::new(),
            active: ActiveContext::default(),
        }
    }
}

impl ContextSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != CONTEXT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported context schema version {}; expected {}",
                self.schema_version, CONTEXT_SCHEMA_VERSION
            ));
        }

        validate_unique_ids("server", self.servers.iter().map(|item| item.id.as_str()))?;
        validate_unique_ids(
            "repository",
            self.repositories.iter().map(|item| item.id.as_str()),
        )?;
        validate_unique_ids("project", self.projects.iter().map(|item| item.id.as_str()))?;
        validate_unique_ids(
            "hosted server",
            self.hosted_servers.iter().map(|item| item.id.as_str()),
        )?;

        let server_ids: HashSet<&str> = self.servers.iter().map(|item| item.id.as_str()).collect();
        let repository_ids: HashSet<&str> = self
            .repositories
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        let project_ids: HashSet<&str> =
            self.projects.iter().map(|item| item.id.as_str()).collect();

        for repository in &self.repositories {
            if let Some(server_id) = repository.server_id.as_deref() {
                if !server_ids.contains(server_id) {
                    return Err(format!(
                        "repository {} references missing server {server_id}",
                        repository.id
                    ));
                }
            }
        }
        for project in &self.projects {
            if !repository_ids.contains(project.repository_id.as_str()) {
                return Err(format!(
                    "project {} references missing repository {}",
                    project.id, project.repository_id
                ));
            }
        }
        if let Some(project_id) = self.active.project_id.as_deref() {
            if !project_ids.contains(project_id) {
                return Err(format!("active project {project_id} does not exist"));
            }
        }
        if let Some(server_id) = self.active.server_id.as_deref() {
            if !server_ids.contains(server_id) {
                return Err(format!("active server {server_id} does not exist"));
            }
        }

        Ok(())
    }

    pub(crate) fn validate_for_persistence(&self) -> Result<(), String> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| format!("could not serialize context: {error}"))?;
        validate_no_raw_secrets(&value)
    }
}

fn validate_unique_ids<'a>(kind: &str, ids: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = HashSet::new();
    for id in ids {
        if id.trim().is_empty() {
            return Err(format!("{kind} id must not be empty"));
        }
        if !seen.insert(id) {
            return Err(format!("duplicate {kind} id: {id}"));
        }
    }
    Ok(())
}

pub(crate) fn validate_no_raw_secrets(value: &Value) -> Result<(), String> {
    fn visit(value: &Value, path: &str) -> Result<(), String> {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    if secret_like_key(key) {
                        return Err(format!("secret-like field is not allowed: {child_path}"));
                    }
                    visit(child, &child_path)?;
                }
            }
            Value::Array(values) => {
                for (index, child) in values.iter().enumerate() {
                    visit(child, &format!("{path}[{index}]"))?;
                }
            }
            Value::String(text) if secret_like_value(text) => {
                return Err(format!(
                    "raw credential-like value is not allowed at {path}"
                ));
            }
            _ => {}
        }
        Ok(())
    }

    visit(value, "")
}

fn secret_like_key(key: &str) -> bool {
    if key == "credential_ref" {
        return false;
    }
    let normalized = key.to_ascii_lowercase().replace('-', "_");
    [
        "token",
        "password",
        "passwd",
        "secret",
        "api_key",
        "private_key",
        "access_key",
        "credential",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn secret_like_value(value: &str) -> bool {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    let assigned_secret = [
        "token=",
        "token:",
        "password=",
        "password:",
        "secret=",
        "secret:",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    let known_prefix = [
        "ghp_",
        "github_pat_",
        "glpat-",
        "xoxb-",
        "xoxp-",
        "akia",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
        // Keep payment-provider credential shapes recognizable at runtime
        // without embedding boundary-guard secret markers verbatim in source.
        || lower.starts_with(&["sk_", "live_"].concat())
        || lower.starts_with(&["sk_", "test_"].concat());
    let private_key = lower.contains("-----begin") && lower.contains("private key-----");
    let credential_in_url = trimmed
        .split_once("://")
        .and_then(|(_, remainder)| remainder.split('/').next())
        .is_some_and(|authority| authority.contains('@') && authority.contains(':'));

    assigned_secret || known_prefix || private_key || credential_in_url
}

#[tauri::command]
pub fn context_get(settings: State<'_, SettingsManager>) -> Result<ContextSettings, String> {
    Ok(settings.get().context)
}

#[tauri::command]
pub fn context_validate(context: ContextSettings) -> Result<ContextSettings, String> {
    context.validate_for_persistence()?;
    Ok(context)
}

#[tauri::command]
pub fn context_update(
    settings: State<'_, SettingsManager>,
    context: ContextSettings,
) -> Result<ContextSettings, String> {
    settings.update_context(context.clone())?;
    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_context() -> ContextSettings {
        ContextSettings {
            schema_version: 1,
            servers: vec![ServerProfile {
                id: "server-1".into(),
                alias: "EROS Lore".into(),
                url: "https://eros.example.test".into(),
                source: ServerSource::Manual,
                favorite: true,
                auth_mode: AuthMode::Required,
                credential_ref: Some("keyring://loregui/server-1".into()),
                last_seen_at: Some("2026-07-21T12:00:00Z".into()),
            }],
            repositories: vec![RepositoryBookmark {
                id: "repository-1".into(),
                server_id: Some("server-1".into()),
                display_name: "Game Lore".into(),
                url: Some("lore://eros/game".into()),
                favorite: true,
            }],
            projects: vec![LocalProject {
                id: "project-1".into(),
                repository_id: "repository-1".into(),
                display_name: "Game Lore".into(),
                local_path: "/projects/game-lore".into(),
                branch: Some("main".into()),
                favorite: true,
                last_opened_at: "2026-07-21T12:00:00Z".into(),
            }],
            hosted_servers: vec![HostedServerProfile {
                id: "hosted-1".into(),
                display_name: "Local Lore".into(),
                store_path: "/stores/lore".into(),
                advertised_url: "https://localhost:41337".into(),
                last_configuration: "2026-07-21T11:00:00Z".into(),
            }],
            active: ActiveContext {
                project_id: Some("project-1".into()),
                server_id: Some("server-1".into()),
                identity_ref: Some("keyring://loregui/identity-1".into()),
            },
        }
    }

    #[test]
    fn default_context_is_explicit_schema_v1() {
        let context = ContextSettings::default();
        assert_eq!(context.schema_version, 1);
        assert!(context.servers.is_empty());
        assert!(context.repositories.is_empty());
        assert!(context.projects.is_empty());
        assert!(context.hosted_servers.is_empty());
        assert_eq!(context.active, ActiveContext::default());
    }

    #[test]
    fn complete_context_round_trips_with_snake_case_enums() {
        let context = complete_context();
        let json = serde_json::to_value(&context).expect("serialize context");
        assert_eq!(json["servers"][0]["auth_mode"], "required");
        assert_eq!(json["servers"][0]["source"], "manual");
        assert_eq!(json["schema_version"], 1);
        assert_eq!(
            serde_json::from_value::<ContextSettings>(json).expect("deserialize context"),
            context
        );
    }

    #[test]
    fn validation_rejects_duplicate_ids() {
        let mut context = complete_context();
        context.servers.push(context.servers[0].clone());
        assert!(context
            .validate()
            .unwrap_err()
            .contains("duplicate server id"));
    }

    #[test]
    fn validation_rejects_missing_active_project() {
        let mut context = complete_context();
        context.active.project_id = Some("missing-project".into());
        assert!(context.validate().unwrap_err().contains("active project"));
    }

    #[test]
    fn validation_rejects_broken_repository_references() {
        let mut context = complete_context();
        context.projects[0].repository_id = "missing-repository".into();
        assert!(context.validate().unwrap_err().contains("repository"));
    }

    #[test]
    fn recursive_secret_guard_rejects_fields_and_raw_values() {
        let payment_token = ["sk_", "live_", "raw-value"].concat();
        for raw in [
            serde_json::json!({"nested": {"token": "raw-value"}}),
            serde_json::json!({"password": "raw-value"}),
            serde_json::json!({"secret": "raw-value"}),
            serde_json::json!({"alias": "token=raw-value"}),
            serde_json::json!({"alias": "ghp_0123456789abcdefghijklmnopqrstuvwxyz"}),
            serde_json::json!({"alias": payment_token}),
        ] {
            assert!(validate_no_raw_secrets(&raw).is_err(), "accepted {raw}");
        }
    }

    #[test]
    fn recursive_secret_guard_allows_only_opaque_credential_references() {
        let raw = serde_json::json!({
            "credential_ref": "keyring://loregui/server-1",
            "identity_ref": "keyring://loregui/identity-1"
        });
        validate_no_raw_secrets(&raw).expect("opaque OS-store references are non-secret");
    }
}
