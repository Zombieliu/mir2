//! Isolated ordinary GatewaySession adapter evidence, without sockets or a
//! native/public client. Accounts, learned level-three magic, carried supplies
//! and a real TownTeleport bind destination are prepared. Every Curse,
//! Purification and map change is produced by normal client packets. Poison
//! transfer is exercised by the ordinary Manager test. The ordinary hostile
//! Player powder preflight regressions retain the original RED logs 05–10.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use mir2_gateway::{
    GatewayConfig, GatewaySession, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRegistry,
};
use mir2_protocol::{
    ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point, ServerPacket, Spell,
};
use mir2_simulation::{
    AccountRecord, CharacterBindPoint, CharacterRecord, EquipmentSlot, ItemContainer,
    MonsterSpawnSource, SafeZoneRecord,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SOURCE: &str = "purification-normal-adapter";
const DESTINATION: &str = "0";
const PASSWORD: &str = "isolated-purification-adapter-password";

fn account(name: &str) -> String {
    format!("purification-adapter-{name}")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn item(name: &str, slot: u8, quantity: u32) -> String {
    let item = mir2_game_data::crystal_item_by_name(name).unwrap();
    json!({"key":format!("crystal-item-{}",item.item_index),"name":item.name,
        "icon":item.image,"slot":slot,"unique_id":u64::from(slot)+1,
        "container":ItemContainer::Bag1,"quantity":quantity,"description":"",
        "durability_current":item.durability.max(1),"durability_max":item.durability.max(1),
        "weight":item.weight,"equip_slot":null,"attack":0,"defence":0,"heal_hp":0,"heal_mp":0})
    .to_string()
}

fn fixture() -> GatewayConfig {
    let mut config = GatewayConfig::default().with_crystal_world_runtime();
    config.monster_spawn_source = MonsterSpawnSource::StarterScenario;
    config.map.file_name = SOURCE.into();
    config.map.title = "Purification normal adapter prepared map".into();
    config.spawn = Point { x: 10, y: 10 };
    config.map_collision.map_file_name = SOURCE.into();
    config.map_collision.map_width = 40;
    config.map_collision.map_height = 40;
    config.map_collision.play_bounds = mir2_game_data::MapBounds {
        min_x: 0,
        max_x: 39,
        min_y: 0,
        max_y: 39,
    };
    config.map_collision.region_bounds = config.map_collision.play_bounds.clone();
    config.map_collision.blocked_cells.clear();
    config.map_collision.doors.clear();
    config.visible_players.clear();
    config.visible_monsters.clear();
    config.visible_npcs.clear();
    config.map_transfers.clear();
    config.safe_zones.clear();
    let amulet = mir2_game_data::crystal_item_by_name("Amulet").unwrap();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for (index, name, x) in [(1, "healer", 10), (2, "friend", 12), (3, "enemy", 14)] {
            let mut record = AccountRecord::new(CharacterRecord {
                index,
                name: format!("PuriAdapter{name}"),
                level: 50,
                class: MirClass::Taoist,
                gender: MirGender::Male,
            });
            record.password = PASSWORD.into();
            let save = record.saves.get_mut(&index).unwrap();
            save.map_file_name = SOURCE.into();
            save.map_title = config.map.title.clone();
            save.position = Point { x, y: 10 };
            save.hp = 1_000;
            save.max_hp = 1_000;
            save.mp = 1_000;
            save.max_mp = 1_000;
            save.bind_point = Some(CharacterBindPoint {
                // Actual imported Bichon safe-zone bind point, as in log16.
                map_file_name: DESTINATION.into(),
                position: Point { x: 345, y: 362 },
            });
            save.inventory_items_json =
                vec![item("TownTeleport", 0, 2), item("GreenPoison", 1, 100)];
            save.equipment_items_json = vec![json!({"key":format!("crystal-item-{}",amulet.item_index),
                "slot":EquipmentSlot::Amulet,"quantity":100,"name":amulet.name,"icon":amulet.image,
                "shape":amulet.shape,"description":"","durability_current":amulet.durability.max(1),
                "durability_max":amulet.durability.max(1),"attack":0,"defence":0,"weight":amulet.weight}).to_string()];
            save.skill_states_json = ["purification", "curse", "poisoning", "soulfireball"]
                .into_iter()
                .map(|key| {
                    json!({"key":key,"name":key,"description":"","level":3,
                    "experience":0,"cooldown_ticks":0,"cooldown_ends_at":0})
                    .to_string()
                })
                .collect();
            store.accounts.insert(account(name), record);
        }
    }
    config
}

fn login(
    config: &GatewayConfig,
    registry: &ZoneRegistry,
    name: &str,
    index: i32,
) -> (GatewaySession, u32) {
    let mut session = GatewaySession::new_with_zone_registry(config.clone(), registry);
    let packets = session
        .try_handle_packet(ClientPacket::Login {
            account_id: account(name),
            password: PASSWORD.into(),
        })
        .unwrap();
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    let packets = session
        .try_handle_packet(ClientPacket::StartGame {
            character_index: index,
        })
        .unwrap();
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::UserInformation { .. })),
        "{packets:?}"
    );
    let owner = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::UserInformation { info } => Some(info.object_id),
            _ => None,
        })
        .unwrap();
    (session, owner)
}

fn zone_player(factory: &SharedInProcessZoneRuntimeFactory, name: &str) -> (String, Value) {
    let checkpoint: Value =
        serde_json::from_slice(&factory.zone_checkpoint_bytes(&ZoneId::primary()).unwrap())
            .unwrap();
    for state in checkpoint["zones"].as_object().unwrap().values() {
        let manager: Value = serde_json::from_slice(
            &STANDARD
                .decode(state["zoneManagerBytes"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        for entry in manager["zones"].as_array().unwrap() {
            let zone: Value =
                serde_json::from_slice(&STANDARD.decode(entry[1].as_str().unwrap()).unwrap())
                    .unwrap();
            for player in zone["players"].as_object().unwrap().values() {
                if player["account_id"] == account(name) {
                    return (
                        entry[0]["map_file_name"].as_str().unwrap().into(),
                        player.clone(),
                    );
                }
            }
        }
    }
    panic!("ordinary authenticated actor missing from live shared Zone");
}

fn magic(
    session: &mut GatewaySession,
    owner: u32,
    spell: Spell,
    target_id: u32,
) -> Vec<ServerPacket> {
    magic_at(session, owner, spell, target_id, Point { x: 12, y: 10 })
}

fn magic_at(
    session: &mut GatewaySession,
    owner: u32,
    spell: Spell,
    target_id: u32,
    location: Point,
) -> Vec<ServerPacket> {
    session
        .try_handle_packet(ClientPacket::Magic {
            object_id: owner,
            spell,
            direction: MirDirection::Right,
            target_id,
            location,
            spell_target_lock: false,
        })
        .unwrap()
}

fn powder_quantity(config: &GatewayConfig) -> u64 {
    let store = config.account_store.lock().unwrap();
    let save = &store.accounts[&account("enemy")].saves[&3];
    save.inventory_items_json
        .iter()
        .map(|raw| serde_json::from_str::<Value>(raw).unwrap())
        .find(|item| item["name"] == "GreenPoison")
        .unwrap()["quantity"]
        .as_u64()
        .unwrap()
}

fn carried_powder(session: &GatewaySession) -> u32 {
    session
        .world_snapshot()
        .inventory_items
        .iter()
        .find(|item| item.name == "GreenPoison")
        .unwrap()
        .quantity
}

fn logout_and_fresh_powder(config: &GatewayConfig, enemy: &mut GatewaySession, expected: u32) {
    let packets = enemy.try_handle_packet(ClientPacket::LogOut).unwrap();
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LogOutSuccess { .. })));
    assert_eq!(powder_quantity(config), u64::from(expected));
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
    let registry = ZoneRegistry::new(ZoneId::primary(), factory);
    let (mut fresh, _) = login(config, &registry, "enemy", 3);
    assert_eq!(carried_powder(&fresh), expected);
    let packets = fresh.try_handle_packet(ClientPacket::LogOut).unwrap();
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LogOutSuccess { .. })));
}

fn assert_rejected_player_resources(before: &Value, after: &Value, label: &str) {
    assert!(
        after["mp"].as_i64().unwrap() >= before["mp"].as_i64().unwrap(),
        "{label}: rejected cast cannot debit MP; independent regeneration may increase it"
    );
    for field in [
        "magic_ready_at_ms",
        "next_spell_ready_at_ms",
        "movement_ready_at_ms",
        "next_attack_ready_at_ms",
    ] {
        assert_eq!(after[field], before[field], "{label}: {field}");
    }
}

fn assert_no_poison_debit(
    config: &GatewayConfig,
    enemy: &mut GatewaySession,
    packets: &[ServerPacket],
    before_store: u64,
    before_carried: u32,
    label: &str,
) {
    assert!(!admitted(packets, Spell::Poisoning), "{label}: {packets:?}");
    assert!(
        !packets
            .iter()
            .any(|p| matches!(p, ServerPacket::DeleteItem { unique_id: 2, .. })),
        "{label}: {packets:?}"
    );
    assert_eq!(
        powder_quantity(config),
        before_store,
        "{label} cannot change the previously saved powder stack"
    );
    assert_eq!(
        carried_powder(enemy),
        before_carried,
        "{label} cannot consume the private powder stack"
    );
}

#[test]
fn normal_gateway_hostile_player_poisoning_consumes_backpack_powder_once_and_rejects_cooldown() {
    let config = fixture();
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
        Duration::from_millis(25),
        BTreeMap::new(),
    ));
    let registry = ZoneRegistry::new(ZoneId::primary(), factory.clone());
    let (mut friend, _) = login(&config, &registry, "friend", 2);
    let (mut enemy, enemy_id) = login(&config, &registry, "enemy", 3);
    enemy
        .try_handle_packet(ClientPacket::ChangeAMode { mode: 5 })
        .unwrap();
    let target_id = zone_player(&factory, "friend").1["object_id"]
        .as_u64()
        .unwrap() as u32;
    let before = powder_quantity(&config);
    let cast = magic(&mut enemy, enemy_id, Spell::Poisoning, target_id);
    assert!(admitted(&cast, Spell::Poisoning), "{cast:?}");
    assert_eq!(
        cast.iter()
            .filter(|p| matches!(
                p,
                ServerPacket::DeleteItem {
                    unique_id: 2,
                    count: 1
                }
            ))
            .count(),
        1,
        "one actual carried powder debit: {cast:?}"
    );
    assert_eq!(carried_powder(&enemy), before as u32 - 1);
    // This default InProcess adapter saves on normal LogOut. The ordinary
    // cast response proves live debit, not a before-ACK durable Source CAS.
    let saved_before_repeat = powder_quantity(&config);
    let resources_before_repeat = zone_player(&factory, "enemy").1;
    let repeat = magic(&mut enemy, enemy_id, Spell::Poisoning, target_id);
    assert_no_poison_debit(
        &config,
        &mut enemy,
        &repeat,
        saved_before_repeat,
        before as u32 - 1,
        "per-spell/action cooldown",
    );
    assert_rejected_player_resources(
        &resources_before_repeat,
        &zone_player(&factory, "enemy").1,
        "per-spell/action cooldown",
    );
    wait_until(|| {
        friend.tick();
        zone_player(&factory, "friend").1["poison"]
            .as_u64()
            .unwrap()
            & 1
            != 0
    });
    logout_and_fresh_powder(&config, &mut enemy, before as u32 - 1);
}

#[test]
fn normal_gateway_poisoning_refuses_safe_friend_stale_point_range_and_mp_without_powder_debit() {
    for rejected in [
        "safe_target",
        "safe_caster",
        "peace",
        "group",
        "point",
        "range",
        "mp",
    ] {
        let mut config = fixture();
        match rejected {
            "safe_target" | "safe_caster" => {
                let x = if rejected == "safe_target" { 12 } else { 14 };
                config.safe_zones.push(SafeZoneRecord {
                    map_file_name: SOURCE.into(),
                    bounds: mir2_game_data::MapBounds {
                        min_x: x,
                        max_x: x,
                        min_y: 10,
                        max_y: 10,
                    },
                });
            }
            "range" => {
                config
                    .account_store
                    .lock()
                    .unwrap()
                    .accounts
                    .get_mut(&account("friend"))
                    .unwrap()
                    .saves
                    .get_mut(&2)
                    .unwrap()
                    .position
                    .x = 35
            }
            "mp" => {
                config
                    .account_store
                    .lock()
                    .unwrap()
                    .accounts
                    .get_mut(&account("enemy"))
                    .unwrap()
                    .saves
                    .get_mut(&3)
                    .unwrap()
                    .mp = 0
            }
            _ => {}
        }
        let factory = Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_millis(25),
            BTreeMap::new(),
        ));
        let registry = ZoneRegistry::new(ZoneId::primary(), factory.clone());
        let (mut friend, _) = login(&config, &registry, "friend", 2);
        let (mut enemy, enemy_id) = login(&config, &registry, "enemy", 3);
        let mode = if rejected == "peace" {
            0
        } else if rejected == "group" {
            1
        } else {
            5
        };
        enemy
            .try_handle_packet(ClientPacket::ChangeAMode { mode })
            .unwrap();
        if rejected == "group" {
            enemy
                .try_handle_packet(ClientPacket::AddMember {
                    name: "PuriAdapterfriend".into(),
                })
                .unwrap();
            assert!(friend
                .tick()
                .iter()
                .any(|p| matches!(p, ServerPacket::GroupInvite { .. })));
            friend
                .try_handle_packet(ClientPacket::GroupInvite {
                    accept_invite: true,
                })
                .unwrap();
            enemy.tick();
            assert!(
                zone_player(&factory, "friend").1["chat_profile"]["group_members"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|n| n == "PuriAdapterenemy")
            );
        }
        let target = zone_player(&factory, "friend").1;
        if rejected == "safe_target" {
            assert_eq!(target["chat_profile"]["in_safe_zone"], true);
        }
        if rejected == "safe_caster" {
            assert_eq!(
                zone_player(&factory, "enemy").1["chat_profile"]["in_safe_zone"],
                true
            );
        }
        if rejected == "mp" {
            assert_eq!(zone_player(&factory, "enemy").1["mp"], 0);
        }
        let mut point: Point = serde_json::from_value(target["position"].clone()).unwrap();
        if rejected == "point" {
            point.x += 1;
        }
        let before = powder_quantity(&config);
        let resources_before = zone_player(&factory, "enemy").1;
        let result = magic_at(
            &mut enemy,
            enemy_id,
            Spell::Poisoning,
            target["object_id"].as_u64().unwrap() as u32,
            point,
        );
        assert_no_poison_debit(
            &config,
            &mut enemy,
            &result,
            before,
            before as u32,
            rejected,
        );
        assert_rejected_player_resources(
            &resources_before,
            &zone_player(&factory, "enemy").1,
            rejected,
        );
        assert_eq!(zone_player(&factory, "friend").1["poison"], 0, "{rejected}");
        logout_and_fresh_powder(&config, &mut enemy, before as u32);
    }
}

#[test]
fn normal_gateway_poisoning_refuses_the_actual_native_dead_player_without_powder_debit() {
    let config = fixture();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut(&account("friend"))
        .unwrap()
        .saves
        .get_mut(&2)
        .unwrap()
        .hp = 1;
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
        Duration::from_millis(25),
        BTreeMap::new(),
    ));
    let registry = ZoneRegistry::new(ZoneId::primary(), factory.clone());
    let (mut friend, _) = login(&config, &registry, "friend", 2);
    let (mut enemy, enemy_id) = login(&config, &registry, "enemy", 3);
    enemy
        .try_handle_packet(ClientPacket::ChangeAMode { mode: 5 })
        .unwrap();
    let target_id = zone_player(&factory, "friend").1["object_id"]
        .as_u64()
        .unwrap() as u32;
    assert!(admitted(
        &magic(&mut enemy, enemy_id, Spell::SoulFireBall, target_id),
        Spell::SoulFireBall
    ));
    wait_until(|| {
        friend.tick();
        let target = zone_player(&factory, "friend").1;
        target["dead"] == true && target["hp"] == 0
    });
    // Wait the independent caster cooldown, so actual target death is the gate.
    std::thread::sleep(Duration::from_millis(1_850));
    let before = powder_quantity(&config);
    let resources_before = zone_player(&factory, "enemy").1;
    let rejected = magic(&mut enemy, enemy_id, Spell::Poisoning, target_id);
    assert_no_poison_debit(
        &config,
        &mut enemy,
        &rejected,
        before,
        before as u32,
        "actual native dead target",
    );
    assert_rejected_player_resources(
        &resources_before,
        &zone_player(&factory, "enemy").1,
        "actual native dead target",
    );
    logout_and_fresh_powder(&config, &mut enemy, before as u32);
}

fn admitted(packets: &[ServerPacket], spell: Spell) -> bool {
    packets.iter().any(|p| match p {
        ServerPacket::Magic {
            spell: actual,
            cast: true,
            ..
        }
        | ServerPacket::ObjectMagic {
            spell: actual,
            cast: true,
            ..
        } => *actual == spell,
        _ => false,
    })
}

fn wait_until(mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !check() {
        assert!(
            Instant::now() < deadline,
            "normal shared source failed its bounded completion"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn town_scroll_unique_id(session: &GatewaySession) -> u64 {
    let template = mir2_game_data::crystal_item_by_name("TownTeleport").unwrap();
    session
        .world_snapshot()
        .inventory_items
        .iter()
        .find(|item| item.key == format!("crystal-item-{}", template.item_index))
        .unwrap()
        .unique_id
}

fn town_scroll(session: &mut GatewaySession, unique_id: u64) -> Vec<ServerPacket> {
    let packets = session
        .try_handle_packet(ClientPacket::UseItem {
            unique_id,
            grid: MirGridType::Inventory,
        })
        .unwrap();
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::UseItem { success: true, .. })),
        "{packets:?}"
    );
    assert!(
        packets.iter().any(
            |p| matches!(p, ServerPacket::MapInformation { info } if info.file_name == DESTINATION)
        ),
        "{packets:?}"
    );
    packets
}

#[test]
fn normal_gateway_purification_survives_real_town_scroll_map_changes_in_both_actor_orders() {
    for target_first in [false, true] {
        let config = fixture();
        let factory = Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_millis(25),
            BTreeMap::new(),
        ));
        let registry = ZoneRegistry::new(ZoneId::primary(), factory.clone());
        let (mut healer, healer_id) = login(&config, &registry, "healer", 1);
        let (mut friend, _) = login(&config, &registry, "friend", 2);
        let (mut enemy, enemy_id) = login(&config, &registry, "enemy", 3);
        let attack_mode = enemy
            .try_handle_packet(ClientPacket::ChangeAMode { mode: 5 })
            .unwrap();
        let (friend_map, live_friend) = zone_player(&factory, "friend");
        let (enemy_map, live_enemy) = zone_player(&factory, "enemy");
        assert_eq!(friend_map, SOURCE);
        assert_eq!(enemy_map, SOURCE);
        assert_eq!(live_friend["position"], json!({"x":12,"y":10}));
        assert_eq!(live_enemy["position"], json!({"x":14,"y":10}));
        assert_eq!(
            live_enemy["chat_profile"]["attack_mode"], 5,
            "{attack_mode:?}"
        );
        let target_id = live_friend["object_id"].as_u64().unwrap() as u32;
        let mut cursed = false;
        for _ in 0..12 {
            // Preserve the independent original SpellTime and skill cooldown.
            std::thread::sleep(Duration::from_millis(1_850));
            let packets = magic(&mut enemy, enemy_id, Spell::Curse, 0);
            assert!(
                admitted(&packets, Spell::Curse),
                "normal Curse rejected: {packets:?}"
            );
            std::thread::sleep(Duration::from_millis(550));
            friend.tick();
            if zone_player(&factory, "friend").1["buffs"]
                .get("12")
                .is_some()
            {
                cursed = true;
                break;
            }
        }
        assert!(
            cursed,
            "normal native Curse did not land in bounded source attempts"
        );
        let original = zone_player(&factory, "friend").1;
        let original_healer = zone_player(&factory, "healer").1;
        let healer_scroll_id = town_scroll_unique_id(&healer);
        let friend_scroll_id = town_scroll_unique_id(&friend);
        assert!(admitted(
            &magic(&mut healer, healer_id, Spell::Purification, target_id),
            Spell::Purification
        ));
        let action_clock = zone_player(&factory, "healer").1["movement_ready_at_ms"]
            .as_u64()
            .unwrap();
        let completion_deadline = action_clock - 100;
        let before_scrolls = now_ms();
        let between_scrolls;
        if target_first {
            town_scroll(&mut friend, friend_scroll_id);
            between_scrolls = now_ms();
            town_scroll(&mut healer, healer_scroll_id);
        } else {
            town_scroll(&mut healer, healer_scroll_id);
            between_scrolls = now_ms();
            town_scroll(&mut friend, friend_scroll_id);
        }
        let after_scrolls = now_ms();
        assert!(after_scrolls < completion_deadline,
            "normal scroll chain missed the real +500ms boundary: targetFirst={target_first}, deadline={completion_deadline}, before={before_scrolls}, between={between_scrolls}, after={after_scrolls}, target={:?}, caster={:?}",
            zone_player(&factory, "friend"), zone_player(&factory, "healer"));
        let (map, current) = zone_player(&factory, "friend");
        assert_eq!(map, DESTINATION);
        assert_eq!(current["object_id"], original["object_id"]);
        assert_eq!(current["life_generation"], original["life_generation"]);
        assert_eq!(
            current["buffs"]["12"], original["buffs"]["12"],
            "Join cannot lose or renew actual Curse"
        );
        let (map, current_healer) = zone_player(&factory, "healer");
        assert_eq!(map, DESTINATION);
        assert_eq!(current_healer["object_id"], original_healer["object_id"]);
        assert_eq!(
            current_healer["life_generation"],
            original_healer["life_generation"]
        );
        let mut completion_packets = Vec::new();
        wait_until(|| {
            completion_packets.extend(friend.tick());
            let current = zone_player(&factory, "friend").1;
            current["buffs"].get("12").is_none()
        });
        assert!(now_ms() >= completion_deadline,
            "actual state disappeared before native completion: targetFirst={target_first}, deadline={completion_deadline}, afterScrolls={after_scrolls}, originalCurse={:?}, packets={completion_packets:?}", original["buffs"]["12"]);
        assert_eq!(completion_packets.iter().filter(|p| matches!(p,
            ServerPacket::RemoveBuff { buff_type: 12, .. })).count(), 1,
            "ordinary destination owner must receive exactly one actual removal: {completion_packets:?}");
        let settled = friend.tick();
        assert!(
            !settled
                .iter()
                .any(|p| matches!(p, ServerPacket::RemoveBuff { buff_type: 12, .. })),
            "completed removal was consumed in wait loop; no repeated source completion"
        );
        for session in [&mut healer, &mut friend, &mut enemy] {
            let logged_out = session.try_handle_packet(ClientPacket::LogOut).unwrap();
            assert!(logged_out
                .iter()
                .any(|p| matches!(p, ServerPacket::LogOutSuccess { .. })));
        }
        eprintln!(
            "normal purification scroll chain completed: targetFirst={target_first}, deadline={completion_deadline}, before={before_scrolls}, between={between_scrolls}, after={after_scrolls}"
        );
    }
}


#[test]
fn normal_gateway_private_ticks_cannot_expire_authoritative_curse() {
    let config = fixture();
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
        Duration::from_millis(25), BTreeMap::new(),
    ));
    let registry = ZoneRegistry::new(ZoneId::primary(), factory.clone());
    let (mut friend, _) = login(&config, &registry, "friend", 2);
    let (mut enemy, enemy_id) = login(&config, &registry, "enemy", 3);
    enemy.try_handle_packet(ClientPacket::ChangeAMode { mode: 5 }).unwrap();
    let mut cursed = false;
    for _ in 0..12 {
        std::thread::sleep(Duration::from_millis(1_850));
        assert!(admitted(&magic(&mut enemy, enemy_id, Spell::Curse, 0), Spell::Curse));
        std::thread::sleep(Duration::from_millis(550));
        friend.tick();
        if zone_player(&factory, "friend").1["buffs"].get("12").is_some() {
            cursed = true;
            break;
        }
    }
    assert!(cursed, "ordinary native Curse did not land in bounded source attempts");
    let original = zone_player(&factory, "friend").1;
    let expires = original["buffs"]["12"]["expires_at_ms"].as_u64().unwrap();
    let started = now_ms();
    let mut packets = Vec::new();
    for _ in 0..40 {
        packets.extend(friend.tick());
    }
    let completed = now_ms();
    let current = zone_player(&factory, "friend").1;
    eprintln!("ordinary private tick clock: calls=40 elapsedMs={} actualCurseExpiry={expires} ended={completed}",
        completed.saturating_sub(started));
    assert!(completed < expires, "the actual pre-expiry interval must be observed");
    assert_eq!(current["buffs"]["12"], original["buffs"]["12"],
        "private ticks must neither clear nor renew actual Zone Curse");
    assert!(!packets.iter().any(|packet| matches!(packet,
        ServerPacket::RemoveBuff { buff_type: 12, .. })),
        "personal tick expiry cannot echo an early native removal: {packets:?}");
    for session in [&mut friend, &mut enemy] {
        assert!(session.try_handle_packet(ClientPacket::LogOut).unwrap().iter()
            .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    }
}
