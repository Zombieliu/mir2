use mir2_game_data::{
    crystal_item_by_name, crystal_monster_manifest, crystal_respawn_manifest, platinum_176_profile,
};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, EquipmentSlot, SimulationConfig,
    SimulationSession, WorldEntityKind,
};
use serde::Serialize;
use serde_json::json;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const COMBAT_REPORT_ENV: &str = "MIR2_PLATINUM_COMBAT_REPORT";
const MAX_ACTIONS: usize = 4;
const TICKS_PER_ACTION: usize = 2;

#[derive(Clone, Copy)]
struct Milestone {
    level: u16,
    map_file_name: &'static str,
    monster_name: &'static str,
}

const MILESTONES: [Milestone; 5] = [
    Milestone {
        level: 22,
        map_file_name: "D003",
        monster_name: "BoneWarrior",
    },
    Milestone {
        level: 35,
        map_file_name: "D612",
        monster_name: "Tongs",
    },
    Milestone {
        level: 40,
        map_file_name: "D502",
        monster_name: "GiantRat",
    },
    Milestone {
        level: 45,
        map_file_name: "D504",
        monster_name: "ZumaGuardian",
    },
    Milestone {
        level: 50,
        map_file_name: "D10052",
        monster_name: "BigApe",
    },
];

#[derive(Clone, Copy)]
enum MeasuredAction {
    Melee(Spell),
    Magic(Spell),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CombatMilestoneReport {
    schema: &'static str,
    generated_at_unix_ms: u128,
    profile_id: String,
    profile_version: u32,
    fixture_notice: &'static str,
    cases: Vec<CombatCaseReport>,
    assertions: CombatAssertions,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CombatCaseReport {
    class: &'static str,
    level: u16,
    map_file_name: String,
    monster_name: String,
    monster_level: u16,
    monster_max_hp: i32,
    monster_ac: String,
    monster_mac: String,
    weapon: String,
    armour: String,
    reagent: Option<String>,
    measured_skill: String,
    known_profile_skills: Vec<String>,
    nearby_hostiles_at_start: usize,
    actions_issued: usize,
    action_packets: usize,
    ticks_observed: usize,
    damage_model: &'static str,
    damage: i32,
    player_hp_before: i32,
    player_hp_after: i32,
    player_mp_before: i32,
    player_mp_after: i32,
    player_damage_taken: i32,
    mp_spent: i32,
    estimated_actions_to_kill: Option<u32>,
    emitted_combat_packet: bool,
    profile_sources_valid: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CombatAssertions {
    all_fifteen_cases_measured: bool,
    every_target_is_profile_map_real_respawn: bool,
    every_fixture_item_and_skill_is_profile_allowed: bool,
    every_action_emits_combat_packet: bool,
    every_case_deals_positive_damage: bool,
    no_case_kills_the_player_during_observation: bool,
    all_classes_replace_weapon_and_armour_by_level_40: bool,
}

fn class_name(class: MirClass) -> &'static str {
    match class {
        MirClass::Warrior => "Warrior",
        MirClass::Wizard => "Wizard",
        MirClass::Taoist => "Taoist",
        _ => panic!("post-1.76 class {class:?} is outside this certificate"),
    }
}

fn equipment_for(class: MirClass, level: u16) -> (&'static str, &'static str) {
    match (class, level >= 40) {
        (MirClass::Warrior, false) => ("PowerAxe", "HeavyArmour(M)"),
        (MirClass::Warrior, true) => ("DragonSlayer", "SteelArmour(M)"),
        (MirClass::Wizard, false) => ("MageStaff", "MagicRobe(M)"),
        (MirClass::Wizard, true) => ("DragonStaff", "DragonRobe(M)"),
        (MirClass::Taoist, false) => ("SerpentSword", "SoulArmour(M)"),
        (MirClass::Taoist, true) => ("SoulSabre", "TitanArmour(M)"),
        _ => panic!("unsupported class {class:?}"),
    }
}

fn measured_action_for(class: MirClass, level: u16) -> (&'static str, MeasuredAction) {
    match class {
        MirClass::Warrior if level >= 35 => {
            ("FlamingSword", MeasuredAction::Melee(Spell::FlamingSword))
        }
        MirClass::Warrior => ("Slaying", MeasuredAction::Melee(Spell::Slaying)),
        MirClass::Wizard if level >= 35 => ("IceStorm", MeasuredAction::Magic(Spell::IceStorm)),
        MirClass::Wizard => ("GreatFireBall", MeasuredAction::Magic(Spell::GreatFireBall)),
        MirClass::Taoist if level >= 35 => ("Poisoning", MeasuredAction::Magic(Spell::Poisoning)),
        MirClass::Taoist => ("SoulFireBall", MeasuredAction::Magic(Spell::SoulFireBall)),
        _ => panic!("unsupported class {class:?}"),
    }
}

fn reagent_for(class: MirClass, measured_skill: &str) -> Option<&'static str> {
    match (class, measured_skill) {
        (MirClass::Taoist, "Poisoning") => Some("GreenPoison"),
        (MirClass::Taoist, "SoulFireBall") => Some("Amulet"),
        _ => None,
    }
}

fn skill_state_json(spell_name: &str) -> String {
    json!({
        "key": spell_name
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            })
            .collect::<String>(),
        "name": spell_name,
        "description": format!("Platinum 1.76 measured milestone fixture for {spell_name}."),
        "level": 3,
        "experience": 0,
        "hotkey": 0,
        "cooldown_ticks": 1,
        "delay_ms": 1,
        "cooldown_ends_at": 0,
        "cast_time_ms": 0
    })
    .to_string()
}

fn equipment_state_json(template_name: &str, slot: EquipmentSlot) -> String {
    let template = crystal_item_by_name(template_name)
        .unwrap_or_else(|| panic!("{template_name} should exist in the Crystal item manifest"));
    let durability = template.durability.max(1);
    json!({
        "key": format!("crystal-item-{}", template.item_index),
        "slot": slot,
        "name": template.name,
        "icon": template.image,
        "shape": u16::try_from(template.shape).ok(),
        "description": template.tooltip.unwrap_or_default(),
        "durability_current": durability,
        "durability_max": durability,
        "socket_slots": template.slots,
        "attack": 0,
        "defence": 0
    })
    .to_string()
}

fn start_milestone_session(
    class: MirClass,
    milestone: Milestone,
    weapon: &str,
    armour: &str,
    measured_skill: &str,
) -> SimulationSession {
    let account_id = format!(
        "p176-measured-{}-{}",
        class_name(class).to_ascii_lowercase(),
        milestone.level
    );
    let character = CharacterRecord {
        index: 0,
        name: format!("Measured{}{}", class_name(class), milestone.level),
        level: milestone.level,
        class,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.map_file_name = milestone.map_file_name.to_string();
    save.map_title = milestone.map_file_name.to_string();
    save.position = Point { x: 100, y: 100 };
    save.direction = MirDirection::Right;
    save.hp = save.max_hp;
    save.mp = save.max_mp;
    save.skill_states_json = vec![skill_state_json(measured_skill)];
    save.equipment_items_json = vec![
        equipment_state_json(weapon, EquipmentSlot::Weapon),
        equipment_state_json(armour, EquipmentSlot::Armour),
    ];
    if let Some(reagent) = reagent_for(class, measured_skill) {
        save.equipment_items_json
            .push(equipment_state_json(reagent, EquipmentSlot::Amulet));
    }

    let config = SimulationConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_176_profile();
    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned")
        .accounts
        .insert(account_id.clone(), account);

    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id,
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    session
}

fn emitted_player_combat_packet(packet: &ServerPacket, player_object_id: u32) -> bool {
    matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == player_object_id
    ) || matches!(
        packet,
        ServerPacket::ObjectMagic { object_id, .. } if *object_id == player_object_id
    ) || matches!(
        packet,
        ServerPacket::ObjectProjectile { source_id, .. } if *source_id == player_object_id
    ) || matches!(
        packet,
        ServerPacket::ObjectStruck { info } if info.attacker_id == player_object_id
    )
}

fn visible_target(
    session: &SimulationSession,
    monster_name: &str,
) -> Option<mir2_simulation::WorldEntitySnapshot> {
    let entities = session.world_snapshot().entities;
    entities
        .iter()
        .filter(|entity| {
            entity.kind == WorldEntityKind::Monster
                && entity.name == monster_name
                && !entity.dead
                && entity.hp.is_some_and(|hp| hp > 0)
        })
        .min_by_key(|target| {
            let left_occupied = entities.iter().any(|entity| {
                entity.object_id != target.object_id
                    && !entity.dead
                    && entity.x == target.x.saturating_sub(1)
                    && entity.y == target.y
            });
            let nearby_hostiles = entities
                .iter()
                .filter(|entity| {
                    entity.object_id != target.object_id
                        && entity.kind == WorldEntityKind::Monster
                        && !entity.dead
                        && entity.x.abs_diff(target.x) <= 10
                        && entity.y.abs_diff(target.y) <= 10
                })
                .count();
            (left_occupied, nearby_hostiles, target.object_id)
        })
        .cloned()
}

fn nearby_hostile_count(session: &SimulationSession, target_object_id: u32) -> usize {
    let snapshot = session.world_snapshot();
    let target = snapshot
        .entities
        .iter()
        .find(|entity| entity.object_id == target_object_id)
        .expect("measured target should remain visible before combat");
    snapshot
        .entities
        .iter()
        .filter(|entity| {
            entity.object_id != target.object_id
                && entity.kind == WorldEntityKind::Monster
                && !entity.dead
                && entity.x.abs_diff(target.x) <= 10
                && entity.y.abs_diff(target.y) <= 10
        })
        .count()
}

fn load_real_respawn_target(
    session: &mut SimulationSession,
    milestone: Milestone,
) -> mir2_simulation::WorldEntitySnapshot {
    if let Some(target) = visible_target(session, milestone.monster_name) {
        return target;
    }
    let manifest = crystal_respawn_manifest();
    let map = manifest
        .maps
        .iter()
        .find(|map| map.map_file_name == milestone.map_file_name)
        .unwrap_or_else(|| panic!("{} should exist in respawns", milestone.map_file_name));
    for respawn in map
        .respawns
        .iter()
        .filter(|respawn| respawn.monster_name == milestone.monster_name)
    {
        session.transfer_map(&format!(
            "crystal:{}:{}:{}",
            milestone.map_file_name, respawn.location.x, respawn.location.y
        ));
        if let Some(target) = visible_target(session, milestone.monster_name) {
            return target;
        }
    }
    panic!(
        "{} should spawn on {} after loading its real respawn centers",
        milestone.monster_name, milestone.map_file_name
    )
}

fn measure_case(class: MirClass, milestone: Milestone) -> CombatCaseReport {
    let profile = platinum_176_profile();
    let monsters = crystal_monster_manifest();
    let monster = monsters
        .monsters
        .iter()
        .find(|monster| monster.name == milestone.monster_name)
        .unwrap_or_else(|| panic!("{} should exist", milestone.monster_name));
    let (weapon, armour) = equipment_for(class, milestone.level);
    let (measured_skill, action) = measured_action_for(class, milestone.level);
    let reagent = reagent_for(class, measured_skill);
    let known_profile_skills = profile
        .skills
        .iter()
        .filter(|skill| skill.class == class && skill.required_level <= milestone.level)
        .map(|skill| skill.spell.clone())
        .collect::<Vec<_>>();
    let mut session = start_milestone_session(class, milestone, weapon, armour, measured_skill);
    let target = load_real_respawn_target(&mut session, milestone);
    let nearby_hostiles_at_start = nearby_hostile_count(&session, target.object_id);
    // The production skill integration tests use adjacent target-lock casts.
    // Keeping every class at the same one-tile distance also prevents a range
    // rejection from being mistaken for zero combat output.
    session.force_authoritative_player_transform(
        Point {
            x: target.x.saturating_sub(1),
            y: target.y,
        },
        MirDirection::Right,
    );

    let before = session.world_snapshot();
    let player_hp_before = before.player_hp.expect("player hp before action");
    let player_mp_before = before.player_mp.expect("player mp before action");
    let target_hp_before = target.hp.expect("target hp before action");
    let self_object_id = before.player_object_id.expect("player object id");
    let mut packets = Vec::new();
    if let MeasuredAction::Melee(spell) = action {
        packets.extend(session.handle_packet(ClientPacket::SpellToggle {
            spell,
            toggle_state: 1,
        }));
    }
    let mut actions_issued = 0;
    let mut ticks_observed = 0;
    'actions: for _ in 0..MAX_ACTIONS {
        let Some(current_target) = session
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.object_id == target.object_id && !entity.dead)
        else {
            break;
        };
        if current_target.hp.unwrap_or(0) < target_hp_before {
            break;
        }
        let target_location = Point {
            x: current_target.x,
            y: current_target.y,
        };
        session.force_authoritative_player_transform(
            Point {
                x: current_target.x.saturating_sub(1),
                y: current_target.y,
            },
            MirDirection::Right,
        );
        let action_packets = match action {
            MeasuredAction::Melee(spell) => session.handle_packet(ClientPacket::Attack {
                direction: MirDirection::Right,
                spell,
            }),
            MeasuredAction::Magic(spell) => session.handle_packet(ClientPacket::Magic {
                object_id: self_object_id,
                spell,
                direction: MirDirection::Right,
                target_id: target.object_id,
                location: target_location,
                spell_target_lock: true,
            }),
        };
        actions_issued += 1;
        packets.extend(action_packets);
        for _ in 0..TICKS_PER_ACTION {
            packets.extend(session.tick());
            ticks_observed += 1;
            let snapshot = session.world_snapshot();
            let current = snapshot
                .entities
                .iter()
                .find(|entity| entity.object_id == target.object_id);
            if current.is_none_or(|entity| entity.dead || entity.hp.unwrap_or(0) < target_hp_before)
                || snapshot.player_hp.unwrap_or(0) == 0
            {
                break 'actions;
            }
        }
    }

    let after = session.world_snapshot();
    let target_hp_after = after
        .entities
        .iter()
        .find(|entity| entity.object_id == target.object_id)
        .and_then(|entity| entity.hp)
        .unwrap_or(0)
        .max(0);
    let damage = target_hp_before.saturating_sub(target_hp_after);
    let damage_model = if measured_skill == "Poisoning" {
        "damage-over-time-first-observed-pulse"
    } else {
        "direct"
    };
    let player_hp_after = after.player_hp.unwrap_or(0);
    let player_mp_after = after.player_mp.unwrap_or(0);
    let respawn_is_real = crystal_respawn_manifest().maps.iter().any(|map| {
        map.map_file_name == milestone.map_file_name
            && map
                .respawns
                .iter()
                .any(|respawn| respawn.monster_name == milestone.monster_name)
    });
    let profile_sources_valid = profile.map_whitelist.iter().any(|map| {
        map.file_name == milestone.map_file_name
            && map.recommended_min_level <= milestone.level
            && milestone.level <= map.recommended_max_level
    }) && profile
        .monster_whitelist
        .iter()
        .any(|name| name == milestone.monster_name)
        && profile.item_whitelist.iter().any(|name| name == weapon)
        && profile.item_whitelist.iter().any(|name| name == armour)
        && reagent.is_none_or(|reagent| profile.item_whitelist.iter().any(|name| name == reagent))
        && profile
            .skills
            .iter()
            .any(|skill| skill.class == class && skill.spell == measured_skill)
        && respawn_is_real;

    CombatCaseReport {
        class: class_name(class),
        level: milestone.level,
        map_file_name: milestone.map_file_name.to_string(),
        monster_name: milestone.monster_name.to_string(),
        monster_level: monster.level,
        monster_max_hp: monster.hp,
        monster_ac: format!("{}-{}", monster.min_ac, monster.max_ac),
        monster_mac: format!("{}-{}", monster.min_mac, monster.max_mac),
        weapon: weapon.to_string(),
        armour: armour.to_string(),
        reagent: reagent.map(str::to_string),
        measured_skill: measured_skill.to_string(),
        known_profile_skills,
        nearby_hostiles_at_start,
        actions_issued,
        action_packets: packets.len(),
        ticks_observed,
        damage_model,
        damage,
        player_hp_before,
        player_hp_after,
        player_mp_before,
        player_mp_after,
        player_damage_taken: player_hp_before.saturating_sub(player_hp_after),
        mp_spent: player_mp_before.saturating_sub(player_mp_after),
        estimated_actions_to_kill: if damage > 0 && damage_model == "direct" {
            let actions = i32::try_from(actions_issued).unwrap_or(i32::MAX);
            Some(
                u32::try_from(
                    monster
                        .hp
                        .saturating_mul(actions)
                        .saturating_add(damage - 1)
                        / damage,
                )
                .unwrap_or(u32::MAX),
            )
        } else {
            None
        },
        emitted_combat_packet: packets
            .iter()
            .any(|packet| emitted_player_combat_packet(packet, self_object_id)),
        profile_sources_valid,
    }
}

fn write_report_if_requested(report: &CombatMilestoneReport) {
    let Some(output_path) = env::var_os(COMBAT_REPORT_ENV) else {
        return;
    };
    let output_path = Path::new(&output_path);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!(
                "failed to create combat report directory {}: {error}",
                parent.display()
            )
        });
    }
    let mut bytes =
        serde_json::to_vec_pretty(report).expect("combat report should serialize as JSON");
    bytes.push(b'\n');
    fs::write(output_path, bytes).unwrap_or_else(|error| {
        panic!(
            "failed to write combat report {}: {error}",
            output_path.display()
        )
    });
}

#[test]
fn all_three_classes_deal_real_damage_at_22_to_50_milestones() {
    let profile = platinum_176_profile();
    let mut cases = Vec::new();
    for class in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist] {
        for milestone in MILESTONES {
            cases.push(measure_case(class, milestone));
        }
    }

    let all_classes_replace_weapon_and_armour_by_level_40 =
        [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist]
            .into_iter()
            .all(|class| {
                let class = class_name(class);
                let early = cases
                    .iter()
                    .find(|case| case.class == class && case.level == 22)
                    .expect("level-22 case");
                let late = cases
                    .iter()
                    .find(|case| case.class == class && case.level == 40)
                    .expect("level-40 case");
                early.weapon != late.weapon && early.armour != late.armour
            });
    let assertions = CombatAssertions {
        all_fifteen_cases_measured: cases.len() == 15,
        every_target_is_profile_map_real_respawn: cases
            .iter()
            .all(|case| case.profile_sources_valid),
        every_fixture_item_and_skill_is_profile_allowed: cases.iter().all(|case| {
            profile.item_whitelist.contains(&case.weapon)
                && profile.item_whitelist.contains(&case.armour)
                && case
                    .reagent
                    .as_ref()
                    .is_none_or(|reagent| profile.item_whitelist.contains(reagent))
                && profile.skills.iter().any(|skill| {
                    class_name(skill.class) == case.class
                        && skill.spell == case.measured_skill
                        && skill.required_level <= case.level
                })
        }),
        every_action_emits_combat_packet: cases.iter().all(|case| case.emitted_combat_packet),
        every_case_deals_positive_damage: cases.iter().all(|case| case.damage > 0),
        no_case_kills_the_player_during_observation: cases
            .iter()
            .all(|case| case.player_hp_after > 0),
        all_classes_replace_weapon_and_armour_by_level_40,
    };
    let report = CombatMilestoneReport {
        schema: "mir2-platinum-176-combat-milestones/1",
        generated_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_millis(),
        profile_id: profile.profile_id,
        profile_version: profile.version,
        fixture_notice: "Levels, learned skills, and representative equipment are seeded fixtures. Damage, HP/MP cost, packets, maps, and monsters are measured through the real SimulationSession runtime; this does not certify natural item acquisition or live pacing.",
        cases,
        assertions,
    };
    write_report_if_requested(&report);
    assert!(
        report.assertions.all_fifteen_cases_measured
            && report.assertions.every_target_is_profile_map_real_respawn
            && report
                .assertions
                .every_fixture_item_and_skill_is_profile_allowed
            && report.assertions.every_action_emits_combat_packet
            && report.assertions.every_case_deals_positive_damage
            && report
                .assertions
                .no_case_kills_the_player_during_observation
            && report
                .assertions
                .all_classes_replace_weapon_and_armour_by_level_40,
        "measured Platinum combat milestone assertions failed: {:#?}; cases: {:#?}",
        report.assertions,
        report.cases
    );
}
