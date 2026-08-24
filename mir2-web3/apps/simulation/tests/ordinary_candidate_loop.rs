use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    GroundDropLootSnapshot, ItemContainer, QuestStage, SimulationConfig, SimulationSession,
    VisibleNpcRecord, WorldEntityKind, WorldEntitySnapshot,
};

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
    let path = std::env::temp_dir().join(format!(
        "mir2-ordinary-candidate-{}-{nanos}.json",
        std::process::id()
    ));
    SaveFileGuard(path)
}

fn player(session: &SimulationSession) -> WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("ordinary StartGame should expose the self player")
}

fn nearby_npc(session: &SimulationSession, name: &str) -> WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::Npc && entity.name == name)
        .unwrap_or_else(|| panic!("ordinary StartGame should expose NPC {name}"))
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

fn tick_after_ordinary_action(session: &mut SimulationSession) {
    for _ in 0..8 {
        session.tick();
    }
}

fn walk_within(session: &mut SimulationSession, target: &Point, maximum_distance: i32) {
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
            tick_after_ordinary_action(session);
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
        "ordinary Walk path could not reach required distance: current={current_point:?}, target={target:?}, maximum_distance={maximum_distance}"
    );
}

fn walk_toward(session: &mut SimulationSession, target: &Point) {
    walk_within(session, target, 1);
}

fn walk_onto(session: &mut SimulationSession, target: &Point) {
    walk_within(session, target, 0);
}

fn login(session: &mut SimulationSession, account_id: &str, password: &str) {
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: account_id.to_string(),
        password: password.to_string(),
    });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "ordinary Login should succeed: {packets:?}"
    );
}

fn start_game(session: &mut SimulationSession, character_index: i32) {
    let packets = session.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::StartGame {
                    result: 4,
                    resolution
                } if *resolution > 0
            )
        }),
        "ordinary StartGame should enter the world: {packets:?}"
    );
}

#[test]
fn ordinary_candidate_loop_persists_new_warrior_progress_across_logout() {
    let save_guard = unique_save_path();
    let save_path = save_guard.0.clone();
    let account_id = format!(
        "ordinary_candidate_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    );
    let password = "OrdinaryCandidate42!";

    let config = SimulationConfig::default().with_account_store_path(save_path.clone());
    let mut first = SimulationSession::new(config);

    let created = first.handle_packet(ClientPacket::NewAccount {
        account_id: account_id.clone(),
        password: password.to_string(),
        birth_date_binary: 0,
        user_name: String::new(),
        secret_question: String::new(),
        secret_answer: String::new(),
        email_address: String::new(),
    });
    assert!(
        created
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })),
        "ordinary NewAccount should create the account: {created:?}"
    );
    login(&mut first, &account_id, password);

    let created_character = first.handle_packet(ClientPacket::NewCharacter {
        name: format!("Ordinary{}", std::process::id()),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let character_index = created_character
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap_or_else(|| panic!("ordinary NewCharacter should succeed: {created_character:?}"));
    start_game(&mut first, character_index);
    let start_snapshot = first.world_snapshot();
    assert_eq!(
        start_snapshot.map_file_name.as_deref(),
        Some("0"),
        "ordinary new character must enter the authoritative Bichon map"
    );
    assert!(
        start_snapshot
            .map_title
            .as_deref()
            .is_some_and(|title| title.to_ascii_lowercase().contains("bichon")),
        "ordinary new character must expose the Bichon map title: {:?}",
        start_snapshot.map_title
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
        "ordinary AcceptQuest must not work without an active nearby quest NPC: {remote_accept:?}"
    );
    assert!(!first
        .world_snapshot()
        .quest_log
        .iter()
        .any(|quest| { quest.quest_id == 1001 && quest.stage != QuestStage::Available }));

    let initial = player(&first);
    first.handle_packet(ClientPacket::Turn {
        direction: MirDirection::Left,
    });
    assert_eq!(player(&first).direction, MirDirection::Left);

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
        (initial.x, initial.y),
        "ordinary Walk should change the authoritative position"
    );

    let opened = first.handle_packet(ClientPacket::CallNpc {
        object_id: guide.object_id,
        key: "@Main".to_string(),
    });
    let offer_snapshot = first.world_snapshot();
    assert!(
        opened.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectChat { object_id, .. } if *object_id == guide.object_id
            )
        }) && offer_snapshot.active_npc_dialog.is_some(),
        "ordinary CallNpc should open the nearby Village Guide: {opened:?}"
    );
    assert!(!opened.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 1001,
            taken: true,
            ..
        }
    )));
    assert!(offer_snapshot
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::Available));
    assert!(offer_snapshot
        .active_npc_dialog
        .as_ref()
        .is_some_and(|dialog| {
            dialog
                .links
                .iter()
                .any(|link| link.target == "@AcceptQuest:1001")
        }));

    let accepted = first.select_npc_dialog_target("@AcceptQuest:1001");
    let accepted_snapshot = first.world_snapshot();
    assert!(
        accepted.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 1001,
                taken: true,
                ..
            }
        )),
        "the explicit AcceptQuest action must start the offered quest: {accepted:?}"
    );
    assert!(accepted_snapshot
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::InProgress));
    assert!(accepted_snapshot.active_npc_dialog.is_none());

    let mut defeated_wasp = false;
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
            tick_after_ordinary_action(&mut first);
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
        first.handle_packet(ClientPacket::Turn { direction });
        let attack_packets = first.handle_packet(ClientPacket::Attack {
            direction,
            spell: Spell::None,
        });
        defeated_wasp |= attack_packets.iter().any(|packet| {
            matches!(packet, ServerPacket::ObjectDied { info } if info.object_id == wasp.object_id)
        });
        tick_after_ordinary_action(&mut first);
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
        after_combat
            .quest_log
            .iter()
            .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::ReadyToTurnIn),
        "ordinary Attack must reach the exact quest-ready state: defeated_wasp={defeated_wasp}; snapshot={after_combat:?}"
    );
    assert!(
        after_combat.inventory_items.iter().any(|item| {
            item.container == ItemContainer::Quest && item.key == "quest-wasp-stinger"
        }),
        "ordinary quest kill must place the Wasp Stinger in quest inventory: {:?}",
        after_combat.inventory_items
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
        .unwrap_or_else(|| {
            panic!(
                "ordinary Field Wasp kill must create a visible gold drop: {:?}",
                after_combat.ground_drops
            )
        });
    let gold_before_pickup = after_combat.gold;
    walk_onto(&mut first, &wasp_gold.1);
    let pickup_packets = first.handle_packet(ClientPacket::PickUp);
    let after_pickup = first.world_snapshot();
    assert_eq!(
        after_pickup.gold,
        gold_before_pickup + wasp_gold.2,
        "ordinary PickUp must transfer the exact ground-gold amount"
    );
    assert!(
        !after_pickup
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == wasp_gold.0),
        "picked-up ground drop must disappear from the authoritative snapshot"
    );
    assert!(
        pickup_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedGold { gold } if *gold == wasp_gold.2
        )),
        "ordinary PickUp must emit the authoritative GainedGold packet: {pickup_packets:?}"
    );
    let gold_before_turn_in = after_pickup.gold;

    let remote_finish = first.handle_packet(ClientPacket::FinishQuest {
        quest_index: 1001,
        selected_item_index: -1,
    });
    let after_remote_finish = first.world_snapshot();
    assert!(
        !remote_finish.iter().any(|packet| matches!(
            packet,
            ServerPacket::CompleteQuest { completed_quests }
                if completed_quests.contains(&1001)
        )),
        "ordinary FinishQuest must not work away from the active finish NPC: {remote_finish:?}"
    );
    assert_eq!(after_remote_finish.gold, gold_before_turn_in);
    assert!(after_remote_finish
        .quest_log
        .iter()
        .any(|quest| { quest.quest_id == 1001 && quest.stage == QuestStage::ReadyToTurnIn }));

    walk_toward(
        &mut first,
        &Point {
            x: guide.x,
            y: guide.y,
        },
    );
    let turn_in_dialog_packets = first.handle_packet(ClientPacket::CallNpc {
        object_id: guide.object_id,
        key: "@Main".to_string(),
    });
    let turn_in_dialog = first.world_snapshot();
    assert!(
        !turn_in_dialog_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::CompleteQuest { completed_quests }
                if completed_quests.contains(&1001)
        )),
        "opening the turn-in dialog must not complete the quest"
    );
    assert!(turn_in_dialog
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::ReadyToTurnIn));
    assert!(turn_in_dialog
        .active_npc_dialog
        .as_ref()
        .is_some_and(|dialog| {
            dialog
                .links
                .iter()
                .any(|link| link.target == "@FinishQuest:1001")
        }));

    let finished = first.select_npc_dialog_target("@FinishQuest:1001");
    let after_finish = first.world_snapshot();
    assert!(
        finished.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::CompleteQuest { completed_quests }
                    if completed_quests.contains(&1001)
            )
        }),
        "ordinary quest turn-in should complete starter quest: {finished:?}; snapshot={after_finish:?}"
    );
    assert!(after_finish
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::Completed));
    assert!(after_finish.active_npc_dialog.is_none());
    assert_eq!(
        after_finish.gold,
        gold_before_turn_in + 300,
        "starter quest must grant the configured 300 gold reward"
    );
    assert!(
        after_finish.inventory_items.iter().any(|item| {
            matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
                && item.key == "repair-powder"
                && item.quantity == 2
        }),
        "starter quest must place Repair Powder x2 in the ordinary bag: {:?}",
        after_finish.inventory_items
    );
    assert!(
        !after_finish
            .inventory_items
            .iter()
            .any(|item| item.key == "quest-wasp-stinger"),
        "quest proof must be consumed by the successful hand-in"
    );
    assert!(
        after_finish
            .equipment_items
            .iter()
            .any(|item| item.name == "Guide Ring")
            || after_finish
                .inventory_items
                .iter()
                .any(|item| item.name == "Guide Ring"),
        "starter quest must grant Guide Ring: {after_finish:?}"
    );

    let repair_reward = after_finish
        .inventory_items
        .iter()
        .find(|item| {
            matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
                && item.key == "repair-powder"
                && item.quantity == 2
        })
        .expect("starter reward must expose an ordinary bag item with a unique id");
    let dropped_item_packets = first.handle_packet(ClientPacket::DropItem {
        unique_id: repair_reward.unique_id,
        count: 1,
        hero_inventory: false,
    });
    assert!(dropped_item_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::DropItem {
            unique_id,
            count: 1,
            hero_inventory: false,
            success: true,
        } if *unique_id == repair_reward.unique_id
    )));
    let after_item_drop = first.world_snapshot();
    let dropped_repair = after_item_drop
        .ground_drops
        .iter()
        .find_map(|drop| match &drop.loot {
            GroundDropLootSnapshot::InventoryItem { key, .. } if key == "repair-powder" => Some((
                drop.object_id,
                Point {
                    x: drop.x,
                    y: drop.y,
                },
            )),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "ordinary DropItem must create a visible inventory-item drop: {:?}",
                after_item_drop.ground_drops
            )
        });
    assert_eq!(
        after_item_drop
            .inventory_items
            .iter()
            .filter(|item| item.key == "repair-powder")
            .map(|item| item.quantity)
            .sum::<u32>(),
        1,
        "dropping one reward item must remove exactly one from the bag"
    );
    walk_onto(&mut first, &dropped_repair.1);
    let object_pickup_packets = first.pick_up(dropped_repair.0);
    let after_object_pickup = first.world_snapshot();
    assert!(object_pickup_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::GainedItem { item } if item.count == 2
    )), "object-id pickup must emit GainedItem: packets={object_pickup_packets:?}; snapshot={after_object_pickup:?}");
    assert!(object_pickup_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == dropped_repair.0
    )));
    assert!(!after_object_pickup
        .ground_drops
        .iter()
        .any(|drop| drop.object_id == dropped_repair.0));
    assert_eq!(
        after_object_pickup
            .inventory_items
            .iter()
            .filter(|item| item.key == "repair-powder")
            .map(|item| item.quantity)
            .sum::<u32>(),
        2,
        "object-id pickup must restore the exact dropped item quantity to the bag"
    );

    let before_logout = first.world_snapshot();
    assert!(before_logout.gold > 0, "starter reward should change gold");
    assert!(before_logout
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::Completed));
    let before_player = player(&first);
    let before_position = (before_player.x, before_player.y);
    let before_direction = before_player.direction;
    let before_inventory = before_logout.inventory_items.clone();
    let before_belt = before_logout.belt_items.clone();
    let before_equipment = before_logout.equipment_items.clone();
    let before_quest_log = before_logout.quest_log.clone();
    let before_known_skills = before_logout.known_skills.clone();
    let before_vitals = (
        before_logout.player_hp,
        before_logout.player_max_hp,
        before_logout.player_mp,
        before_logout.player_max_mp,
        before_logout.player_experience,
        before_logout.player_max_experience,
    );
    let before_identity = (
        before_player.class,
        before_player.gender,
        before_player.level,
        before_player.hp,
        before_player.max_hp,
        before_player.dead,
    );

    let logout = first.handle_packet(ClientPacket::LogOut);
    assert!(
        logout
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })),
        "ordinary LogOut should succeed: {logout:?}"
    );
    drop(first);

    let reload_config = SimulationConfig::default().with_account_store_path(save_path.clone());
    let mut second = SimulationSession::new(reload_config);
    login(&mut second, &account_id, password);
    start_game(&mut second, character_index);

    let after_reload = second.world_snapshot();
    let reloaded_player = player(&second);
    assert_eq!((reloaded_player.x, reloaded_player.y), before_position);
    assert_eq!(reloaded_player.direction, before_direction);
    assert_eq!(after_reload.gold, before_logout.gold);
    assert_eq!(after_reload.inventory_items, before_inventory);
    assert_eq!(after_reload.belt_items, before_belt);
    assert_eq!(after_reload.equipment_items, before_equipment);
    assert_eq!(after_reload.quest_log, before_quest_log);
    assert_eq!(after_reload.known_skills, before_known_skills);
    assert_eq!(
        (
            after_reload.player_hp,
            after_reload.player_max_hp,
            after_reload.player_mp,
            after_reload.player_max_mp,
            after_reload.player_experience,
            after_reload.player_max_experience,
        ),
        before_vitals
    );
    assert_eq!(
        (
            reloaded_player.class,
            reloaded_player.gender,
            reloaded_player.level,
            reloaded_player.hp,
            reloaded_player.max_hp,
            reloaded_player.dead,
        ),
        before_identity
    );
    assert!(after_reload.inventory_items.iter().any(|item| {
        matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
            && item.key == "repair-powder"
            && item.quantity == 2
    }));
    assert!(after_reload
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::Completed));

    drop(second);
    drop(save_guard);
    assert!(
        !Path::new(&save_path).exists(),
        "test save file should be cleaned up"
    );
}

#[test]
fn stale_npc_dialog_action_is_rejected_after_ordinary_walks_out_of_range() {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: 4_991,
        name: "Warehouse Keeper".to_string(),
        image: 5,
        colour_argb: -1,
        position: Point { x: 331, y: 270 },
        direction: MirDirection::Left,
        quest_ids: Vec::new(),
        script_key: Some("BichonProvince/Warehouse-D002".to_string()),
    });
    let mut session = SimulationSession::new(config);
    login(&mut session, "demo", "demo");
    start_game(&mut session, 0);

    let opened = session.interact(4_991);
    assert!(opened.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectChat {
            object_id: 4_991,
            ..
        }
    )));
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .as_ref()
        .is_some_and(|dialog| dialog
            .links
            .iter()
            .any(|link| link.target.eq_ignore_ascii_case("@Storage"))));

    for _ in 0..4 {
        session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Left,
        });
        tick_after_ordinary_action(&mut session);
    }
    let current = player(&session);
    assert!(
        tile_distance(
            &Point {
                x: current.x,
                y: current.y,
            },
            &Point { x: 331, y: 270 }
        ) > 1,
        "ordinary movement must leave the active NPC's interaction range"
    );

    let replay = session.select_npc_dialog_target("@Storage");
    assert!(
        replay.is_empty(),
        "a stale remote NPC action must not execute: {replay:?}"
    );
    assert!(session.world_snapshot().active_npc_dialog.is_none());
}
