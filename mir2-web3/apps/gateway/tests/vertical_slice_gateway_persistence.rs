//! Gateway-layer evidence for the ordinary Bichon vertical slice.
//!
//! The richer combat, quest, drop, pickup, and reward sequence is exercised
//! without privileged commands by `mir2-simulation`'s
//! `ordinary_candidate_loop` integration test.  This companion test owns the
//! missing boundary: a fresh account and character must traverse the Gateway
//! command route, save through logout, then reload through a newly constructed
//! Gateway session.  It deliberately uses only normal Crystal client packets.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::{GatewayConfig, GatewaySession};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    GroundDropLootSnapshot, ItemContainer, QuestStage, WorldEntityKind, WorldEntitySnapshot,
};

const TEST_RECOVERY_MAC_KEY: [u8; 32] = [
    0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed, 0xfe, 0x0f,
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xf1, 0x02,
];

struct SaveFileGuard(PathBuf);

impl Drop for SaveFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn unique_save_path() -> SaveFileGuard {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    SaveFileGuard(std::env::temp_dir().join(format!(
        "mir2-gateway-vertical-slice-{}-{nanos}.json",
        std::process::id()
    )))
}

fn file_backed_gateway_config(path: PathBuf) -> GatewayConfig {
    GatewayConfig::default()
        .with_account_store_path(path)
        .with_save_recovery_mac_key(TEST_RECOVERY_MAC_KEY)
        .expect("test-only file store must have a valid recovery MAC key")
}

fn login(session: &mut GatewaySession, account_id: &str, password: &str) {
    let packets = session
        .try_handle_packet(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: password.to_string(),
        })
        .expect("Gateway Login should execute");
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "Gateway Login should succeed: {packets:?}"
    );
}

fn player(session: &GatewaySession) -> mir2_simulation::WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("Gateway StartGame should expose the self player")
}

fn nearby_npc(session: &GatewaySession, name: &str) -> WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::Npc && entity.name == name)
        .unwrap_or_else(|| panic!("Gateway StartGame should expose NPC {name}"))
}

fn direction_toward(from: &Point, to: &Point) -> MirDirection {
    match ((to.x - from.x).signum(), (to.y - from.y).signum()) {
        (0, -1) => MirDirection::Up,
        (1, -1) => MirDirection::UpRight,
        (1, 0) => MirDirection::Right,
        (1, 1) => MirDirection::DownRight,
        (0, 1) => MirDirection::Down,
        (-1, 1) => MirDirection::DownLeft,
        (-1, 0) => MirDirection::Left,
        (-1, -1) => MirDirection::UpLeft,
        _ => MirDirection::Down,
    }
}

fn tile_distance(left: &Point, right: &Point) -> i32 {
    (left.x - right.x).abs().max((left.y - right.y).abs())
}

fn tick_after_client_action(session: &mut GatewaySession) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    for _ in 0..8 {
        packets.extend(session.tick());
    }
    packets
}

fn walk_within(session: &mut GatewaySession, target: &Point, maximum_distance: i32) {
    for _ in 0..80 {
        let current = player(session);
        let current_point = Point {
            x: current.x,
            y: current.y,
        };
        if tile_distance(&current_point, target) <= maximum_distance {
            return;
        }

        let horizontal = if target.x > current.x {
            Some(MirDirection::Right)
        } else if target.x < current.x {
            Some(MirDirection::Left)
        } else {
            None
        };
        let vertical = if target.y > current.y {
            Some(MirDirection::Down)
        } else if target.y < current.y {
            Some(MirDirection::Up)
        } else {
            None
        };

        let before = (current.x, current.y);
        for direction in [horizontal, vertical].into_iter().flatten() {
            session.handle_packet(ClientPacket::Walk { direction });
            tick_after_client_action(session);
            let after = player(session);
            if (after.x, after.y) != before {
                break;
            }
        }
    }

    let current = player(session);
    let current_point = Point {
        x: current.x,
        y: current.y,
    };
    assert!(
        tile_distance(&current_point, target) <= maximum_distance,
        "Gateway Walk path could not reach required distance: current={current_point:?}, target={target:?}, maximum_distance={maximum_distance}"
    );
}

fn walk_toward(session: &mut GatewaySession, target: &Point) {
    walk_within(session, target, 1);
}

fn walk_onto(session: &mut GatewaySession, target: &Point) {
    walk_within(session, target, 0);
}

#[test]
fn gateway_fresh_account_bichon_logout_and_new_session_reload_are_authoritative() {
    let save_guard = unique_save_path();
    let save_path = save_guard.0.clone();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let account_id = format!("gateway_slice_{}_{}", std::process::id(), suffix);
    let password = "GatewaySlice42!";

    let mut first = GatewaySession::new(file_backed_gateway_config(save_path.clone()));
    let created = first
        .try_handle_packet(ClientPacket::NewAccount {
            account_id: account_id.clone(),
            password: password.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        })
        .expect("Gateway NewAccount should execute");
    assert!(
        created
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })),
        "Gateway NewAccount should create an ordinary account: {created:?}"
    );
    login(&mut first, &account_id, password);

    let created_character = first.handle_packet(ClientPacket::NewCharacter {
        name: format!("GateSlice{}", std::process::id()),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let character_index = created_character
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Gateway NewCharacter should succeed: {created_character:?}"));

    let started = first.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        started.iter().any(|packet| matches!(
            packet,
            ServerPacket::StartGame {
                result: 4,
                resolution
            } if *resolution > 0
        )),
        "Gateway StartGame should enter Bichon: {started:?}"
    );
    let initial = first.world_snapshot();
    assert_eq!(initial.map_file_name.as_deref(), Some("0"));
    assert!(initial
        .map_title
        .as_deref()
        .is_some_and(|title| title.to_ascii_lowercase().contains("bichon")));

    let player_before = player(&first);
    let turned = first.handle_packet(ClientPacket::Turn {
        direction: MirDirection::Left,
    });
    assert!(
        turned
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })),
        "Gateway Turn must reach the authoritative Zone: {turned:?}"
    );
    let player_after = player(&first);
    assert_eq!(player_after.direction, MirDirection::Left);
    assert_eq!(
        (player_after.x, player_after.y),
        (player_before.x, player_before.y),
        "a turn must not manufacture movement"
    );

    let logout = first.handle_packet(ClientPacket::LogOut);
    assert!(
        logout
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })),
        "Gateway LogOut should save then acknowledge: {logout:?}"
    );
    drop(first);

    let mut second = GatewaySession::new(file_backed_gateway_config(save_path.clone()));
    login(&mut second, &account_id, password);
    let restarted = second.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        restarted
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "new Gateway session should restore its selected character: {restarted:?}"
    );
    let reloaded = player(&second);
    assert_eq!(
        (reloaded.x, reloaded.y, reloaded.direction),
        (player_after.x, player_after.y, player_after.direction),
        "a new Gateway session must reload the saved authoritative transform"
    );

    drop(second);
    drop(save_guard);
    assert!(
        !save_path.exists(),
        "Gateway evidence save file should be cleaned up"
    );
}

#[test]
fn gateway_ordinary_bichon_starter_loop_uses_client_packets_and_reloads() {
    let save_guard = unique_save_path();
    let save_path = save_guard.0.clone();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let account_id = format!("gateway_ordinary_{}_{}", std::process::id(), suffix);
    let password = "GatewayOrdinary42!";

    let mut first = GatewaySession::new(file_backed_gateway_config(save_path.clone()));
    let created = first
        .try_handle_packet(ClientPacket::NewAccount {
            account_id: account_id.clone(),
            password: password.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        })
        .expect("Gateway NewAccount should execute");
    assert!(
        created
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })),
        "Gateway NewAccount should create an ordinary account: {created:?}"
    );
    login(&mut first, &account_id, password);

    let created_character = first.handle_packet(ClientPacket::NewCharacter {
        name: format!("GateOrdinary{}", std::process::id()),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let character_index = created_character
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Gateway NewCharacter should succeed: {created_character:?}"));
    let started = first.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        started
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "Gateway StartGame should enter Bichon: {started:?}"
    );

    let remote_accept = first.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 4001,
        quest_index: 1001,
    });
    assert!(
        !remote_accept.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 1001,
                taken: true,
                ..
            }
        )),
        "Gateway must reject AcceptQuest without a nearby active dialog: {remote_accept:?}"
    );

    let initial_player = player(&first);
    let guide = nearby_npc(&first, "Village Guide");
    walk_toward(
        &mut first,
        &Point {
            x: guide.x,
            y: guide.y,
        },
    );
    let after_walk = player(&first);
    assert_ne!(
        (after_walk.x, after_walk.y),
        (initial_player.x, initial_player.y),
        "ordinary Gateway Walk packets must change the authoritative transform"
    );

    let opened = first.handle_packet(ClientPacket::CallNpc {
        object_id: guide.object_id,
        key: "@Main".to_string(),
    });
    assert!(
        opened.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectChat { object_id, .. } if *object_id == guide.object_id
        )) && first
            .world_snapshot()
            .active_npc_dialog
            .as_ref()
            .is_some_and(|dialog| dialog
                .links
                .iter()
                .any(|link| link.target == "@AcceptQuest:1001")),
        "ordinary Gateway CallNpc must expose the starter quest link: {opened:?}"
    );

    let accepted = first.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 4001,
        quest_index: 1001,
    });
    assert!(
        accepted.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 1001,
                taken: true,
                ..
            }
        )),
        "ordinary Gateway AcceptQuest packet must start the offered quest: {accepted:?}"
    );
    assert!(first
        .world_snapshot()
        .quest_log
        .iter()
        .any(|quest| { quest.quest_id == 1001 && quest.stage == QuestStage::InProgress }));

    let mut combat_packets = Vec::new();
    for _ in 0..180 {
        let Some(wasp) = first.world_snapshot().entities.into_iter().find(|entity| {
            entity.kind == WorldEntityKind::Monster
                && entity.name == "Field Wasp"
                && !entity.dead
                && entity.hp.unwrap_or(1) > 0
        }) else {
            break;
        };
        walk_toward(
            &mut first,
            &Point {
                x: wasp.x,
                y: wasp.y,
            },
        );
        let Some(wasp) = first.world_snapshot().entities.into_iter().find(|entity| {
            entity.object_id == wasp.object_id && !entity.dead && entity.hp.unwrap_or(1) > 0
        }) else {
            combat_packets.extend(tick_after_client_action(&mut first));
            continue;
        };
        let current = player(&first);
        let direction = direction_toward(
            &Point {
                x: current.x,
                y: current.y,
            },
            &Point {
                x: wasp.x,
                y: wasp.y,
            },
        );
        combat_packets.extend(first.handle_packet(ClientPacket::Turn { direction }));
        combat_packets.extend(first.handle_packet(ClientPacket::Attack {
            direction,
            spell: Spell::None,
        }));
        combat_packets.extend(tick_after_client_action(&mut first));
        if first
            .world_snapshot()
            .quest_log
            .iter()
            .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::ReadyToTurnIn)
        {
            break;
        }
    }

    let after_combat = first.world_snapshot();
    assert!(
        after_combat.quest_log.iter().any(|quest| {
            quest.quest_id == 1001 && quest.stage == QuestStage::ReadyToTurnIn
        }),
        "ordinary Gateway Attack packets must make the starter quest ready; packets={combat_packets:?}"
    );
    assert!(
        after_combat.inventory_items.iter().any(|item| {
            item.container == ItemContainer::Quest && item.key == "crystal-item-876"
        }),
        "the player-owned Field Wasp death must grant the quest proof"
    );

    let wasp_gold = after_combat
        .ground_drops
        .iter()
        .find_map(|drop| match drop.loot {
            GroundDropLootSnapshot::Gold { amount } if drop.source_monster == "Field Wasp" => {
                Some((
                    drop.object_id,
                    Point {
                        x: drop.x,
                        y: drop.y,
                    },
                    amount,
                ))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Field Wasp must create a visible gold drop: {after_combat:?}"));
    let gold_before_pickup = after_combat.gold;
    walk_onto(&mut first, &wasp_gold.1);
    let pickup = first.handle_packet(ClientPacket::PickUp);
    let after_pickup = first.world_snapshot();
    assert!(pickup.iter().any(|packet| matches!(
        packet,
        ServerPacket::GainedGold { gold } if *gold == wasp_gold.2
    )));
    assert_eq!(after_pickup.gold, gold_before_pickup + wasp_gold.2);
    assert!(!after_pickup
        .ground_drops
        .iter()
        .any(|drop| drop.object_id == wasp_gold.0));

    let remote_finish = first.handle_packet(ClientPacket::FinishQuest {
        quest_index: 1001,
        selected_item_index: -1,
    });
    assert!(
        !remote_finish.iter().any(|packet| matches!(
            packet,
            ServerPacket::CompleteQuest { completed_quests }
                if completed_quests.contains(&1001)
        )),
        "Gateway must reject FinishQuest away from an active finish dialog"
    );

    walk_toward(
        &mut first,
        &Point {
            x: guide.x,
            y: guide.y,
        },
    );
    let opened_finish = first.handle_packet(ClientPacket::CallNpc {
        object_id: guide.object_id,
        key: "@Main".to_string(),
    });
    assert!(
        first
            .world_snapshot()
            .active_npc_dialog
            .as_ref()
            .is_some_and(|dialog| dialog
                .links
                .iter()
                .any(|link| link.target == "@FinishQuest:1001")),
        "ordinary Gateway CallNpc must expose the finish link: {opened_finish:?}"
    );

    let gold_before_finish = first.world_snapshot().gold;
    let finished = first.handle_packet(ClientPacket::FinishQuest {
        quest_index: 1001,
        selected_item_index: -1,
    });
    let after_finish = first.world_snapshot();
    assert!(finished.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests }
            if completed_quests.contains(&1001)
    )));
    assert_eq!(after_finish.gold, gold_before_finish + 300);
    assert!(after_finish
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::Completed));
    assert!(!after_finish
        .inventory_items
        .iter()
        .any(|item| item.key == "crystal-item-876"));
    assert!(after_finish.inventory_items.iter().any(|item| {
        matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
            && item.key == "crystal-item-1135"
            && item.quantity == 2
    }));
    assert!(
        after_finish
            .equipment_items
            .iter()
            .any(|item| item.name == "CopperRing")
            || after_finish
                .inventory_items
                .iter()
                .any(|item| item.name == "CopperRing")
    );

    let before_logout = first.world_snapshot();
    let before_player = player(&first);
    let logout = first.handle_packet(ClientPacket::LogOut);
    assert!(logout
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    drop(first);

    let mut second = GatewaySession::new(file_backed_gateway_config(save_path.clone()));
    login(&mut second, &account_id, password);
    let restarted = second.handle_packet(ClientPacket::StartGame { character_index });
    assert!(restarted
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    let after_reload = second.world_snapshot();
    let reloaded_player = player(&second);
    assert_eq!(after_reload.gold, before_logout.gold);
    assert_eq!(after_reload.inventory_items, before_logout.inventory_items);
    assert_eq!(after_reload.equipment_items, before_logout.equipment_items);
    assert_eq!(after_reload.quest_log, before_logout.quest_log);
    assert_eq!(
        (
            reloaded_player.x,
            reloaded_player.y,
            reloaded_player.direction,
        ),
        (before_player.x, before_player.y, before_player.direction,)
    );
    assert!(after_reload
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::Completed));

    drop(second);
    drop(save_guard);
    assert!(!save_path.exists(), "Gateway save file should be removed");
}
