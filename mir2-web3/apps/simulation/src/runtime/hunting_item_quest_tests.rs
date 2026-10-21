//! Actual newcomer-v2 NPC report rewards, full PG checkpoints and original
//! uncertain-source recovery. Fixture funding/profile/position are synthetic;
//! acceptance and completion use the ordinary NPC/packet implementation.
use super::*;
use crate::runtime::items::hunting_item_binding::{belongs_to_current_owner, has_bound};
use crate::runtime::items::{embedded_item_state_from_template, ItemState};
use mir2_protocol::MirClass;
use std::collections::BTreeSet;

fn profile(session: &mut SimulationSession) {
    let mut quests = session.app.world_mut().resource_mut::<QuestResource>();
    quests.newcomer_v1_cadence = false;
    quests.newcomer_v2_cadence = true;
}

fn kit_started(config: SimulationConfig, account: &str) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    profile(&mut session);
    session.enable_shared_guild_authority();
    let packets = session
        .try_handle_packet(ClientPacket::Login {
            account_id: account.into(),
            password: "demo".into(),
        })
        .unwrap();
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })),
        "{packets:?}"
    );
    let packets = session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap();
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })),
        "{packets:?}"
    );
    session
}

struct KitPgFixture {
    client: postgres::Client,
    session: SimulationSession,
    account: String,
    realm: String,
    url: String,
}

impl KitPgFixture {
    fn new(class: MirClass) -> Self {
        Self::new_with_second_character(class, false)
    }

    fn new_with_second_character(class: MirClass, second: bool) -> Self {
        let base = test_url();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("hunting_bound_kit_{}_{}", std::process::id(), nonce);
        postgres::Client::connect(&base, postgres::NoTls)
            .unwrap()
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let separator = if base.contains('?') { '&' } else { '?' };
        let url = format!("{base}{separator}options=-csearch_path%3D{schema}");
        let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
        assert_eq!(
            client
                .query_one("SELECT current_schema()", &[])
                .unwrap()
                .get::<_, String>(0),
            schema
        );
        crate::apply_migrations(&mut client).unwrap();
        let mut seed = kit_started(
            SimulationConfig::default().with_crystal_world_runtime(),
            "demo",
        );
        seed.force_authoritative_player_transform(Point { x: 285, y: 606 }, MirDirection::Up);
        seed.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 123;
        let mut save = seed.active_character_checkpoint().unwrap();
        save.character.class = class;
        let account = format!("bound-kit-{nonce}");
        let mut record = AccountRecord::new(save.character.clone());
        if second {
            let mut other = save.clone();
            other.character.index = 1;
            other.character.name = "BoundKitSecond".into();
            record.characters.push(other.character.clone());
            record.saves.insert(1, other);
        }
        record.saves.insert(0, save);
        let mut store = AccountStore::new(record.characters[0].clone());
        store.accounts.clear();
        store.accounts.insert(account.clone(), record);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();
        let realm = format!("bound-kit-{nonce}");
        let policy = serde_json::from_value(json!({"schema_version":1,"policy_version":"bound-kit-v1","enabled":true,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":2,"denominator":100},
            "daily_decay":{"numerator":1,"denominator":150},"baseline_daily_mint_atoms":720000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})).unwrap();
        mir2_economy_service::initialize_realm(&mut client, &realm, &policy).unwrap();
        let config = SimulationConfig::default()
            .with_crystal_world_runtime()
            .with_postgres_account_store(&url)
            .unwrap()
            .with_hunting_economy_realm(&realm)
            .unwrap();
        let session = kit_started(config, &account);
        Self {
            client,
            session,
            account,
            realm,
            url,
        }
    }

    fn open(&mut self) {
        assert!(
            entity_by_object_id(self.session.app.world(), 3).is_some(),
            "real catalog Jane3"
        );
        assert!(crate::runtime::npc::crystal_npc_object_in_data_range(
            self.session.app.world(),
            3
        ));
        self.session.handle_packet(ClientPacket::CallNpc {
            object_id: 3,
            key: "@Main".into(),
        });
        assert_eq!(
            self.session
                .app
                .world()
                .resource::<NpcStateResource>()
                .active_npc_dialog
                .as_ref()
                .map(|d| d.npc_object_id),
            Some(3)
        );
    }

    fn accept(&mut self) {
        self.open();
        let packets = self.session.handle_packet(ClientPacket::AcceptQuest {
            npc_index: 3,
            quest_index: BOUND_KIT_QUEST,
        });
        assert!(
            packets.iter().any(|p| matches!(
                p,
                ServerPacket::ChangeQuest {
                    quest_id: BOUND_KIT_QUEST,
                    taken: true,
                    ..
                }
            )),
            "{packets:?}"
        );
        assert_eq!(
            quests::quest_stage(self.session.app.world(), BOUND_KIT_QUEST),
            Some(QuestStage::ReadyToTurnIn),
            "ordinary initialNpcReport event, not a forced Ready fixture"
        );
    }

    fn finish(&mut self) -> Vec<ServerPacket> {
        if self
            .session
            .app
            .world()
            .resource::<NpcStateResource>()
            .active_npc_dialog
            .as_ref()
            .map(|dialog| dialog.npc_object_id)
            != Some(3)
        {
            self.open();
        }
        self.session.handle_packet(ClientPacket::FinishQuest {
            quest_index: BOUND_KIT_QUEST,
            selected_item_index: -1,
        })
    }

    fn counts(&mut self) -> (i64, i64, i64) {
        let row = self.client.query_one("SELECT (SELECT count(*) FROM hunting_economy_wallets),(SELECT count(*) FROM hunting_economy_operations),(SELECT count(*) FROM hunting_quest_source_checkpoints)", &[]).unwrap();
        (row.get(0), row.get(1), row.get(2))
    }

    fn durable(&mut self) -> CharacterSaveRecord {
        serde_json::from_value(self.client.query_one("SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=0", &[&self.account]).unwrap().get(0)).unwrap()
    }

    fn cold_config(&self) -> SimulationConfig {
        SimulationConfig::default()
            .with_crystal_world_runtime()
            .with_postgres_account_store(&self.url)
            .unwrap()
            .with_hunting_economy_realm(&self.realm)
            .unwrap()
    }
}

fn roots(session: &SimulationSession) -> Vec<ItemState> {
    let inventory = session.app.world().resource::<InventoryResource>();
    inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .cloned()
        .collect()
}

fn assert_kit(session: &SimulationSession) {
    let world = session.app.world();
    let info = quests::effective_crystal_quest_info_by_id(world, BOUND_KIT_QUEST).unwrap();
    let items = roots(session);
    let bound: Vec<_> = items.iter().filter(|item| has_bound(item)).collect();
    assert!(!bound.is_empty());
    assert!(bound
        .iter()
        .all(|item| belongs_to_current_owner(world, item)));
    let mut ids = BTreeSet::new();
    for item in &bound {
        let uid = super::super::super::items::item_unique_id(item);
        assert_ne!(uid, 0);
        assert!(ids.insert(uid));
        assert_eq!(
            items
                .iter()
                .filter(|other| super::super::super::items::item_unique_id(other) == uid)
                .count(),
            1,
            "every fresh bound UID is distinct from all existing roots"
        );
    }
    for reward in info
        .rewards_fixed_item
        .iter()
        .filter(|reward| reward.count > 0)
    {
        let key = crate::runtime::items::crystal_item_key_for_template(
            &mir2_game_data::crystal_item_by_index(reward.item.index).unwrap(),
        );
        let total: u32 = bound
            .iter()
            .filter(|item| item.key == key)
            .map(|item| item.quantity)
            .sum();
        assert_eq!(
            total,
            u32::from(reward.count),
            "actual configured reward {}",
            reward.item.name
        );
    }
    assert_eq!(session.active_character_checkpoint().unwrap().gold, 123);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_bound_kit_postgres_actual_npc_claim_all_three_classes_and_cold_login() {
    for class in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist] {
        let mut db = KitPgFixture::new(class);
        let legacy = roots(&db.session);
        assert!(legacy.iter().all(|item| !has_bound(item)));
        db.accept();
        let packets = db.finish();
        assert!(
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::CompleteQuest { .. })),
            "{packets:?}"
        );
        assert_kit(&db.session);
        for original in legacy {
            let current = roots(&db.session)
                .into_iter()
                .find(|item| {
                    item.unique_id == original.unique_id
                        && item.container == original.container
                        && item.slot == original.slot
                })
                .unwrap();
            assert_eq!(
                serde_json::to_value(current).unwrap(),
                serde_json::to_value(original).unwrap(),
                "legacy stack not reclassified or merged"
            );
        }
        assert_eq!(db.counts(), (1, 1, 1));
        let amount: i64 = db.client.query_one("SELECT bound_gold FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2", &[&db.realm, &db.account]).unwrap().get(0);
        assert_eq!(amount, 100);
        assert_eq!(
            db.durable().inventory_items_json,
            db.session
                .active_character_checkpoint()
                .unwrap()
                .inventory_items_json
        );
        let logout = db.session.handle_packet(ClientPacket::LogOut);
        assert!(
            db.session.active_zone_join_snapshot("kit").is_none(),
            "{logout:?}"
        );
        let cold = kit_started(db.cold_config(), &db.account);
        assert_kit(&cold);
        db.session = cold;
        let before =
            inventory_image(db.session.app.world().resource::<InventoryResource>()).unwrap();
        let _ = db.finish();
        assert_eq!(
            inventory_image(db.session.app.world().resource::<InventoryResource>()).unwrap(),
            before
        );
        assert_eq!(db.counts(), (1, 1, 1));
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_bound_kit_postgres_common_rollback_publishes_no_items_or_wallet_then_retries() {
    let mut db = KitPgFixture::new(MirClass::Wizard);
    db.accept();
    let before = inventory_image(db.session.app.world().resource::<InventoryResource>()).unwrap();
    db.session
        .app
        .world_mut()
        .insert_resource(HuntingQuestTestFault(1));
    let packets = db.finish();
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::CompleteQuest { .. })));
    assert_eq!(db.counts(), (0, 0, 0));
    assert_eq!(
        inventory_image(db.session.app.world().resource::<InventoryResource>()).unwrap(),
        before
    );
    assert_eq!(
        quests::quest_stage(db.session.app.world(), BOUND_KIT_QUEST),
        Some(QuestStage::ReadyToTurnIn)
    );
    assert!(db
        .session
        .app
        .world()
        .get_resource::<PendingQuestItemProjection>()
        .is_none());
    let packets = db.finish();
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::CompleteQuest { .. })),
        "{packets:?}"
    );
    assert_kit(&db.session);
    assert_eq!(db.counts(), (1, 1, 1));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_bound_kit_postgres_unknown_committed_original_request_projects_once() {
    for fault in [2, 5] {
        let mut db = KitPgFixture::new(MirClass::Wizard);
        db.accept();
        let before =
            inventory_image(db.session.app.world().resource::<InventoryResource>()).unwrap();
        db.session
            .app
            .world_mut()
            .insert_resource(HuntingQuestTestFault(fault));
        let packets = db.finish();
        assert!(!packets
            .iter()
            .any(|p| matches!(p, ServerPacket::CompleteQuest { .. })));
        assert_eq!(db.counts(), (1, 1, 1));
        assert_eq!(
            inventory_image(db.session.app.world().resource::<InventoryResource>()).unwrap(),
            before
        );
        assert!(db
            .session
            .app
            .world()
            .get_resource::<PendingQuestItemProjection>()
            .is_some());
        let packets = db.finish();
        assert!(
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::CompleteQuest { .. })),
            "{packets:?}"
        );
        assert_kit(&db.session);
        assert!(db
            .session
            .app
            .world()
            .get_resource::<PendingQuestItemProjection>()
            .is_none());
        assert_eq!(db.counts(), (1, 1, 1));
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_bound_kit_postgres_unknown_absent_original_request_can_claim_once() {
    let mut db = KitPgFixture::new(MirClass::Warrior);
    db.accept();
    db.session
        .app
        .world_mut()
        .insert_resource(HuntingQuestTestFault(4));
    let _ = db.finish();
    assert_eq!(db.counts(), (0, 0, 0));
    let packets = db.finish();
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::CompleteQuest { .. })),
        "{packets:?}"
    );
    assert_eq!(db.counts(), (1, 1, 1));
    assert_kit(&db.session);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_bound_kit_postgres_changed_live_inventory_refences_original_for_login() {
    let mut db = KitPgFixture::new(MirClass::Warrior);
    db.accept();
    db.session
        .app
        .world_mut()
        .insert_resource(HuntingQuestTestFault(2));
    let _ = db.finish();
    assert_eq!(db.counts(), (1, 1, 1));
    // Adversarial authoritative in-memory change after the unknown boundary,
    // not a client bypass of the ordinary write freeze.
    {
        let mut inventory = db
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        let template = mir2_game_data::crystal_item_by_name("Dagger").unwrap();
        let mut item = embedded_item_state_from_template(&template, crate::ItemContainer::Bag1, 39);
        item.unique_id = 900_001;
        assert!(!inventory.inventory_items.iter().any(|item| item.slot == 39));
        inventory.inventory_items.push(item);
    }
    let changed = inventory_image(db.session.app.world().resource::<InventoryResource>()).unwrap();
    let packets = db.finish();
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::CompleteQuest { .. })));
    assert_eq!(
        inventory_image(db.session.app.world().resource::<InventoryResource>()).unwrap(),
        changed,
        "never restore historical inventory"
    );
    let config = db
        .session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    assert!(config.ensure_account_store_writable().is_err());
    // The retained opaque original-source seal, not a generic unowned freeze,
    // lets ordinary account refresh recover and load the current PG image.
    db.session = kit_started(config.clone(), &db.account);
    config.ensure_account_store_writable().unwrap();
    assert_kit(&db.session);
    assert_eq!(db.counts(), (1, 1, 1));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_bound_kit_postgres_same_session_login_and_second_character_clear_old_pending() {
    let mut db = KitPgFixture::new_with_second_character(MirClass::Warrior, true);
    db.accept();
    db.session
        .app
        .world_mut()
        .insert_resource(HuntingQuestTestFault(2));
    let _ = db.finish();
    assert_eq!(db.counts(), (1, 1, 1));
    assert!(db
        .session
        .app
        .world()
        .contains_resource::<PendingQuestItemProjection>());

    let packets = db.session.handle_packet(ClientPacket::Login {
        account_id: db.account.clone(),
        password: "demo".into(),
    });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "{packets:?}"
    );
    let packets = db
        .session
        .handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "{packets:?}"
    );
    assert!(!db
        .session
        .app
        .world()
        .contains_resource::<PendingQuestItemProjection>());
    assert_kit(&db.session);
    db.session.handle_packet(ClientPacket::LogOut);
    let packets = db
        .session
        .handle_packet(ClientPacket::StartGame { character_index: 1 });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "{packets:?}"
    );
    assert!(roots(&db.session).iter().all(|item| !has_bound(item)));
    db.accept();
    let packets = db.finish();
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })),
        "{packets:?}"
    );
    assert_kit(&db.session);
    assert_eq!(db.counts(), (2, 2, 2));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_bound_kit_postgres_full_legacy_bag_does_not_consume_partial_reward() {
    let mut db = KitPgFixture::new(MirClass::Wizard);
    db.accept();
    {
        let mut inventory = db
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        inventory.inventory_items.clear();
        let template = mir2_game_data::crystal_item_by_name("Dagger").unwrap();
        let slots = super::super::super::inventory::empty_slots_for_inventory_container(
            &inventory.inventory_items,
            crate::ItemContainer::Bag1,
            inventory.inventory_capacity,
        );
        assert!(!slots.is_empty());
        for (ordinal, (container, slot)) in slots.into_iter().enumerate() {
            let mut item = embedded_item_state_from_template(&template, container, slot);
            item.unique_id = 500_000 + ordinal as u64;
            inventory.inventory_items.push(item);
        }
    }
    let before = inventory_image(db.session.app.world().resource::<InventoryResource>()).unwrap();
    let packets = db.finish();
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::CompleteQuest { .. })));
    assert_eq!(
        inventory_image(db.session.app.world().resource::<InventoryResource>()).unwrap(),
        before
    );
    assert_eq!(db.counts(), (0, 0, 0));
    assert_eq!(
        quests::quest_stage(db.session.app.world(), BOUND_KIT_QUEST),
        Some(QuestStage::ReadyToTurnIn)
    );
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_bound_kit_postgres_equipped_cold_load_rejects_realm_switch_but_allows_policy_disable() {
    let mut db = KitPgFixture::new(MirClass::Warrior);
    db.accept();
    let packets = db.finish();
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::CompleteQuest { .. })),
        "{packets:?}"
    );
    let sword = roots(&db.session)
        .into_iter()
        .find(|item| has_bound(item) && item.equip_slot == Some(crate::EquipmentSlot::Weapon))
        .unwrap();
    let slot = crate::runtime::equipment::equipment_slot_index(crate::EquipmentSlot::Weapon)
        .unwrap() as i32;
    let packets = db.session.handle_packet(ClientPacket::EquipItem {
        grid: mir2_protocol::MirGridType::Inventory,
        unique_id: sword.unique_id,
        to: slot,
    });
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::EquipItem { success: true, .. })),
        "{packets:?}"
    );
    assert!(db
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .any(crate::runtime::items::hunting_item_binding::equipment_has_bound));
    db.session.handle_packet(ClientPacket::LogOut);
    assert!(db
        .session
        .active_zone_join_snapshot("bound-realm")
        .is_none());
    let durable = db.durable();
    let original_equipment = durable.equipment_items_json.clone();

    let policy_json: serde_json::Value = db.client.query_one(
        "SELECT policy_json FROM hunting_economy_policies WHERE realm_id=$1 AND policy_version='bound-kit-v1'",
        &[&db.realm],
    ).unwrap().get(0);
    let foreign = format!("{}-foreign", db.realm);
    mir2_economy_service::initialize_realm(
        &mut db.client,
        &foreign,
        &serde_json::from_value(policy_json.clone()).unwrap(),
    )
    .unwrap();
    for realm in [Some(foreign.as_str()), None] {
        let config = SimulationConfig::default()
            .with_crystal_world_runtime()
            .with_postgres_account_store(&db.url)
            .unwrap();
        let config = match realm {
            Some(realm) => config.with_hunting_economy_realm(realm).unwrap(),
            None => config,
        };
        let mut session = SimulationSession::new(config);
        profile(&mut session);
        session.enable_shared_guild_authority();
        let packets = session.handle_packet(ClientPacket::Login {
            account_id: db.account.clone(),
            password: "demo".into(),
        });
        assert!(
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })),
            "{packets:?}"
        );
        let before = inventory_image(session.app.world().resource::<InventoryResource>()).unwrap();
        let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::StartGame { result: 2, .. })),
            "realm {realm:?}: {packets:?}"
        );
        assert!(session.active_zone_join_snapshot("bound-realm").is_none());
        assert_eq!(
            inventory_image(session.app.world().resource::<InventoryResource>()).unwrap(),
            before,
            "rejected owner must not partially load inventory or equipment"
        );
        assert_eq!(db.durable().equipment_items_json, original_equipment);
        assert_eq!(db.counts(), (1, 1, 1));
    }

    let mut disabled_json = policy_json;
    disabled_json["policy_version"] = json!("bound-kit-disabled-v1");
    disabled_json["enabled"] = json!(false);
    mir2_economy_service::register_policy(
        &mut db.client,
        &db.realm,
        &serde_json::from_value(disabled_json).unwrap(),
    )
    .unwrap();
    mir2_economy_service::activate_policy(&mut db.client, &db.realm, "bound-kit-disabled-v1", true)
        .unwrap();
    let cold = kit_started(db.cold_config(), &db.account);
    let equipment = &cold
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items;
    assert!(equipment
        .iter()
        .any(crate::runtime::items::hunting_item_binding::equipment_has_bound));
    assert!(equipment.iter().all(|item| {
        crate::runtime::items::hunting_item_binding::equipment_belongs_to_current_owner(
            cold.app.world(),
            item,
        )
    }));
    assert_eq!(db.durable().equipment_items_json, original_equipment);
    assert_eq!(db.counts(), (1, 1, 1));
}
