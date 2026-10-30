//! Shared ordinary/drop UID authority against strictly owned disposable PG.
//! The acknowledgement hook follows a real COMMIT; it is not a process-kill test.
use super::Database;
use crate::economy::PostgresEconomyStore;
use mir2_simulation::{
    AccountStoreDatabaseMode, PlayerDropOperationBinding, SimulationConfig, UserItemUidAllocator,
    UserItemUidError, UserItemUidReason, ZoneSaveFence,
};
use std::collections::BTreeSet;

const CENSUS: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn initialized(db: &Database, floor: u64) -> SimulationConfig {
    db.store()
        .initialize_player_drop_item_uid_authority(floor, CENSUS, "owned-shared-uid-fixture")
        .unwrap();
    bound(db, "owner-a", 7)
}

fn bound(db: &Database, owner: &str, epoch: u64) -> SimulationConfig {
    db.config(owner, epoch)
        .with_postgres_item_uid_allocator()
        .unwrap()
}

fn issue(allocator: &UserItemUidAllocator) -> u64 {
    allocator
        .issue(UserItemUidReason::NpcTradePurchase)
        .unwrap()
        .get()
}

fn binding(db: &Database, key: &str, sequence: u64) -> PlayerDropOperationBinding {
    PlayerDropOperationBinding::new(
        key.into(),
        "shared-uid-original-rpc".into(),
        "shared-uid-source".into(),
        0,
        db.zone().into(),
        "owner-a".into(),
        7,
        sequence,
    )
    .unwrap()
}

fn counts(db: &Database) -> (i64, i64, i64, i64) {
    let row = db
        .connect()
        .query_one(
            "SELECT (SELECT count(*) FROM public.zone_player_drop_item_uid_authority),\
             (SELECT count(*) FROM public.zone_player_drop_item_uid_census_audit),\
             (SELECT count(*) FROM public.zone_player_drop_item_uid_batches),\
             (SELECT count(*) FROM public.zone_player_drop_item_uid_reservations)",
            &[],
        )
        .unwrap();
    (row.get(0), row.get(1), row.get(2), row.get(3))
}

fn with_option(url: &str, option: &str) -> String {
    format!("{url}{}{option}", if url.contains('?') { '&' } else { '?' })
}

fn shared_uid_stored_character(
    db: &Database,
    account: &str,
    character_index: i32,
) -> mir2_simulation::CharacterSaveRecord {
    let value: serde_json::Value = db
        .connect()
        .query_one(
            "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2",
            &[&account, &character_index],
        )
        .unwrap()
        .get(0);
    serde_json::from_value(value).unwrap()
}

fn shared_uid_owned_character_ids(save: &mir2_simulation::CharacterSaveRecord) -> BTreeSet<u64> {
    fn collect(value: &serde_json::Value, ids: &mut BTreeSet<u64>) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    if matches!(
                        key.as_str(),
                        "unique_id" | "uniqueId" | "user_item_unique_id"
                    ) {
                        let id = value
                            .as_u64()
                            .or_else(|| value.as_str().and_then(|v| v.parse().ok()))
                            .expect("owned saved tree requires an exact UID");
                        assert!(
                            id > 0,
                            "owned saved tree must not retain an unassigned zero"
                        );
                        ids.insert(id);
                    }
                    collect(value, ids);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    collect(value, ids);
                }
            }
            _ => {}
        }
    }
    let mut ids = BTreeSet::new();
    for encoded in save
        .inventory_items_json
        .iter()
        .chain(&save.belt_items_json)
        .chain(&save.storage_items_json)
        .chain(&save.equipment_items_json)
        .chain(&save.hero_inventory_items_json)
        .chain(&save.hero_equipment_items_json)
    {
        collect(
            &serde_json::from_str::<serde_json::Value>(encoded).unwrap(),
            &mut ids,
        );
    }
    ids
}

fn shared_uid_character_assets(save: &mir2_simulation::CharacterSaveRecord) -> serde_json::Value {
    serde_json::json!({
        "gold": save.gold, "credit": save.credit,
        "inventory": save.inventory_items_json, "belt": save.belt_items_json,
        "storage": save.storage_items_json, "equipment": save.equipment_items_json,
        "hero_inventory": save.hero_inventory_items_json,
        "hero_equipment": save.hero_equipment_items_json,
        "stage5": save.stage5_systems_json,
    })
}

fn shared_uid_relogin(
    config: SimulationConfig,
    account: &str,
    character_index: i32,
) -> mir2_simulation::InProcessWorldRuntime {
    use mir2_protocol::{ClientPacket, ServerPacket};
    use mir2_simulation::{WorldCommand, WorldRuntime};
    let mut runtime = mir2_simulation::InProcessWorldRuntime::new(config);
    assert!(runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: account.into(),
            password: account.into(),
        }))
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index
        }))
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert_eq!(
        runtime.active_identity().unwrap().character_index,
        character_index
    );
    runtime
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_shared_uid_pg_runtime_new_character_starter_survives_save_and_relogin_without_remint(
) {
    use mir2_simulation::WorldRuntime;
    let db = Database::new();
    let config = initialized(&db, 2_000_000);
    let allocator = config.server_item_uid_allocator().unwrap();
    let floor = allocator.issued_through().unwrap();
    let account = "shared-uid-starter-login";
    let mut runtime =
        crate::economy::start_test_runtime_with_config(config.clone(), account, "UidStarterLogin")
            .unwrap();
    let identity = runtime
        .active_identity()
        .expect("actual NewCharacter and StartGame succeeded");
    let active = runtime.active_character_checkpoint().unwrap();
    let stored = shared_uid_stored_character(&db, account, identity.character_index);
    let owned = shared_uid_owned_character_ids(&active);
    assert!(
        !owned.is_empty(),
        "the fixture must exercise real starter item roots"
    );
    let through = allocator.issued_through().unwrap();
    assert_eq!(
        owned,
        (floor + 1..=through).collect(),
        "every newly issued PG identity is in the complete starter save"
    );
    assert_eq!(shared_uid_owned_character_ids(&stored), owned);
    runtime.save_active_character().unwrap();
    let committed = shared_uid_stored_character(&db, account, identity.character_index);
    assert_eq!(shared_uid_owned_character_ids(&committed), owned);
    assert_eq!(allocator.issued_through().unwrap(), through);
    drop(runtime);

    let mut relogged = shared_uid_relogin(config, account, identity.character_index);
    let restored = relogged.active_character_checkpoint().unwrap();
    assert_eq!(shared_uid_owned_character_ids(&restored), owned);
    assert_eq!(
        allocator.issued_through().unwrap(),
        through,
        "StartGame restores the original exact identities"
    );
    relogged.save_active_character().unwrap();
    assert_eq!(
        shared_uid_owned_character_ids(&shared_uid_stored_character(
            &db,
            account,
            identity.character_index,
        )),
        owned
    );
    assert_eq!(allocator.issued_through().unwrap(), through);
    assert_eq!(
        counts(&db),
        (1, 1, 0, 0),
        "starter issuance shares the scalar floor without fake drop batches"
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_shared_uid_pg_public_system_mail_claim_commits_once_and_old_owner_cannot_issue() {
    use mir2_protocol::{ClientPacket, ServerPacket};
    use mir2_simulation::{
        deliver_stage5_system_mail, Stage5MailDelivery, Stage5MailTargetKind, WorldCommand,
        WorldRuntime,
    };
    let db = Database::new();
    let config = initialized(&db, 3_000_000);
    let allocator = config.server_item_uid_allocator().unwrap();
    let account = "shared-uid-mail-claim";
    let character_name = "UidMailClaim";
    let mut runtime =
        crate::economy::start_test_runtime_with_config(config.clone(), account, character_name)
            .unwrap();
    runtime.save_active_character().unwrap();
    let identity = runtime.active_identity().unwrap();
    let before = shared_uid_stored_character(&db, account, identity.character_index);
    let before_ids = shared_uid_owned_character_ids(&before);
    let before_floor = allocator.issued_through().unwrap();
    let helmet = mir2_game_data::crystal_item_by_name("BronzeHelmet").unwrap();
    let bell = mir2_game_data::crystal_item_by_name("BronzeBell").unwrap();
    let receipt = deliver_stage5_system_mail(
        &config,
        Stage5MailDelivery {
            target_kind: Stage5MailTargetKind::Character,
            target_id: character_name.into(),
            from: "Owned PG fixture".into(),
            subject: "Fresh shared authority reward".into(),
            body: "Public delivery; collect with the real packet path".into(),
            gold: 77,
            items: vec![
                format!("crystal-item-{}", helmet.item_index),
                format!("crystal-item-{}", bell.item_index),
            ],
        },
    )
    .unwrap();
    assert_eq!(receipt.delivered_count, 1);
    assert_eq!(receipt.mail_ids.len(), 1);
    assert_eq!(
        allocator.issued_through().unwrap(),
        before_floor,
        "delivery publishes the trusted reward, claim performs issuance"
    );
    let mail_id = receipt.mail_ids[0];
    let delivered = shared_uid_stored_character(&db, account, identity.character_index);
    let delivered_systems: mir2_simulation::Stage5SystemsState =
        serde_json::from_str(delivered.stage5_systems_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        delivered_systems
            .mail
            .iter()
            .find(|mail| mail.id == mail_id)
            .unwrap()
            .attachment_origin,
        mir2_simulation::Stage5MailAttachmentOrigin::FreshGrant,
        "the public canonical delivery, not this test, authorizes fresh issuance"
    );
    drop(runtime);

    let mut runtime = shared_uid_relogin(config.clone(), account, identity.character_index);
    assert!(runtime
        .world_snapshot()
        .stage5_systems
        .mail
        .iter()
        .any(|mail| mail.id == mail_id && !mail.claimed));
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::CollectParcel {
            mail_id: u64::from(mail_id),
        }))
        .unwrap();
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ParcelCollected { result: 1 })));
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 77 })));
    let packet_items: Vec<_> = packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::GainedItem { item } => Some(item),
            _ => None,
        })
        .collect();
    assert_eq!(packet_items.len(), 2);
    assert_eq!(
        packet_items
            .iter()
            .map(|item| item.item_index)
            .collect::<BTreeSet<_>>(),
        [helmet.item_index, bell.item_index].into_iter().collect()
    );
    let committed = shared_uid_stored_character(&db, account, identity.character_index);
    let after_ids = shared_uid_owned_character_ids(&committed);
    let fresh: BTreeSet<_> = after_ids.difference(&before_ids).copied().collect();
    let after_floor = allocator.issued_through().unwrap();
    assert_eq!(fresh, (before_floor + 1..=after_floor).collect());
    assert_eq!(fresh.len(), 2);
    assert_eq!(
        packet_items
            .iter()
            .map(|item| item.unique_id)
            .collect::<BTreeSet<_>>(),
        fresh
    );
    assert_eq!(committed.gold, before.gold.checked_add(77).unwrap());
    let systems: mir2_simulation::Stage5SystemsState =
        serde_json::from_str(committed.stage5_systems_json.as_deref().unwrap()).unwrap();
    let claimed = systems.mail.iter().find(|mail| mail.id == mail_id).unwrap();
    assert!(claimed.claimed);
    assert_eq!(claimed.gold, 0);
    assert!(claimed.items.is_empty() && claimed.item_states_json.is_empty());
    assert_eq!(
        shared_uid_character_assets(&runtime.active_character_checkpoint().unwrap()),
        shared_uid_character_assets(&committed)
    );

    let repeat = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::CollectParcel {
            mail_id: u64::from(mail_id),
        }))
        .unwrap();
    assert!(!repeat.iter().any(|packet| matches!(
        packet,
        ServerPacket::ParcelCollected { result: 1 }
            | ServerPacket::GainedGold { .. }
            | ServerPacket::GainedItem { .. }
    )));
    assert_eq!(allocator.issued_through().unwrap(), after_floor);
    assert_eq!(
        shared_uid_character_assets(&shared_uid_stored_character(
            &db,
            account,
            identity.character_index
        )),
        shared_uid_character_assets(&committed)
    );
    drop(runtime);
    let mut runtime = shared_uid_relogin(config.clone(), account, identity.character_index);
    assert_eq!(
        shared_uid_owned_character_ids(&runtime.active_character_checkpoint().unwrap()),
        after_ids
    );
    assert_eq!(allocator.issued_through().unwrap(), after_floor);

    let repeat_after_relogin = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::CollectParcel {
            mail_id: u64::from(mail_id),
        }))
        .unwrap();
    assert!(!repeat_after_relogin.iter().any(|packet| matches!(
        packet,
        ServerPacket::ParcelCollected { result: 1 }
            | ServerPacket::GainedGold { .. }
            | ServerPacket::GainedItem { .. }
    )));
    assert_eq!(allocator.issued_through().unwrap(), after_floor);
    assert_eq!(runtime.world_snapshot().gold, committed.gold);

    let fenced_receipt = deliver_stage5_system_mail(
        &config,
        Stage5MailDelivery {
            target_kind: Stage5MailTargetKind::Character,
            target_id: character_name.into(),
            from: "Owned PG fixture".into(),
            subject: "Must remain unclaimed after takeover".into(),
            body: "Old owner cannot mint or publish assets".into(),
            gold: 19,
            items: vec![format!("crystal-item-{}", helmet.item_index)],
        },
    )
    .unwrap();
    assert!(runtime.refresh_active_external_mail());
    let before_fenced_live =
        shared_uid_character_assets(&runtime.active_character_checkpoint().unwrap());
    let before_fenced_db = serde_json::to_value(shared_uid_stored_character(
        &db,
        account,
        identity.character_index,
    ))
    .unwrap();
    db.lease("owner-b", 8);
    let rejected = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::CollectParcel {
            mail_id: u64::from(fenced_receipt.mail_ids[0]),
        }))
        .unwrap();
    assert!(!rejected.iter().any(|packet| matches!(
        packet,
        ServerPacket::ParcelCollected { result: 1 }
            | ServerPacket::GainedGold { .. }
            | ServerPacket::GainedItem { .. }
    )));
    assert_eq!(allocator.issued_through().unwrap(), after_floor);
    assert_eq!(
        shared_uid_character_assets(&runtime.active_character_checkpoint().unwrap()),
        before_fenced_live
    );
    assert_eq!(
        serde_json::to_value(shared_uid_stored_character(
            &db,
            account,
            identity.character_index
        ))
        .unwrap(),
        before_fenced_db
    );
    assert_eq!(counts(&db), (1, 1, 0, 0));
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_shared_uid_pg_independent_issuers_drop_batches_and_restart_share_one_floor() {
    let db = Database::new();
    let first_config = initialized(&db, 10_000);
    let first = first_config.server_item_uid_allocator().unwrap();
    let second_config = bound(&db, "owner-a", 7);
    let second = second_config.server_item_uid_allocator().unwrap();
    assert!(!first.shares_authority_with(&second));
    assert_eq!(issue(&first), 10_001);
    let drop_binding = binding(&db, "shared-uid-interleave", 1);
    let permit = first_config.zone_economy_write_permit().unwrap();
    let reserved = db
        .store()
        .reserve_player_drop_item_uids(&permit, &drop_binding, 2)
        .unwrap();
    assert_eq!(reserved, vec![10_002, 10_003]);
    assert_eq!(issue(&second), 10_004);
    assert_eq!(first.issued_through().unwrap(), 10_004);
    assert_eq!(counts(&db), (1, 1, 1, 2));

    drop(first);
    drop(first_config);
    let restarted_config = bound(&db, "owner-a", 7);
    let restarted = restarted_config.server_item_uid_allocator().unwrap();
    assert_eq!(issue(&restarted), 10_005);
    assert_eq!(
        db.store()
            .reserve_player_drop_item_uids(&permit, &drop_binding, 2)
            .unwrap(),
        reserved,
        "an exact drop receipt remains queryable without allocating again"
    );
    assert_eq!(restarted.issued_through().unwrap(), 10_005);

    // Independent ordinary handles and independent SQL connections contend on
    // the same lease/floor. Ordinary issuance must not create fake drop batches.
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let allocator = bound(&db, "owner-a", 7)
                .server_item_uid_allocator()
                .unwrap();
            std::thread::spawn(move || (0..4).map(|_| issue(&allocator)).collect::<Vec<_>>())
        })
        .collect();
    let concurrent: BTreeSet<_> = handles
        .into_iter()
        .flat_map(|handle| handle.join().expect("owned UID issuer thread"))
        .collect();
    assert_eq!(concurrent, (10_006..=10_021).collect());
    assert_eq!(second.issued_through().unwrap(), 10_021);
    assert_eq!(counts(&db), (1, 1, 1, 2));
    let old_allocator_table: Option<String> = db
        .connect()
        .query_one(
            "SELECT to_regclass('shared_item_identity_allocator')::text",
            &[],
        )
        .unwrap()
        .get(0);
    assert!(
        old_allocator_table.is_none(),
        "no second UID floor is provisioned"
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_shared_uid_pg_live_fence_and_source_configuration_cannot_be_substituted() {
    let db = Database::new();
    let config = initialized(&db, 20_000);
    let allocator = config.server_item_uid_allocator().unwrap();
    assert_eq!(issue(&allocator), 20_001);
    assert!(SimulationConfig::default()
        .with_item_uid_allocator(allocator.clone())
        .is_err());
    assert_eq!(
        allocator.issued_through().unwrap(),
        20_001,
        "a PG-bound handle cannot be admitted as a File source"
    );

    let mut changed_dsn = config.clone();
    changed_dsn.account_store_database_url =
        Some(with_option(db.url(), "application_name=uid-changed"));
    assert!(changed_dsn.server_item_uid_allocator().is_err());
    let mut detached = config.clone();
    detached.account_store_database_url = None;
    assert!(detached.server_item_uid_allocator().is_err());
    let mut changed_mode = config.clone();
    changed_mode.account_store_database_mode = AccountStoreDatabaseMode::Mirror;
    assert!(changed_mode.server_item_uid_allocator().is_err());
    assert!(config
        .clone()
        .with_zone_save_fence(ZoneSaveFence::new(db.zone().into(), "owner-b".into(), 8).unwrap())
        .is_err());

    let independent = db.config("owner-a", 7);
    let mut changed_store = config.clone();
    changed_store.account_store = independent.account_store;
    assert!(changed_store.server_item_uid_allocator().is_err());
    let shadow = config.fork_for_replica_apply().unwrap();
    assert!(shadow.server_item_uid_allocator().is_err());
    assert!(shadow.with_postgres_item_uid_allocator().is_err());

    let wrong_zone = db
        .config("owner-a", 7)
        .with_zone_save_fence(
            ZoneSaveFence::new("unleased-shared-uid-zone".into(), "owner-a".into(), 7).unwrap(),
        )
        .unwrap();
    assert!(wrong_zone.with_postgres_item_uid_allocator().is_err());
    let wrong_owner = db.config("not-the-owner", 7);
    assert!(wrong_owner.with_postgres_item_uid_allocator().is_err());

    db.lease("owner-b", 8);
    assert!(allocator.issue(UserItemUidReason::ItemStackSplit).is_err());
    assert!(allocator.ensure_issued_through_at_least(0).is_err());
    assert_eq!(allocator.issued_through().unwrap(), 20_001);
    let successor = bound(&db, "owner-b", 8);
    let successor_allocator = successor.server_item_uid_allocator().unwrap();
    assert_eq!(issue(&successor_allocator), 20_002);
    db.connect()
        .execute(
            "UPDATE public.zone_owner_leases SET expires_at_ms=\
             floor(extract(epoch FROM clock_timestamp())*1000)::bigint-1 WHERE zone_id=$1",
            &[&db.zone()],
        )
        .unwrap();
    assert!(successor_allocator
        .issue(UserItemUidReason::ItemStackSplit)
        .is_err());
    assert!(successor_allocator
        .ensure_issued_through_at_least(0)
        .is_err());
    assert_eq!(successor_allocator.issued_through().unwrap(), 20_002);
    assert_eq!(counts(&db), (1, 1, 0, 0));
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_shared_uid_pg_missing_census_never_initializes_or_falls_back() {
    let db = Database::new();
    let config = db.config("owner-a", 7);
    assert!(config.server_item_uid_allocator().is_err());
    assert!(config.clone().with_postgres_item_uid_allocator().is_err());
    assert!(config.clone().with_postgres_item_uid_allocator().is_err());
    let permit = config.zone_economy_write_permit().unwrap();
    assert!(db
        .store()
        .reserve_player_drop_item_uids(&permit, &binding(&db, "uncensused-drop", 2), 1)
        .is_err());
    assert_eq!(counts(&db), (0, 0, 0, 0));

    // The only admitted path is the explicit audited fixture initialization.
    let now_bound = initialized(&db, 30_000);
    assert_eq!(
        issue(&now_bound.server_item_uid_allocator().unwrap()),
        30_001
    );
    assert_eq!(counts(&db), (1, 1, 0, 0));
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_shared_uid_pg_committed_lost_ack_burns_uid_and_freezes_all_source_clones() {
    let db = Database::new();
    let config = initialized(&db, 40_000);
    let clone = config.clone();
    let allocator = config.server_item_uid_allocator().unwrap();
    let allocator_clone = allocator.clone();
    allocator
        .lose_next_postgres_commit_acknowledgement_for_test()
        .unwrap();
    assert!(matches!(
        allocator.issue(UserItemUidReason::NpcTradePurchase),
        Err(UserItemUidError::DatabaseCommitOutcomeUnknown)
    ));
    assert_eq!(allocator.issued_through().unwrap(), 40_001);
    assert_eq!(allocator_clone.issued_through().unwrap(), 40_001);
    for source in [&config, &clone] {
        let permit = source.zone_economy_write_permit().unwrap();
        assert!(permit.validate_source_write_state().is_err());
        let from_config = source.server_item_uid_allocator().unwrap();
        assert!(from_config
            .issue(UserItemUidReason::NpcTradePurchase)
            .is_err());
        assert!(from_config.ensure_issued_through_at_least(0).is_err());
        assert!(db
            .store()
            .reserve_player_drop_item_uids(&permit, &binding(&db, "frozen-drop", 3), 1)
            .is_err());
    }
    assert!(allocator_clone
        .issue(UserItemUidReason::NpcTradePurchase)
        .is_err());
    assert_eq!(counts(&db), (1, 1, 0, 0));

    db.lease("owner-b", 8);
    let successor = bound(&db, "owner-b", 8);
    let next = successor.server_item_uid_allocator().unwrap();
    assert_eq!(
        issue(&next),
        40_002,
        "the unacknowledged committed UID is retired"
    );
    assert_eq!(allocator.issued_through().unwrap(), 40_002);
    assert!(allocator
        .issue(UserItemUidReason::NpcTradePurchase)
        .is_err());
    assert_eq!(counts(&db), (1, 1, 0, 0));
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_shared_uid_pg_full_u64_and_monthly_reserved_boundaries_are_atomic() {
    {
        let db = Database::new();
        let config = initialized(&db, u64::MAX - 1);
        let allocator = config.server_item_uid_allocator().unwrap();
        let permit = config.zone_economy_write_permit().unwrap();
        assert!(db
            .store()
            .reserve_player_drop_item_uids(&permit, &binding(&db, "overflowing-two", 4), 2)
            .is_err());
        assert_eq!(allocator.issued_through().unwrap(), u64::MAX - 1);
        assert_eq!(counts(&db), (1, 1, 0, 0));
        assert_eq!(issue(&allocator), u64::MAX);
        assert!(matches!(
            allocator.issue(UserItemUidReason::NpcTradePurchase),
            Err(UserItemUidError::Exhausted { .. })
        ));
        assert!(db
            .store()
            .reserve_player_drop_item_uids(&permit, &binding(&db, "overflowing-one", 5), 1)
            .is_err());
        assert_eq!(
            allocator.ensure_issued_through_at_least(0).unwrap(),
            u64::MAX
        );
        assert_eq!(counts(&db), (1, 1, 0, 0));
    }
    {
        let db = Database::new();
        let reserved_start = 0x001d_0000_0000_0000;
        let reserved_end_exclusive = 0x001e_0000_0000_0000;
        let config = initialized(&db, reserved_start - 2);
        let allocator = config.server_item_uid_allocator().unwrap();
        assert_eq!(issue(&allocator), reserved_start - 1);
        let permit = config.zone_economy_write_permit().unwrap();
        assert_eq!(
            db.store()
                .reserve_player_drop_item_uids(&permit, &binding(&db, "skip-monthly-range", 6), 2)
                .unwrap(),
            vec![reserved_end_exclusive, reserved_end_exclusive + 1]
        );
        assert_eq!(issue(&allocator), reserved_end_exclusive + 2);
        assert_eq!(
            allocator.issued_through().unwrap(),
            reserved_end_exclusive + 2
        );
        assert_eq!(counts(&db), (1, 1, 1, 2));
    }
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_shared_uid_pg_namespace_isolation_and_matching_census_audit_are_required() {
    let db = Database::new();
    let public = initialized(&db, 50_000);
    let public_allocator = public.server_item_uid_allocator().unwrap();
    db.connect()
        .batch_execute("CREATE SCHEMA uid_secondary; CREATE SCHEMA uid_unaudited")
        .unwrap();
    let second_url = with_option(db.url(), "options=-csearch_path%3Duid_secondary%2Cpublic");
    PostgresEconomyStore::new(&second_url)
        .initialize_player_drop_item_uid_authority(90_000, CENSUS, "owned-shared-uid-fixture")
        .unwrap();
    let mut second_config = db.config("owner-a", 7);
    second_config.account_store_database_url = Some(second_url.clone());
    let second_config = second_config.with_postgres_item_uid_allocator().unwrap();
    let second = second_config.server_item_uid_allocator().unwrap();
    assert_eq!(issue(&public_allocator), 50_001);
    assert_eq!(issue(&second), 90_001);
    assert_eq!(public_allocator.issued_through().unwrap(), 50_001);
    assert_eq!(second.issued_through().unwrap(), 90_001);

    let mut substituted = public.clone();
    substituted.account_store_database_url = Some(second_url);
    assert!(substituted.server_item_uid_allocator().is_err());
    let wrong_target_permit = public.zone_economy_write_permit().unwrap();
    assert!(PostgresEconomyStore::new(
        second_config.account_store_database_url.as_deref().unwrap()
    )
    .reserve_player_drop_item_uids(
        &wrong_target_permit,
        &binding(&db, "namespace-substitution", 7),
        1
    )
    .is_err());

    // An operator-corrupted row is not evidence of a complete-history census.
    db.connect()
        .execute(
            "INSERT INTO public.zone_player_drop_item_uid_authority\
             (source_namespace,singleton,issued_through,census_digest,initialized_by,initialized_at_ms)\
             VALUES('uid_unaudited',TRUE,70000,$1,'missing-audit-fixture',1)",
            &[&CENSUS],
        )
        .unwrap();
    let mut unaudited = db.config("owner-a", 7);
    unaudited.account_store_database_url = Some(with_option(
        db.url(),
        "options=-csearch_path%3Duid_unaudited%2Cpublic",
    ));
    assert!(unaudited.with_postgres_item_uid_allocator().is_err());
    assert_eq!(counts(&db), (3, 2, 0, 0));
    let rogue_floor: String = db
        .connect()
        .query_one(
            "SELECT issued_through::text FROM public.zone_player_drop_item_uid_authority \
             WHERE source_namespace='uid_unaudited'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(rogue_floor, "70000");
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_shared_uid_pg_actual_runtime_stack_split_uses_and_persists_shared_authority() {
    use mir2_protocol::{ClientPacket, MirGridType, ServerPacket, UserItem, UserItemStat};
    use mir2_simulation::{
        GroundDropItemPayload, GroundDropLootSnapshot, GroundDropSnapshot, ItemContainer,
        WorldCommand, WorldRuntime,
    };

    let db = Database::new();
    let config = initialized(&db, 1_000_000);
    let allocator = config.server_item_uid_allocator().unwrap();
    let mut runtime = crate::economy::start_test_runtime_with_config(
        config.clone(),
        "shared-uid-split-account",
        "SharedUidSplit",
    )
    .unwrap();
    let template = mir2_game_data::crystal_item_by_name("(HP)DrugSmall").unwrap();
    assert!(template.stack_size >= 5);
    let source_uid = 765_001;
    let exact = UserItem {
        unique_id: source_uid,
        item_index: template.item_index,
        current_dura: 123,
        max_dura: 456,
        count: 5,
        soul_bound_id: 0,
        identified: false,
        cursed: true,
        slots: vec![],
        gem_count: 3,
        added_stats: vec![UserItemStat { stat: 17, value: 6 }],
        awake_type: 2,
        awake_values: vec![4, 5],
        refined_value: 6,
        refine_added: 7,
        refine_success_chance: 88,
        wedding_ring: 23,
        expire_info: None,
        rental_information: None,
        is_shop_item: true,
        sealed_info: None,
        gm_made: true,
    };
    let pickup = runtime.commit_shared_ground_drop_pickup_transaction(&GroundDropSnapshot {
        object_id: 0x7f00_7099,
        name: template.name.clone(),
        name_colour_argb: -1,
        icon: template.image,
        x: 0,
        y: 0,
        quantity: 5,
        source_monster: "owned shared UID runtime fixture".into(),
        owner_object_id: None,
        ownership_remaining_ticks: None,
        loot: GroundDropLootSnapshot::InventoryItem {
            key: format!("crystal-item-{}", template.item_index),
            name: template.name.clone(),
            description: "retained shared UID split fixture".into(),
            weight: u16::from(template.weight),
            durability_current: Some(exact.current_dura),
            durability_max: Some(exact.max_dura),
            added_attack: 0,
            added_defence: 0,
            added_stats: exact.added_stats.clone(),
            cursed: exact.cursed,
            socket_slots: 0,
            show_group_pickup: template.show_group_pickup,
            exact_item: Some(GroundDropItemPayload {
                item: exact.clone(),
                uid_assigned: true,
            }),
        },
    });
    assert!(pickup.committed);
    assert!(pickup
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedItem { item } if item == &exact)));
    let snapshot = runtime.world_snapshot();
    if let Some(belt) = snapshot
        .belt_items
        .iter()
        .find(|item| item.unique_id == source_uid)
    {
        let empty = (0..snapshot.inventory_capacity.saturating_sub(6).min(40))
            .find(|slot| {
                !snapshot.inventory_items.iter().any(|item| {
                    item.container == ItemContainer::Bag1 && u16::from(item.slot) == *slot
                })
            })
            .unwrap();
        let from = i32::from(belt.slot);
        let to = i32::from(empty) + 6;
        let moved = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::MoveItem {
                grid: MirGridType::Belt,
                from,
                to,
            }))
            .unwrap();
        assert!(moved.contains(&ServerPacket::MoveItem {
            grid: MirGridType::Belt,
            from,
            to,
            success: true,
        }));
    }
    runtime.save_active_character().unwrap();
    let before = allocator.issued_through().unwrap();
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::SplitItem {
            grid: MirGridType::Inventory,
            unique_id: source_uid,
            count: 2,
        }))
        .unwrap();
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::SplitItem1 { success: true, .. })));
    let split = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::SplitItem {
                item: Some(item),
                grid: MirGridType::Inventory,
            } => Some(item),
            _ => None,
        })
        .expect("actual inventory split packet");
    assert!(split.unique_id > before);
    assert_ne!(split.unique_id, source_uid);
    let mut expected = exact;
    expected.unique_id = split.unique_id;
    expected.count = 2;
    assert_eq!(
        split, &expected,
        "the split keeps the complete original metadata"
    );
    assert_eq!(allocator.issued_through().unwrap(), split.unique_id);
    let after = runtime.world_snapshot();
    assert_eq!(
        after
            .inventory_items
            .iter()
            .find(|item| item.unique_id == source_uid)
            .unwrap()
            .quantity,
        3
    );
    let carriers: Vec<_> = after
        .inventory_items
        .iter()
        .chain(after.belt_items.iter())
        .filter(|item| item.unique_id == source_uid || item.unique_id == split.unique_id)
        .collect();
    assert_eq!(
        carriers.len(),
        2,
        "one source and one split carrier have sole custody"
    );
    let split_carrier = carriers
        .iter()
        .find(|item| item.unique_id == split.unique_id)
        .unwrap();
    assert!(matches!(
        split_carrier.container,
        ItemContainer::Bag1 | ItemContainer::Belt
    ));
    assert_eq!(split_carrier.quantity, 2);
    assert_eq!(carriers.iter().map(|item| item.quantity).sum::<u32>(), 5);
    runtime.save_active_character().unwrap();
    let checkpoint = runtime.active_character_checkpoint().unwrap();
    let identity = runtime.active_identity().unwrap();
    let stored: serde_json::Value = db
        .connect()
        .query_one(
            "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2",
            &[&identity.account_id, &identity.character_index],
        )
        .unwrap()
        .get(0);
    assert!(
        stored == serde_json::to_value(checkpoint).unwrap(),
        "split checkpoint is durable"
    );
    let restarted = bound(&db, "owner-a", 7)
        .server_item_uid_allocator()
        .unwrap();
    assert_eq!(issue(&restarted), split.unique_id + 1);
    assert_eq!(
        counts(&db),
        (1, 1, 0, 0),
        "ordinary runtime split does not fabricate drop batches"
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_shared_uid_pg_queued_issuer_rechecks_shared_freeze_after_uid_row_lock() {
    use std::time::{Duration, Instant};

    let db = Database::new();
    initialized(&db, 60_000);
    let queued_url = with_option(db.url(), "application_name=mir2-shared-uid-queued");
    let mut config = db.config("owner-a", 7);
    config.account_store_database_url = Some(queued_url);
    let config = config.with_postgres_item_uid_allocator().unwrap();
    let allocator = config.server_item_uid_allocator().unwrap();
    let permit = config.zone_economy_write_permit().unwrap();
    let mut blocker = db.connect();
    let blocker_pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let mut locked_floor = blocker.transaction().unwrap();
    locked_floor
        .query_one(
            "SELECT issued_through::text FROM public.zone_player_drop_item_uid_authority \
             WHERE source_namespace='public' AND singleton=TRUE FOR UPDATE",
            &[],
        )
        .unwrap();
    let mut observer = db.connect();
    let queued_allocator = allocator.clone();
    let worker =
        std::thread::spawn(move || queued_allocator.issue(UserItemUidReason::NpcTradePurchase));
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut observed_wait = false;
    while Instant::now() < deadline {
        observed_wait = observer
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity \
                 WHERE datname=current_database() \
                   AND application_name='mir2-shared-uid-queued' \
                   AND state='active' AND wait_event_type='Lock' \
                   AND query LIKE '%zone_player_drop_item_uid_authority%' \
                   AND query LIKE '%FOR UPDATE%' \
                   AND $1=ANY(pg_catalog.pg_blocking_pids(pid)))",
                &[&blocker_pid],
            )
            .unwrap()
            .get(0);
        if observed_wait || worker.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut lease_was_locked = false;
    if observed_wait {
        // The issuer has already acquired the original lease before waiting
        // on this UID row. NOWAIT probes that actual PG lock, without taking it.
        let mut probe = observer.transaction().unwrap();
        lease_was_locked = probe
            .query_one(
                "SELECT owner_id FROM public.zone_owner_leases WHERE zone_id=$1 FOR UPDATE NOWAIT",
                &[&db.zone()],
            )
            .is_err_and(|error| {
                error.code() == Some(&postgres::error::SqlState::LOCK_NOT_AVAILABLE)
            });
        probe.rollback().unwrap();
        // This is an explicit shared-gate race, not a failed SQL COMMIT or an
        // acknowledgement-loss injection. No database result is made uncertain.
        permit.commit_outcome_unknown();
    }
    // Always release the owned blocker and join before assertions/DB cleanup.
    locked_floor.rollback().unwrap();
    let result = worker.join().expect("owned queued UID issuer thread");
    assert!(
        observed_wait,
        "pg_stat_activity must prove the intended UID-row wait"
    );
    assert!(
        lease_was_locked,
        "the waiting issuer must already hold the Zone lease"
    );
    assert!(
        matches!(result, Err(UserItemUidError::DatabaseAuthorityRejected { reason })
            if reason == "live source write permit is required"),
        "the shared freeze must reject the issuer; an ordinary SQL lock timeout is insufficient"
    );
    assert!(permit.validate_source_write_state().is_err());
    assert_eq!(allocator.issued_through().unwrap(), 60_000);
    assert_eq!(counts(&db), (1, 1, 0, 0));
}
