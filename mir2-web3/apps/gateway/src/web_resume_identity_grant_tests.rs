//! Resume bearer re-delivery uses current authority for the original session.
//! All token comparisons avoid assertion output containing bearer contents.
use super::tests::{issue_test_identity, store_native_resume_fixture};
use super::*;
use crate::cache::{GatewaySessionCache, InMemoryGatewaySessionCache};
use crate::identity::{IdentityService, VerifiedIdentitySession};
use std::time::Duration;

/// The HTTP upgrade completes before the ordinary unauthenticated connect
/// snapshot has necessarily reached the client. Drain and prove that baseline
/// before sending ResumeSession so it cannot be mistaken for owner bootstrap.
pub(super) fn assert_unselected_native_resume_connect(events: &[Value]) {
    assert!(
        !events.iter().any(|event| matches!(
            event["type"].as_str(),
            Some("identitySession" | "sessionResumed" | "resumeCredential")
        )),
        "an unauthenticated physical connection must not contain resume authority"
    );
    let snapshots: Vec<_> = events
        .iter()
        .filter(|event| event["type"] == "worldSnapshot")
        .collect();
    assert_eq!(snapshots.len(), 1, "one ordinary connect snapshot required");
    let payload = &snapshots[0]["payload"];
    assert!(
        payload.is_object()
            && payload.get("mapFileName").is_some_and(Value::is_null)
            && payload.get("mapTitle").is_some_and(Value::is_null),
        "connect snapshot must precede character selection"
    );
}

/// Called by the existing physical WebSocket resume test. Only the assertion
/// hook lives there; this file owns the additional wire/security expectations.
pub(super) fn assert_resumed_grant_matches_original(login: &[Value], resumed: &[Value]) {
    let originals: Vec<_> = login
        .iter()
        .filter(|event| event["type"] == "identitySession")
        .collect();
    let deliveries: Vec<_> = resumed
        .iter()
        .enumerate()
        .filter(|(_, event)| event["type"] == "identitySession")
        .collect();
    assert_eq!(
        originals.len(),
        1,
        "ordinary Login must issue exactly one identity grant"
    );
    assert_eq!(
        deliveries.len(),
        1,
        "accepted native resume must re-deliver exactly one identity grant"
    );
    let original = originals[0];
    let (grant_at, delivery) = deliveries[0];
    assert!(original["token"]
        .as_str()
        .is_some_and(|token| !token.is_empty()));
    assert!(
        delivery["token"] == original["token"],
        "resume must preserve the original signed bearer"
    );
    for field in ["accountId", "sessionId", "issuedAtMs", "expiresAtMs"] {
        assert!(
            delivery["session"][field] == original["session"][field],
            "resume must preserve identity session fields"
        );
    }
    assert!(delivery["session"]["revokedAtMs"].is_null());
    assert!(delivery["session"]["current"] == true);
    let resumed_at = resumed
        .iter()
        .position(|event| event["type"] == "sessionResumed")
        .expect("resume acknowledgement required");
    let snapshot_at = resumed
        .iter()
        .position(|event| event["type"] == "worldSnapshot")
        .expect("resume canonical snapshot required");
    assert!(
        grant_at < resumed_at && resumed_at < snapshot_at,
        "physical client must receive the trusted grant before owner bootstrap"
    );
    assert!(
        resumed[snapshot_at]["payload"]["mapFileName"]
            .as_str()
            .is_some_and(|name| !name.is_empty()),
        "post-resume snapshot must belong to the restored selected character"
    );
}

#[test]
fn resume_grant_reuses_original_signed_session_without_writes_or_ttl_extension() {
    let identity = IdentityService::local_for_tests();
    let original = identity
        .issue_session(
            "resume_grant",
            "password",
            "resume_grant",
            "127.0.0.1",
            "fixture",
        )
        .expect("synthetic session issue");
    let verified = identity
        .verify_session_token(&original.token)
        .expect("synthetic token verify");
    let before = identity
        .operator_account_security("resume_grant")
        .expect("synthetic authority view");
    let grant = identity
        .reissue_verified_session_grant(&verified)
        .expect("same active session reissue");
    assert!(
        grant.token == original.token,
        "original payload and signature must be reused"
    );
    assert_eq!(grant.session.session_id, original.session.session_id);
    assert_eq!(grant.session.account_id, original.session.account_id);
    assert_eq!(grant.session.issued_at_ms, original.session.issued_at_ms);
    assert_eq!(grant.session.expires_at_ms, original.session.expires_at_ms);
    assert!(grant.session.current);
    assert_eq!(
        identity
            .verify_session_token(&grant.token)
            .expect("reissued token verify"),
        verified
    );
    assert!(
        identity
            .operator_account_security("resume_grant")
            .expect("unchanged authority view")
            == before,
        "re-delivery must not create sessions/credentials/audit or touch lifetime fields"
    );
}

#[test]
fn resume_grant_rejects_forged_verified_fields_and_revoked_current_store() {
    let (identity, verified) = issue_test_identity();
    for changed in [
        VerifiedIdentitySession {
            account_id: "another_account".into(),
            ..verified.clone()
        },
        VerifiedIdentitySession {
            session_id: "missing_session".into(),
            ..verified.clone()
        },
        VerifiedIdentitySession {
            expires_at_ms: verified.expires_at_ms + 1,
            ..verified.clone()
        },
        VerifiedIdentitySession {
            expires_at_ms: 0,
            ..verified.clone()
        },
    ] {
        assert!(identity.reissue_verified_session_grant(&changed).is_err());
    }
    assert!(identity
        .revoke_session(&verified, &verified.session_id, "fixture revoke")
        .expect("synthetic revoke"));
    assert!(identity.reissue_verified_session_grant(&verified).is_err());
}

#[test]
fn resume_grant_only_after_exact_committed_binding_and_original_credential_cannot_replay() {
    let store = ReconnectSessionStore::default();
    let (identity, verified) = issue_test_identity();
    let (credential, expected) =
        store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
    let cache = InMemoryGatewaySessionCache::default();
    let (restore, binding, resumed) = validate_and_commit_native_resume_for_test(
        &store,
        &cache,
        &identity,
        &credential,
        gateway_unix_ms(),
    )
    .expect("real resume reservation must commit first");
    assert_eq!(binding, expected);
    let before = identity
        .list_sessions(&verified)
        .expect("existing session list");
    let grant = reissue_native_resume_identity_grant(
        &cache,
        &identity,
        &binding,
        &resumed,
        gateway_unix_ms(),
    )
    .expect("committed active binding re-delivery");
    assert_eq!(
        identity
            .verify_session_token(&grant.token)
            .expect("same bearer authority"),
        verified
    );
    assert_eq!(grant.session.issued_at_ms, before[0].issued_at_ms);
    assert_eq!(grant.session.expires_at_ms, before[0].expires_at_ms);
    assert_eq!(
        identity
            .list_sessions(&verified)
            .expect("no additional sessions")
            .len(),
        before.len()
    );
    assert!(validate_and_commit_native_resume_for_test(
        &store,
        &cache,
        &identity,
        &credential,
        gateway_unix_ms()
    )
    .is_none());
    drop(restore);
}

#[test]
fn resume_grant_rejects_binding_mismatch_expiry_and_shared_cache_revocation() {
    let store = ReconnectSessionStore::default();
    let (identity, verified) = issue_test_identity();
    let (credential, _) =
        store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
    let cache = InMemoryGatewaySessionCache::default();
    let (restore, binding, resumed) = validate_and_commit_native_resume_for_test(
        &store,
        &cache,
        &identity,
        &credential,
        gateway_unix_ms(),
    )
    .expect("synthetic native resume commit");
    for changed in [
        ResumeBinding {
            account_id: "another_account".into(),
            ..binding.clone()
        },
        ResumeBinding {
            identity_session_id: "another_session".into(),
            ..binding.clone()
        },
        ResumeBinding {
            identity_expires_at_ms: binding.identity_expires_at_ms + 1,
            ..binding.clone()
        },
    ] {
        assert!(reissue_native_resume_identity_grant(
            &cache,
            &identity,
            &changed,
            &resumed,
            gateway_unix_ms()
        )
        .is_err());
    }
    assert!(reissue_native_resume_identity_grant(
        &cache,
        &identity,
        &binding,
        &resumed,
        resumed.expires_at_ms
    )
    .is_err());
    cache
        .revoke_identity_session(&resumed.session_id, 60)
        .expect("synthetic shared cache revocation");
    assert!(reissue_native_resume_identity_grant(
        &cache,
        &identity,
        &binding,
        &resumed,
        gateway_unix_ms()
    )
    .is_err());
    // The cache gate matters even while the backing identity row is still live.
    assert!(identity.reissue_verified_session_grant(&resumed).is_ok());
    drop(restore);
}

#[test]
fn resume_grant_rejects_identity_revoked_after_resume_commit() {
    let store = ReconnectSessionStore::default();
    let (identity, verified) = issue_test_identity();
    let (credential, _) =
        store_native_resume_fixture(&store, &verified, None, None, Duration::from_secs(30));
    let cache = InMemoryGatewaySessionCache::default();
    let (restore, binding, resumed) = validate_and_commit_native_resume_for_test(
        &store,
        &cache,
        &identity,
        &credential,
        gateway_unix_ms(),
    )
    .expect("synthetic native resume commit");
    assert!(identity
        .revoke_session(&resumed, &resumed.session_id, "after commit")
        .expect("synthetic revoke"));
    assert!(reissue_native_resume_identity_grant(
        &cache,
        &identity,
        &binding,
        &resumed,
        gateway_unix_ms()
    )
    .is_err());
    drop(restore);
}

#[test]
#[ignore = "requires an explicitly isolated loopback PostgreSQL acceptance database"]
fn resume_grant_postgres_reads_the_current_session_and_preserves_its_lifetime() {
    use std::net::IpAddr;
    let base = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
        .expect("explicit isolated PostgreSQL URL required");
    assert!(base.starts_with("postgres://") || base.starts_with("postgresql://"));
    let parsed: postgres::Config = base
        .parse()
        .unwrap_or_else(|_| panic!("invalid isolated PostgreSQL configuration"));
    assert!(
        parsed
            .get_dbname()
            .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")),
        "acceptance database prefix required"
    );
    assert!(!parsed.get_hosts().is_empty() && parsed.get_hosts().iter().all(|host|
        matches!(host, postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost")
            || name.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()))), "loopback PostgreSQL required");
    assert!(parsed.get_hostaddrs().iter().all(IpAddr::is_loopback));
    let schema = format!(
        "resume_grant_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("fixture clock")
            .as_nanos()
    );
    let mut admin = postgres::Client::connect(&base, postgres::NoTls)
        .unwrap_or_else(|_| panic!("isolated PostgreSQL connection failed"));
    admin
        .batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap_or_else(|_| panic!("isolated identity schema creation failed"));
    let separator = if base.contains('?') { '&' } else { '?' };
    let scoped_url = format!("{base}{separator}options=-csearch_path%3D{schema}");
    let mut scoped = postgres::Client::connect(&scoped_url, postgres::NoTls)
        .unwrap_or_else(|_| panic!("isolated identity schema connection failed"));
    let actual: String = scoped
        .query_one("SELECT current_schema()", &[])
        .unwrap_or_else(|_| panic!("isolated schema check failed"))
        .get(0);
    assert_eq!(actual, schema);
    mir2_simulation::apply_migrations(&mut scoped)
        .unwrap_or_else(|_| panic!("isolated migrations failed"));
    // Identity-only trusted fixture, never a usable gameplay login/character.
    scoped
        .execute(
            "INSERT INTO accounts(account_id,raw_json) VALUES($1,'{}'::jsonb)",
            &[&"resume_grant_pg"],
        )
        .unwrap_or_else(|_| panic!("isolated identity account fixture failed"));
    let identity = IdentityService::from_env(Some(scoped_url))
        .unwrap_or_else(|_| panic!("explicit test identity configuration required"));
    let original = identity
        .issue_session(
            "resume_grant_pg",
            "password",
            "resume_grant_pg",
            "127.0.0.1",
            "isolated fixture",
        )
        .unwrap_or_else(|_| panic!("isolated session issue failed"));
    let verified = identity
        .verify_session_token(&original.token)
        .expect("isolated original token verify");
    let grant = identity
        .reissue_verified_session_grant(&verified)
        .expect("isolated current session reissue");
    assert!(
        grant.token == original.token,
        "no new timestamp/session/signature input"
    );
    assert_eq!(grant.session.issued_at_ms, original.session.issued_at_ms);
    assert_eq!(grant.session.expires_at_ms, original.session.expires_at_ms);
    let count: i64 = scoped
        .query_one(
            "SELECT count(*) FROM identity_sessions WHERE account_id=$1",
            &[&"resume_grant_pg"],
        )
        .unwrap_or_else(|_| panic!("isolated session count failed"))
        .get(0);
    assert_eq!(count, 1);
    scoped
        .execute(
            "UPDATE identity_sessions SET revoked_at_ms=$1 WHERE session_id=$2",
            &[&(gateway_unix_ms() as i64), &verified.session_id],
        )
        .unwrap_or_else(|_| panic!("isolated session revocation failed"));
    assert!(
        identity.reissue_verified_session_grant(&verified).is_err(),
        "fresh PG revocation must be observed"
    );
    assert!(
        identity.verify_session_token(&grant.token).is_err(),
        "re-delivery cannot outlive shared revocation"
    );
    // Preserve the uniquely named schema as audit evidence; no database cleanup.
}
