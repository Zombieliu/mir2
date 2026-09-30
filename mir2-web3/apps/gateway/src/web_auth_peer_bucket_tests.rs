use super::{enforce_auth_rate_limits, AuthSecurityAction, AuthSecurityContext};
use crate::cache::InMemoryGatewaySessionCache;
use crate::identity::IdentityService;

fn registration(account: &str) -> AuthSecurityContext {
    AuthSecurityContext {
        account_id: account.into(),
        action: AuthSecurityAction::Registration,
    }
}

#[test]
fn auth_peer_bucket_same_native_or_unknown_user_agent_does_not_block_other_peers() {
    for user_agent in ["unknown", "", "Mir2Native/1"] {
        let cache = InMemoryGatewaySessionCache::default();
        let identity = IdentityService::local_for_tests();
        for n in 0..5 {
            enforce_auth_rate_limits(
                &cache,
                &identity,
                "192.0.2.1",
                user_agent,
                &registration(&format!("first{n}")),
            )
            .unwrap();
        }
        assert!(enforce_auth_rate_limits(
            &cache,
            &identity,
            "192.0.2.1",
            user_agent,
            &registration("firstsixth")
        )
        .is_err());
        for n in 0..5 {
            enforce_auth_rate_limits(
                &cache,
                &identity,
                "192.0.2.2",
                user_agent,
                &registration(&format!("second{n}")),
            )
            .unwrap();
        }
    }
}

#[test]
fn auth_peer_bucket_changing_user_agent_cannot_bypass_peer_registration_limit() {
    let cache = InMemoryGatewaySessionCache::default();
    let identity = IdentityService::local_for_tests();
    for n in 0..5 {
        enforce_auth_rate_limits(
            &cache,
            &identity,
            "192.0.2.3",
            &format!("changed-{n}"),
            &registration(&format!("changed{n}")),
        )
        .unwrap();
    }
    assert!(enforce_auth_rate_limits(
        &cache,
        &identity,
        "192.0.2.3",
        "brand-new-ua",
        &registration("sixthaccount")
    )
    .is_err());
}

#[test]
fn auth_peer_bucket_global_account_limit_survives_peer_and_user_agent_changes() {
    let cache = InMemoryGatewaySessionCache::default();
    let identity = IdentityService::local_for_tests();
    for n in 1..=3 {
        enforce_auth_rate_limits(
            &cache,
            &identity,
            &format!("192.0.2.{n}"),
            &format!("device-{n}"),
            &registration("oneaccount"),
        )
        .unwrap();
    }
    assert!(enforce_auth_rate_limits(
        &cache,
        &identity,
        "192.0.2.99",
        "other-device",
        &registration("ONEACCOUNT")
    )
    .is_err());
}

#[test]
fn auth_peer_bucket_login_pair_limit_survives_user_agent_changes() {
    let cache = InMemoryGatewaySessionCache::default();
    let identity = IdentityService::local_for_tests();
    let context = AuthSecurityContext {
        account_id: "loginaccount".into(),
        action: AuthSecurityAction::Login,
    };
    for n in 0..8 {
        enforce_auth_rate_limits(
            &cache,
            &identity,
            "192.0.2.4",
            &format!("login-device-{n}"),
            &context,
        )
        .unwrap();
    }
    assert!(enforce_auth_rate_limits(
        &cache,
        &identity,
        "192.0.2.4",
        "another-login-device",
        &context
    )
    .is_err());
}
