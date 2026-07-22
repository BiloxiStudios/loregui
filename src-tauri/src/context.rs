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

#[derive(Debug, Default)]
pub struct ContextSelectionCoordinator {
    latest_generation: u64,
}

impl ContextSelectionCoordinator {
    pub fn register(&mut self, generation: u64) -> Result<(), String> {
        if generation == 0 || generation <= self.latest_generation {
            return Err("context selection request is stale".into());
        }
        self.latest_generation = generation;
        Ok(())
    }

    pub fn ensure_current(&self, generation: u64) -> Result<(), String> {
        if generation != self.latest_generation {
            return Err("context selection request is stale".into());
        }
        Ok(())
    }
}

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
    pub last_configured_at: String,
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
        for server in &self.servers {
            validate_url_without_userinfo("server url", &server.url)?;
            if let Some(reference) = server.credential_ref.as_deref() {
                validate_credential_reference("credential_ref", reference)?;
            }
        }
        for repository in &self.repositories {
            if let Some(url) = repository.url.as_deref() {
                validate_url_without_userinfo("repository url", url)?;
            }
        }
        if let Some(reference) = self.active.identity_ref.as_deref() {
            validate_credential_reference("identity_ref", reference)?;
        }
        for server in &self.hosted_servers {
            validate_url_without_userinfo("hosted server advertised_url", &server.advertised_url)?;
            if !is_utc_timestamp(&server.last_configured_at) {
                return Err("last_configured_at must be a UTC timestamp".into());
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

fn is_utc_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
        || bytes.iter().enumerate().any(|(index, byte)| {
            !matches!(index, 4 | 7 | 10 | 13 | 16 | 19) && !byte.is_ascii_digit()
        })
    {
        return false;
    }

    let number = |start: usize, end: usize| {
        value[start..end]
            .parse::<u32>()
            .expect("timestamp digits were validated")
    };
    let year = number(0, 4);
    let month = number(5, 7);
    let day = number(8, 10);
    let hour = number(11, 13);
    let minute = number(14, 16);
    let second = number(17, 19);
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return false,
    };

    year > 0 && (1..=days_in_month).contains(&day) && hour < 24 && minute < 60 && second < 60
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
                    if matches!(key.as_str(), "credential_ref" | "identity_ref") {
                        match child {
                            Value::Null => {}
                            Value::String(reference) => {
                                validate_credential_reference(key, reference)?;
                            }
                            _ => return Err(invalid_credential_reference(key)),
                        }
                    }
                    visit(child, &child_path)?;
                }
            }
            Value::Array(values) => {
                for (index, child) in values.iter().enumerate() {
                    visit(child, &format!("{path}[{index}]"))?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    visit(value, "")
}

fn secret_like_key(key: &str) -> bool {
    if matches!(key, "credential_ref" | "identity_ref") {
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

const MAX_CREDENTIAL_REFERENCE_LEN: usize = 256;
const CREDENTIAL_REFERENCE_PREFIXES: [&str; 3] = [
    "windows-credential-manager://",
    "macos-keychain://",
    "linux-secret-service://",
];

fn validate_credential_reference(field: &str, reference: &str) -> Result<(), String> {
    if reference.is_empty()
        || reference.len() > MAX_CREDENTIAL_REFERENCE_LEN
        || reference.chars().any(char::is_control)
    {
        return Err(invalid_credential_reference(field));
    }
    let Some(opaque_id) = CREDENTIAL_REFERENCE_PREFIXES
        .iter()
        .find_map(|prefix| reference.strip_prefix(prefix))
    else {
        return Err(invalid_credential_reference(field));
    };
    if opaque_id.is_empty()
        || opaque_id.starts_with('/')
        || opaque_id.ends_with('/')
        || opaque_id.contains("//")
        || !opaque_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
        || opaque_id
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(invalid_credential_reference(field));
    }

    Ok(())
}

fn invalid_credential_reference(field: &str) -> String {
    format!("{field} must be an approved opaque OS credential-store reference")
}

fn validate_url_without_userinfo(field: &str, value: &str) -> Result<(), String> {
    if let Ok(url) = tauri::Url::parse(value) {
        if !url.username().is_empty() || url.password().is_some() {
            return Err(format!("{field} must not contain embedded credentials"));
        }
    }
    Ok(())
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
                credential_ref: Some("windows-credential-manager://loregui/server/server-1".into()),
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
                last_configured_at: "2026-07-21T11:00:00Z".into(),
            }],
            active: ActiveContext {
                project_id: Some("project-1".into()),
                server_id: Some("server-1".into()),
                identity_ref: Some(
                    "windows-credential-manager://loregui/identity/identity-1".into(),
                ),
            },
        }
    }

    #[test]
    fn coordinator_rejects_zero_duplicate_and_superseded_generations() {
        let mut coordinator = ContextSelectionCoordinator::default();
        assert!(coordinator.register(0).is_err());
        coordinator.register(1).expect("generation one");
        assert!(coordinator.register(1).is_err());
        coordinator.register(2).expect("generation two");
        assert!(coordinator.ensure_current(1).is_err());
        coordinator.ensure_current(2).expect("current generation");
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
    fn recursive_secret_guard_rejects_forbidden_secret_bearing_fields() {
        for raw in [
            serde_json::json!({"nested": {"token": "raw-value"}}),
            serde_json::json!({"password": "raw-value"}),
            serde_json::json!({"secret": "raw-value"}),
        ] {
            assert!(validate_no_raw_secrets(&raw).is_err(), "accepted {raw}");
        }
    }

    #[test]
    fn typed_product_strings_that_resemble_secret_markers_are_allowed() {
        let mut context = complete_context();
        context.servers[0].alias = "AKIA Animation".into();
        context.projects[0].branch = Some("ghp_feature".into());
        context.hosted_servers[0].display_name = "sk_live sessions".into();
        context.projects[0].local_path = "/projects/github_pat_reference".into();
        context.repositories[0].url = Some("lore://eros/glpat-assets/xoxb-scenes".into());

        context
            .validate_for_persistence()
            .expect("typed product strings are not credential material");
    }

    #[test]
    fn recursive_secret_guard_allows_marker_like_noncredential_values() {
        let raw = serde_json::json!({
            "alias": "token=design",
            "display_name": "Password: The Design Document",
            "local_path": r"C:\Projects\token=design",
            "store_path": "/srv/lore/secret=design",
            "url": "lore://server/repositories/token=design"
        });
        validate_no_raw_secrets(&raw).expect("ordinary product strings are not credentials");
    }

    #[test]
    fn approved_os_credential_references_validate_for_both_reference_fields() {
        for reference in [
            "windows-credential-manager://loregui/server/server-1",
            "macos-keychain://loregui/server/server-1",
            "linux-secret-service://loregui/server/server-1",
            "windows-credential-manager://loregui/server/ghp_project",
        ] {
            let mut context = complete_context();
            context.servers[0].credential_ref = Some(reference.into());
            context.active.identity_ref = Some(reference.into());
            context
                .validate_for_persistence()
                .expect("approved OS credential-store reference");
        }
    }

    #[test]
    fn hosted_server_serde_accepts_only_timestamp_metadata_not_raw_configuration() {
        let mut raw = serde_json::to_value(complete_context()).expect("serialize fixture");
        let hosted = raw["hosted_servers"][0]
            .as_object_mut()
            .expect("hosted server object");
        hosted.remove("last_configuration");
        hosted.insert(
            "last_configured_at".into(),
            Value::String("2026-07-21T11:00:00Z".into()),
        );

        let context: ContextSettings =
            serde_json::from_value(raw.clone()).expect("timestamp metadata deserializes");
        context
            .validate_for_persistence()
            .expect("timestamp metadata validates");
        let serialized = serde_json::to_value(context).expect("serialize context");
        assert_eq!(
            serialized["hosted_servers"][0]["last_configured_at"],
            "2026-07-21T11:00:00Z"
        );
        assert!(serialized["hosted_servers"][0]
            .get("last_configuration")
            .is_none());

        let mut raw_configuration = raw;
        let hosted = raw_configuration["hosted_servers"][0]
            .as_object_mut()
            .expect("hosted server object");
        hosted.remove("last_configured_at");
        hosted.insert(
            "last_configuration".into(),
            Value::String("[aws]\nsecret_access_key=raw-value".into()),
        );
        assert!(serde_json::from_value::<ContextSettings>(raw_configuration).is_err());
    }

    #[test]
    fn hosted_server_rejects_non_timestamp_configuration_metadata() {
        let mut raw = serde_json::to_value(complete_context()).expect("serialize fixture");
        let hosted = raw["hosted_servers"][0]
            .as_object_mut()
            .expect("hosted server object");
        hosted.remove("last_configuration");
        hosted.insert(
            "last_configured_at".into(),
            Value::String("AWS_SECRET_ACCESS_KEY=raw-value".into()),
        );

        let context: ContextSettings =
            serde_json::from_value(raw).expect("typed metadata deserializes");
        let error = context.validate_for_persistence().unwrap_err();
        assert_eq!(error, "last_configured_at must be a UTC timestamp");
    }

    #[test]
    fn credential_and_identity_references_reject_non_opaque_or_unsafe_values() {
        let overlong = format!("windows-credential-manager://loregui/{}", "a".repeat(300));
        let invalid = [
            "".to_owned(),
            "raw-password-value".to_owned(),
            "keyring://loregui/server-1".to_owned(),
            "https://example.test/credential".to_owned(),
            overlong,
            "macos-keychain://loregui/bad\nreference".to_owned(),
        ];

        for reference in invalid {
            for field in ["credential_ref", "identity_ref"] {
                let mut context = complete_context();
                if field == "credential_ref" {
                    context.servers[0].credential_ref = Some(reference.clone());
                } else {
                    context.active.identity_ref = Some(reference.clone());
                }
                let error = context.validate_for_persistence().unwrap_err();
                assert_eq!(
                    error,
                    format!("{field} must be an approved opaque OS credential-store reference")
                );
            }
        }
    }

    #[test]
    fn recursive_secret_guard_allows_approved_opaque_references() {
        let raw = serde_json::json!({
            "credential_ref": "macos-keychain://loregui/server/ghp_project",
            "identity_ref": "linux-secret-service://loregui/identity/identity-1"
        });
        validate_no_raw_secrets(&raw).expect("opaque OS-store references are non-secret");
    }

    #[test]
    fn typed_url_fields_reject_embedded_userinfo() {
        let mut context = complete_context();
        context.servers[0].url = "https://user:raw-password@example.test/lore".into();
        assert!(context.validate_for_persistence().is_err());

        let mut context = complete_context();
        context.repositories[0].url = Some("lore://user:raw-password@eros/game".into());
        assert!(context.validate_for_persistence().is_err());

        let mut context = complete_context();
        context.hosted_servers[0].advertised_url =
            "https://user:raw-password@localhost:41337".into();
        assert!(context.validate_for_persistence().is_err());
    }
}
