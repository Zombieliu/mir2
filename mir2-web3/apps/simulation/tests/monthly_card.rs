use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::monthly_card::{
    monthly_card_now_ms, MonthlyCardPolicy, MONTHLY_CARD_DURATION_MS,
};
use mir2_simulation::{
    AccountRecord, AccountStoreRepository, FileAccountStoreRepository, SimulationConfig,
    SimulationSession,
};

const KEY: &str = "test-only-monthly-issuance-key-20261006-never-deploy";
fn config(required: bool) -> SimulationConfig {
    let config = SimulationConfig::default()
        .with_monthly_card_policy(MonthlyCardPolicy::new(required, Some(KEY)).unwrap());
    let character = config.default_character.clone();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert("other".into(), AccountRecord::new(character));
    config
}
fn start(session: &mut SimulationSession, index: i32) -> bool {
    session
        .handle_packet(ClientPacket::StartGame {
            character_index: index,
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. }))
}

#[test]
fn monthly_card_calendar_boundary_and_renewal_are_account_wide() {
    let config = config(true);
    let now = 1_800_000_000_000;
    let missing = config.monthly_card_status("demo", now).unwrap();
    assert!(!missing.can_enter_game && !missing.active);
    let issued = config
        .issue_monthly_card("demo", "order-20261006-01", now)
        .unwrap();
    assert_eq!(issued.duration_days, 30);
    assert!(
        !config.monthly_card_status("demo", now).unwrap().active,
        "issuance does not activate access"
    );
    let activated = config
        .redeem_monthly_card("demo", &issued.code, now + 5_000)
        .unwrap();
    let expires = now + 5_000 + MONTHLY_CARD_DURATION_MS;
    assert_eq!(activated.status.expires_at_ms, Some(expires));
    assert!(
        config
            .monthly_card_status("demo", expires - 1)
            .unwrap()
            .can_enter_game
    );
    assert!(
        !config
            .monthly_card_status("demo", expires)
            .unwrap()
            .can_enter_game
    );
    assert_eq!(
        config
            .monthly_card_status("demo", expires + 3_000)
            .unwrap()
            .remaining_ms,
        0
    );
    let renewed = config
        .issue_monthly_card("demo", "order-20261006-02", now + 10_000)
        .unwrap();
    assert_eq!(
        config
            .redeem_monthly_card("demo", &renewed.code, now + 20_000)
            .unwrap()
            .status
            .expires_at_ms,
        Some(expires + MONTHLY_CARD_DURATION_MS)
    );
    assert_eq!(
        config
            .monthly_card_status("other", now)
            .unwrap()
            .expires_at_ms,
        None
    );
}

#[test]
fn monthly_card_expired_subscription_renews_from_now_and_unused_code_expires() {
    let config = config(true);
    let now = 1_800_000_000_000;
    let first = config
        .issue_monthly_card("demo", "order-first-00001", now)
        .unwrap();
    config
        .redeem_monthly_card("demo", &first.code, now)
        .unwrap();
    let later = now + MONTHLY_CARD_DURATION_MS + 99_000;
    let second = config
        .issue_monthly_card("demo", "order-second-0002", later)
        .unwrap();
    assert_eq!(
        config
            .redeem_monthly_card("demo", &second.code, later)
            .unwrap()
            .status
            .expires_at_ms,
        Some(later + MONTHLY_CARD_DURATION_MS)
    );
    let config = config.with_monthly_card_policy(
        MonthlyCardPolicy::new(true, Some(KEY))
            .unwrap()
            .with_code_validity_days(Some(1))
            .unwrap(),
    );
    let unused = config
        .issue_monthly_card("other", "order-unused-0003", now)
        .unwrap();
    assert_eq!(
        config
            .redeem_monthly_card("other", &unused.code, unused.redeem_before_ms.unwrap())
            .unwrap_err(),
        "monthlyCardCodeExpired"
    );
    assert!(!config.monthly_card_status("other", later).unwrap().active);
}

#[test]
fn monthly_card_issue_and_redeem_retries_do_not_duplicate_credit() {
    let config = config(true);
    let now = 1_800_000_000_000;
    let issued = config
        .issue_monthly_card("demo", "payment-retry-0001", now)
        .unwrap();
    let replayed = config
        .issue_monthly_card("demo", "payment-retry-0001", now + 1_000)
        .unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.code, issued.code);
    assert_eq!(replayed.redeem_before_ms, issued.redeem_before_ms);
    let first = config
        .redeem_monthly_card("demo", &issued.code, now)
        .unwrap();
    let again = config
        .redeem_monthly_card("demo", &issued.code, now + MONTHLY_CARD_DURATION_MS + 1)
        .unwrap();
    assert!(again.replayed);
    assert_eq!(first.status.expires_at_ms, again.status.expires_at_ms);
    assert!(!again.status.active);
    let serialized = serde_json::to_string(&*config.account_store.lock().unwrap()).unwrap();
    assert!(!serialized.contains(KEY) && !serialized.contains(&issued.code));
    assert!(!format!("{issued:?}").contains(&issued.code));
    assert!(!format!("{:?}", config.monthly_card_policy).contains(KEY));
}

#[test]
fn monthly_card_concurrent_redeem_is_once_and_codes_cannot_cross_accounts() {
    let config = config(true);
    let now = 1_800_000_000_000;
    let issued = config
        .issue_monthly_card("demo", "payment-race-0001", now)
        .unwrap();
    assert_eq!(
        config
            .redeem_monthly_card("other", &issued.code, now)
            .unwrap_err(),
        "monthlyCardCodeInvalid"
    );
    assert!(config
        .redeem_monthly_card("demo", "MC1-invalid", now)
        .is_err());
    let threads: Vec<_> = (0..6)
        .map(|_| {
            let config = config.clone();
            let code = issued.code.clone();
            std::thread::spawn(move || config.redeem_monthly_card("demo", &code, now).unwrap())
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| !r.replayed).count(), 1);
    assert_eq!(
        config
            .monthly_card_status("demo", now)
            .unwrap()
            .expires_at_ms,
        Some(now + MONTHLY_CARD_DURATION_MS)
    );
}

#[test]
fn monthly_card_authentication_remains_available_but_world_entry_is_gated() {
    let config = config(true);
    let mut session = SimulationSession::new(config.clone());
    let packets = session.passkey_login("demo");
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    assert!(!start(&mut session, config.default_character.index));
    assert!(session.active_identity().is_none());
    let now = monthly_card_now_ms();
    let code = config
        .issue_monthly_card("demo", "auth-entry-20261006", now)
        .unwrap();
    config.redeem_monthly_card("demo", &code.code, now).unwrap();
    assert!(start(&mut session, config.default_character.index));
    session.handle_packet(ClientPacket::LogOut);
    assert!(session.active_identity().is_none());
}

#[test]
fn monthly_card_durable_restart_keeps_expiry_and_single_use_receipt() {
    let root = std::env::temp_dir().join(format!(
        "mir2-monthly-card-{}-{}",
        std::process::id(),
        monthly_card_now_ms()
    ));
    let path = root.join("accounts.json");
    let config = config(true).with_account_store_path(&path);
    let now = monthly_card_now_ms();
    let code = config
        .issue_monthly_card("demo", "durable-monthly-0001", now)
        .unwrap();
    let redeemed = config.redeem_monthly_card("demo", &code.code, now).unwrap();
    let repository = FileAccountStoreRepository::new(&path);
    let loaded = repository.load(config.default_character.clone()).unwrap();
    let ledger = loaded.accounts["demo"].monthly_card.as_ref().unwrap();
    assert_eq!(Some(ledger.expires_at_ms), redeemed.status.expires_at_ms);
    assert_eq!(
        ledger
            .codes
            .values()
            .filter(|row| row.redeemed_at_ms.is_some())
            .count(),
        1
    );
    ledger.validate().unwrap();
    let raw = std::fs::read_to_string(path).unwrap();
    assert!(raw.contains("monthlyCard") && !raw.contains(&code.code));
    let reopened = SimulationConfig::default()
        .with_account_store_path(root.join("accounts.json"))
        .with_monthly_card_policy(MonthlyCardPolicy::new(true, Some(KEY)).unwrap());
    assert_eq!(
        reopened.monthly_card_status("demo", now).unwrap(),
        redeemed.status
    );
    assert!(
        reopened
            .redeem_monthly_card("demo", &code.code, now + 1)
            .unwrap()
            .replayed
    );
}

#[test]
fn monthly_card_free_mode_and_legacy_accounts_do_not_get_implicit_paid_access() {
    let config = config(false);
    let status = config
        .monthly_card_status("demo", monthly_card_now_ms())
        .unwrap();
    assert!(status.can_enter_game && !status.active && !status.required);
    let json =
        serde_json::to_value(&config.account_store.lock().unwrap().accounts["demo"]).unwrap();
    assert!(json.get("monthlyCard").is_none());
    let legacy: AccountRecord = serde_json::from_value(json).unwrap();
    assert!(legacy.monthly_card.is_none());
    assert!(MonthlyCardPolicy::new(true, None).is_err());
    assert!(MonthlyCardPolicy::new(true, Some("short")).is_err());
}

#[test]
fn monthly_card_unused_codes_have_no_implicit_expiration_and_key_rotation_preserves_redemption() {
    let config = config(true);
    let now = 1_800_000_000_000;
    let issued = config
        .issue_monthly_card("demo", "unused-forever-0001", now)
        .unwrap();
    assert!(issued.redeem_before_ms.is_none());
    let changed = config.with_monthly_card_policy(
        MonthlyCardPolicy::new(true, Some("different-monthly-qa-issuance-key-20261006")).unwrap(),
    );
    assert_eq!(
        changed
            .issue_monthly_card("demo", "unused-forever-0001", now)
            .unwrap_err(),
        "monthlyCardIssuanceKeyChanged"
    );
    let later = now + MONTHLY_CARD_DURATION_MS * 12;
    assert_eq!(
        changed
            .redeem_monthly_card("demo", &issued.code, later)
            .unwrap()
            .status
            .expires_at_ms,
        Some(later + MONTHLY_CARD_DURATION_MS)
    );
}

#[cfg(feature = "test-support")]
#[test]
fn monthly_card_failed_durable_write_rolls_back_credit_and_receipt() {
    use mir2_simulation::AccountStoreTransactionFault;
    let root = std::env::temp_dir().join(format!(
        "mir2-monthly-fault-{}-{}",
        std::process::id(),
        monthly_card_now_ms()
    ));
    let config = config(true).with_account_store_path(root.join("accounts.json"));
    let now = monthly_card_now_ms();
    let issued = config
        .issue_monthly_card("demo", "write-fault-000001", now)
        .unwrap();
    let before = config.account_store.lock().unwrap().clone();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(config
        .redeem_monthly_card("demo", &issued.code, now)
        .is_err());
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert!(!config.monthly_card_status("demo", now).unwrap().active);
    let receipt = config
        .redeem_monthly_card("demo", &issued.code, now)
        .unwrap();
    assert!(!receipt.replayed);
    assert_eq!(receipt.status.remaining_ms, MONTHLY_CARD_DURATION_MS);
}
