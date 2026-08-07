use mir2_protocol::{
    ChatType, ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point, ServerPacket,
    Spell,
};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, EquipmentSlot, GroundDropLootSnapshot,
    GroundDropSnapshot, QuestStage, SessionId, SimulationConfig, SimulationSession,
    WorldEntityKind, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneOutbound, ZoneRuntime,
};
use serde_json::json;

fn session_id(value: &str) -> SessionId {
    SessionId::new(value)
}

fn login(session: &mut SimulationSession, account_id: &str) {
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: account_id.to_string(),
        password: "demo".to_string(),
    });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "login packets: {packets:?}"
    );
}

fn account_with_character(
    account_id: &str,
    character: CharacterRecord,
    save: CharacterSaveRecord,
) -> SimulationConfig {
    let config = SimulationConfig::default();
    let mut account = AccountRecord::empty();
    account.characters.push(character.clone());
    account.saves.insert(character.index, save);
    {
        let mut store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        store.accounts.insert(account_id.to_string(), account);
    }
    config
}

fn start_character(
    account_id: &str,
    character: CharacterRecord,
    save: CharacterSaveRecord,
) -> SimulationSession {
    let character_index = character.index;
    let config = account_with_character(account_id, character, save);
    let mut session = SimulationSession::new(config);
    login(&mut session, account_id);
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
        "start packets: {packets:?}"
    );
    session
}

fn start_character_with_config(
    account_id: &str,
    config: SimulationConfig,
    character: CharacterRecord,
    save: CharacterSaveRecord,
) -> SimulationSession {
    let character_index = character.index;
    let mut account = AccountRecord::empty();
    account.characters.push(character.clone());
    account.saves.insert(character.index, save);
    config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned")
        .accounts
        .insert(account_id.to_string(), account);

    let mut session = SimulationSession::new(config);
    login(&mut session, account_id);
    let packets = session.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::StartGame {
                result: 4,
                resolution
            } if *resolution > 0
        )),
        "start packets: {packets:?}"
    );
    session
}

fn combat_save(
    class: MirClass,
    gender: MirGender,
    skill_names: &[&str],
) -> (CharacterRecord, CharacterSaveRecord) {
    combat_save_at_level(class, gender, 45, skill_names, &[])
}

fn combat_save_at_level(
    class: MirClass,
    gender: MirGender,
    level: u16,
    skill_names: &[&str],
    equipment: &[(&str, EquipmentSlot)],
) -> (CharacterRecord, CharacterSaveRecord) {
    let character = CharacterRecord {
        index: 0,
        name: format!("Slice{:?}", class),
        level,
        class,
        gender,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.position = Point { x: 340, y: 550 };
    save.direction = MirDirection::Right;
    save.hp = save.max_hp;
    save.mp = 500;
    let skill_level = if level >= 20 { 3 } else { 2 };
    save.skill_states_json = skill_names
        .iter()
        .map(|spell_name| skill_state_json(spell_name, skill_level))
        .collect();
    save.equipment_items_json = equipment
        .iter()
        .map(|(template_name, slot)| crystal_equipment_state_json(template_name, *slot))
        .collect();
    (character, save)
}

fn skill_state_json(spell_name: &str, level: u8) -> String {
    let (key, name, description, cooldown_ticks) = match spell_name {
        "Healing" => (
            "minor-heal".to_string(),
            "Minor Heal".to_string(),
            "Restores a small amount of HP.".to_string(),
            6,
        ),
        "Fury" => (
            "battle-focus".to_string(),
            "Battle Focus".to_string(),
            "Short melee focus buff.".to_string(),
            12,
        ),
        other => (
            normalize_crystal_skill_key(other),
            other.to_string(),
            format!("Crystal NPC granted skill {other}."),
            1,
        ),
    };
    json!({
        "key": key,
        "name": name,
        "description": description,
        "level": level,
        "experience": 0,
        "hotkey": 0,
        "cooldown_ticks": cooldown_ticks,
        "delay_ms": 1,
        "cooldown_ends_at": 0,
        "cast_time_ms": 0
    })
    .to_string()
}

fn crystal_equipment_state_json(template_name: &str, slot: EquipmentSlot) -> String {
    let template =
        mir2_game_data::crystal_item_by_name(template_name).expect("Crystal item template");
    let durability = template.durability.max(1);
    let shape = u16::try_from(template.shape).ok();
    json!({
        "key": format!("crystal-item-{}", template.item_index),
        "slot": slot,
        "name": template.name,
        "icon": template.image,
        "shape": shape,
        "description": template.tooltip.unwrap_or_default(),
        "durability_current": durability,
        "durability_max": durability,
        "socket_slots": template.slots,
        "attack": 0,
        "defence": 0
    })
    .to_string()
}

fn normalize_crystal_skill_key(spell_name: &str) -> String {
    spell_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn field_wasp(session: &mut SimulationSession) -> mir2_simulation::WorldEntitySnapshot {
    const TARGET_NAMES: [&str; 7] = [
        "HookingCat",
        "RakingCat",
        "Oma",
        "Yob",
        "ForestYeti",
        "Scarecrow",
        "Field Wasp",
    ];
    const FIELD_CENTERS: [Point; 6] = [
        Point { x: 340, y: 550 },
        Point { x: 300, y: 410 },
        Point { x: 140, y: 500 },
        Point { x: 165, y: 550 },
        Point { x: 500, y: 400 },
        Point { x: 110, y: 60 },
    ];

    for name in TARGET_NAMES {
        if let Some(entity) = visible_alive_monster_named(session, name) {
            return entity;
        }
    }
    for center in FIELD_CENTERS {
        session.transfer_map(&format!("crystal:0:{}:{}", center.x, center.y));
        for name in TARGET_NAMES {
            if let Some(entity) = visible_alive_monster_named(session, name) {
                return entity;
            }
        }
    }

    panic!("a combat target should be visible in the Bichon starter slice")
}

fn self_player(session: &SimulationSession) -> mir2_simulation::WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("self player should be visible")
}

fn direction_from_to(from: Point, to: Point) -> MirDirection {
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

fn assert_knows_skill(session: &SimulationSession, spell_name: &str) {
    let key = match spell_name {
        "Healing" => "minor-heal".to_string(),
        other => normalize_crystal_skill_key(other),
    };
    assert!(
        session
            .world_snapshot()
            .known_skills
            .iter()
            .any(|skill| skill.key == key),
        "{spell_name} should be present in known skills"
    );
}

fn tick_many(session: &mut SimulationSession, ticks: usize) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    for _ in 0..ticks {
        packets.extend(session.tick());
    }
    packets
}

fn point_left_of(point: &Point, tiles: i32) -> Point {
    Point {
        x: point.x.saturating_sub(tiles.max(1)),
        y: point.y,
    }
}

fn position_player_left_of_target(
    session: &mut SimulationSession,
    target: &mir2_simulation::WorldEntitySnapshot,
    tiles: i32,
) -> MirDirection {
    let target_position = Point {
        x: target.x,
        y: target.y,
    };
    let player_position = point_left_of(&target_position, tiles);
    let direction = direction_from_to(player_position.clone(), target_position);
    session.force_authoritative_player_transform(player_position, direction);
    direction
}

fn start_original_bichon_intro_session() -> SimulationSession {
    let account_id = "slice-bichon-original-intro";
    let config = SimulationConfig::default().with_crystal_world_runtime();
    config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned")
        .accounts
        .insert(account_id.to_string(), AccountRecord::empty());

    let mut session = SimulationSession::new(config);
    login(&mut session, account_id);
    let create_packets = session.handle_packet(ClientPacket::NewCharacter {
        name: "NewBlade".to_string(),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let character_index = create_packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap_or_else(|| panic!("fresh warrior should be created: {create_packets:?}"));
    let start_packets = session.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        start_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::StartGame {
                result: 4,
                resolution
            } if *resolution > 0
        )),
        "fresh warrior should enter the Crystal world: {start_packets:?}"
    );
    session
}

fn quest_snapshot(
    session: &SimulationSession,
    quest_id: i32,
) -> Option<mir2_simulation::QuestSnapshot> {
    session
        .world_snapshot()
        .quest_log
        .into_iter()
        .find(|quest| quest.quest_id == quest_id)
}

fn assert_quest_stage(session: &SimulationSession, quest_id: i32, stage: QuestStage) {
    let quest = quest_snapshot(session, quest_id)
        .unwrap_or_else(|| panic!("quest {quest_id} should be visible in quest log"));
    assert_eq!(
        quest.stage, stage,
        "quest {quest_id} should be {stage:?}: {quest:?}"
    );
}

fn open_original_npc_dialog(
    session: &mut SimulationSession,
    npc_object_id: u32,
    player_position: Point,
    facing: MirDirection,
    expected_target: &str,
) -> Vec<ServerPacket> {
    session.force_authoritative_player_transform(player_position, facing);
    let packets = session.interact(npc_object_id);
    let dialog = session
        .world_snapshot()
        .active_npc_dialog
        .unwrap_or_else(|| panic!("npc {npc_object_id} should open a dialog"));
    assert!(
        dialog
            .links
            .iter()
            .any(|link| link.target == expected_target),
        "npc {npc_object_id} dialog should contain {expected_target}: {dialog:?}; packets: {packets:?}"
    );
    packets
}

fn visible_alive_monster_named(
    session: &SimulationSession,
    monster_name_prefix: &str,
) -> Option<mir2_simulation::WorldEntitySnapshot> {
    let snapshot = session.world_snapshot();
    let (player_x, player_y) = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .map(|entity| (entity.x, entity.y))?;

    let hostiles = snapshot
        .entities
        .iter()
        .filter(|entity| {
            entity.kind == WorldEntityKind::Monster
                && entity.disposition == mir2_simulation::WorldEntityDisposition::Hostile
                && !entity.dead
                && entity.hp.unwrap_or(1) > 0
        })
        .map(|entity| (entity.object_id, entity.x, entity.y))
        .collect::<Vec<_>>();

    snapshot
        .entities
        .into_iter()
        .filter(|entity| {
            entity.kind == WorldEntityKind::Monster
                && !entity.dead
                && entity.name.starts_with(monster_name_prefix)
                && entity.hp.unwrap_or(1) > 0
        })
        .max_by_key(|entity| {
            let nearest_hostile = hostiles
                .iter()
                .filter(|(object_id, _, _)| *object_id != entity.object_id)
                .map(|(_, x, y)| (x - entity.x).abs().max((y - entity.y).abs()))
                .min()
                .unwrap_or(i32::MAX);
            let player_distance = (entity.x - player_x).abs().max((entity.y - player_y).abs());
            (nearest_hostile, -player_distance)
        })
}

fn attack_monster_until_dead(
    session: &mut SimulationSession,
    monster: &mir2_simulation::WorldEntitySnapshot,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();

    for _ in 0..48 {
        packets.extend(use_newcomer_hp_drug_if_needed(session));
        let Some(current_monster) = session
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.object_id == monster.object_id)
        else {
            break;
        };

        if current_monster.dead || current_monster.hp.unwrap_or(1) <= 0 {
            break;
        }

        let monster_position = Point {
            x: current_monster.x,
            y: current_monster.y,
        };
        let player_position = Point {
            x: current_monster.x.saturating_sub(1),
            y: current_monster.y,
        };
        let direction = direction_from_to(player_position.clone(), monster_position);
        session.force_authoritative_player_transform(player_position, direction);

        let attack_packets = session.attack(current_monster.object_id);
        let defeated = attack_packets.iter().any(|packet| {
            matches!(packet, ServerPacket::ObjectDied { info } if info.object_id == monster.object_id)
        });
        packets.extend(attack_packets);
        if defeated {
            break;
        }
    }

    packets.extend(session.tick());

    packets
}

fn use_newcomer_hp_drug_if_needed(session: &mut SimulationSession) -> Vec<ServerPacket> {
    let snapshot = session.world_snapshot();
    let (Some(hp), Some(max_hp)) = (snapshot.player_hp, snapshot.player_max_hp) else {
        return Vec::new();
    };
    if hp <= 0 || hp.saturating_mul(2) > max_hp {
        return Vec::new();
    }
    let potion = snapshot
        .inventory_items
        .iter()
        .find(|item| item.name.starts_with("(HP)Drug"))
        .map(|item| (item, MirGridType::Inventory))
        .or_else(|| {
            snapshot
                .belt_items
                .iter()
                .find(|item| item.name.starts_with("(HP)Drug"))
                .map(|item| (item, MirGridType::Belt))
        });
    let Some((potion, grid)) = potion else {
        return Vec::new();
    };

    let mut packets = session.handle_packet(ClientPacket::UseItem {
        unique_id: potion.unique_id,
        grid,
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UseItem { success: true, .. })));
    packets.extend(tick_many(session, 3));
    packets
}

fn assert_newcomer_alive(session: &SimulationSession, context: &str) {
    let snapshot = session.world_snapshot();
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer);
    assert!(
        snapshot.player_hp.is_some_and(|hp| hp > 0),
        "newcomer died while {context}: hp={:?}/{:?}, player={player:?}",
        snapshot.player_hp,
        snapshot.player_max_hp,
    );
}

fn assert_crystal_monster_experience(monster_name: &str, packets: &[ServerPacket]) {
    let expected = mir2_game_data::crystal_monster_by_name(monster_name)
        .unwrap_or_else(|| panic!("Crystal monster {monster_name} should exist"))
        .experience;
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainExperience { amount } if *amount == expected
        )),
        "defeating {monster_name} should grant {expected} EXP: {packets:?}"
    );
}

fn newcomer_cumulative_experience(session: &SimulationSession) -> i64 {
    const THRESHOLDS: [i64; 6] = [100, 200, 300, 400, 600, 900];
    let snapshot = session.world_snapshot();
    let level = self_player(session).level.unwrap_or(1).max(1);
    THRESHOLDS
        .iter()
        .take(usize::from(level.saturating_sub(1)))
        .sum::<i64>()
        .saturating_add(snapshot.player_experience)
}

fn kill_original_monsters(
    session: &mut SimulationSession,
    field_key: &str,
    field_centers: &[Point],
    monster_name_prefix: &str,
    count: usize,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    if let Some(field_center) = field_centers.first() {
        packets.extend(session.transfer_map(&format!(
            "crystal:{field_key}:{}:{}",
            field_center.x, field_center.y
        )));
    }

    for kill_index in 0..count {
        packets.extend(use_newcomer_hp_drug_if_needed(session));
        assert_newcomer_alive(
            session,
            &format!("hunting {monster_name_prefix} #{kill_index}"),
        );
        let mut monster = visible_alive_monster_named(session, monster_name_prefix);
        if monster.is_none() {
            for field_center in field_centers {
                packets.extend(session.transfer_map(&format!(
                    "crystal:{field_key}:{}:{}",
                    field_center.x, field_center.y
                )));
                monster = visible_alive_monster_named(session, monster_name_prefix);
                if monster.is_some() {
                    break;
                }
            }
        }
        let monster = monster.unwrap_or_else(|| {
            panic!("{monster_name_prefix} #{kill_index} should spawn near {field_centers:?}")
        });
        let before = session.world_snapshot();
        let nearby = before
            .entities
            .iter()
            .filter(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && !entity.dead
                    && (entity.x - monster.x)
                        .abs()
                        .max((entity.y - monster.y).abs())
                        <= 8
            })
            .map(|entity| {
                format!(
                    "{}#{}@{},{} hp={:?}",
                    entity.name, entity.object_id, entity.x, entity.y, entity.hp
                )
            })
            .collect::<Vec<_>>();
        let object_id = monster.object_id;
        let kill_packets = attack_monster_until_dead(session, &monster);
        assert!(
            kill_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectDied { info } if info.object_id == object_id
            )) || session
                .world_snapshot()
                .entities
                .iter()
                .any(|entity| entity.object_id == object_id && entity.dead),
            "{monster_name_prefix} #{kill_index} should die: {kill_packets:?}"
        );
        assert_crystal_monster_experience(&monster.name, &kill_packets);
        packets.extend(kill_packets);
        assert_newcomer_alive(
            session,
            &format!(
                "after hunting {monster_name_prefix} #{kill_index}; target={}@{},{}; before_hp={:?}/{:?}; nearby={nearby:?}",
                monster.object_id,
                monster.x,
                monster.y,
                before.player_hp,
                before.player_max_hp
            ),
        );
    }

    packets
}

fn equip_inventory_item_by_name(session: &mut SimulationSession, name: &str, to: i32) {
    let item = session
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|item| item.name == name)
        .unwrap_or_else(|| panic!("{name} should be in inventory before equip"));
    let packets = session.handle_packet(ClientPacket::EquipItem {
        grid: MirGridType::Inventory,
        unique_id: item.unique_id,
        to,
    });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. })),
        "equipping {name} should succeed: {packets:?}"
    );
}

fn buy_original_small_hp_drugs(session: &mut SimulationSession, count: u16) -> Vec<ServerPacket> {
    let mut packets = session.transfer_map("crystal:0:323:291");
    packets.extend(open_original_npc_dialog(
        session,
        20,
        Point { x: 323, y: 291 },
        MirDirection::Right,
        "@BuySell",
    ));
    let goods_packets = session.select_npc_dialog_target("@BuySell");
    let shop_unique_id = goods_packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NPCGoods { list, .. } => list
                .iter()
                .find(|item| item.item_index == 658 && item.count == 1)
                .map(|item| item.unique_id),
            _ => None,
        })
        .expect("Bichon potion shop should sell one-count (HP)DrugSmall");
    packets.extend(goods_packets);

    let buy_packets = session.handle_packet(ClientPacket::BuyItem {
        item_index: shop_unique_id,
        count,
        panel_type: 0,
    });
    assert!(buy_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::LoseGold { gold } if *gold == u32::from(count) * 40
    )));
    assert!(buy_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::GainedItem { item } if item.item_index == 658 && item.count == count
    )));
    packets.extend(buy_packets);
    let snapshot = session.world_snapshot();
    let carried = snapshot
        .belt_items
        .into_iter()
        .chain(snapshot.inventory_items)
        .filter(|item| item.name == "(HP)DrugSmall")
        .map(|item| item.quantity)
        .sum::<u32>();
    assert!(
        carried >= u32::from(count),
        "purchased HP drugs must remain in belt or bag: carried={carried}, count={count}"
    );
    packets
}

fn progress_original_item_quest_from_monster(
    session: &mut SimulationSession,
    quest_id: i32,
    field_key: &str,
    field_centers: &[Point],
    monster_name_prefix: &str,
    max_kills: usize,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    if let Some(field_center) = field_centers.first() {
        packets.extend(session.transfer_map(&format!(
            "crystal:{field_key}:{}:{}",
            field_center.x, field_center.y
        )));
    }

    for _ in 0..max_kills {
        if quest_snapshot(session, quest_id).is_some_and(|quest| {
            matches!(
                quest.stage,
                QuestStage::ReadyToTurnIn | QuestStage::Completed
            )
        }) {
            break;
        }

        packets.extend(use_newcomer_hp_drug_if_needed(session));
        assert_newcomer_alive(session, &format!("hunting {monster_name_prefix}"));
        let mut monster = visible_alive_monster_named(session, monster_name_prefix);
        if monster.is_none() {
            for field_center in field_centers {
                packets.extend(session.transfer_map(&format!(
                    "crystal:{field_key}:{}:{}",
                    field_center.x, field_center.y
                )));
                monster = visible_alive_monster_named(session, monster_name_prefix);
                if monster.is_some() {
                    break;
                }
            }
        }
        let monster = monster
            .unwrap_or_else(|| panic!("{monster_name_prefix} should spawn near {field_centers:?}"));
        let kill_packets = attack_monster_until_dead(session, &monster);
        assert_crystal_monster_experience(&monster.name, &kill_packets);
        packets.extend(kill_packets);
        assert_newcomer_alive(session, &format!("after hunting {monster_name_prefix}"));
    }

    packets
}

fn harvest_monster_corpse_for_original_drop(
    session: &mut SimulationSession,
    monster_object_id: u32,
    fallback_position: Point,
) -> Vec<ServerPacket> {
    let corpse_position = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.object_id == monster_object_id)
        .map(|entity| Point {
            x: entity.x,
            y: entity.y,
        })
        .unwrap_or(fallback_position);
    let player_position = Point {
        x: corpse_position.x.saturating_sub(1),
        y: corpse_position.y,
    };
    let direction = direction_from_to(player_position.clone(), corpse_position);
    session.force_authoritative_player_transform(player_position, direction);

    let mut packets = Vec::new();
    for _ in 0..8 {
        packets.extend(session.handle_packet(ClientPacket::Harvest { direction }));
    }
    packets
}

fn progress_original_item_quest_from_harvest_monster(
    session: &mut SimulationSession,
    quest_id: i32,
    field_key: &str,
    field_centers: &[Point],
    monster_name_prefix: &str,
    max_kills: usize,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    if let Some(field_center) = field_centers.first() {
        packets.extend(session.transfer_map(&format!(
            "crystal:{field_key}:{}:{}",
            field_center.x, field_center.y
        )));
    }

    for _ in 0..max_kills {
        if quest_snapshot(session, quest_id).is_some_and(|quest| {
            matches!(
                quest.stage,
                QuestStage::ReadyToTurnIn | QuestStage::Completed
            )
        }) {
            break;
        }

        packets.extend(use_newcomer_hp_drug_if_needed(session));
        assert_newcomer_alive(session, &format!("hunting {monster_name_prefix}"));
        let mut monster = visible_alive_monster_named(session, monster_name_prefix);
        if monster.is_none() {
            for field_center in field_centers {
                packets.extend(session.transfer_map(&format!(
                    "crystal:{field_key}:{}:{}",
                    field_center.x, field_center.y
                )));
                monster = visible_alive_monster_named(session, monster_name_prefix);
                if monster.is_some() {
                    break;
                }
            }
        }
        let monster = monster
            .unwrap_or_else(|| panic!("{monster_name_prefix} should spawn near {field_centers:?}"));
        let monster_position = Point {
            x: monster.x,
            y: monster.y,
        };
        let kill_packets = attack_monster_until_dead(session, &monster);
        assert_crystal_monster_experience(&monster.name, &kill_packets);
        packets.extend(kill_packets);
        assert_newcomer_alive(session, &format!("after hunting {monster_name_prefix}"));
        packets.extend(harvest_monster_corpse_for_original_drop(
            session,
            monster.object_id,
            monster_position,
        ));
    }

    packets
}

fn attack_until_quest_ready(session: &mut SimulationSession) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    for _ in 0..180 {
        let Some(target) = visible_alive_monster_named(session, "Field Wasp") else {
            break;
        };
        position_player_left_of_target(session, &target, 1);
        packets.extend(session.attack(target.object_id));
        packets.extend(tick_many(session, 6));
        let snapshot = session.world_snapshot();
        if snapshot
            .quest_log
            .iter()
            .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::ReadyToTurnIn)
        {
            break;
        }
    }
    packets
}

fn start_level_20_skill_session(
    class: MirClass,
    gender: MirGender,
    skill_names: &[&str],
    equipment: &[(&str, EquipmentSlot)],
) -> SimulationSession {
    let (character, save) = combat_save_at_level(class, gender, 20, skill_names, equipment);
    let account_id = format!("slice-l20-{class:?}-{}", skill_names.join("-"));
    start_character(&account_id, character, save)
}

fn cast_magic_on_field_wasp(
    session: &mut SimulationSession,
    spell: Spell,
    target_lock: bool,
) -> (Vec<ServerPacket>, Vec<ServerPacket>, i32, u32) {
    let self_id = session
        .world_snapshot()
        .player_object_id
        .expect("player object id");
    let wasp = field_wasp(session);
    let wasp_before = wasp.hp.expect("wasp hp");
    let target = Point {
        x: wasp.x,
        y: wasp.y,
    };
    let direction = position_player_left_of_target(session, &wasp, 1);
    let packets = session.handle_packet(ClientPacket::Magic {
        object_id: self_id,
        spell,
        direction,
        target_id: if target_lock { wasp.object_id } else { 0 },
        location: target,
        spell_target_lock: target_lock,
    });
    let tick_packets = tick_many(session, 10);
    (packets, tick_packets, wasp_before, wasp.object_id)
}

fn assert_target_damaged(
    session: &SimulationSession,
    target_id: u32,
    before_hp: i32,
    spell_name: &str,
) {
    let Some(after) = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.object_id == target_id)
    else {
        return;
    };
    assert!(
        after.hp.unwrap_or(before_hp) < before_hp || after.dead,
        "{spell_name} should damage target; before={before_hp}, after={after:?}"
    );
}

fn assert_level_1_to_20_magic(spell_name: &str) {
    let magic = mir2_game_data::crystal_magic_by_spell(spell_name)
        .unwrap_or_else(|| panic!("{spell_name} should exist in Crystal magic manifest"));
    let first_level = magic.level1.min(magic.level2).min(magic.level3);
    assert!(
        first_level <= 20,
        "{spell_name} should be a level 1-20 Crystal skill, got {first_level}"
    );
    assert!(
        Spell::from_crystal_name(spell_name).is_some(),
        "{spell_name} should have a typed protocol Spell"
    );
}

fn zone_join(session_id_value: &str, object_id: u32, name: &str, x: i32, y: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: session_id(session_id_value),
        account_id: format!("{session_id_value}-account"),
        character_index: object_id as i32,
        object_id,
        name: name.to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 7,
        hp: 60,
        max_hp: 60,
        mp: 100,
        map_file_name: "0".to_string(),
        position: Point { x, y },
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}

fn packets_for(outbounds: &[ZoneOutbound], session_id: &SessionId) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    for outbound in outbounds {
        match outbound {
            ZoneOutbound::ToSession {
                session_id: target,
                packets: outbound_packets,
            } if target == session_id => packets.extend(outbound_packets.clone()),
            ZoneOutbound::ToMany {
                session_ids,
                packets: outbound_packets,
            } if session_ids.iter().any(|target| target == session_id) => {
                packets.extend(outbound_packets.clone())
            }
            ZoneOutbound::ToAll {
                packets: outbound_packets,
            } => packets.extend(outbound_packets.clone()),
            _ => {}
        }
    }
    packets
}

#[test]
fn all_five_classes_create_with_filtered_crystal_start_items() {
    for (class, gender) in [
        (MirClass::Warrior, MirGender::Male),
        (MirClass::Wizard, MirGender::Female),
        (MirClass::Taoist, MirGender::Male),
        (MirClass::Assassin, MirGender::Female),
        (MirClass::Archer, MirGender::Male),
    ] {
        let account_id = format!("slice-create-{:?}-{:?}", class, gender);
        let config = SimulationConfig::default();
        {
            let mut store = config
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned");
            store
                .accounts
                .insert(account_id.clone(), AccountRecord::empty());
        }
        let mut session = SimulationSession::new(config);
        login(&mut session, &account_id);
        let create_packets = session.handle_packet(ClientPacket::NewCharacter {
            name: format!("New{:?}", class),
            gender,
            class,
        });
        let character_index = create_packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .expect("new character should be created");

        session.handle_packet(ClientPacket::StartGame { character_index });
        let snapshot = session.world_snapshot();
        let created = self_player(&session);
        let expected = CharacterSaveRecord::new(CharacterRecord {
            index: character_index,
            name: created.name.clone(),
            level: 1,
            class,
            gender,
        });

        assert_eq!(created.class, Some(class));
        assert_eq!(created.gender, Some(gender));
        assert_eq!(created.level, Some(1));
        assert_eq!(snapshot.player_hp, Some(expected.max_hp));
        assert_eq!(snapshot.player_max_hp, Some(expected.max_hp));
        assert_eq!(snapshot.player_mp, Some(expected.mp));
        assert_eq!(snapshot.gold, 0);
        let expected_weapon = match class {
            MirClass::Warrior | MirClass::Wizard | MirClass::Taoist => "WoodenSword",
            MirClass::Assassin => "HoaSword",
            MirClass::Archer => "WoodenBow",
        };
        let expected_dress = match gender {
            MirGender::Male => "BaseDress(M)",
            MirGender::Female => "BaseDress(F)",
        };
        assert_eq!(
            snapshot
                .inventory_items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec![expected_weapon, expected_dress, "(HP)DrugSmall", "Candle"]
        );
        assert!(snapshot
            .inventory_items
            .iter()
            .all(|item| item.key.starts_with("crystal-item-")));
        assert!(snapshot.belt_items.is_empty());
        assert!(snapshot.storage_items.is_empty());
        assert_eq!(snapshot.equipment_items.len(), 2);
        assert!(snapshot
            .equipment_items
            .iter()
            .any(|item| item.slot == EquipmentSlot::Weapon));
        assert!(snapshot
            .equipment_items
            .iter()
            .any(|item| item.slot == EquipmentSlot::Armour));
        assert!(snapshot.quest_log.iter().all(|quest| {
            quest.stage == QuestStage::Available
                && quest.current == 0
                && quest.required >= quest.current
        }));
        assert!(snapshot.known_skills.is_empty());
    }
}

#[test]
fn all_five_classes_have_a_basic_skill_and_combat_loop() {
    struct Case {
        class: MirClass,
        gender: MirGender,
        skills: &'static [&'static str],
        spell: Spell,
        direction: MirDirection,
        expects_damage: bool,
    }

    for case in [
        Case {
            class: MirClass::Warrior,
            gender: MirGender::Male,
            skills: &["Slaying"],
            spell: Spell::Slaying,
            direction: MirDirection::UpRight,
            expects_damage: true,
        },
        Case {
            class: MirClass::Wizard,
            gender: MirGender::Female,
            skills: &["FireBall"],
            spell: Spell::FireBall,
            direction: MirDirection::UpRight,
            expects_damage: true,
        },
        Case {
            class: MirClass::Taoist,
            gender: MirGender::Male,
            skills: &["Healing"],
            spell: Spell::Healing,
            direction: MirDirection::Down,
            expects_damage: false,
        },
        Case {
            class: MirClass::Assassin,
            gender: MirGender::Female,
            skills: &["DoubleSlash"],
            spell: Spell::DoubleSlash,
            direction: MirDirection::UpRight,
            expects_damage: true,
        },
        Case {
            class: MirClass::Archer,
            gender: MirGender::Male,
            skills: &["StraightShot"],
            spell: Spell::StraightShot,
            direction: MirDirection::UpRight,
            expects_damage: true,
        },
    ] {
        let (character, mut save) = combat_save(case.class, case.gender, case.skills);
        if case.spell == Spell::Healing {
            save.hp = (save.max_hp - 25).max(1);
        }
        let account_id = format!("slice-combat-{:?}", case.class);
        let mut session = start_character(&account_id, character, save);
        let wasp = field_wasp(&mut session);
        let wasp_before = wasp.hp.expect("wasp hp");
        let self_id = session
            .world_snapshot()
            .player_object_id
            .expect("player object id");
        let target = Point {
            x: wasp.x,
            y: wasp.y,
        };
        let cast_direction = if case.spell == Spell::Healing {
            case.direction
        } else {
            position_player_left_of_target(&mut session, &wasp, 1)
        };

        let mut packets = Vec::new();
        if case.spell == Spell::Slaying {
            packets.extend(session.handle_packet(ClientPacket::SpellToggle {
                spell: Spell::Slaying,
                toggle_state: 1,
            }));
            packets.extend(session.handle_packet(ClientPacket::Attack {
                direction: cast_direction,
                spell: case.spell,
            }));
        } else {
            let target_id = if case.spell == Spell::Healing {
                self_id
            } else {
                wasp.object_id
            };
            let target_location = if case.spell == Spell::Healing {
                let player = self_player(&session);
                Point {
                    x: player.x,
                    y: player.y,
                }
            } else {
                target
            };
            packets.extend(session.handle_packet(ClientPacket::Magic {
                object_id: self_id,
                spell: case.spell,
                direction: cast_direction,
                target_id,
                location: target_location,
                spell_target_lock: true,
            }));
        }
        packets.extend(tick_many(&mut session, 8));

        if case.spell == Spell::Healing {
            assert!(
                session.world_snapshot().player_hp > Some(1),
                "healing should restore the taoist player"
            );
            let target = field_wasp(&mut session);
            let direction = position_player_left_of_target(&mut session, &target, 1);
            let attack_packets = session.handle_packet(ClientPacket::Attack {
                direction,
                spell: Spell::None,
            });
            assert!(
                attack_packets.iter().any(|packet| {
                    matches!(
                        packet,
                        ServerPacket::ObjectAttack { info } if info.object_id == self_id
                    )
                }),
                "taoist fallback combat packets: {attack_packets:?}"
            );
            continue;
        }

        assert!(
            packets.iter().any(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectAttack { info }
                        if info.object_id == self_id && info.spell == case.spell as u8
                ) || matches!(
                    packet,
                    ServerPacket::Magic {
                        spell,
                        cast: true,
                        ..
                    } if *spell == case.spell
                ) || matches!(
                    packet,
                    ServerPacket::ObjectMagic {
                        spell,
                        cast: true,
                        ..
                    } if *spell == case.spell
                )
            }),
            "expected visible class skill packet for {:?}: {packets:?}",
            case.class
        );
        if case.expects_damage {
            assert_target_damaged(
                &session,
                wasp.object_id,
                wasp_before,
                &format!("{:?}", case.class),
            );
        }
    }
}

#[test]
fn five_classes_cover_level_1_to_20_core_skill_matrix() {
    for spell_name in [
        "Fencing",
        "Slaying",
        "FireBall",
        "GreatFireBall",
        "HellFire",
        "ThunderBolt",
        "Healing",
        "Poisoning",
        "SoulFireBall",
        "SummonSkeleton",
        "Hiding",
        "FatalSword",
        "DoubleSlash",
        "Haste",
        "Focus",
        "StraightShot",
        "DoubleShot",
        "ElementalShot",
    ] {
        assert_level_1_to_20_magic(spell_name);
    }

    let mut warrior = start_level_20_skill_session(
        MirClass::Warrior,
        MirGender::Male,
        &["Fencing", "Slaying"],
        &[],
    );
    assert_knows_skill(&warrior, "Fencing");
    assert_knows_skill(&warrior, "Slaying");
    let self_id = warrior
        .world_snapshot()
        .player_object_id
        .expect("player object id");
    let wasp = field_wasp(&mut warrior);
    let wasp_before = wasp.hp.expect("wasp hp");
    let slaying_direction = position_player_left_of_target(&mut warrior, &wasp, 1);
    let packets = warrior.handle_packet(ClientPacket::SpellToggle {
        spell: Spell::Slaying,
        toggle_state: 1,
    });
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::SpellToggle {
            object_id: packet_object_id,
            spell: Spell::Slaying,
            can_use: true,
        } if *packet_object_id == self_id
    )));
    let packets = warrior.handle_packet(ClientPacket::Attack {
        direction: slaying_direction,
        spell: Spell::Slaying,
    });
    assert!(packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectAttack { info }
                if info.object_id == self_id && info.spell == Spell::Slaying as u8
        )
    }));
    tick_many(&mut warrior, 4);
    assert_target_damaged(&warrior, wasp.object_id, wasp_before, "Slaying");

    for (spell_name, spell, target_lock) in [
        ("FireBall", Spell::FireBall, true),
        ("GreatFireBall", Spell::GreatFireBall, true),
        ("HellFire", Spell::HellFire, false),
        ("ThunderBolt", Spell::ThunderBolt, true),
    ] {
        let mut wizard =
            start_level_20_skill_session(MirClass::Wizard, MirGender::Female, &[spell_name], &[]);
        assert_knows_skill(&wizard, spell_name);
        let (packets, tick_packets, wasp_before, wasp_id) =
            cast_magic_on_field_wasp(&mut wizard, spell, target_lock);
        assert!(
            packets.iter().chain(tick_packets.iter()).any(|packet| {
                matches!(
                    packet,
                    ServerPacket::Magic {
                        spell: packet_spell,
                        cast: true,
                        ..
                    } if *packet_spell == spell
                ) || matches!(
                    packet,
                    ServerPacket::ObjectMagic {
                        spell: packet_spell,
                        cast: true,
                        ..
                    } if *packet_spell == spell
                ) || matches!(
                    packet,
                    ServerPacket::ObjectProjectile {
                        spell: packet_spell,
                        ..
                    } if *packet_spell == spell
                )
            }),
            "{spell_name} should emit visible Crystal magic packets"
        );
        if spell != Spell::HellFire {
            assert_target_damaged(&wizard, wasp_id, wasp_before, spell_name);
        }
    }

    let mut taoist =
        start_level_20_skill_session(MirClass::Taoist, MirGender::Male, &["Healing"], &[]);
    taoist.force_authoritative_player_transform(Point { x: 333, y: 267 }, MirDirection::Down);
    let damaged_hp = taoist.world_snapshot().player_max_hp.expect("max hp") / 2;
    let (character, mut save) =
        combat_save_at_level(MirClass::Taoist, MirGender::Male, 20, &["Healing"], &[]);
    save.hp = damaged_hp.max(1);
    let mut taoist = start_character("slice-l20-taoist-healing-low-hp", character, save);
    assert_knows_skill(&taoist, "Healing");
    let self_id = taoist
        .world_snapshot()
        .player_object_id
        .expect("player object id");
    let before_hp = taoist.world_snapshot().player_hp.expect("player hp");
    let packets = taoist.handle_packet(ClientPacket::Magic {
        object_id: self_id,
        spell: Spell::Healing,
        direction: MirDirection::Down,
        target_id: self_id,
        location: Point { x: 333, y: 267 },
        spell_target_lock: true,
    });
    let tick_packets = tick_many(&mut taoist, 5);
    assert!(packets.iter().chain(tick_packets.iter()).any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectEffect { info }
                if info.object_id == self_id && info.effect == 3
        )
    }));
    assert!(
        taoist.world_snapshot().player_hp.expect("player hp after") > before_hp,
        "Healing should restore the player"
    );

    let mut soul = start_level_20_skill_session(
        MirClass::Taoist,
        MirGender::Female,
        &["SoulFireBall"],
        &[("Amulet", EquipmentSlot::Amulet)],
    );
    assert_knows_skill(&soul, "SoulFireBall");
    let (packets, tick_packets, wasp_before, wasp_id) =
        cast_magic_on_field_wasp(&mut soul, Spell::SoulFireBall, true);
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::DeleteItem { count: 1, .. })));
    assert!(packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectProjectile {
                spell: Spell::SoulFireBall,
                destination_id,
                ..
            } if *destination_id == wasp_id
        )
    }));
    assert!(tick_packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectStruck { info }
                if info.object_id == wasp_id
        )
    }));
    assert_target_damaged(&soul, wasp_id, wasp_before, "SoulFireBall");

    let mut poison = start_level_20_skill_session(
        MirClass::Taoist,
        MirGender::Female,
        &["Poisoning"],
        &[("GreenPoison", EquipmentSlot::BraceletRight)],
    );
    assert_knows_skill(&poison, "Poisoning");
    let (packets, tick_packets, wasp_before, wasp_id) =
        cast_magic_on_field_wasp(&mut poison, Spell::Poisoning, true);
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::DeleteItem { count: 1, .. })));
    assert!(tick_packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectPoisoned {
                object_id,
                poison: 1,
            } if *object_id == wasp_id
        )
    }));
    assert_target_damaged(&poison, wasp_id, wasp_before, "Poisoning");

    let mut summon = start_level_20_skill_session(
        MirClass::Taoist,
        MirGender::Male,
        &["SummonSkeleton"],
        &[("Amulet", EquipmentSlot::Amulet)],
    );
    assert_knows_skill(&summon, "SummonSkeleton");
    summon.force_authoritative_player_transform(Point { x: 333, y: 300 }, MirDirection::Right);
    let self_id = summon
        .world_snapshot()
        .player_object_id
        .expect("player object id");
    let packets = summon.handle_packet(ClientPacket::Magic {
        object_id: self_id,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target_id: 0,
        location: Point { x: 334, y: 300 },
        spell_target_lock: false,
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::DeleteItem { count: 1, .. })));
    let tick_packets = tick_many(&mut summon, 6);
    assert!(tick_packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectMonster { info }
                if info.name == "BoneFamiliar" && info.extra
        )
    }));

    for (spell_name, key, spell) in [
        ("Haste", "haste", Spell::Haste),
        ("Hiding", "hiding", Spell::Hiding),
    ] {
        let mut assassin =
            start_level_20_skill_session(MirClass::Assassin, MirGender::Female, &[spell_name], &[]);
        assert_knows_skill(&assassin, spell_name);
        let object_id = assassin
            .world_snapshot()
            .player_object_id
            .expect("player object id");
        let packets = assassin.cast_skill(key);
        assert!(
            packets.iter().any(|packet| {
                matches!(
                    packet,
                    ServerPacket::AddBuff { buff } if buff.object_id == object_id
                ) || matches!(
                    packet,
                    ServerPacket::ObjectHidden {
                        object_id: packet_object_id,
                        hidden: true,
                    } if *packet_object_id == object_id && spell == Spell::Hiding
                )
            }),
            "{spell_name} should emit its self-buff surface: {packets:?}"
        );
    }
    let mut double = start_level_20_skill_session(
        MirClass::Assassin,
        MirGender::Female,
        &["FatalSword", "DoubleSlash"],
        &[],
    );
    assert_knows_skill(&double, "FatalSword");
    assert_knows_skill(&double, "DoubleSlash");
    let (packets, tick_packets, wasp_before, wasp_id) =
        cast_magic_on_field_wasp(&mut double, Spell::DoubleSlash, true);
    assert!(packets.iter().chain(tick_packets.iter()).any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectStruck { info }
                if info.object_id == wasp_id
        )
    }));
    assert_target_damaged(&double, wasp_id, wasp_before, "DoubleSlash");

    let mut focus =
        start_level_20_skill_session(MirClass::Archer, MirGender::Male, &["Focus"], &[]);
    assert_knows_skill(&focus, "Focus");
    let wasp = field_wasp(&mut focus);
    let before_hp = wasp.hp.expect("wasp hp");
    position_player_left_of_target(&mut focus, &wasp, 4);
    let self_position = self_player(&focus);
    let packets = focus.handle_packet(ClientPacket::RangeAttack {
        direction: direction_from_to(
            Point {
                x: self_position.x,
                y: self_position.y,
            },
            Point {
                x: wasp.x,
                y: wasp.y,
            },
        ),
        location: Point {
            x: self_position.x,
            y: self_position.y,
        },
        target_id: wasp.object_id,
        target_location: Point {
            x: wasp.x,
            y: wasp.y,
        },
    });
    assert!(
        packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::RangeAttack {
                    spell: Spell::Focus,
                    target_id,
                    ..
                } if *target_id == wasp.object_id
            ) || matches!(
                packet,
                ServerPacket::ObjectRangeAttack { info }
                    if info.object_id == focus.world_snapshot().player_object_id.unwrap()
                        && info.target_id == wasp.object_id
                        && info.spell == Spell::Focus as u8
            )
        }),
        "Focus packets: {packets:?}"
    );
    tick_many(&mut focus, 8);
    assert_target_damaged(&focus, wasp.object_id, before_hp, "Focus");

    for (spell_name, spell, expected_hits) in [
        ("StraightShot", Spell::StraightShot, 1_usize),
        ("DoubleShot", Spell::DoubleShot, 2_usize),
    ] {
        let mut archer =
            start_level_20_skill_session(MirClass::Archer, MirGender::Male, &[spell_name], &[]);
        assert_knows_skill(&archer, spell_name);
        let (_packets, tick_packets, before_hp, wasp_id) =
            cast_magic_on_field_wasp(&mut archer, spell, true);
        let hits = tick_packets
            .iter()
            .filter(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectStruck { info }
                        if info.object_id == wasp_id
                )
            })
            .count();
        assert!(
            hits >= expected_hits,
            "{spell_name} should hit {expected_hits} time(s), got {hits}: {tick_packets:?}"
        );
        assert_target_damaged(&archer, wasp_id, before_hp, spell_name);
    }

    let mut elemental =
        start_level_20_skill_session(MirClass::Archer, MirGender::Female, &["ElementalShot"], &[]);
    assert_knows_skill(&elemental, "ElementalShot");
    let self_id = elemental
        .world_snapshot()
        .player_object_id
        .expect("player object id");
    let wasp = field_wasp(&mut elemental);
    let before_hp = wasp.hp.expect("wasp hp");
    let target = Point {
        x: wasp.x,
        y: wasp.y,
    };
    let direction = position_player_left_of_target(&mut elemental, &wasp, 3);
    let gather_packets = elemental.handle_packet(ClientPacket::Magic {
        object_id: self_id,
        spell: Spell::ElementalShot,
        direction,
        target_id: wasp.object_id,
        location: target.clone(),
        spell_target_lock: true,
    });
    let gathered_elemental = gather_packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::SetElemental {
                object_id,
                enabled: true,
                ..
            } if *object_id == self_id
        )
    });
    if !gathered_elemental {
        return;
    }
    tick_many(&mut elemental, 3);
    let spend_packets = elemental.handle_packet(ClientPacket::Magic {
        object_id: self_id,
        spell: Spell::ElementalShot,
        direction,
        target_id: wasp.object_id,
        location: target,
        spell_target_lock: true,
    });
    let tick_packets = tick_many(&mut elemental, 6);
    assert!(spend_packets
        .iter()
        .chain(tick_packets.iter())
        .any(|packet| {
            matches!(
                packet,
                ServerPacket::SetElemental {
                    object_id,
                    enabled: false,
                    ..
                } if *object_id == self_id
            )
        }));
    assert!(tick_packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectStruck { info }
                if info.object_id == wasp.object_id
        )
    }));
    assert_target_damaged(&elemental, wasp.object_id, before_hp, "ElementalShot");
}

#[test]
fn bichon_starter_npc_monster_quest_drop_and_level_loop_closes() {
    let config = SimulationConfig::default();
    {
        let mut store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        let save = store
            .accounts
            .get_mut("demo")
            .and_then(|account| account.saves.get_mut(&0))
            .expect("demo starter save");
        save.position = Point { x: 327, y: 271 };
        save.direction = MirDirection::Left;
    }
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let guide = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::Npc && entity.name == "Village Guide")
        .expect("Village Guide should be visible");
    let npc_packets = session.handle_packet(ClientPacket::CallNpc {
        object_id: guide.object_id,
        key: "@Main".to_string(),
    });
    assert!(
        npc_packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectChat {
                    object_id,
                    text,
                    ..
                } if *object_id == guide.object_id && text.contains("Field Wasp")
            )
        }) || session.world_snapshot().active_npc_dialog.is_some(),
        "Village Guide should respond: {npc_packets:?}"
    );

    let accept_packets = session.handle_packet(ClientPacket::AcceptQuest {
        npc_index: 0,
        quest_index: 1001,
    });
    assert!(
        accept_packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ChangeQuest {
                    quest_id: 1001,
                    taken: true,
                    ..
                }
            )
        }),
        "accept packets: {accept_packets:?}"
    );
    assert!(session
        .world_snapshot()
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::InProgress));

    session.force_authoritative_player_transform(Point { x: 333, y: 267 }, MirDirection::UpRight);
    let combat_packets = attack_until_quest_ready(&mut session);
    let ready_snapshot = session.world_snapshot();
    assert!(
        ready_snapshot
            .quest_log
            .iter()
            .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::ReadyToTurnIn),
        "combat packets: {combat_packets:?}; quest log: {:?}",
        ready_snapshot.quest_log
    );
    assert!(
        ready_snapshot.inventory_items.iter().any(|item| {
            item.container == mir2_simulation::ItemContainer::Quest
                && item.key == "quest-wasp-stinger"
        }) || ready_snapshot.ground_drops.iter().any(|drop| {
            matches!(
                &drop.loot,
                GroundDropLootSnapshot::InventoryItem { key, .. } if key == "quest-wasp-stinger"
            )
        }),
        "quest proof should exist in quest inventory or visible drops: {:?}",
        ready_snapshot.inventory_items
    );

    session.force_authoritative_player_transform(Point { x: 327, y: 271 }, MirDirection::Left);
    let before_turn_in = session.world_snapshot();
    let finish_packets = session.handle_packet(ClientPacket::FinishQuest {
        quest_index: 1001,
        selected_item_index: -1,
    });
    let after_turn_in = session.world_snapshot();
    assert!(
        finish_packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::CompleteQuest { completed_quests }
                    if completed_quests.contains(&1001)
            )
        }),
        "finish packets: {finish_packets:?}"
    );
    assert!(after_turn_in
        .quest_log
        .iter()
        .any(|quest| quest.quest_id == 1001 && quest.stage == QuestStage::Completed));
    assert!(after_turn_in.gold > before_turn_in.gold);
    assert!(
        after_turn_in
            .equipment_items
            .iter()
            .any(|item| item.name == "Guide Ring")
            || after_turn_in
                .inventory_items
                .iter()
                .any(|item| item.name == "Guide Ring"),
        "turn-in should award Guide Ring: {:?}",
        after_turn_in.equipment_items
    );
}

#[test]
fn original_fresh_warrior_can_hunt_a_deer_with_starter_stats() {
    let mut session = start_original_bichon_intro_session();
    let packets = kill_original_monsters(
        &mut session,
        "0",
        &[Point { x: 247, y: 565 }, Point { x: 270, y: 625 }],
        "Deer",
        1,
    );

    assert_newcomer_alive(&session, "after first Deer hunt");
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id != 1_000
    )));
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainExperience { amount: 18 })));
    assert!(session.world_snapshot().player_experience >= 18);
}

#[test]
fn original_deer_harvest_q_drop_advances_quest_four() {
    let (character, mut save) = combat_save_at_level(
        MirClass::Warrior,
        MirGender::Male,
        45,
        &[],
        &[("SpiritBlade", EquipmentSlot::Weapon)],
    );
    save.map_file_name = "0".to_string();
    save.map_title = "BichonProvince".to_string();
    save.position = Point { x: 0, y: 0 };
    save.quest_states_json = vec![json!({
        "quest_id": 4,
        "title": "Hunt for the Butcher",
        "summary": "Collect DeerMeat by hunting Deer.",
        "reward_preview": "EXP 80, Gold 20, OldCopperRing x1",
        "required": 5,
        "current": 0,
        "stage": "inProgress",
        "task_progress": {}
    })
    .to_string()];
    let mut session = start_character_with_config(
        "slice-bichon-deer-q-drop",
        SimulationConfig::default().with_crystal_world_runtime(),
        character,
        save,
    );
    assert_quest_stage(&session, 4, QuestStage::InProgress);

    let packets = progress_original_item_quest_from_harvest_monster(
        &mut session,
        4,
        "0",
        &[
            Point { x: 210, y: 185 },
            Point { x: 265, y: 125 },
            Point { x: 310, y: 95 },
            Point { x: 205, y: 325 },
            Point { x: 260, y: 380 },
            Point { x: 295, y: 625 },
        ],
        "Deer",
        18,
    );

    assert_quest_stage(&session, 4, QuestStage::ReadyToTurnIn);
    assert!(packets.iter().any(|packet| {
        matches!(packet, ServerPacket::GainedItem { item } if item.item_index == 856)
    }));
    assert_eq!(
        session
            .world_snapshot()
            .inventory_items
            .iter()
            .filter(|item| item.name == "DeerMeat")
            .map(|item| item.quantity)
            .sum::<u32>(),
        5
    );
}

#[test]
fn original_bichon_fresh_warrior_reaches_level_six_through_quests_1_to_9() {
    let mut session = start_original_bichon_intro_session();
    let initial_player = self_player(&session);
    assert_eq!(initial_player.level, Some(1));
    assert_eq!((initial_player.x, initial_player.y), (288, 616));
    assert_eq!(session.world_snapshot().player_experience, 0);
    assert_eq!(session.world_snapshot().player_max_experience, 100);

    assert_quest_stage(&session, 1, QuestStage::Available);
    open_original_npc_dialog(
        &mut session,
        3,
        Point { x: 283, y: 606 },
        MirDirection::Right,
        "@quest:accept:1",
    );
    let accept_q1 = session.select_npc_dialog_target("@quest:accept:1");
    assert!(
        accept_q1.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ChangeQuest {
                    quest_id: 1,
                    taken: true,
                    completed: true,
                    ..
                }
            )
        }),
        "q1 accept should become ready immediately through carry-item semantics: {accept_q1:?}"
    );
    assert_quest_stage(&session, 1, QuestStage::ReadyToTurnIn);

    open_original_npc_dialog(
        &mut session,
        4,
        Point { x: 293, y: 619 },
        MirDirection::Right,
        "@quest:finish:1",
    );
    let before_q1_reward_exp = newcomer_cumulative_experience(&session);
    let finish_q1 = session.select_npc_dialog_target("@quest:finish:1");
    assert!(
        finish_q1.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::CompleteQuest { completed_quests }
                    if completed_quests.contains(&1)
            )
        }),
        "q1 finish packets: {finish_q1:?}"
    );
    assert_eq!(
        newcomer_cumulative_experience(&session) - before_q1_reward_exp,
        10,
        "q1 hand-in must grant its original 10 EXP"
    );
    assert_quest_stage(&session, 1, QuestStage::Completed);
    assert_quest_stage(&session, 2, QuestStage::Available);

    open_original_npc_dialog(
        &mut session,
        4,
        Point { x: 293, y: 619 },
        MirDirection::Right,
        "@quest:accept:2",
    );
    let accept_q2 = session.select_npc_dialog_target("@quest:accept:2");
    assert!(
        accept_q2.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ChangeQuest {
                    quest_id: 2,
                    taken: true,
                    completed: false,
                    ..
                }
            )
        }),
        "q2 accept packets: {accept_q2:?}"
    );
    assert_quest_stage(&session, 2, QuestStage::InProgress);

    open_original_npc_dialog(
        &mut session,
        5,
        Point { x: 295, y: 613 },
        MirDirection::Right,
        "@quest:accept:5",
    );
    let accept_q5 = session.select_npc_dialog_target("@quest:accept:5");
    assert!(
        accept_q5.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 5,
                taken: true,
                completed: false,
                ..
            }
        )),
        "q5 must be independently available at level 1: {accept_q5:?}"
    );
    assert_quest_stage(&session, 5, QuestStage::InProgress);
    let q5_initial = quest_snapshot(&session, 5).expect("q5 should be tracked");
    assert_eq!(
        (q5_initial.current, q5_initial.required),
        (0, 20),
        "q5 must require 10 Deer plus 10 Scarecrow"
    );

    let drop_packets = progress_original_item_quest_from_monster(
        &mut session,
        2,
        "0",
        &[
            Point { x: 270, y: 625 },
            Point { x: 293, y: 619 },
            Point { x: 330, y: 530 },
            Point { x: 110, y: 60 },
            Point { x: 220, y: 60 },
            Point { x: 200, y: 400 },
            Point { x: 500, y: 400 },
            Point { x: 540, y: 530 },
        ],
        "Scarecrow",
        30,
    );
    let q2_ready = quest_snapshot(&session, 2).expect("q2 should stay visible");
    let killed_scarecrow_ids = drop_packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::ObjectDied { info } => Some(info.object_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    let dropped_item_names = drop_packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::ObjectItem { info } => Some(info.name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        q2_ready.stage,
        QuestStage::ReadyToTurnIn,
        "Scarecrow Q drop should advance q2: {q2_ready:?}; killed={killed_scarecrow_ids:?}; drops={dropped_item_names:?}; packet_count={}",
        drop_packets.len()
    );
    assert!(
        drop_packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::GainedItem { item } if item.item_index == 1112
            )
        }) || session.world_snapshot().inventory_items.iter().any(|item| {
            item.container == mir2_simulation::ItemContainer::Quest && item.name == "GingerTea"
        }),
        "q2 should gain the original GingerTea quest item: {drop_packets:?}"
    );

    let _ = session.transfer_map("crystal:0:283:606");
    open_original_npc_dialog(
        &mut session,
        3,
        Point { x: 283, y: 606 },
        MirDirection::Right,
        "@quest:finish:2",
    );
    let before_q2_reward_exp = newcomer_cumulative_experience(&session);
    let finish_q2 = session.select_npc_dialog_target("@quest:finish:2");
    assert!(
        finish_q2.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::CompleteQuest { completed_quests }
                    if completed_quests.contains(&2)
            )
        }),
        "q2 finish packets: {finish_q2:?}"
    );
    assert_eq!(
        newcomer_cumulative_experience(&session) - before_q2_reward_exp,
        30,
        "q2 hand-in must grant its original 30 EXP"
    );
    assert_quest_stage(&session, 2, QuestStage::Completed);
    assert_quest_stage(&session, 3, QuestStage::Available);

    open_original_npc_dialog(
        &mut session,
        3,
        Point { x: 283, y: 606 },
        MirDirection::Right,
        "@quest:accept:3",
    );
    let accept_q3 = session.select_npc_dialog_target("@quest:accept:3");
    assert!(
        accept_q3.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ChangeQuest {
                    quest_id: 3,
                    taken: true,
                    completed: true,
                    ..
                }
            )
        }),
        "q3 accept should become ready immediately: {accept_q3:?}"
    );
    assert_quest_stage(&session, 3, QuestStage::ReadyToTurnIn);

    open_original_npc_dialog(
        &mut session,
        6,
        Point { x: 291, y: 603 },
        MirDirection::Right,
        "@quest:finish:3",
    );
    let choose_q3_reward = session.select_npc_dialog_target("@quest:finish:3");
    assert!(
        !choose_q3_reward
            .iter()
            .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })),
        "q3 must not finish before a starter weapon is selected"
    );
    assert_quest_stage(&session, 3, QuestStage::ReadyToTurnIn);
    let q3_reward_dialog = session
        .world_snapshot()
        .active_npc_dialog
        .expect("q3 reward selection dialog");
    assert!(q3_reward_dialog
        .links
        .iter()
        .any(|link| link.target == "@quest:finish:3:0" && link.text.contains("SharpDagger")));
    assert!(q3_reward_dialog
        .links
        .iter()
        .any(|link| link.target == "@quest:finish:3:1" && link.text.contains("ToughHoaSword")));
    assert!(q3_reward_dialog
        .links
        .iter()
        .any(|link| link.target == "@quest:finish:3:2" && link.text.contains("StiffWoodenBow")));
    let before_q3_reward_exp = newcomer_cumulative_experience(&session);
    let finish_q3 = session.select_npc_dialog_target("@quest:finish:3:0");
    assert!(
        finish_q3.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::CompleteQuest { completed_quests }
                    if completed_quests.contains(&3)
            )
        }),
        "q3 finish packets: {finish_q3:?}"
    );
    assert_eq!(
        newcomer_cumulative_experience(&session) - before_q3_reward_exp,
        10,
        "q3 hand-in must grant its original 10 EXP"
    );
    assert_quest_stage(&session, 3, QuestStage::Completed);
    assert!(session
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.name == "SharpDagger"));
    if self_player(&session).level.is_some_and(|level| level >= 2) {
        equip_inventory_item_by_name(&mut session, "SharpDagger", 0);
    }
    assert_quest_stage(&session, 4, QuestStage::Available);

    open_original_npc_dialog(
        &mut session,
        6,
        Point { x: 291, y: 603 },
        MirDirection::Right,
        "@quest:accept:4",
    );
    let accept_q4 = session.select_npc_dialog_target("@quest:accept:4");
    assert!(
        accept_q4.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ChangeQuest {
                    quest_id: 4,
                    taken: true,
                    completed: false,
                    ..
                }
            )
        }),
        "q4 accept packets: {accept_q4:?}"
    );
    assert_quest_stage(&session, 4, QuestStage::InProgress);

    let deer_packets = progress_original_item_quest_from_harvest_monster(
        &mut session,
        4,
        "0",
        &[
            Point { x: 273, y: 614 },
            Point { x: 247, y: 565 },
            Point { x: 270, y: 625 },
            Point { x: 295, y: 625 },
            Point { x: 340, y: 550 },
            Point { x: 210, y: 185 },
            Point { x: 265, y: 125 },
            Point { x: 310, y: 95 },
            Point { x: 205, y: 325 },
            Point { x: 260, y: 380 },
        ],
        "Deer",
        18,
    );
    let q4_ready = quest_snapshot(&session, 4).expect("q4 should stay visible");
    assert_eq!(
        q4_ready.stage,
        QuestStage::ReadyToTurnIn,
        "Deer harvest must produce all five DeerMeat items: {q4_ready:?}; packets={}",
        deer_packets.len()
    );
    assert!(
        deer_packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::GainedItem { item } if item.item_index == 856
            )
        }) || session.world_snapshot().inventory_items.iter().any(|item| {
            item.container == mir2_simulation::ItemContainer::Quest && item.name == "DeerMeat"
        }),
        "q4 should gain original DeerMeat quest items: {deer_packets:?}"
    );

    let _ = session.transfer_map("crystal:0:291:603");
    open_original_npc_dialog(
        &mut session,
        6,
        Point { x: 291, y: 603 },
        MirDirection::Right,
        "@quest:finish:4",
    );
    let before_q4_reward_exp = newcomer_cumulative_experience(&session);
    let finish_q4 = session.select_npc_dialog_target("@quest:finish:4");
    assert!(
        finish_q4.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::CompleteQuest { completed_quests }
                    if completed_quests.contains(&4)
            )
        }),
        "q4 finish packets: {finish_q4:?}"
    );
    assert_eq!(
        newcomer_cumulative_experience(&session) - before_q4_reward_exp,
        80,
        "q4 hand-in must grant its original 80 EXP"
    );
    assert_quest_stage(&session, 4, QuestStage::Completed);

    const DEER_FIELDS: [Point; 6] = [
        Point { x: 273, y: 614 },
        Point { x: 210, y: 185 },
        Point { x: 265, y: 125 },
        Point { x: 205, y: 325 },
        Point { x: 260, y: 380 },
        Point { x: 295, y: 625 },
    ];
    const SCARECROW_FIELDS: [Point; 6] = [
        Point { x: 110, y: 60 },
        Point { x: 220, y: 60 },
        Point { x: 200, y: 400 },
        Point { x: 500, y: 400 },
        Point { x: 330, y: 530 },
        Point { x: 270, y: 625 },
    ];
    let mut q5_hunt_packets = Vec::new();
    for _ in 0..20 {
        if quest_snapshot(&session, 5).is_some_and(|quest| quest.stage == QuestStage::ReadyToTurnIn)
        {
            break;
        }
        q5_hunt_packets.extend(kill_original_monsters(
            &mut session,
            "0",
            &DEER_FIELDS,
            "Deer",
            1,
        ));
        if quest_snapshot(&session, 5).is_some_and(|quest| quest.stage == QuestStage::ReadyToTurnIn)
        {
            break;
        }
        q5_hunt_packets.extend(kill_original_monsters(
            &mut session,
            "0",
            &SCARECROW_FIELDS,
            "Scarecrow",
            1,
        ));
    }
    assert_quest_stage(&session, 5, QuestStage::ReadyToTurnIn);
    let deer_kills = deer_packets
        .iter()
        .chain(q5_hunt_packets.iter())
        .filter(|packet| matches!(packet, ServerPacket::GainExperience { amount: 18 }))
        .count();
    let scarecrow_kills = drop_packets
        .iter()
        .chain(q5_hunt_packets.iter())
        .filter(|packet| matches!(packet, ServerPacket::GainExperience { amount: 15 }))
        .count();
    assert!(
        deer_kills >= 10 && scarecrow_kills >= 10,
        "q5 cannot complete before 10 real kills of each target: Deer={deer_kills}, Scarecrow={scarecrow_kills}"
    );

    let _ = session.transfer_map("crystal:0:295:613");
    open_original_npc_dialog(
        &mut session,
        5,
        Point { x: 295, y: 613 },
        MirDirection::Right,
        "@quest:finish:5",
    );
    let before_q5_reward_exp = newcomer_cumulative_experience(&session);
    let finish_q5 = session.select_npc_dialog_target("@quest:finish:5");
    assert!(finish_q5.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&5)
    )));
    assert_eq!(
        newcomer_cumulative_experience(&session) - before_q5_reward_exp,
        120,
        "q5 hand-in must grant its original 120 EXP"
    );
    assert_quest_stage(&session, 5, QuestStage::Completed);
    assert_quest_stage(&session, 6, QuestStage::Available);

    equip_inventory_item_by_name(&mut session, "GoldenPendant", 4);
    equip_inventory_item_by_name(&mut session, "WornIronBracelet", 5);
    equip_inventory_item_by_name(&mut session, "CopperRing", 7);
    equip_inventory_item_by_name(&mut session, "OldCopperRing", 8);

    let _ = buy_original_small_hp_drugs(&mut session, 6);

    open_original_npc_dialog(
        &mut session,
        5,
        Point { x: 295, y: 613 },
        MirDirection::Right,
        "@quest:accept:6",
    );
    let accept_q6 = session.select_npc_dialog_target("@quest:accept:6");
    assert!(accept_q6.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 6,
            taken: true,
            completed: false,
            ..
        }
    )));
    const CAT_FIELDS: [Point; 5] = [
        Point { x: 180, y: 420 },
        Point { x: 110, y: 80 },
        Point { x: 150, y: 130 },
        Point { x: 340, y: 550 },
        Point { x: 510, y: 410 },
    ];
    let _ = kill_original_monsters(&mut session, "0", &CAT_FIELDS, "HookingCat", 10);
    assert_quest_stage(&session, 6, QuestStage::ReadyToTurnIn);

    let _ = session.transfer_map("crystal:0:295:613");
    open_original_npc_dialog(
        &mut session,
        5,
        Point { x: 295, y: 613 },
        MirDirection::Right,
        "@quest:finish:6",
    );
    let choose_q6_reward = session.select_npc_dialog_target("@quest:finish:6");
    assert!(!choose_q6_reward
        .iter()
        .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. })));
    assert_quest_stage(&session, 6, QuestStage::ReadyToTurnIn);
    let q6_reward_dialog = session
        .world_snapshot()
        .active_npc_dialog
        .expect("q6 reward selection dialog");
    assert!(q6_reward_dialog.links.iter().any(|link| {
        link.target == "@quest:finish:6:0" && link.text.contains("BronzeWarriorSword")
    }));
    let before_q6_reward_exp = newcomer_cumulative_experience(&session);
    let finish_q6 = session.select_npc_dialog_target("@quest:finish:6:0");
    assert!(finish_q6.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&6)
    )));
    assert_eq!(
        newcomer_cumulative_experience(&session) - before_q6_reward_exp,
        150,
        "q6 hand-in must grant its original 150 EXP"
    );
    assert_quest_stage(&session, 6, QuestStage::Completed);
    assert!(session
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.name == "BronzeWarriorSword"));
    assert!(self_player(&session).level.is_some_and(|level| level >= 4));

    let _ = session.transfer_map("crystal:0:283:606");
    open_original_npc_dialog(
        &mut session,
        3,
        Point { x: 283, y: 606 },
        MirDirection::Right,
        "@quest:accept:7",
    );
    let accept_q7 = session.select_npc_dialog_target("@quest:accept:7");
    assert!(accept_q7.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 7,
            taken: true,
            completed: true,
            ..
        }
    )));
    assert_quest_stage(&session, 7, QuestStage::ReadyToTurnIn);

    let _ = session.transfer_map("crystal:0:109:317");
    open_original_npc_dialog(
        &mut session,
        10,
        Point { x: 109, y: 317 },
        MirDirection::Right,
        "@quest:finish:7",
    );
    let before_q7_reward_exp = newcomer_cumulative_experience(&session);
    let finish_q7 = session.select_npc_dialog_target("@quest:finish:7");
    assert!(finish_q7.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&7)
    )));
    assert_eq!(
        newcomer_cumulative_experience(&session) - before_q7_reward_exp,
        48,
        "q7 hand-in must grant its original 48 EXP"
    );
    let q7_snapshot = session.world_snapshot();
    assert!(
        self_player(&session).level.is_some_and(|level| level >= 5),
        "q7 should unlock the level-5 BronzeWarriorSword: level={:?}, exp={}, next={}",
        self_player(&session).level,
        q7_snapshot.player_experience,
        q7_snapshot.player_max_experience,
    );
    equip_inventory_item_by_name(&mut session, "BronzeWarriorSword", 0);

    let _ = buy_original_small_hp_drugs(&mut session, 2);
    let _ = session.transfer_map("crystal:0:109:317");

    open_original_npc_dialog(
        &mut session,
        10,
        Point { x: 109, y: 317 },
        MirDirection::Right,
        "@quest:accept:8",
    );
    let accept_q8 = session.select_npc_dialog_target("@quest:accept:8");
    assert!(accept_q8.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 8,
            taken: true,
            completed: false,
            ..
        }
    )));
    const OMA_FIELDS: [Point; 5] = [
        Point { x: 220, y: 470 },
        Point { x: 180, y: 420 },
        Point { x: 90, y: 240 },
        Point { x: 110, y: 440 },
        Point { x: 140, y: 500 },
    ];
    const RAKING_CAT_FIELDS: [Point; 4] = [
        Point { x: 140, y: 100 },
        Point { x: 180, y: 420 },
        Point { x: 340, y: 550 },
        Point { x: 510, y: 410 },
    ];
    let _ = kill_original_monsters(&mut session, "0", &OMA_FIELDS, "Oma", 10);
    let _ = kill_original_monsters(&mut session, "0", &RAKING_CAT_FIELDS, "RakingCat", 10);
    assert_quest_stage(&session, 8, QuestStage::ReadyToTurnIn);

    let _ = session.transfer_map("crystal:0:109:317");
    open_original_npc_dialog(
        &mut session,
        10,
        Point { x: 109, y: 317 },
        MirDirection::Right,
        "@quest:finish:8",
    );
    let before_q8_reward_exp = newcomer_cumulative_experience(&session);
    let finish_q8 = session.select_npc_dialog_target("@quest:finish:8");
    assert!(finish_q8.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&8)
    )));
    assert_eq!(
        newcomer_cumulative_experience(&session) - before_q8_reward_exp,
        180,
        "q8 hand-in must grant its original 180 EXP"
    );
    assert_quest_stage(&session, 8, QuestStage::Completed);

    open_original_npc_dialog(
        &mut session,
        10,
        Point { x: 109, y: 317 },
        MirDirection::Right,
        "@quest:accept:9",
    );
    let accept_q9 = session.select_npc_dialog_target("@quest:accept:9");
    assert!(accept_q9.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 9,
            taken: true,
            completed: true,
            ..
        }
    )));

    let _ = session.transfer_map("crystal:0:327:258");
    open_original_npc_dialog(
        &mut session,
        26,
        Point { x: 327, y: 258 },
        MirDirection::Right,
        "@quest:finish:9",
    );
    let before_q9_reward_exp = newcomer_cumulative_experience(&session);
    let finish_q9 = session.select_npc_dialog_target("@quest:finish:9");
    assert!(finish_q9.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&9)
    )));
    assert_eq!(
        newcomer_cumulative_experience(&session) - before_q9_reward_exp,
        48,
        "q9 hand-in must grant its original 48 EXP"
    );

    for quest_id in 1..=9 {
        assert_quest_stage(&session, quest_id, QuestStage::Completed);
    }
    let final_snapshot = session.world_snapshot();
    assert_eq!(self_player(&session).level, Some(6));
    assert_eq!(final_snapshot.player_max_experience, 900);
    assert_eq!(
        final_snapshot.gold,
        453 - (8 * 40),
        "all nine quests grant 453 gold before the eight original small-HP-drug purchases"
    );
    for reward_name in ["WornIronBracelet", "OldLoafer", "Fencing"] {
        assert!(
            final_snapshot
                .inventory_items
                .iter()
                .chain(final_snapshot.belt_items.iter())
                .any(|item| item.name == reward_name)
                || final_snapshot
                    .equipment_items
                    .iter()
                    .any(|item| item.name == reward_name),
            "{reward_name} should be retained at the end of the newcomer arc"
        );
    }
}

#[test]
fn shared_multiplayer_presence_movement_chat_and_drop_ownership_are_stable() {
    let first = session_id("slice-a");
    let second = session_id("slice-b");
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());

    let first_join = zone.handle(ZoneCommand::Join(zone_join(
        "slice-a", 5_001, "Alice", 330, 270,
    )));
    let second_join = zone.handle(ZoneCommand::Join(zone_join(
        "slice-b", 5_002, "Bob", 330, 271,
    )));
    assert!(packets_for(&second_join, &second).iter().any(
        |packet| matches!(packet, ServerPacket::ObjectPlayer { info } if info.object_id == 5_001)
    ));
    assert!(
        packets_for(&second_join, &first)
            .iter()
            .chain(packets_for(&first_join, &first).iter())
            .any(|packet| matches!(packet, ServerPacket::ObjectPlayer { info } if info.object_id == 5_002)),
        "join should make both players visible"
    );

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let walk = zone.tick(0);
    assert!(packets_for(&walk, &first)
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
    assert!(packets_for(&walk, &second).iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectWalk { movement }
                if movement.object_id == 5_001 && movement.direction == MirDirection::Right
        )
    }));

    zone.handle(ZoneCommand::Run {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    let run = zone.tick(600);
    assert!(packets_for(&run, &second).iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectRun { movement }
                if movement.object_id == 5_001 && movement.direction == MirDirection::Right
        )
    }));

    zone.handle(ZoneCommand::Turn {
        session_id: first.clone(),
        direction: MirDirection::Left,
        now_ms: 0,
    });
    let turn = zone.tick(1_200);
    assert!(packets_for(&turn, &second).iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectTurn { movement }
                if movement.object_id == 5_001 && movement.direction == MirDirection::Left
        )
    }));

    let chat = zone.handle(ZoneCommand::Chat {
        session_id: first.clone(),
        message: "slice hello".to_string(),
        linked_items: Vec::new(),
        linked_user_items: Vec::new(),
        now_ms: 2_000,
    });
    assert!(packets_for(&chat, &second).iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectChat {
                object_id: 5_001,
                text,
                chat_type: ChatType::Normal
            } if text.contains("slice hello")
        )
    }));

    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![GroundDropSnapshot {
            object_id: 9_001,
            name: "Wasp Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: 333,
            y: 270,
            quantity: 8,
            source_monster: "Field Wasp".to_string(),
            owner_object_id: Some(5_001),
            ownership_remaining_ticks: Some(20),
            loot: GroundDropLootSnapshot::Gold { amount: 8 },
        }],
        now_ms: 2_100,
    });
    let second_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: second.clone(),
        object_id: Some(9_001),
        target: Point { x: 333, y: 270 },
        group_members: Vec::new(),
        now_ms: 2_200,
    });
    assert!(packets_for(&second_claim, &second).iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::Chat {
                chat_type: ChatType::System,
                ..
            }
        )
    }));
    assert!(!second_claim
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::GroundDropClaimed { .. })));

    let first_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first,
        object_id: Some(9_001),
        target: Point { x: 333, y: 270 },
        group_members: Vec::new(),
        now_ms: 2_300,
    });
    assert!(first_claim.iter().any(|outbound| {
        matches!(
            outbound,
            ZoneOutbound::GroundDropClaimed {
                session_id,
                drop
            } if session_id.as_str() == "slice-a" && drop.object_id == 9_001
        )
    }));
}
