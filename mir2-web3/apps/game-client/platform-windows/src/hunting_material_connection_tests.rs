//! Synthetic raw metadata tests only. No socket bootstrap, physical command
//! dispatch, signature verification, gameplay apply or economic mutation.
use super::*;
use serde_json::json;

fn realm(version: Option<Value>, mode: &str) -> String {
    let mut payload = json!({
        "schema":"mir2-realm-handshake/1", "realmId":"fixture-realm",
        "npcRepairMode":mode, "profileId":"test-profile", "profileVersion":1,
        "acceptanceLevel":null, "source":null, "ratePolicy":null,
        "bundleHash":null, "bundleBuiltAt":null, "sourceData":null
    });
    if let Some(version) = version {
        payload["huntingMaterialOwnerProtocolVersion"] = version;
    }
    json!({"type":"realmInfo", "payload":payload}).to_string()
}

fn identity_value(account: &str, session: &str, expires: u64) -> Value {
    // Format-valid synthetic segments, deliberately not a signed login grant.
    json!({
        "type":"identitySession", "token":format!("e30.{}", "A".repeat(43)),
        "session":{
            "accountId":account, "sessionId":session, "expiresAtMs":expires,
            "revokedAtMs":null, "authMethod":"password", "credentialId":"credential-1",
            "issuedAtMs":1000, "lastSeenAtMs":1500, "revokedReason":null,
            "userAgentSummary":"Windows native fixture", "gatewayId":"gateway-fixture",
            "current":true
        }
    })
}

fn identity(account: &str, session: &str, expires: u64) -> String {
    identity_value(account, session, expires).to_string()
}

fn live() -> HuntingMaterialConnection {
    let mut connection = HuntingMaterialConnection::new();
    connection
        .observe(&realm(Some(json!(1)), "hunting-v1"), 2000)
        .unwrap();
    connection
        .observe(&identity("alice", "session-1", 9000), 2000)
        .unwrap();
    connection
}

#[test]
fn native_material_connection_requires_independent_explicit_protocol_one() {
    for version in [None, Some(Value::Null)] {
        for mode in ["hunting-v1", "legacy"] {
            let mut connection = HuntingMaterialConnection::new();
            assert!(connection
                .observe(&realm(version.clone(), mode), 2000)
                .unwrap());
            connection
                .observe(&identity("alice", "session-1", 9000), 2000)
                .unwrap();
            assert!(!connection.supports_protocol_v1());
            assert!(!connection.is_retired());
            assert!(connection.identity(2000).is_none());
            assert!(!connection.identity_matches("alice", "session-1", 2000));
        }
    }
    for mode in ["legacy", "hunting-v1", "unrelated"] {
        let mut connection = HuntingMaterialConnection::new();
        connection
            .observe(&realm(Some(json!(1)), mode), 2000)
            .unwrap();
        assert!(
            connection.supports_protocol_v1(),
            "repair mode must have no material role"
        );
        assert!(
            connection.identity(2000).is_none(),
            "advertisement is not identity"
        );
    }
    for version in [
        json!(0),
        json!(2),
        json!(-1),
        json!("1"),
        json!(true),
        json!(1.0),
        json!({}),
    ] {
        let mut connection = live();
        assert!(connection
            .observe(&realm(Some(version), "hunting-v1"), 2000)
            .is_err());
        assert!(connection.is_retired() && !connection.supports_protocol_v1());
        assert!(connection.identity(2000).is_none());
    }
}

#[test]
fn native_material_connection_same_physical_realm_and_protocol_cannot_change() {
    let mut connection = live();
    let original = connection.identity(2000).unwrap();
    assert!(connection
        .observe(&realm(Some(json!(1)), "legacy"), 2000)
        .unwrap());
    assert!(
        connection.matches(&original, 2000),
        "unchanged material tuple preserves evidence"
    );
    let changed = realm(Some(json!(1)), "hunting-v1").replace("fixture-realm", "foreign-realm");
    assert!(connection.observe(&changed, 2000).is_err());
    assert!(connection.is_retired());
    assert_eq!(connection.realm_id(), None);
    assert!(!connection.matches(&original, 2000));
    connection.clear_identity();
    assert!(connection
        .observe(&realm(Some(json!(1)), "hunting-v1"), 2000)
        .is_err());

    for first in [None, Some(Value::Null)] {
        let mut connection = HuntingMaterialConnection::new();
        connection
            .observe(&realm(first, "hunting-v1"), 2000)
            .unwrap();
        assert!(
            connection
                .observe(&realm(Some(json!(1)), "hunting-v1"), 2000)
                .is_err(),
            "unsupported cannot upgrade on the same physical reader"
        );
    }
    let mut connection = live();
    assert!(connection
        .observe(&realm(None, "hunting-v1"), 2000)
        .is_err());
}

#[test]
fn native_material_connection_identity_is_exact_expiring_and_non_bearer() {
    let connection = live();
    assert_eq!(connection.realm_id(), Some("fixture-realm"));
    assert_eq!(connection.account_id(), Some("alice"));
    assert_eq!(connection.session_id(), Some("session-1"));
    assert_eq!(connection.expires_at_ms(), Some(9000));
    let evidence = connection.identity(8999).unwrap();
    assert_eq!(evidence.realm_id(), "fixture-realm");
    assert_eq!(evidence.account_id(), "alice");
    assert_eq!(evidence.session_id(), "session-1");
    assert_eq!(evidence.expires_at_ms(), 9000);
    assert!(connection.identity_matches("alice", "session-1", 8999));
    assert!(!connection.identity_matches("ALICE", "session-1", 2000));
    assert!(!connection.identity_matches("alice", "session-2", 2000));
    assert!(connection.matches(&evidence, 8999));
    assert!(!connection.matches(&evidence, 9000));
    assert!(!evidence.is_live(9000));
    assert!(connection.identity(9000).is_none());
    assert!(connection.identity(u64::MAX).is_none());
    let debug = format!("{connection:?} {evidence:?}");
    for sensitive in ["e30.", "session-1", "alice"] {
        assert!(!debug.contains(sensitive));
    }
}

#[test]
fn native_material_connection_rotation_clear_and_same_value_aba_revoke_old_witness() {
    let mut connection = live();
    let first = connection.identity(2000).unwrap();
    connection
        .observe(&identity("alice", "session-2", 10_000), 2000)
        .unwrap();
    assert!(!connection.matches(&first, 2000));
    assert!(!connection.identity_matches("alice", "session-1", 2000));
    let second = connection.identity(2000).unwrap();
    connection
        .observe(&identity("alice", "session-2", 10_000), 2000)
        .unwrap();
    assert!(
        !connection.matches(&second, 2000),
        "equal identity delivery advances revision"
    );
    let third = connection.identity(2000).unwrap();
    connection.clear_identity();
    assert!(connection.supports_protocol_v1());
    assert!(!connection.matches(&third, 2000));
    assert_eq!(connection.account_id(), None);
    connection
        .observe(&identity("alice", "session-2", 10_000), 2000)
        .unwrap();
    assert!(!connection.matches(&third, 2000));
    let fresh = live();
    assert!(
        !fresh.matches(&first, 2000),
        "new physical namespace is not equal old identity data"
    );
}

#[test]
fn native_material_connection_rejects_root_and_deep_escaped_duplicate_fields() {
    for raw in [
        r#"{"type":"realmInfo","t\u0079pe":"packet","payload":{}}"#,
        r#"{"type":"packet","t\u0079pe":"realmInfo","payload":{}}"#,
        r#"{"type":"realmInfo","payload":{"schema":"mir2-realm-handshake/1","realmId":"fixture-realm","huntingMaterialOwnerProtocolVersion":1,"huntingMaterialOwnerProtocolVersi\u006fn":1}}"#,
        r#"{"type":"realmInfo","payload":{"schema":"mir2-realm-handshake/1","realmId":"fixture-realm","huntingMaterialOwnerProtocolVersion":1,"ratePolicy":{"nested":[{"x":1,"\u0078":1}]}}}"#,
        r#"{"type":"identitySession","token":"x","session":{"accountId":"alice","account\u0049d":"alice"}}"#,
        r#"{"payload":{"a":,},"type":"realmInfo"}"#,
        r#"{"type":"realmInfo","payload":{} trailing}"#,
    ] {
        let mut connection = live();
        let evidence = connection.identity(2000).unwrap();
        assert!(connection.observe(raw, 2000).is_err());
        assert!(connection.is_retired());
        assert!(!connection.matches(&evidence, 2000));
        assert_eq!(connection.account_id(), None);
    }
    let mut connection = HuntingMaterialConnection::new();
    assert!(connection.observe(
        r#"{"t\u0079pe":"r\u0065almInfo","payload":{"schema":"mir2-realm-handshake/1","realmId":"escaped","huntingMaterialOwnerProtocolVersion":1}}"#,
        2000
    ).unwrap());
    assert!(connection.supports_protocol_v1());
}

#[test]
fn native_material_connection_ordinary_packets_and_nested_claims_never_grant_or_retire() {
    let mut connection = live();
    let evidence = connection.identity(2000).unwrap();
    for raw in [
        r#"{"type":"packet","payload":{"type":"identitySession","token":"untrusted"}}"#,
        r#"{"type":"snapshot","payload":{"type":"realmInfo","huntingMaterialOwnerProtocolVersion":1}}"#,
        r#"{"type":"packet","text":"{\"type\":\"realmInfo\"}"}"#,
        r#"{"type":"packet","payload":{"x":1,"x":2}}"#,
        r#"{"type":"packet","payload": [ broken ordinary data ]}"#,
        r#"{"type":"identitySessionOther"}"#,
        "plain ordinary data",
    ] {
        assert!(!connection.observe(raw, 2000).unwrap());
        assert!(!connection.is_retired());
        assert!(connection.matches(&evidence, 2000));
    }
    let oversized_ordinary =
        json!({"type":"snapshot","payload":"x".repeat(MAX_HANDSHAKE_BYTES + 1)}).to_string();
    assert!(!connection.observe(&oversized_ordinary, 2000).unwrap());
    assert!(connection.matches(&evidence, 2000));
}

#[test]
fn native_material_connection_identity_rejects_missing_revoked_expired_or_inexact_fields() {
    let mut invalid = Vec::new();
    for key in ["accountId", "sessionId", "expiresAtMs", "revokedAtMs"] {
        let mut value = identity_value("alice", "session-1", 9000);
        value["session"].as_object_mut().unwrap().remove(key);
        invalid.push(value);
    }
    for (key, value) in [
        ("accountId", json!("")),
        ("accountId", json!(" alice")),
        ("accountId", json!("a".repeat(161))),
        ("sessionId", json!("bad\n")),
        ("sessionId", json!("s".repeat(257))),
        ("expiresAtMs", json!(2000)),
        ("expiresAtMs", json!("9000")),
        ("expiresAtMs", json!(9000.0)),
        ("expiresAtMs", json!(MAX_EXACT_MS + 1)),
        ("revokedAtMs", json!(1)),
        ("revokedReason", json!("revoked")),
        ("current", json!(false)),
        ("issuedAtMs", json!(9000)),
        ("issuedAtMs", json!(32_001)),
        ("lastSeenAtMs", json!("1500")),
        ("unexpected", json!(true)),
    ] {
        let mut identity = identity_value("alice", "session-1", 9000);
        identity["session"][key] = value;
        invalid.push(identity);
    }
    for value in invalid {
        let mut connection = live();
        assert!(connection.observe(&value.to_string(), 2000).is_err());
        assert!(connection.is_retired() && connection.identity(2000).is_none());
    }
}

#[test]
fn native_material_connection_rejects_token_format_without_retaining_it() {
    for token in [
        String::new(),
        "private-bearer".into(),
        "e30.a".into(),
        format!("e30.{}", "A".repeat(42)),
        format!("e30.{}=", "A".repeat(43)),
        format!("e30.{}", "B".repeat(43)),
        format!("a.{}", "A".repeat(43)),
        format!("e30.{}.extra", "A".repeat(43)),
        format!("e30/.{}", "A".repeat(43)),
        format!("e30.{}\n", "A".repeat(43)),
        "A".repeat(4090),
    ] {
        let mut value = identity_value("alice", "session-1", 9000);
        value["token"] = token.into();
        let mut connection = live();
        assert!(connection.observe(&value.to_string(), 2000).is_err());
        assert!(connection.is_retired());
    }
    // All three legal base64url final lengths are accepted, with no bearer stored.
    for payload in ["e30", "e30A", "e30AAA"] {
        let mut value = identity_value("alice", "session-1", 9000);
        value["token"] = format!("{payload}.{}", "A".repeat(43)).into();
        let mut connection = live();
        connection.observe(&value.to_string(), 2000).unwrap();
        assert!(connection.identity_matches("alice", "session-1", 2000));
        assert!(!format!("{connection:?}").contains(payload));
    }
}

#[test]
fn native_material_connection_enforces_handshake_resource_limits_and_schema() {
    let mut cases = Vec::new();
    let good: Value = serde_json::from_str(&realm(Some(json!(1)), "hunting-v1")).unwrap();
    for value in [
        json!(""),
        json!(" r"),
        json!("r\n"),
        json!("r".repeat(129)),
        json!(12),
    ] {
        let mut case = good.clone();
        case["payload"]["realmId"] = value;
        cases.push(case.to_string());
    }
    let mut case = good.clone();
    case["payload"]["schema"] = "mir2-realm-handshake/2".into();
    cases.push(case.to_string());
    let mut case = good.clone();
    case["unexpected"] = true.into();
    cases.push(case.to_string());
    let mut case = good.clone();
    case["payload"]["unexpected"] = true.into();
    cases.push(case.to_string());
    let mut case = good.clone();
    case["payload"]["sourceData"] = json!({"large":"x".repeat(MAX_STRING_BYTES + 1)});
    cases.push(case.to_string());
    let mut case = good.clone();
    case["payload"]["ratePolicy"] = json!({"many":vec![0; MAX_NODES]});
    cases.push(case.to_string());
    let mut deep = Value::Null;
    for _ in 0..MAX_DEPTH {
        deep = json!([deep]);
    }
    let mut case = good.clone();
    case["payload"]["sourceData"] = deep;
    cases.push(case.to_string());
    let mut case = good;
    case["payload"]["sourceData"] = json!("x".repeat(MAX_HANDSHAKE_BYTES));
    cases.push(case.to_string());
    for raw in cases {
        let mut connection = live();
        assert!(connection.observe(&raw, 2000).is_err());
        assert!(connection.is_retired());
    }
}

#[test]
fn native_material_connection_revision_exhaustion_retires_without_revival() {
    let mut connection = live();
    let evidence = connection.identity(2000).unwrap();
    connection.revision = u64::MAX;
    assert!(connection
        .observe(&identity("alice", "session-2", 9000), 2000)
        .is_err());
    assert!(connection.is_retired());
    assert!(!connection.matches(&evidence, 2000));
    assert_eq!(connection.account_id(), None);
    connection.clear_identity();
    assert!(connection
        .observe(&realm(Some(json!(1)), "hunting-v1"), 2000)
        .is_err());
}
