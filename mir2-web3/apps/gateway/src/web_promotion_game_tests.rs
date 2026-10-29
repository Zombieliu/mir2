//! Synthetic capability/identity tests, not gameplay, wallet approval or payment evidence.
use super::*;
use crate::cache::{GatewaySessionCache, InMemoryGatewaySessionCache};

fn profile() -> Profile { Profile { secret: "synthetic-promotion-game-secret-at-least-32".into() } }
fn claims() -> Claims {
    Claims { auth: TOKEN_AUTH.into(), scope: TOKEN_SCOPE.into(), realm_id: REALM_ID.into(),
        account_id: "obl_fixture".into(), parent_session_id: "parent_fixture".into(),
        parent_expires_at_ms: 1_000_000, credential_id: "wallet_fixture".into(),
        credential_subject: format!("0x{}", "1".repeat(64)), jti: "random_fixture".into(),
        issued_at_ms: 100_000, expires_at_ms: 160_000 }
}

#[test]
fn promotion_game_profile_defaults_closed_and_invalid_flags_still_guard_legacy_paths() {
    for value in [None, Some(""), Some("0"), Some("false"), Some("off")] { assert!(!requested_value(value)); }
    for value in [Some("1"), Some("true"), Some("bad-configuration")] { assert!(requested_value(value)); }
}

#[test]
fn promotion_game_capability_is_pinned_to_scope_realm_account_parent_wallet_and_deadline() {
    let profile = profile(); let original = claims();
    let token = sign(&profile, &original).unwrap();
    assert!(token.starts_with("pg1."));
    assert_eq!(decode(&profile, "obl_fixture", &token, 120_000).unwrap().credential_id, "wallet_fixture");
    assert!(decode(&profile, "obl_other", &token, 120_000).is_err());
    assert!(decode(&profile, "obl_fixture", &token, 160_000).is_err());
    assert!(decode(&Profile { secret: "other-independent-secret-at-least-32".into() }, "obl_fixture", &token, 120_000).is_err());
    for changed in ["realm", "scope", "auth", "expiry", "future", "parent", "wallet"] {
        let mut bad = claims();
        match changed { "realm" => bad.realm_id = "old-game".into(), "scope" => bad.scope = "identity".into(),
            "auth" => bad.auth = "mir2-identity-session-v1".into(), "expiry" => bad.expires_at_ms += 1,
            "future" => bad.issued_at_ms = 130_000, "parent" => bad.parent_expires_at_ms = 150_000,
            "wallet" => bad.credential_subject = "0xshort".into(), _ => unreachable!() }
        assert!(decode(&profile, "obl_fixture", &sign(&profile, &bad).unwrap(), 120_000).is_err(), "{changed}");
    }
    let mut tampered = token.into_bytes(); let end = tampered.len()-2; tampered[end] = if tampered[end] == b'a' { b'b' } else { b'a' };
    assert!(decode(&profile, "obl_fixture", std::str::from_utf8(&tampered).unwrap(), 120_000).is_err());
}

#[test]
fn promotion_game_capability_never_verifies_as_a_commercial_identity_bearer() {
    let service = IdentityService::local_for_tests();
    let token = sign(&profile(), &claims()).unwrap();
    assert!(service.verify_session_token(&token).is_err());
    assert!(crate::auth::verify_gateway_identity_token("obl_fixture", &token).is_err());
    assert!(crate::auth::verify_sui_login_proof(&token).is_err());
}

#[test]
fn promotion_game_capability_id_uses_atomic_existing_replay_cache() {
    let cache = InMemoryGatewaySessionCache::default();
    let claims = claims(); let key = format!("promotion-game:{}:{}", claims.realm_id, claims.jti);
    assert_eq!(cache.consume_auth_token(&key, 60), Ok(true));
    assert_eq!(cache.consume_auth_token(&key, 60), Ok(false));
    assert_eq!(cache.consume_auth_token("promotion-game:other:other-jti", 60), Ok(true));
}

#[test]
fn promotion_game_binding_uses_the_current_wallet_session_and_checks_both_revocation_stores() {
    let identity = IdentityService::local_for_tests(); let cache = InMemoryGatewaySessionCache::default();
    let address = format!("0x{}", "1".repeat(64));
    let grant = identity.issue_session("obl_fixture", "sui_wallet", &address, "127.0.0.1", "synthetic-test").unwrap();
    let parent = identity.verify_session_token(&grant.token).unwrap();
    let wallet = current_wallet(&identity, &cache, &parent, gateway_now_ms()).unwrap();
    let mut bound = claims(); bound.parent_session_id = parent.session_id.clone(); bound.parent_expires_at_ms = parent.expires_at_ms;
    bound.credential_id = wallet.credential_id.clone(); bound.credential_subject = address;
    let binding = GameBinding { claims: bound.clone() };
    assert!(validate_wallet_binding(&identity, &cache, &binding, gateway_now_ms()).is_ok());
    bound.credential_id = "another-linked-wallet".into();
    assert!(validate_wallet_binding(&identity, &cache, &GameBinding { claims: bound }, gateway_now_ms()).is_err());
    cache.revoke_identity_session(&parent.session_id, 60).unwrap();
    assert!(validate_wallet_binding(&identity, &cache, &binding, gateway_now_ms()).is_err());
    let database_only_cache = InMemoryGatewaySessionCache::default();
    identity.revoke_session(&parent, &parent.session_id, "synthetic-revocation").unwrap();
    assert!(validate_wallet_binding(&identity, &database_only_cache, &binding, gateway_now_ms()).is_err());
}

#[test]
fn promotion_game_password_session_cannot_mint_a_wallet_capability_and_memory_is_not_a_hosted_backend() {
    let identity = IdentityService::local_for_tests(); let cache = InMemoryGatewaySessionCache::default();
    let grant = identity.issue_session("fixture", "password", "fixture", "127.0.0.1", "synthetic-test").unwrap();
    let parent = identity.verify_session_token(&grant.token).unwrap();
    assert!(current_wallet(&identity, &cache, &parent, gateway_now_ms()).is_err());
    assert!(require_durable_backend(&GatewayConfig::default(), &identity, &cache).is_err());
}

#[test]
fn promotion_game_public_profile_denies_legacy_and_privileged_commands() {
    for value in [json!({"type":"login","accountId":"victim","password":"guess"}),
        json!({"type":"newAccount","accountId":"victim","password":"guess"}),
        json!({"type":"changePassword","accountId":"victim","currentPassword":"guess","newPassword":"other"}),
        json!({"type":"moveTo","x":100,"y":100}),
        json!({"type":"stage5Command","action":"qa.giveItem","args":[]}),
        json!({"type":"qaControl","token":"guess","action":{"type":"tick"}})] {
        match serde_json::from_value::<BrowserCommand>(value) {
            Ok(command) => assert!(!browser_command_allowed(&command)),
            Err(_) => {} // A command absent from the public schema is already rejected.
        }
    }
    for value in [json!({"type":"newCharacter","name":"Fixture","class":"Warrior","gender":"Male"}),
        json!({"type":"startGame","characterIndex":1}), json!({"type":"walk","direction":"Up"}),
        json!({"type":"attack","objectId":42}), json!({"type":"clientVersion"})] {
        assert!(browser_command_allowed(&serde_json::from_value::<BrowserCommand>(value).unwrap()));
    }
    assert!(serde_json::from_value::<BrowserCommand>(json!({"type":"passkeyLogin","accountId":"victim"})).is_err());
    assert!(serde_json::from_value::<GameSessionRequest>(json!({"accountId":"victim"})).is_err());
    assert!(serde_json::from_value::<GameSessionRequest>(json!({"credentialId":"another-wallet"})).is_err());
    assert!(serde_json::from_value::<GameSessionRequest>(json!({"actor":"admin"})).is_err());
}
