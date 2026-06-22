//! `relay::relay_open` — prepare a bore-relay tunnel configuration.
//!
//! Pure computation op: given the hosted server's local port, relay endpoint,
//! bore HMAC secret, and repository name, computes the auth token the `bore`
//! client will present to the relay server and the public `lore://` URL to
//! advertise to remote clients.
//!
//! This op does NOT spawn the bore client or manage the tunnel lifecycle —
//! that is the responsibility of the src-tauri integration manager. The op
//! produces the typed parameters the manager passes to `bore client`.
//!
//! Flow (SBAI-4072):
//!   1. Premium user hosts a server (SBAI-4065 → server_host.rs).
//!   2. Manager calls `relay_open` to compute tunnel config.
//!   3. Manager spawns `bore client` with the returned auth token + local port.
//!   4. Manager advertises the returned relay URL to remote clients.
//!   5. On server stop, manager kills the bore client (teardown).

use crate::error::{LoreError, Result};
use serde::{Deserialize, Serialize};

/// Default relay port used by the StudioBrain bore infrastructure.
/// Matches the relay server config: `--min-port 10000 --max-port 10100`.
pub const DEFAULT_RELAY_PORT: u16 = 7835;

/// Arguments for [`relay_open`].
///
/// All fields are plain `String` so they serialise cleanly across the Tauri
/// boundary. The `relay_port` defaults to [`DEFAULT_RELAY_PORT`] when absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayOpenArgs {
    /// The local port the hosted loreserver is bound to (e.g. 41337).
    pub local_port: u16,
    /// Relay hostname (e.g. "relay.studiobrain.ai").
    pub relay_host: String,
    /// Relay TCP port (default: [`DEFAULT_RELAY_PORT`]).
    #[serde(default = "default_relay_port")]
    pub relay_port: u16,
    /// Bore HMAC secret shared with the relay server (from Vaultwarden).
    /// Never logged or serialised in results — consumed only for token generation.
    pub bore_secret: String,
    /// Repository name to embed in the advertised relay URL.
    /// When empty, the URL is the bare `lore://host:port`.
    #[serde(default)]
    pub repository_name: String,
}

fn default_relay_port() -> u16 {
    DEFAULT_RELAY_PORT
}

impl RelayOpenArgs {
    /// Validate that all required fields are populated.
    fn validate(&self) -> Result<()> {
        if self.local_port == 0 {
            return Err(LoreError::CommandFailed(
                "local_port must be non-zero".into(),
            ));
        }
        if self.relay_host.is_empty() {
            return Err(LoreError::CommandFailed(
                "relay_host is required".into(),
            ));
        }
        if self.bore_secret.is_empty() {
            return Err(LoreError::Auth(
                "bore_secret is required — cannot open a relay tunnel without auth".into(),
            ));
        }
        Ok(())
    }
}

/// Result of a successful `relay_open` computation.
///
/// Contains all parameters the integration manager needs to spawn the `bore`
/// client and advertise the relay URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayOpenResult {
    /// HMAC auth token to present to the relay server.
    /// Passed as the `--token` flag to `bore client`.
    pub auth_token: String,
    /// The public `lore://` URL to advertise to remote clients.
    /// Format: `lore://relay_host:relay_port[/<repository_name>]`.
    pub relay_url: String,
    /// The local port the bore client forwards to (echo of the input).
    pub local_port: u16,
    /// The relay host (echo of the input).
    pub relay_host: String,
    /// The relay port (echo of the input).
    pub relay_port: u16,
}

/// Generate an HMAC-SHA256 auth token from the bore secret.
///
/// The bore relay authenticates clients by verifying an HMAC token. The token
/// is `HMAC-SHA256(secret, timestamp)` where the timestamp is a Unix epoch
/// string — this ensures each token is unique (replay protection) while the
/// relay can recompute it from the shared secret and the current time window.
///
/// Returns the hex-encoded HMAC digest.
fn generate_auth_token(secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(timestamp.to_string().as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Compute the public relay URL from the relay host/port and repository name.
///
/// Format: `lore://host:port` (bare) or `lore://host:port/<repo>` (with repo).
/// Uses `lore://` (no trailing `s`) so clients skip server-cert validation
/// against the self-signed test cert (same reasoning as server_host.rs).
fn relay_url(relay_host: &str, relay_port: u16, repository_name: &str) -> String {
    let name = repository_name.trim();
    if name.is_empty() {
        format!("lore://{relay_host}:{relay_port}")
    } else {
        format!("lore://{relay_host}:{relay_port}/{name}")
    }
}

/// Prepare a bore-relay tunnel configuration.
///
/// Validates inputs, generates the HMAC auth token, and computes the public
/// relay URL. Pure computation — no side effects, no I/O, no subprocess spawning.
///
/// # Errors
/// Returns [`LoreError::CommandFailed`] if `local_port` is zero, `relay_host`
/// is empty, or `bore_secret` is empty (auth cannot proceed without a secret).
pub async fn relay_open(args: RelayOpenArgs) -> Result<RelayOpenResult> {
    args.validate()?;

    let auth_token = generate_auth_token(&args.bore_secret);
    let relay_url = relay_url(&args.relay_host, args.relay_port, &args.repository_name);

    Ok(RelayOpenResult {
        auth_token,
        relay_url,
        local_port: args.local_port,
        relay_host: args.relay_host,
        relay_port: args.relay_port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_deserialise_defaults() {
        let args: RelayOpenArgs = serde_json::from_str(
            r#"{"local_port":41337,"relay_host":"relay.example.com","bore_secret":"secret"}"#,
        )
        .expect("deserialise");
        assert_eq!(args.local_port, 41337);
        assert_eq!(args.relay_host, "relay.example.com");
        assert_eq!(args.relay_port, DEFAULT_RELAY_PORT);
        assert_eq!(args.bore_secret, "secret");
        assert!(args.repository_name.is_empty());
    }

    #[test]
    fn args_deserialise_with_all_fields() {
        let args: RelayOpenArgs = serde_json::from_str(
            r#"{"local_port":50000,"relay_host":"relay.studiobrain.ai","relay_port":7835,"bore_secret":"hkmac-key-123","repository_name":"myrepo"}"#,
        )
        .expect("deserialise");
        assert_eq!(args.local_port, 50000);
        assert_eq!(args.relay_port, 7835);
        assert_eq!(args.repository_name, "myrepo");
    }

    #[test]
    fn validate_rejects_zero_port() {
        let args = RelayOpenArgs {
            local_port: 0,
            relay_host: "relay.example.com".into(),
            relay_port: DEFAULT_RELAY_PORT,
            bore_secret: "secret".into(),
            repository_name: String::new(),
        };
        assert!(args.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_relay_host() {
        let args = RelayOpenArgs {
            local_port: 41337,
            relay_host: String::new(),
            relay_port: DEFAULT_RELAY_PORT,
            bore_secret: "secret".into(),
            repository_name: String::new(),
        };
        assert!(args.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_bore_secret() {
        let args = RelayOpenArgs {
            local_port: 41337,
            relay_host: "relay.example.com".into(),
            relay_port: DEFAULT_RELAY_PORT,
            bore_secret: String::new(),
            repository_name: String::new(),
        };
        assert!(args.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_args() {
        let args = RelayOpenArgs {
            local_port: 41337,
            relay_host: "relay.example.com".into(),
            relay_port: DEFAULT_RELAY_PORT,
            bore_secret: "secret".into(),
            repository_name: "myrepo".into(),
        };
        assert!(args.validate().is_ok());
    }

    #[test]
    fn relay_url_bare() {
        assert_eq!(
            relay_url("relay.studiobrain.ai", 7835, ""),
            "lore://relay.studiobrain.ai:7835"
        );
        assert_eq!(
            relay_url("relay.studiobrain.ai", 7835, "  "),
            "lore://relay.studiobrain.ai:7835"
        );
    }

    #[test]
    fn relay_url_with_repo() {
        assert_eq!(
            relay_url("relay.studiobrain.ai", 7835, "myrepo"),
            "lore://relay.studiobrain.ai:7835/myrepo"
        );
    }

    #[test]
    fn auth_token_is_hex_64_chars() {
        // HMAC-SHA256 produces 32 bytes = 64 hex chars.
        let token = generate_auth_token("test-secret");
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn auth_token_changes_each_call() {
        // Timestamp-based tokens should differ between calls (unless called in the
        // same second, but a sleep ensures they differ).
        let token1 = generate_auth_token("test-secret");
        std::thread::sleep(std::time::Duration::from_secs(1));
        let token2 = generate_auth_token("test-secret");
        assert_ne!(token1, token2);
    }

    #[test]
    fn auth_token_differs_per_secret() {
        let token_a = generate_auth_token("secret-a");
        let token_b = generate_auth_token("secret-b");
        assert_ne!(token_a, token_b);
    }

    #[test]
    fn result_serialises() {
        let result = RelayOpenResult {
            auth_token: "abc123".into(),
            relay_url: "lore://relay.studiobrain.ai:7835/myrepo".into(),
            local_port: 41337,
            relay_host: "relay.studiobrain.ai".into(),
            relay_port: 7835,
        };
        let json = serde_json::to_string(&result).expect("serialise");
        assert!(json.contains("abc123"));
        assert!(json.contains("myrepo"));
        assert!(json.contains("41337"));
    }

    #[test]
    fn result_deserialises() {
        let json = r#"{
            "auth_token":"hex-token",
            "relay_url":"lore://relay.studiobrain.ai:7835/repo",
            "local_port":50000,
            "relay_host":"relay.studiobrain.ai",
            "relay_port":7835
        }"#;
        let result: RelayOpenResult = serde_json::from_str(json).expect("deserialise");
        assert_eq!(result.local_port, 50000);
        assert_eq!(result.relay_port, 7835);
        assert_eq!(result.relay_host, "relay.studiobrain.ai");
    }

    #[tokio::test]
    async fn relay_open_returns_token_and_url() {
        let args = RelayOpenArgs {
            local_port: 41337,
            relay_host: "relay.studiobrain.ai".into(),
            relay_port: 7835,
            bore_secret: "test-secret".into(),
            repository_name: "myrepo".into(),
        };
        let result = relay_open(args).await.expect("relay_open should succeed");
        assert_eq!(result.local_port, 41337);
        assert_eq!(result.relay_port, 7835);
        assert_eq!(result.relay_host, "relay.studiobrain.ai");
        assert_eq!(
            result.relay_url,
            "lore://relay.studiobrain.ai:7835/myrepo"
        );
        assert_eq!(result.auth_token.len(), 64);
        assert!(result.auth_token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn relay_open_rejects_zero_port() {
        let args = RelayOpenArgs {
            local_port: 0,
            relay_host: "relay.example.com".into(),
            relay_port: DEFAULT_RELAY_PORT,
            bore_secret: "secret".into(),
            repository_name: String::new(),
        };
        assert!(relay_open(args).await.is_err());
    }

    #[tokio::test]
    async fn relay_open_rejects_empty_secret() {
        let args = RelayOpenArgs {
            local_port: 41337,
            relay_host: "relay.example.com".into(),
            relay_port: DEFAULT_RELAY_PORT,
            bore_secret: String::new(),
            repository_name: String::new(),
        };
        let err = relay_open(args).await.unwrap_err();
        // Should be an Auth error (secret is auth material).
        match &err {
            LoreError::Auth(_) => {}
            other => panic!("expected LoreError::Auth, got: {other:?}"),
        }
    }
}
