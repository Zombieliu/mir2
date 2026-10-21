//! NPC exchange planning and ordinary SELL boundaries, with no external services.
use super::*;
use crate::runtime::items::hunting_item_binding::HuntingItemBinding;
use serde_json::{json, Value};

fn session() -> SimulationSession {
    let mut config = crate::SimulationConfig::default();
    config.visible_npcs.push(crate::VisibleNpcRecord {
        object_id: 4990,
        name: "Wicked Trader".into(),
        image: 5,
        colour_argb: -1,
        position: Point { x: 331, y: 271 },
        direction: MirDirection::Left,
        quest_ids: vec![],
        script_key: Some("BichonProvince/NaturalCave/WickedTrader".into()),
    });
    let mut s = SimulationSession::new(config);
    s.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    s.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(s.active_zone_join_snapshot("npc-bound-test").is_some());
    s.app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some("npc-bound-test-realm".into());
    s.app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation =
        Some("pg-character-v1:22222222-2222-4222-8222-222222222222".into());
    s.force_authoritative_player_transform(Point { x: 331, y: 271 }, MirDirection::Right);
    s
}
fn item(s: &SimulationSession, key: &str, uid: u64, quantity: u32, bound: bool) -> ItemState {
    let mut item = embedded_item_state_from_template(
        &crystal_item_template_for_item_key(key).unwrap(),
        ItemContainer::Bag1,
        8,
    );
    item.key = key.into();
    item.unique_id = uid;
    item.quantity = quantity;
    if bound {
        item.economic_binding = Some(
            HuntingItemBinding::for_current_owner(s.app.world(), "quest-once-v1:2110001:reward:0")
                .unwrap(),
        );
    }
    item
}
fn inventory(s: &mut SimulationSession, items: Vec<ItemState>) {
    let mut inv = s.app.world_mut().resource_mut::<InventoryResource>();
    inv.inventory_items = items;
    inv.belt_items.clear();
    inv.storage_items.clear();
    inv.equipment_items.clear();
    inv.reserved_item_unique_ids.clear();
}
fn census(s: &SimulationSession) -> Value {
    let world = s.app.world();
    let inv = world.resource::<InventoryResource>();
    let npc = world.resource::<NpcStateResource>();
    json!({"items":inv.inventory_items,"belt":inv.belt_items,"gold":world.resource::<PlayerRuntimeResource>().gold,
        "credit":world.resource::<PlayerRuntimeResource>().credit,"buyback":npc.npc_buy_back_items,"used":npc.npc_used_goods_items,
        "systems":world.resource::<Stage5SystemsResource>().stage5_systems})
}
fn section(label: &str, lines: &[&str]) -> CrystalNpcSection {
    CrystalNpcSection {
        label: label.into(),
        line_number: 0,
        lines: lines.iter().map(|s| (*s).into()).collect(),
    }
}
fn script(sections: Vec<CrystalNpcSection>) -> CrystalNpcScript {
    CrystalNpcScript {
        script_key: "binding-exchange-fixture".into(),
        relative_path: "fixture".into(),
        raw_text: String::new(),
        lines: vec![],
        line_count: 0,
        non_empty_line_count: 0,
        label_count: sections.len(),
        insert_count: 0,
        command_directives: vec![],
        labels: vec![],
        sections,
        inserts: vec![],
    }
}
fn context() -> NpcInteractionContext {
    NpcInteractionContext {
        object_id: 4990,
        name: "Trader".into(),
        name_key: None,
        position: Point { x: 331, y: 271 },
        quest_ids: vec![],
        script_key: None,
        args: vec![],
        input: None,
    }
}

#[test]
fn hunting_bound_npc_exchange_preflight_rejects_cross_goto_before_any_gold_or_legacy_input() {
    let mut s = session();
    let legacy = item(&s, "dagger", 92_001, 1, false);
    let bound = item(&s, "red-potion", 92_002, 2, true);
    inventory(&mut s, vec![legacy, bound]);
    // Tags outlive the producer feature and must not become legacy input on disable.
    s.app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = None;
    let plan = script(vec![
        section(
            "@exchange",
            &["#ACT", "TAKEGOLD 10", "TAKEITEM dagger 1", "GOTO @finish"],
        ),
        section(
            "@finish",
            &[
                "#ACT",
                "GIVEGOLD 500",
                "TAKEITEM red-potion 1",
                "GIVEITEM dagger 1",
            ],
        ),
    ]);
    let before = census(&s);
    let result =
        run_crystal_npc_script_impl(s.app.world_mut(), &context(), &plan, "@exchange").unwrap();
    assert!(result.packets.is_empty());
    assert_eq!(census(&s), before);
    // Merely talking to the same NPC is still available with a Bound bag.
    let talk = script(vec![section("@hello", &["#SAY", "Hello adventurer."])]);
    assert!(
        run_crystal_npc_script_impl(s.app.world_mut(), &context(), &talk, "@hello")
            .unwrap()
            .dialog
            .is_some()
    );
    assert_eq!(census(&s), before);
}

#[test]
fn hunting_bound_npc_take_is_atomic_and_false_stops_the_following_give() {
    let mut s = session();
    let legacy = item(&s, "red-potion", 92_003, 2, false);
    inventory(&mut s, vec![legacy]);
    let before = census(&s);
    assert!(!crystal_npc_take_item(
        s.app.world_mut(),
        &["red-potion", "3"]
    ));
    assert_eq!(census(&s), before);
    let result = execute_crystal_npc_section(
        s.app.world_mut(),
        &section("@bad", &["#ACT", "TAKEITEM red-potion 3", "GIVEGOLD 999"]),
        &CrystalNpcContextState {
            npc_position: Point { x: 331, y: 271 },
        },
        &mut CrystalNpcExecutionState::default(),
    );
    assert!(result.close_dialog);
    assert!(result.packets.is_empty());
    assert_eq!(census(&s), before);
    assert!(crystal_npc_take_item(
        s.app.world_mut(),
        &["red-potion", "1"]
    ));
    assert_eq!(
        s.app
            .world()
            .resource::<InventoryResource>()
            .inventory_items[0]
            .quantity,
        1
    );
}

#[test]
fn hunting_bound_npc_ordinary_sell_never_pays_gold_or_loses_custody() {
    for kind in 0..3 {
        let bound = kind != 0;
        let mut s = session();
        let npc = entity_by_object_id(s.app.world(), 4990).expect("configured Wicked Trader");
        let npc_position = entity_position(s.app.world(), npc).unwrap();
        assert_eq!(
            s.app
                .world()
                .entity(npc)
                .get::<NpcAgent>()
                .unwrap()
                .script_key
                .as_deref(),
            Some("BichonProvince/NaturalCave/WickedTrader")
        );
        // Ordinary interaction rejects occupying the NPC's own tile. Stand
        // beside the real configured merchant, within its data range.
        s.force_authoritative_player_transform(
            Point {
                x: npc_position.x - 1,
                y: npc_position.y,
            },
            MirDirection::Right,
        );
        assert!(crystal_npc_object_in_data_range(s.app.world(), 4990));
        let dialog = s.interact(4990);
        assert_eq!(
            s.app
                .world()
                .resource::<NpcStateResource>()
                .active_npc_dialog
                .as_ref()
                .map(|dialog| dialog.npc_object_id),
            Some(4990),
            "{dialog:?}"
        );
        let opened = s.select_npc_dialog_target("@BuySell");
        assert!(
            opened
                .iter()
                .any(|packet| matches!(packet, ServerPacket::NPCSell)),
            "{opened:?}"
        );
        assert!(
            current_crystal_npc_service_in_range(s.app.world()).is_some_and(|service| {
                active_crystal_sell_service(&service)
                    && service.npc_object_id == 4990
                    && service.script_key == "BichonProvince/NaturalCave/WickedTrader"
            }),
            "{opened:?}"
        );
        let mut source = item(&s, "dagger", 92_004, 1, false);
        if kind == 1 {
            source.economic_binding = Some(
                HuntingItemBinding::for_current_owner(
                    s.app.world(),
                    "quest-once-v1:2110001:reward:0",
                )
                .unwrap(),
            );
        } else if kind == 2 {
            let mut child = embedded_item_state_from_template(
                &mir2_game_data::crystal_item_by_name("BronzeBell").unwrap(),
                ItemContainer::Bag1,
                0,
            );
            child.unique_id = 92_007;
            let mut wire = try_user_item_from_item_state(&source).unwrap();
            wire.slots = vec![None, Some(try_user_item_from_item_state(&child).unwrap())];
            source = try_item_state_from_user_item(source, &wire).unwrap();
            source.socketed[0].economic_binding = Some(
                HuntingItemBinding::for_current_owner(
                    s.app.world(),
                    "quest-once-v1:2110001:reward:0",
                )
                .unwrap(),
            );
        }
        inventory(&mut s, vec![source]);
        s.app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .hunting_economy_realm = None;
        let before = census(&s);
        let replies = s.handle_packet(ClientPacket::SellItem {
            unique_id: 92_004,
            count: 1,
        });
        let success = replies
            .iter()
            .any(|p| matches!(p, ServerPacket::SellItem { success: true, .. }));
        assert_eq!(success, !bound, "{replies:?}");
        if bound {
            assert_eq!(census(&s), before);
        }
    }
}

#[test]
fn hunting_bound_npc_dynamic_command_cannot_hide_a_later_take_or_goto() {
    for action in [
        "MOV A0 TAKEITEM\nGIVEGOLD 500\n%A0 red-potion 1",
        "GIVEGOLD 500\nMOV A0 GOTO\n%A0 @finish",
    ] {
        let mut s = session();
        let bound = item(&s, "red-potion", 92_005, 2, true);
        inventory(&mut s, vec![bound]);
        let mut lines = vec!["#ACT"];
        lines.extend(action.split('\n'));
        let plan = script(vec![
            section("@exchange", &lines),
            section("@finish", &["#ACT", "TAKEITEM red-potion 1"]),
        ]);
        let before = census(&s);
        let result =
            run_crystal_npc_script_impl(s.app.world_mut(), &context(), &plan, "@exchange").unwrap();
        assert!(result.packets.is_empty());
        assert_eq!(census(&s), before);
    }
}

#[test]
fn hunting_bound_npc_seal_preflight_rejects_hero_custody_before_prior_gold_action() {
    for equipment in [false, true] {
        let mut s = session();
        s.app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .hunting_economy_realm = None;
        s.stage5_command("hero.recruit", vec!["Aide".into()]);
        s.app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .hunting_economy_realm = Some("npc-bound-test-realm".into());
        let bound = item(&s, "dagger", 92_006, 1, true);
        {
            let mut hero = s.app.world_mut().resource_mut::<HeroInventoryResource>();
            if equipment {
                hero.equipment = vec![bound];
            } else {
                hero.items = vec![bound];
            }
        }
        let plan = script(vec![section(
            "@seal",
            &["#ACT", "GIVEGOLD 500", "SEALHERO"],
        )]);
        let before = census(&s);
        let hero = format!("{:?}", s.app.world().resource::<HeroInventoryResource>());
        let result =
            run_crystal_npc_script_impl(s.app.world_mut(), &context(), &plan, "@seal").unwrap();
        assert!(result.packets.is_empty());
        assert_eq!(census(&s), before);
        crystal_npc_seal_hero(s.app.world_mut());
        assert_eq!(census(&s), before);
        assert_eq!(
            format!("{:?}", s.app.world().resource::<HeroInventoryResource>()),
            hero
        );
    }
}
