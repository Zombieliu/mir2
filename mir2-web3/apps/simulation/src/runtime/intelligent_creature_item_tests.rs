use super::resources::{InventoryResource, Stage5SystemsResource};
use crate::{ItemContainer, SimulationConfig, SimulationSession};
use mir2_protocol::{ClientPacket, MirGridType, ServerPacket};

fn session() -> SimulationSession {
    let mut config = SimulationConfig::default();
    config.default_character.level = 100;
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
        .app
        .world_mut()
        .resource_mut::<super::resources::SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 100;
    let player = super::components::player_entity(session.app.world()).unwrap();
    session
        .app
        .world_mut()
        .entity_mut(player)
        .get_mut::<super::components::CharacterBody>()
        .unwrap()
        .level = 100;
    session
}

fn give_pet_item(
    session: &mut SimulationSession,
    shape: i16,
) -> (u64, mir2_game_data::CrystalItemTemplate) {
    let template = mir2_game_data::crystal_item_manifest()
        .items
        .into_iter()
        .find(|t| t.item_type == 36 && t.shape == shape)
        .expect("Crystal creature item");
    let key = super::items::crystal_item_key_for_template(&template);
    let item = super::inventory::add_or_increment_item(
        session.app.world_mut(),
        ItemContainer::Bag1,
        &key,
        &template.name,
        "",
        20,
        2,
        u16::from(template.weight),
    );
    (super::items::item_unique_id(&item), template)
}
fn use_uid(session: &mut SimulationSession, uid: u64) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::UseItem {
        unique_id: uid,
        grid: MirGridType::Inventory,
    })
}

#[test]
fn creature_ordinary_use_item_acquires_all_shipped_eggs_and_duplicate_preserves_item() {
    let kinds = mir2_game_data::crystal_item_manifest()
        .items
        .into_iter()
        .filter(|t| t.item_type == 36 && (0..=14).contains(&t.shape))
        .map(|t| t.shape)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(kinds.len(), 11, "review newly shipped egg definitions");
    for kind in kinds {
        let mut session = session();
        let (uid, _) = give_pet_item(&mut session, kind);
        let packets = use_uid(&mut session, uid);
        assert!(
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::UseItem { success: true, .. })),
            "type={kind}, packets={packets:?}"
        );
        assert!(packets.iter().any(|p| matches!(p, ServerPacket::NewIntelligentCreature { creature } if creature.pet_type == kind as u8)));
        let state = session.world_snapshot().stage5_systems;
        assert_eq!(state.intelligent_creatures.len(), 1);
        assert_eq!(state.intelligent_creatures[0].fullness, 7500);
        assert_eq!(state.intelligent_creatures[0].pet_mode, 1);
        assert!(state.active_intelligent_creature().is_none());
        let (second_uid, _) = give_pet_item(&mut session, kind);
        let before = serde_json::to_value(
            &session
                .app
                .world()
                .resource::<InventoryResource>()
                .inventory_items,
        )
        .unwrap();
        assert!(use_uid(&mut session, second_uid)
            .iter()
            .any(|p| matches!(p, ServerPacket::UseItem { success: false, .. })));
        assert_eq!(
            serde_json::to_value(
                &session
                    .app
                    .world()
                    .resource::<InventoryResource>()
                    .inventory_items
            )
            .unwrap(),
            before
        );
        session.handle_packet(ClientPacket::LogOut);
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert_eq!(
            session
                .world_snapshot()
                .stage5_systems
                .intelligent_creatures[0]
                .pet_type,
            kind as u8
        );
    }
}

#[test]
fn creature_maintenance_uses_seconds_food_protection_expiry_and_blackstone_reward() {
    let mut session = session();
    let (uid, _) = give_pet_item(&mut session, 1);
    use_uid(&mut session, uid);
    let pet = session
        .world_snapshot()
        .stage5_systems
        .intelligent_creatures[0]
        .clone();
    session.handle_packet(ClientPacket::UpdateIntelligentCreature {
        creature: pet,
        summon_me: true,
        unsummon_me: false,
        release_me: false,
    });
    {
        let mut state = session
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>();
        let pet = &mut state.stage5_systems.intelligent_creatures[0];
        pet.blackstone_time = 10799;
        pet.maintain_food_time = 2;
    }
    session
        .app
        .world_mut()
        .remove_resource::<super::intelligent_creatures::CreatureClock>();
    let stone_index = mir2_game_data::crystal_item_manifest()
        .items
        .into_iter()
        .find(|t| t.name == "BlackCreatureStone")
        .unwrap()
        .item_index;
    let now = super::inventory::current_binary_datetime();
    let mut packets = Vec::new();
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 0, now, &mut packets);
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 1000, now, &mut packets);
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedItem { item } if item.item_index == stone_index)));
    let state = session.world_snapshot().stage5_systems;
    assert_eq!(state.intelligent_creatures[0].blackstone_time, 0);
    assert_eq!(state.intelligent_creatures[0].fullness, 7500);
    assert_eq!(state.intelligent_creatures[0].maintain_food_time, 1);
    packets.clear();
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 1000, now, &mut packets);
    assert!(packets.is_empty());
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 2000, now, &mut packets);
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .intelligent_creatures[0]
            .fullness,
        7499
    );
    session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .intelligent_creatures[0]
        .expire_binary_datetime = 1;
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 3000, now, &mut packets);
    assert!(session
        .world_snapshot()
        .stage5_systems
        .intelligent_creatures
        .is_empty());
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .summoned_intelligent_creature_type,
        Some(99)
    );
}

#[test]
fn creature_attack_operation_receipt_is_persisted_and_not_repeated_after_dismiss() {
    let mut session = session();
    session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .intelligent_creature_operations = 999;
    assert!(!session
        .record_shared_intelligent_creature_operation(0, "zone:spawn:attack:1")
        .is_empty());
    assert!(session
        .record_shared_intelligent_creature_operation(0, "zone:spawn:attack:1")
        .is_empty());
    let state = session.world_snapshot().stage5_systems;
    assert_eq!(state.intelligent_creature_pearls, 1);
    assert_eq!(state.intelligent_creature_operations, 0);
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session
        .record_shared_intelligent_creature_operation(0, "zone:spawn:attack:1")
        .is_empty());
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .intelligent_creature_pearls,
        1
    );
}

#[test]
fn creature_attack_wallet_and_receipt_rollback_together_on_save_failure() {
    let mut session = session();
    session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .intelligent_creature_operations = 999;
    session.save_active_character().unwrap();
    session
        .app
        .world()
        .resource::<super::resources::RuntimeConfigResource>()
        .config
        .inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(session
        .commit_shared_intelligent_creature_operation(0, "retry-operation")
        .is_err());
    let state = session.world_snapshot().stage5_systems;
    assert_eq!(state.intelligent_creature_pearls, 0);
    assert_eq!(state.intelligent_creature_operations, 999);
    assert!(!state
        .intelligent_creature_operation_receipts
        .contains("retry-operation"));
    session
        .commit_shared_intelligent_creature_operation(0, "retry-operation")
        .unwrap();
    session
        .commit_shared_intelligent_creature_operation(0, "retry-operation")
        .unwrap();
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .intelligent_creature_pearls,
        1
    );
}

#[test]
fn creature_template_handler_accepts_all_fifteen_original_pet_types() {
    let template = mir2_game_data::crystal_item_manifest()
        .items
        .into_iter()
        .find(|t| t.item_type == 36 && t.shape == 0)
        .unwrap();
    for kind in 0..=14 {
        let mut session = session();
        // Four types have no egg definition in the shipped Crystal DB. This fixture
        // exercises the server handler without inventing production item records.
        let mut fixture = template.clone();
        fixture.shape = kind;
        let result =
            super::intelligent_creatures::use_creature_template(session.app.world_mut(), &fixture)
                .unwrap();
        assert!(result.0);
        assert_eq!(
            session
                .world_snapshot()
                .stage5_systems
                .intelligent_creatures[0]
                .pet_type,
            kind as u8
        );
    }
}

#[test]
fn creature_blackstone_uses_mail_when_bag_is_full_and_does_not_repeat_same_second() {
    let mut session = session();
    let (uid, _) = give_pet_item(&mut session, 1);
    use_uid(&mut session, uid);
    {
        let mut resource = session
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>();
        resource.stage5_systems.summoned_intelligent_creature_type = Some(1);
        resource.stage5_systems.intelligent_creatures[0].blackstone_time = 10799;
    }
    let capacity = crate::config::crystal_bag_slot_capacity(
        session
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_capacity,
    );
    let egg = mir2_game_data::crystal_item_manifest()
        .items
        .into_iter()
        .find(|item| item.item_type == 36 && item.shape == 0)
        .unwrap();
    let egg_key = super::items::crystal_item_key_for_template(&egg);
    for slot in 0..capacity {
        if !session
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items
            .iter()
            .any(|item| item.container == ItemContainer::Bag1 && u16::from(item.slot) == slot)
        {
            super::inventory::add_or_increment_item(
                session.app.world_mut(),
                ItemContainer::Bag1,
                &egg_key,
                &egg.name,
                "",
                slot as u8,
                1,
                u16::from(egg.weight),
            );
        }
    }
    session
        .app
        .world_mut()
        .remove_resource::<super::intelligent_creatures::CreatureClock>();
    let now = super::inventory::current_binary_datetime();
    let before = session.world_snapshot().stage5_systems.mail.len();
    let mut packets = Vec::new();
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 0, now, &mut packets);
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 1000, now, &mut packets);
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 1000, now, &mut packets);
    let state = session.world_snapshot().stage5_systems;
    assert_eq!(state.mail.len(), before + 1);
    let mail = state.mail.last().unwrap();
    assert_eq!(mail.subject, "BlackStone");
    assert_eq!(mail.item_states_json.len(), 1);
    assert_eq!(state.intelligent_creatures[0].blackstone_time, 0);
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedItem { .. })));
    session.save_active_character().unwrap();
}

#[test]
fn creature_updates_subscription_controls_periodic_packets_without_stopping_maintenance() {
    let mut session = session();
    let (uid, _) = give_pet_item(&mut session, 1);
    use_uid(&mut session, uid);
    session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .summoned_intelligent_creature_type = Some(1);
    session
        .app
        .world_mut()
        .remove_resource::<super::intelligent_creatures::CreatureClock>();
    let now = super::inventory::current_binary_datetime();
    let mut packets = vec![];
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 0, now, &mut packets);
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 1000, now, &mut packets);
    assert!(packets.is_empty());
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .intelligent_creatures[0]
            .blackstone_time,
        1
    );
    session.handle_packet(ClientPacket::RequestIntelligentCreatureUpdates { update: true });
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 2000, now, &mut packets);
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::UpdateIntelligentCreatureList { .. })));
    packets.clear();
    assert!(session
        .handle_packet(ClientPacket::RequestIntelligentCreatureUpdates { update: false })
        .is_empty());
    super::intelligent_creatures::maintenance_at(session.app.world_mut(), 3000, now, &mut packets);
    assert!(packets.is_empty());
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .intelligent_creatures[0]
            .blackstone_time,
        3
    );
}

#[test]
fn creature_wonder_drug_uses_instance_stats_and_rejects_second_use_without_consumption() {
    use super::crystal_compat::CRYSTAL_STAT_HP;
    let mut session = session();
    let (uid, template) = give_pet_item(&mut session, 26);
    {
        let mut inv = session.app.world_mut().resource_mut::<InventoryResource>();
        let item = inv
            .inventory_items
            .iter_mut()
            .find(|item| item.unique_id == uid)
            .unwrap();
        item.added_stats = vec![mir2_protocol::UserItemStat {
            stat: CRYSTAL_STAT_HP,
            value: 123,
        }];
    }
    let hp_before = super::stats::player_stats(session.app.world()).max_hp();
    let packets = use_uid(&mut session, uid);
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::UseItem { success: true, .. })));
    let buff = session
        .app
        .world()
        .resource::<super::resources::BuffResource>()
        .buffs
        .iter()
        .find(|b| b.key == "wonder-drug")
        .unwrap()
        .clone();
    assert_eq!(
        buff.stats,
        vec![mir2_protocol::UserItemStat {
            stat: CRYSTAL_STAT_HP,
            value: 123
        }]
    );
    assert_eq!(
        buff.expires_at_tick - super::session::runtime_tick(session.app.world()),
        u64::from(template.durability) * 60
    );
    assert_eq!(
        super::stats::player_stats(session.app.world()).max_hp(),
        hp_before + 123
    );
    let (next_uid, _) = give_pet_item(&mut session, 26);
    let before = serde_json::to_value(
        &session
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items,
    )
    .unwrap();
    let packets = use_uid(&mut session, next_uid);
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::UseItem { success: false, .. })));
    assert_eq!(
        serde_json::to_value(
            &session
                .app
                .world()
                .resource::<InventoryResource>()
                .inventory_items
        )
        .unwrap(),
        before
    );
}

#[test]
fn creature_knapsack_stacks_duration_preserves_first_stats_and_consumes_once() {
    use super::crystal_compat::{CRYSTAL_STAT_BAG_WEIGHT, CRYSTAL_STAT_LUCK};
    let mut session = session();
    let (uid, template) = give_pet_item(&mut session, 28);
    let before_quantity = session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .find(|item| item.unique_id == uid)
        .unwrap()
        .quantity;
    use_uid(&mut session, uid);
    let first = session
        .app
        .world()
        .resource::<super::resources::BuffResource>()
        .buffs
        .iter()
        .find(|b| b.key == "knapsack")
        .unwrap()
        .clone();
    assert_eq!(
        first.stats,
        vec![mir2_protocol::UserItemStat {
            stat: CRYSTAL_STAT_BAG_WEIGHT,
            value: super::items::crystal_item_stat_value(&template, CRYSTAL_STAT_LUCK)
        }]
    );
    let after_quantity = session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .find(|item| item.unique_id == uid)
        .map_or(0, |i| i.quantity);
    assert_eq!(before_quantity - after_quantity, 1);
    let (next_uid, _) = give_pet_item(&mut session, 28);
    {
        let mut inv = session.app.world_mut().resource_mut::<InventoryResource>();
        inv.inventory_items
            .iter_mut()
            .find(|item| item.unique_id == next_uid)
            .unwrap()
            .added_stats = vec![mir2_protocol::UserItemStat {
            stat: CRYSTAL_STAT_LUCK,
            value: 50,
        }];
    }
    let packets = use_uid(&mut session, next_uid);
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::UseItem { success: true, .. })));
    let second = session
        .app
        .world()
        .resource::<super::resources::BuffResource>()
        .buffs
        .iter()
        .find(|b| b.key == "knapsack")
        .unwrap();
    assert_eq!(second.stats, first.stats);
    let expected = first.remaining_ms(0) + u64::from(template.durability) * 60_000;
    assert!(second.remaining_ms(0).abs_diff(expected) < 1000);
}

#[test]
fn creature_box_selection_uses_minimum_score_first_tie_and_shipped_tables() {
    let first = super::intelligent_creatures::select_creature_box_reward("00Strongbox", 1.0, |_| 0)
        .unwrap();
    assert_eq!(first.name, "WonderDrug(EXP)");
    let draws = [99, 99, 8, 0, 8, 8, 8, 99, 199, 299];
    let mut index = 0;
    let winner =
        super::intelligent_creatures::select_creature_box_reward("00Strongbox", 1.0, |upper| {
            let draw = draws[index];
            index += 1;
            assert!(draw < upper);
            draw
        })
        .unwrap();
    assert_eq!(index, 10);
    assert_eq!(winner.name, "WonderDrug(MP)");
    let mut index = 0;
    let scaled =
        super::intelligent_creatures::select_creature_box_reward("00Strongbox", 10.0, |_| {
            let draw = draws[index];
            index += 1;
            draw
        })
        .unwrap();
    assert_eq!(scaled.name, "WonderDrug(HP)");
    assert!(super::intelligent_creatures::select_creature_box_reward(
        "00Blackstone",
        1.0,
        |_| panic!("empty source table must not roll")
    )
    .is_none());
}

#[test]
fn creature_dynamic_wonder_drugs_preserve_all_seven_original_three_tier_values() {
    let template = mir2_game_data::crystal_item_by_name("WonderDrug(EXP)").unwrap();
    for (effect, stat, values) in [
        (0, 100, [5, 10, 20]),
        (1, 101, [10, 20, 50]),
        (2, 12, [50, 100, 200]),
        (3, 13, [50, 100, 200]),
        (4, 1, [1, 3, 5]),
        (5, 3, [1, 3, 5]),
        (6, 14, [2, 3, 4]),
    ] {
        for tier in 0..3 {
            let mut item =
                super::items::embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
            super::intelligent_creatures::apply_dynamic_wonder_drug(&mut item, effect, tier);
            assert_eq!(item.durability_current, Some(1));
            assert_eq!(
                item.added_stats,
                vec![mir2_protocol::UserItemStat {
                    stat,
                    value: values[tier as usize]
                }]
            );
        }
    }
}

#[test]
fn creature_strongbox_reuses_only_freed_slot_once_and_preserves_reward_on_relogin() {
    let mut session = session();
    let template = mir2_game_data::crystal_item_by_name("WonderBox(L)").unwrap();
    let source_uid = 80_000;
    {
        let mut inv = session.app.world_mut().resource_mut::<InventoryResource>();
        inv.inventory_items.clear();
        inv.belt_items.clear();
        let slots = super::inventory::empty_slots_for_inventory_container(
            &[],
            ItemContainer::Bag1,
            inv.inventory_capacity,
        );
        for (index, (container, slot)) in slots.into_iter().enumerate() {
            let mut item =
                super::items::embedded_item_state_from_template(&template, container, slot);
            item.unique_id = source_uid + index as u64;
            inv.inventory_items.push(item);
        }
        for slot in 0..6 {
            let mut item = super::items::embedded_item_state_from_template(
                &template,
                ItemContainer::Belt,
                slot,
            );
            item.unique_id = 90_000 + u64::from(slot);
            inv.belt_items.push(item);
        }
    }
    let count = session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .len();
    let packets = use_uid(&mut session, source_uid);
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::UseItem { success: true, .. }))
            .count(),
        1
    );
    let rewards = packets
        .iter()
        .filter_map(|p| {
            if let ServerPacket::GainedItem { item } = p {
                Some(item)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(rewards.len(), 1, "{packets:?}");
    let reward = rewards[0].clone();
    let inv = session.app.world().resource::<InventoryResource>();
    assert_eq!(inv.inventory_items.len(), count);
    assert!(inv
        .inventory_items
        .iter()
        .all(|i| i.unique_id != source_uid));
    let actual = inv
        .inventory_items
        .iter()
        .find(|i| i.unique_id == reward.unique_id)
        .unwrap();
    assert_eq!(actual.slot, 0);
    assert_eq!(
        super::items::try_user_item_from_item_state(actual).unwrap(),
        reward
    );
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let inv = session.app.world().resource::<InventoryResource>();
    let restored = inv
        .inventory_items
        .iter()
        .find(|i| i.unique_id == reward.unique_id)
        .unwrap();
    assert_eq!(
        super::items::try_user_item_from_item_state(restored).unwrap(),
        reward
    );
}

#[test]
fn creature_blackstone_ordinary_use_consumes_once_and_does_not_invent_rewards() {
    let mut session = session();
    let (uid, _) = give_pet_item(&mut session, 21);
    let before = session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .find(|i| i.unique_id == uid)
        .unwrap()
        .quantity;
    let packets = use_uid(&mut session, uid);
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::UseItem { success: true, .. }))
            .count(),
        1
    );
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedItem { .. })));
    let after = session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .find(|i| i.unique_id == uid)
        .map_or(0, |i| i.quantity);
    assert_eq!(before - after, 1);
}

#[test]
fn creature_real_duration_ignores_movement_ticks_and_expires_before_maintenance() {
    let mut session = session();
    let (uid, _) = give_pet_item(&mut session, 26);
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items
        .iter_mut()
        .find(|item| item.unique_id == uid)
        .unwrap()
        .added_stats = vec![mir2_protocol::UserItemStat {
        stat: 12,
        value: 123,
    }];
    let before = super::stats::player_stats(session.app.world()).max_hp();
    use_uid(&mut session, uid);
    for _ in 0..1000 {
        session.force_authoritative_player_transform(
            mir2_protocol::Point { x: 10, y: 10 },
            mir2_protocol::MirDirection::Up,
        );
    }
    assert_eq!(
        super::stats::player_stats(session.app.world()).max_hp(),
        before + 123
    );
    let buff = session
        .app
        .world()
        .resource::<super::resources::BuffResource>()
        .buffs
        .iter()
        .find(|b| b.key == "wonder-drug")
        .unwrap();
    assert!(buff.remaining_ms(u64::MAX) > 59_000);
    assert!(
        !buff
            .snapshot(u64::MAX, mir2_game_data::LanguageCode::English)
            .infinite
    );
    session
        .app
        .world_mut()
        .resource_mut::<super::resources::BuffResource>()
        .buffs
        .iter_mut()
        .find(|b| b.key == "wonder-drug")
        .unwrap()
        .real_time_duration
        .as_mut()
        .unwrap()
        .elapse_for_test(60_000);
    assert_eq!(
        super::stats::player_stats(session.app.world()).max_hp(),
        before
    );
    let mut packets = Vec::new();
    super::buffs::tick_buffs(session.app.world_mut(), &mut packets);
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::RemoveBuff { buff_type: 208, .. })));
}

#[test]
fn creature_real_duration_relogin_keeps_remaining_not_full_duration() {
    let mut session = session();
    let (uid, _) = give_pet_item(&mut session, 28);
    use_uid(&mut session, uid);
    session
        .app
        .world_mut()
        .resource_mut::<super::resources::BuffResource>()
        .buffs
        .iter_mut()
        .find(|b| b.key == "knapsack")
        .unwrap()
        .real_time_duration
        .as_mut()
        .unwrap()
        .elapse_for_test(120_000);
    let before = session
        .app
        .world()
        .resource::<super::resources::BuffResource>()
        .buffs
        .iter()
        .find(|b| b.key == "knapsack")
        .unwrap()
        .remaining_ms(0);
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let buff = session
        .app
        .world()
        .resource::<super::resources::BuffResource>()
        .buffs
        .iter()
        .find(|b| b.key == "knapsack")
        .unwrap();
    let remaining = buff.remaining_ms(0);
    assert!(remaining <= before && before - remaining < 2000);
    let packet = super::buffs::client_buff_for_state(session.app.world(), buff).unwrap();
    assert!(packet.expire_time as u64 <= before && before - (packet.expire_time as u64) < 2000);
}


#[test]
fn reward_rate_wonder_drug_changes_shared_experience_award_and_authoritative_drop_stats() {
    let mut session=session();
    super::leveling::apply_level_change(session.app.world_mut(), 20);
    let base=session.shared_monster_kill_experience_balance_delta(100);
    assert!(base>0);
    let (uid,_)=give_pet_item(&mut session,26);
    {
        let mut inv=session.app.world_mut().resource_mut::<InventoryResource>();
        inv.inventory_items.iter_mut().find(|i|i.unique_id==uid).unwrap().added_stats =
            vec![mir2_protocol::UserItemStat{stat:100,value:20},mir2_protocol::UserItemStat{stat:101,value:50}];
    }
    assert!(use_uid(&mut session,uid).iter().any(|p|matches!(p,ServerPacket::UseItem{success:true,..})));
    assert_eq!(session.shared_monster_kill_experience_balance_delta(100),base+base/5);
    assert_eq!(session.zone_player_combat_stats().item_drop_rate_percent,50);
    let award=session.commit_shared_monster_kill_award_transaction(990001,"Scarecrow",100);
    assert!(award.committed);
    let gained=award.packets.iter().find_map(|p|if let ServerPacket::GainExperience{amount}=p{Some(*amount)}else{None}).unwrap();
    assert_eq!(i64::from(gained),base+base/5);
    let before = session.app.world().resource::<super::resources::PlayerRuntimeResource>().experience;
    super::npc_script::crystal_npc_give_exp(session.app.world_mut(), &["100"]);
    let after = session.app.world().resource::<super::resources::PlayerRuntimeResource>().experience;
    assert_eq!(after - before, 120, "ordinary NPC XP uses the same active rate");
    session.app.world_mut().resource_mut::<super::resources::BuffResource>().buffs.iter_mut()
        .find(|buff| buff.key == "wonder-drug").unwrap().real_time_duration.as_mut().unwrap()
        .elapse_for_test(60_000);
    assert_eq!(session.zone_player_combat_stats().item_drop_rate_percent, 0);
    assert_eq!(session.shared_monster_kill_experience_balance_delta(100), base);
}

#[test]
fn creature_real_duration_world_snapshot_uses_clock_not_legacy_tick_deadline() {
    let mut session = session();
    let (uid, _) = give_pet_item(&mut session, 26);
    use_uid(&mut session, uid);
    {
        let mut buffs=session.app.world_mut().resource_mut::<super::resources::BuffResource>();
        let buff=buffs.buffs.iter_mut().find(|buff|buff.key=="wonder-drug").unwrap();
        buff.expires_at_tick=0;
    }
    assert!(session.world_snapshot().active_buffs.iter().any(|buff|buff.key=="wonder-drug"));
    {
        let mut buffs=session.app.world_mut().resource_mut::<super::resources::BuffResource>();
        let buff=buffs.buffs.iter_mut().find(|buff|buff.key=="wonder-drug").unwrap();
        buff.expires_at_tick=u64::MAX;
        buff.real_time_duration.as_mut().unwrap().elapse_for_test(60_000);
    }
    assert!(!session.world_snapshot().active_buffs.iter().any(|buff|buff.key=="wonder-drug"));
}
