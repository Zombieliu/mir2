//! Pure server-capability tests. No connection, lease result or PostgreSQL
//! publication is simulated here; the coordinator owns actual transaction QA.
use super::*;
use crate::{InProcessWorldRuntime, ZoneSaveOperation, ZoneSaveOperationBinding};

const SOURCE_URL: &str = "postgresql://economy_user:private-test-password@zone-economy-test.invalid:55491/mir2_economy_test?options=-c%20search_path%3Daccount_namespace";

fn fence(owner: &str, generation: u64) -> ZoneSaveFence {
    ZoneSaveFence::new("economy-permit-zone".into(), owner.into(), generation).unwrap()
}

fn source_config() -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config.account_store_database_url = Some(SOURCE_URL.into());
    config
}

fn bound_config() -> SimulationConfig {
    source_config()
        .with_zone_save_fence(fence("owner-a", 17))
        .unwrap()
}

#[test]
fn zone_economy_permit_requires_existing_bound_postgres_source_and_no_file_or_mirror() {
    assert!(
        SimulationConfig::default()
            .zone_economy_write_permit()
            .is_err()
    );
    assert!(source_config().zone_economy_write_permit().is_err());
    let config = bound_config();
    assert!(config.zone_economy_write_permit().is_ok());
    for alteration in 0..5 {
        let mut invalid = config.clone();
        match alteration {
            0 => invalid.account_store_database_mode = AccountStoreDatabaseMode::Mirror,
            1 => invalid.account_store_path = Some("unused-zone-economy-test.json".into()),
            2 => invalid.account_store_database_url = None,
            3 => {
                invalid.account_store_database_url = Some("invalid://private-test-password".into())
            }
            _ => invalid.account_store_database_url = Some("postgresql://localhost/".into()),
        }
        assert!(invalid.zone_economy_write_permit().is_err());
    }
}

#[test]
fn zone_economy_permit_matches_full_database_configuration_and_account_namespace() {
    let permit = bound_config().zone_economy_write_permit().unwrap();
    permit.validate_target_database(SOURCE_URL).unwrap();
    for target in [
        SOURCE_URL.replace(":55491/", ":55492/"),
        SOURCE_URL.replace("mir2_economy_test?", "other_database?"),
        SOURCE_URL.replace("economy_user:", "other_user:"),
        SOURCE_URL.replace("account_namespace", "other_namespace"),
        SOURCE_URL.replace("private-test-password", "other-password"),
        SOURCE_URL.replace("postgresql://", "postgres://"),
        SOURCE_URL.split('?').next().unwrap().to_string(),
        format!(" {SOURCE_URL}"),
    ] {
        let error = permit.validate_target_database(&target).unwrap_err();
        assert_eq!(error, REJECTED);
        assert!(!error.contains("private-test-password"));
        assert!(!error.contains("account_namespace"));
    }
}

#[test]
fn zone_economy_permit_matches_exact_zone_generation_without_granting_new_authority() {
    let config = bound_config();
    let permit = config.zone_economy_write_permit().unwrap();
    assert_eq!(permit.fence(), &fence("owner-a", 17));
    permit.validate_context("economy-permit-zone", 17).unwrap();
    permit
        .validate_mutation_admission(SOURCE_URL, "economy-permit-zone", 17)
        .unwrap();
    for (zone, generation) in [
        ("foreign-zone", 17),
        ("economy-permit-zone", 0),
        ("economy-permit-zone", 16),
        ("economy-permit-zone", 18),
        ("economy-permit-zone ", 17),
        ("economy-permit-zone", u64::MAX),
    ] {
        assert_eq!(
            permit.validate_context(zone, generation),
            Err(REJECTED.into())
        );
        assert_eq!(
            permit.validate_mutation_admission(SOURCE_URL, zone, generation),
            Err(REJECTED.into())
        );
    }
    let replacement = config.with_zone_save_fence(fence("owner-b", 18)).unwrap();
    assert_eq!(permit.fence(), &fence("owner-a", 17));
    assert_eq!(
        replacement.zone_economy_write_permit().unwrap().fence(),
        &fence("owner-b", 18)
    );
}

#[test]
fn zone_economy_permit_unknown_freezes_shared_source_clones_but_preserves_readonly_binding() {
    let config = bound_config();
    let shared = config.clone();
    let permit = config.zone_economy_write_permit().unwrap();
    let clone = permit.clone();
    let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    permit.validate_source_write_state().unwrap();
    let error = permit.commit_outcome_unknown();
    assert!(
        clone
            .validate_source_write_state()
            .unwrap_err()
            .contains("COMMIT_OUTCOME_UNKNOWN")
    );
    assert!(error.starts_with(zone_save_fence::COMMIT_OUTCOME_UNKNOWN));
    assert!(error.contains("ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN"));
    assert!(!error.contains("private-test-password"));
    assert!(!error.contains("zone-economy-test.invalid"));
    assert!(
        config
            .ensure_account_store_writable()
            .unwrap_err()
            .contains("writes are frozen")
    );
    assert!(
        shared
            .ensure_account_store_writable()
            .unwrap_err()
            .contains("writes are frozen")
    );
    assert!(
        clone
            .validate_mutation_admission(SOURCE_URL, "economy-permit-zone", 17)
            .unwrap_err()
            .contains("writes are frozen")
    );
    // Issuance, context and target checks remain usable for an old receipt.
    for readonly in [clone, config.zone_economy_write_permit().unwrap()] {
        readonly.validate_target_database(SOURCE_URL).unwrap();
        readonly
            .validate_context("economy-permit-zone", 17)
            .unwrap();
    }
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        before
    );
}

#[test]
fn zone_economy_permit_unknown_keeps_prior_freeze_but_never_returns_prior_private_detail() {
    let config = bound_config();
    let permit = config.zone_economy_write_permit().unwrap();
    config.freeze_account_store_writes("prior-private-driver-detail".into());
    let error = permit.commit_outcome_unknown();
    assert!(error.starts_with(zone_save_fence::COMMIT_OUTCOME_UNKNOWN));
    assert!(!error.contains("prior-private-driver-detail"));
    assert!(
        config
            .ensure_account_store_writable()
            .unwrap_err()
            .contains("prior-private-driver-detail")
    );
}

#[test]
fn zone_economy_permit_redacts_configuration_and_is_not_issued_from_operation_scoped_config() {
    let config = bound_config();
    let permit = config.zone_economy_write_permit().unwrap();
    let debug = format!("{permit:?}");
    assert!(debug.contains("[REDACTED]"));
    for secret in [
        SOURCE_URL,
        "private-test-password",
        "economy_user",
        "account_namespace",
        "owner-a",
    ] {
        assert!(!debug.contains(secret));
    }
    let frozen = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    let mut expected = frozen.clone();
    expected.revision += 1;
    let operation = ZoneSaveOperation::new(
        ZoneSaveOperationBinding::new(
            "economy-operation-scoped".into(),
            "rpc-scoped".into(),
            fence("owner-a", 17),
        )
        .unwrap(),
        "demo".into(),
        frozen,
        expected,
        Some(42),
    )
    .unwrap();
    let scoped = config.clone().with_zone_save_operation(operation).unwrap();
    assert_eq!(
        scoped.zone_economy_write_permit().unwrap_err().to_string(),
        REJECTED
    );
    assert!(config.zone_economy_write_permit().is_ok());
}

#[test]
fn zone_economy_permit_runtime_getter_uses_the_runtime_source_binding_and_shared_freeze() {
    let memory = InProcessWorldRuntime::new(SimulationConfig::default());
    assert!(memory.zone_economy_write_permit().is_err());
    let source = bound_config();
    let runtime = InProcessWorldRuntime::new(source.clone());
    let permit = runtime.zone_economy_write_permit().unwrap();
    assert_eq!(permit.fence(), &fence("owner-a", 17));
    permit.commit_outcome_unknown();
    let readonly = runtime.zone_economy_write_permit().unwrap();
    readonly
        .validate_context("economy-permit-zone", 17)
        .unwrap();
    assert!(
        readonly
            .validate_mutation_admission(SOURCE_URL, "economy-permit-zone", 17)
            .is_err()
    );
    assert!(source.ensure_account_store_writable().is_err());
}
