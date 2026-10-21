//! Real packet/session boundary tests; no PostgreSQL, external transport or UI claim.
use super::*;
use crate::runtime::items::hunting_item_binding::HuntingItemBinding;
use crate::runtime::items::{
    embedded_item_state_from_template, try_item_state_from_user_item, try_user_item_from_item_state,
};
use crate::runtime::resources::{ActiveItemRentalState, ItemRentalResource};
use crate::runtime::session::{SharedTradeOffer, SharedTradeOfferItem};
use serde_json::{json, Value};

fn started() -> SimulationSession {
    let config = SimulationConfig::default();
    let mut session = SimulationSession::new(config);
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session.active_zone_join_snapshot("outflow-test").is_some());
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some("outflow-test-realm".into());
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation =
        Some("pg-character-v1:11111111-1111-4111-8111-111111111111".into());
    session
}
fn item(session: &SimulationSession, nested: bool, bound: bool) -> ItemState {
    let template = crystal_item_template_for_item_key("dagger").unwrap();
    let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, 8);
    item.unique_id = 91_001;
    if nested {
        let child = embedded_item_state_from_template(
            &mir2_game_data::crystal_item_by_name("BronzeBell").unwrap(),
            ItemContainer::Bag1,
            0,
        );
        let mut child_wire = try_user_item_from_item_state(&child).unwrap();
        child_wire.unique_id = 91_002;
        let mut wire = try_user_item_from_item_state(&item).unwrap();
        wire.slots = vec![None, Some(child_wire)];
        item = try_item_state_from_user_item(item, &wire).unwrap();
    }
    if bound {
        let binding = HuntingItemBinding::for_current_owner(
            session.app.world(),
            "quest-once-v1:2110001:reward:0",
        )
        .unwrap();
        if nested {
            item.socketed[0].economic_binding = Some(binding);
        } else {
            item.economic_binding = Some(binding);
        }
    }
    item
}
fn install(session: &mut SimulationSession, item: ItemState) {
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    inventory.inventory_items = vec![item];
    inventory.belt_items.clear();
    inventory.storage_items.clear();
    inventory.equipment_items.clear();
    inventory.reserved_item_unique_ids.clear();
}
fn census(session: &SimulationSession) -> Value {
    let world = session.app.world();
    let inv = world.resource::<InventoryResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    json!({"items":inv.inventory_items,"belt":inv.belt_items,"storage":inv.storage_items,
        "equipment":inv.equipment_items,"gold":world.resource::<PlayerRuntimeResource>().gold,
        "systems":world.resource::<Stage5SystemsResource>().stage5_systems,
        "store": &*config.account_store.lock().unwrap()})
}
fn switch_off(session: &mut SimulationSession) {
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = None;
}

#[test]
fn hunting_bound_outflow_trade_consign_drop_and_mail_reject_full_tree_when_disabled() {
    for nested in [false, true] {
        for action in 0..4 {
            let mut session = started();
            let source = item(&session, nested, true);
            install(&mut session, source);
            if action == 0 {
                session.trade_request("Partner");
            }
            switch_off(&mut session);
            let before = census(&session);
            let packet = match action {
                0 => ClientPacket::DepositTradeItem { from: 8, to: 0 },
                1 => ClientPacket::ConsignItem {
                    unique_id: 91_001,
                    price: 10,
                    market_type: 0,
                },
                2 => ClientPacket::DropItem {
                    unique_id: 91_001,
                    count: 1,
                    hero_inventory: false,
                },
                _ => ClientPacket::SendMail {
                    name: "Scout".into(),
                    message: "bound parcel".into(),
                    gold: 0,
                    items_idx: [91_001, 0, 0, 0, 0],
                    stamped: false,
                },
            };
            let replies = session.handle_packet(packet);
            assert!(
                !replies.iter().any(|p| matches!(
                    p,
                    ServerPacket::DepositTradeItem { success: true, .. }
                        | ServerPacket::ConsignItem { success: true, .. }
                        | ServerPacket::DropItem { success: true, .. }
                        | ServerPacket::MailSent { result: 1 }
                )),
                "{action}/{nested}: {replies:?}"
            );
            assert_eq!(census(&session), before, "{action}/{nested}");
        }
    }
}

#[test]
fn hunting_bound_outflow_legacy_trade_and_consignment_are_still_real_successes() {
    for action in [0, 1] {
        let mut session = started();
        let source = item(&session, false, false);
        install(&mut session, source);
        if action == 0 {
            session.trade_request("Partner");
        }
        switch_off(&mut session);
        let replies = session.handle_packet(if action == 0 {
            ClientPacket::DepositTradeItem { from: 8, to: 0 }
        } else {
            ClientPacket::ConsignItem {
                unique_id: 91_001,
                price: 10,
                market_type: 0,
            }
        });
        assert!(
            replies.iter().any(|p| matches!(
                p,
                ServerPacket::DepositTradeItem { success: true, .. }
                    | ServerPacket::ConsignItem { success: true, .. }
            )),
            "{action}: {replies:?}"
        );
    }
}

#[test]
fn hunting_bound_outflow_shared_trade_confirmation_rechecks_a_late_binding() {
    for nested in [false, true] {
        let mut session = started();
        let source = item(&session, nested, false);
        install(&mut session, source);
        session.trade_request("Partner");
        let accepted = session.handle_packet(ClientPacket::DepositTradeItem { from: 8, to: 0 });
        assert!(
            accepted
                .iter()
                .any(|p| matches!(p, ServerPacket::DepositTradeItem { success: true, .. })),
            "{accepted:?}"
        );
        let tagged = item(&session, nested, true);
        session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items[0] = tagged;
        switch_off(&mut session);
        let before = census(&session);
        let (_, offer) = session.shared_trade_confirm();
        assert!(offer.is_none());
        assert_eq!(census(&session), before);
    }
}

#[test]
fn hunting_bound_outflow_market_buy_and_sold_receipt_never_debit_or_mint_old_gold() {
    for nested in [false, true] {
        for sold in [false, true] {
            let mut session = started();
            let source = item(&session, nested, true);
            session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>()
                .inventory_items
                .clear();
            let seller = if sold {
                stage5_player_name(session.app.world())
            } else {
                "Other Seller".into()
            };
            session
                .app
                .world_mut()
                .resource_mut::<Stage5SystemsResource>()
                .stage5_systems
                .auction
                .push(Stage5AuctionListing {
                    item_state_json: Some(serde_json::to_string(&source).unwrap()),
                    id: 501,
                    seller,
                    item_key: source.key.clone(),
                    price: 10,
                    currency: crate::config::CurrencyKind::Gold,
                    sold,
                    cancelled: false,
                    expired: false,
                });
            switch_off(&mut session);
            let before = census(&session);
            let replies = session.handle_packet(if sold {
                ClientPacket::MarketGetBack {
                    mode: 0,
                    auction_id: 501,
                }
            } else {
                ClientPacket::MarketBuy {
                    auction_id: 501,
                    bid_price: 0,
                }
            });
            assert!(
                replies
                    .iter()
                    .any(|p| matches!(p, ServerPacket::MarketFail { .. })),
                "{replies:?}"
            );
            assert_eq!(census(&session), before);
        }
    }
}

#[test]
fn hunting_bound_outflow_rental_deposit_and_final_confirm_leave_both_custodians_intact() {
    for nested in [false, true] {
        for final_check in [false, true] {
            let mut session = started();
            let source = item(&session, nested, true);
            install(&mut session, source.clone());
            session
                .app
                .world_mut()
                .resource_mut::<ItemRentalResource>()
                .active = Some(ActiveItemRentalState {
                transaction_nonce: "a".repeat(32),
                partner_name: "Partner".into(),
                fee: 10,
                days: 1,
                deposited_item: final_check.then_some(source),
                deposited_from: Some(8),
                gold_locked: final_check,
                item_locked: final_check,
            });
            if final_check {
                session
                    .app
                    .world_mut()
                    .resource_mut::<InventoryResource>()
                    .inventory_items
                    .clear();
            }
            switch_off(&mut session);
            let before = census(&session);
            let held_before = format!("{:?}", session.app.world().resource::<ItemRentalResource>());
            if final_check {
                crate::runtime::rental::confirm_item_rental_impl(session.app.world_mut());
            } else {
                crate::runtime::rental::deposit_rental_item_impl(session.app.world_mut(), 8, 0);
            }
            assert_eq!(census(&session), before);
            assert_eq!(
                format!("{:?}", session.app.world().resource::<ItemRentalResource>()),
                held_before
            );
        }
    }
}

#[test]
fn hunting_bound_outflow_final_shared_projection_rejects_either_full_offer_before_gold() {
    for nested in [false, true] {
        for incoming_bound in [false, true] {
            let mut session = started();
            let tagged = item(&session, nested, true);
            install(&mut session, tagged.clone());
            let (account_id, character_index, name) = {
                let s = session.app.world().resource::<SessionResource>();
                (
                    s.account_id.clone().unwrap(),
                    s.selected_character.as_ref().unwrap().index,
                    s.selected_character.as_ref().unwrap().name.clone(),
                )
            };
            let entry = SharedTradeOfferItem {
                item_state_json: serde_json::to_string(&tagged).unwrap(),
                key: tagged.key.clone(),
                unique_id: tagged.unique_id,
            };
            let own = SharedTradeOffer {
                settlement_nonce: "a".repeat(32),
                account_id,
                character_index,
                character_name: name.clone(),
                partner_name: "Partner".into(),
                gold: 3,
                items: if incoming_bound {
                    vec![]
                } else {
                    vec![entry.clone()]
                },
            };
            let incoming = SharedTradeOffer {
                settlement_nonce: "b".repeat(32),
                account_id: "partner".into(),
                character_index: 0,
                character_name: "Partner".into(),
                partner_name: name,
                gold: 17,
                items: if incoming_bound { vec![entry] } else { vec![] },
            };
            switch_off(&mut session);
            let before = census(&session);
            let error = session
                .apply_shared_trade_settlement_projection(&"c".repeat(64), &own, &incoming)
                .unwrap_err();
            assert!(error.contains("bound custody"), "{error}");
            assert_eq!(census(&session), before);
        }
    }
}

#[test]
fn hunting_bound_outflow_guild_packets_reject_deposit_and_withdrawal_before_both_sides_change() {
    for nested in [false, true] {
        for withdraw in [false, true] {
            let mut session = started();
            let source = item(&session, nested, true);
            switch_off(&mut session);
            session.stage5_command("guild.create", vec!["BoundGuard".into()]);
            session
                .app
                .world_mut()
                .resource_mut::<RuntimeConfigResource>()
                .config
                .hunting_economy_realm = Some("outflow-test-realm".into());
            assert!(stage5_guild_player_in_safe_zone(session.app.world()));
            assert!(stage5_current_guild_has_option(
                session.app.world(),
                GUILD_OPTION_CAN_STORE_ITEM
            ));
            install(&mut session, source.clone());
            if withdraw {
                session
                    .app
                    .world_mut()
                    .resource_mut::<InventoryResource>()
                    .inventory_items
                    .clear();
                let mut systems = session
                    .app
                    .world_mut()
                    .resource_mut::<Stage5SystemsResource>();
                let guild = &mut systems.stage5_systems.guild;
                guild.storage_items.insert(0, source.key.clone());
                guild
                    .storage_item_states
                    .insert(0, serde_json::to_string(&source).unwrap());
                guild.storage_item_users.insert(0, 0);
            }
            switch_off(&mut session);
            let before = census(&session);
            let replies = session.handle_packet(ClientPacket::GuildStorageItemChange {
                change_type: if withdraw { 1 } else { 0 },
                from: if withdraw { 0 } else { 8 },
                to: if withdraw { 8 } else { 0 },
            });
            assert!(
                !replies.iter().any(|p| matches!(
                    p,
                    ServerPacket::GuildStorageItemChange {
                        change_type: 0,
                        item: Some(_),
                        ..
                    } | ServerPacket::GuildStorageItemChange {
                        change_type: 1,
                        item: None,
                        ..
                    }
                )),
                "{replies:?}"
            );
            assert_eq!(census(&session), before);
        }
    }
}

#[test]
fn hunting_bound_outflow_hero_transfer_preserves_same_owner_and_rejects_new_incarnation() {
    for nested in [false, true] {
        let mut session = started();
        switch_off(&mut session);
        session.stage5_command("hero.recruit", vec!["Aide".into()]);
        session
            .app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .hunting_economy_realm = Some("outflow-test-realm".into());
        assert!(stage5_hero_ready_for_inventory(session.app.world()));
        let source = item(&session, nested, true);
        install(&mut session, source.clone());
        let replies = session.handle_packet(ClientPacket::TransferHeroItem { from: 8, to: 2 });
        assert!(
            replies
                .iter()
                .any(|p| matches!(p, ServerPacket::TransferHeroItem { success: true, .. })),
            "{replies:?}"
        );
        let mut expected = source.clone();
        expected.slot = 2;
        assert_eq!(
            serde_json::to_value(
                &session
                    .app
                    .world()
                    .resource::<HeroInventoryResource>()
                    .items[0]
            )
            .unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        let replies = session.handle_packet(ClientPacket::TakeBackHeroItem { from: 2, to: 8 });
        assert!(
            replies
                .iter()
                .any(|p| matches!(p, ServerPacket::TakeBackHeroItem { success: true, .. })),
            "{replies:?}"
        );
        assert_eq!(
            serde_json::to_value(
                &session
                    .app
                    .world()
                    .resource::<InventoryResource>()
                    .inventory_items[0]
            )
            .unwrap(),
            serde_json::to_value(&source).unwrap()
        );
        session
            .app
            .world_mut()
            .resource_mut::<SessionResource>()
            .hunting_character_incarnation =
            Some("pg-character-v1:33333333-3333-4333-8333-333333333333".into());
        let before = census(&session);
        let hero = format!(
            "{:?}",
            session.app.world().resource::<HeroInventoryResource>()
        );
        assert!(session
            .handle_packet(ClientPacket::TransferHeroItem { from: 8, to: 2 })
            .iter()
            .any(|p| matches!(p, ServerPacket::TransferHeroItem { success: false, .. })));
        assert_eq!(census(&session), before);
        assert_eq!(
            format!(
                "{:?}",
                session.app.world().resource::<HeroInventoryResource>()
            ),
            hero
        );
    }
}

fn fishing_fixture() -> SimulationSession {
    let mut session = started();
    let world = session.app.world();
    let map = world.resource::<MapRuntimeResource>();
    let player = player_entity(world).unwrap();
    let origin = map
        .fishing_cells
        .keys()
        .find_map(|&(x, y)| {
            [
                (Point { x, y: y + 3 }, MirDirection::Up),
                (Point { x: x - 3, y }, MirDirection::Right),
                (Point { x, y: y - 3 }, MirDirection::Down),
                (Point { x: x + 3, y }, MirDirection::Left),
            ]
            .into_iter()
            .find(|(point, _)| {
                crate::runtime::movement::can_occupy(world, point.clone(), Some(player))
            })
        })
        .expect("real fishable map cell");
    session.force_authoritative_player_transform(origin.0, origin.1);
    let mut rod = embedded_item_state_from_template(
        &mir2_game_data::crystal_item_by_name("BlueFishingRod").unwrap(),
        ItemContainer::Bag1,
        8,
    );
    rod.unique_id = 94_000;
    // The raw Crystal template declares zero ordinary sockets. Canonical
    // fishing custody has five attachment positions; hydrate the whole rod
    // before converting it to equipment, keeping its live width and sidecar
    // consistent instead of injecting a half-normalized legacy carrier.
    let rod_wire = try_user_item_from_item_state(&rod).unwrap();
    assert_eq!(rod_wire.slots.len(), 5);
    rod = try_item_state_from_user_item(rod, &rod_wire).unwrap();
    assert_eq!(rod.socket_slots, 5);
    assert_eq!(try_user_item_from_item_state(&rod).unwrap(), rod_wire);
    install(&mut session, rod.clone());
    {
        let mut inv = session.app.world_mut().resource_mut::<InventoryResource>();
        inv.inventory_items.clear();
        inv.equipment_items
            .push(crate::runtime::equipment::equipment_state_from_item_state(
                &rod,
                EquipmentSlot::Weapon,
            ));
    }
    for (slot, name, uid, quantity) in [(0, "FishingHook", 94_001, 1), (2, "FishBait", 94_002, 3)] {
        let mut input = embedded_item_state_from_template(
            &mir2_game_data::crystal_item_by_name(name).unwrap(),
            ItemContainer::Bag1,
            slot,
        );
        input.unique_id = uid;
        input.quantity = quantity;
        session
            .app
            .world_mut()
            .resource_mut::<FishingResource>()
            .slot_items[usize::from(slot)] = Some(input);
    }
    assert!(crate::runtime::fishing::persist_fishing_slots_to_equipment(
        session.app.world_mut()
    ));
    // The authoritative fixture move changed AOI since StartGame. The normal
    // dispatcher finalizes even an empty fishing rejection by emitting that
    // pending visibility delta. Synchronize it before capturing the rejection
    // baseline, using an ordinary packet rather than changing the AOI cache.
    let synchronized = session.handle_packet(ClientPacket::KeepAlive { time: 0 });
    assert!(
        synchronized
            .iter()
            .any(|packet| matches!(packet, ServerPacket::KeepAlive { time: 0 })),
        "{synchronized:?}"
    );
    assert_eq!(
        session.visible_objects,
        collect_visible_objects(session.app.world())
            .keys()
            .copied()
            .collect::<BTreeSet<_>>()
    );
    session
}

#[test]
fn hunting_bound_outflow_fishing_cast_and_late_reel_never_consume_or_strip_bound_inputs() {
    // First prove the exact rod, attachments and fishable position can cast.
    let mut control = fishing_fixture();
    assert!(control
        .handle_packet(ClientPacket::FishingCast { cast_out: true })
        .iter()
        .any(|p| matches!(p, ServerPacket::FishingUpdate { fishing: true, .. })));
    for _ in 0..4 {
        crate::runtime::fishing::tick_fishing(control.app.world_mut(), &mut Vec::new());
    }
    assert!(control.app.world().resource::<FishingResource>().found_fish);
    assert!(control
        .handle_packet(ClientPacket::FishingCast { cast_out: false })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::FishingUpdate { fishing: false, .. })));
    for child in [false, true] {
        for late in [false, true] {
            let mut session = fishing_fixture();
            if late {
                assert!(session
                    .handle_packet(ClientPacket::FishingCast { cast_out: true })
                    .iter()
                    .any(|p| matches!(p, ServerPacket::FishingUpdate { fishing: true, .. })));
                // Reach the actual reel-ready phase through authoritative ticks
                // before introducing the late binding; pulling now would settle.
                for _ in 0..4 {
                    crate::runtime::fishing::tick_fishing(session.app.world_mut(), &mut Vec::new());
                }
                assert!(session.app.world().resource::<FishingResource>().found_fish);
            }
            let binding = HuntingItemBinding::for_current_owner(
                session.app.world(),
                "quest-once-v1:2110001:reward:0",
            )
            .unwrap();
            if child {
                session
                    .app
                    .world_mut()
                    .resource_mut::<InventoryResource>()
                    .equipment_items[0]
                    .socketed[0]
                    .economic_binding = Some(binding.clone());
                session
                    .app
                    .world_mut()
                    .resource_mut::<FishingResource>()
                    .slot_items[0]
                    .as_mut()
                    .unwrap()
                    .economic_binding = Some(binding);
            } else {
                session
                    .app
                    .world_mut()
                    .resource_mut::<InventoryResource>()
                    .equipment_items[0]
                    .economic_binding = Some(binding);
            }
            switch_off(&mut session);
            let before = census(&session);
            let fishing = format!("{:?}", session.app.world().resource::<FishingResource>());
            let cast_replies = session.handle_packet(ClientPacket::FishingCast { cast_out: !late });
            assert!(
                cast_replies.is_empty(),
                "bound cast/reel child={child} late={late}: {cast_replies:?}"
            );
            let autocast_replies =
                session.handle_packet(ClientPacket::FishingChangeAutocast { auto_cast: true });
            assert!(
                autocast_replies.is_empty(),
                "bound autocast child={child} late={late}: {autocast_replies:?}"
            );
            let mut packets = vec![];
            crate::runtime::fishing::tick_fishing(session.app.world_mut(), &mut packets);
            assert!(packets.is_empty());
            assert_eq!(census(&session), before);
            assert_eq!(
                format!("{:?}", session.app.world().resource::<FishingResource>()),
                fishing
            );
        }
    }
}

#[test]
fn hunting_bound_outflow_death_penalty_keeps_bound_inventory_and_equipment_trees() {
    for nested in [false, true] {
        for equipped in [false, true] {
            let mut session = started();
            let source = item(&session, nested, true);
            install(&mut session, source.clone());
            if equipped {
                let mut inv = session.app.world_mut().resource_mut::<InventoryResource>();
                inv.inventory_items.clear();
                inv.equipment_items.push(
                    crate::runtime::equipment::equipment_state_from_item_state(
                        &source,
                        EquipmentSlot::Weapon,
                    ),
                );
            }
            let player = player_entity(session.app.world()).unwrap();
            session
                .app
                .world_mut()
                .entity_mut(player)
                .get_mut::<crate::runtime::components::PlayerVitals>()
                .unwrap()
                .hp = 0;
            session
                .app
                .world_mut()
                .resource_mut::<PlayerRuntimeResource>()
                .pk_points = 300;
            switch_off(&mut session);
            let before = census(&session);
            let drop_count = session
                .app
                .world()
                .iter_entities()
                .filter(|entity| entity.contains::<crate::runtime::components::GroundDrop>())
                .count();
            assert!(
                crate::runtime::drops::drop_player_death_penalty(session.app.world_mut())
                    .is_empty()
            );
            assert_eq!(census(&session), before);
            assert_eq!(
                session
                    .app
                    .world()
                    .iter_entities()
                    .filter(|entity| entity.contains::<crate::runtime::components::GroundDrop>())
                    .count(),
                drop_count
            );
        }
    }
}
