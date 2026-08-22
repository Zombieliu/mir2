use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    QuestStage, SimulationConfig, SimulationSession, WorldEntityKind, WorldEntitySnapshot,
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

fn walk_toward(session: &mut SimulationSession, target: &Point) {
    for _ in 0..40 {
        let current = player(session);
        let current_point = Point {
            x: current.x,
            y: current.y,
        };
        if tile_distance(&current_point, target) <= 1 {
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
        tile_distance(&current_point, target) <= 1,
        "ordinary Walk path could not approach target: current={current_point:?}, target={target:?}"
    );
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

fn item_fingerprint(session: &SimulationSession) -> Vec<(u64, String, u32)> {
    session
        .world_snapshot()
        .inventory_items
        .into_iter()
        .map(|item| (item.unique_id, item.key, item.quantity))
        .collect()
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
    assert!(
        opened.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectChat { object_id, .. } if *object_id == guide.object_id
            )
        }) || first.world_snapshot().active_npc_dialog.is_some(),
        "ordinary CallNpc should open the nearby Village Guide: {opened:?}"
    );

    let accepted = first.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 0,
        quest_index: 1001,
    });
    assert!(
        accepted.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ChangeQuest {
                    quest_id: 1001,
                    taken: true,
                    ..
                }
            )
        }),
        "ordinary AcceptQuest should accept starter quest 1001: {accepted:?}"
    );
    assert!(first
        .world_snapshot()
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::InProgress));

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
        defeated_wasp
            || after_combat.player_experience > 0
            || after_combat
                .quest_log
                .iter()
                .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::ReadyToTurnIn),
        "ordinary Attack should produce combat progress or a quest-ready state: {after_combat:?}"
    );

    walk_toward(
        &mut first,
        &Point {
            x: guide.x,
            y: guide.y,
        },
    );
    first.handle_packet(ClientPacket::CallNpc {
        object_id: guide.object_id,
        key: "@Main".to_string(),
    });
    let finished = first.handle_packet(ClientPacket::FinishQuest {
        quest_index: 1001,
        selected_item_index: -1,
    });
    let after_finish = first.world_snapshot();
    assert!(
        finished.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::CompleteQuest { completed_quests }
                    if completed_quests.contains(&1001)
            )
        }) || after_finish
            .quest_log
            .iter()
            .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::Completed),
        "ordinary quest turn-in should complete starter quest: {finished:?}; snapshot={after_finish:?}"
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
    let before_inventory = item_fingerprint(&first);

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
    assert_eq!(item_fingerprint(&second), before_inventory);
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
