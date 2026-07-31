//! SBAI-5910: consumer-side DENY guard for the cached-token credential
//! boundary.
//!
//! The fail-closed behavior loregui depends on lives in the pinned
//! `BiloxiStudios/lore` maintenance fork (SBAI-5909): a cached
//! `IdentityToken` carrying NO `acceptable_root_domains` must never be
//! selected for any remote — legacy unscoped tokens have to be reissued
//! rather than handed to whatever host asks. Upstream carries its own tests,
//! but a later repin (or a repin back to a tree without the backport) would
//! take those tests with it and leave loregui silently exposed.
//!
//! This test binds the guarantee HERE, in the consumer, against the exact
//! pinned rev — so the protection cannot be deleted by changing a pin.
//! Upstream follow-up is SBAI-5917; the runtime delivery fix is SBAI-5916.

use lore_credential::token_store::{tokens_only_for_recipient_domain, IdentityToken};

/// Build an `IdentityToken` through its serde surface (fields are private).
fn token(acceptable_root_domains: &[&str]) -> IdentityToken {
    let domains = serde_json::to_string(acceptable_root_domains).expect("serialize domains");
    serde_json::from_str(&format!(
        r#"{{"user_id":"alice","token":"encrypted","acceptable_root_domains":{domains}}}"#
    ))
    .expect("IdentityToken deserializes from its stored form")
}

fn selected_for(t: &IdentityToken, domain: &str) -> bool {
    std::iter::once(t)
        .find(tokens_only_for_recipient_domain(domain.to_string()))
        .is_some()
}

#[test]
fn legacy_unscoped_cached_token_is_denied_for_every_domain() {
    let legacy = token(&[]);
    for domain in [
        "epicgames.net",
        "lore.epicgames.net",
        "evilepicgames.net",
        "attacker.example",
    ] {
        assert!(
            !selected_for(&legacy, domain),
            "a cached token with no acceptable_root_domains must never be selected (domain {domain})"
        );
    }
}

#[test]
fn scoped_cached_token_requires_a_label_boundary() {
    let scoped = token(&["epicgames.net"]);
    assert!(
        selected_for(&scoped, "epicgames.net"),
        "apex domain must still be accepted"
    );
    assert!(
        selected_for(&scoped, "lore.epicgames.net"),
        "true subdomain must still be accepted"
    );
    assert!(
        !selected_for(&scoped, "evilepicgames.net"),
        "mid-label look-alike must be denied (exact-domain label boundary)"
    );
    assert!(
        !selected_for(&scoped, "epicgames.net.attacker.example"),
        "suffix-extension look-alike must be denied"
    );
}
