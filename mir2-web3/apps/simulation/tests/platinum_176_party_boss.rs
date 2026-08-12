use mir2_game_data::{crystal_monster_manifest, crystal_respawn_manifest, platinum_176_profile};
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    GroundDropSnapshot, SessionId, ZoneChatProfile, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterDefense, ZoneMonsterSpawn, ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const REPORT_ENV: &str = "MIR2_PLATINUM_PARTY_BOSS_REPORT";
const BOSS_NAME: &str = "RedMoonEvil";
const BOSS_MAP: &str = "D10062";
const BOSS_OBJECT_ID: u32 = 9_100;
const ACTION_INTERVAL_MS: u64 = 1_000;
const MAX_ROUNDS: u32 = 180;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PartyBossReport {
    schema: &'static str,
    generated_at_unix_ms: u128,
    profile_id: String,
    profile_version: u32,
    fixture_notice: &'static str,
    boss: BossSourceReport,
    solo: CombatRunReport,
    party: CombatRunReport,
    assertions: PartyBossAssertions,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BossSourceReport {
    name: String,
    map_file_name: String,
    level: u16,
    max_hp: i32,
    min_ac: i32,
    max_ac: i32,
    min_mac: i32,
    max_mac: i32,
    min_dc: i32,
    max_dc: i32,
    respawn_x: i32,
    respawn_y: i32,
    respawn_delay_minutes: u32,
    recommended_min_level: u16,
    recommended_max_level: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CombatRunReport {
    participants: Vec<&'static str>,
    rounds: u32,
    elapsed_ms: u64,
    boss_killed: bool,
    total_damage_observed: i32,
    damage_by_class: BTreeMap<&'static str, i32>,
    attack_packets_observed: u32,
    magic_packets_observed: u32,
    player_deaths: Vec<&'static str>,
    kill_awarded_to: Option<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PartyBossAssertions {
    boss_is_profile_whitelisted: bool,
    boss_uses_real_profile_respawn: bool,
    map_is_level_48_to_50_boss_tier: bool,
    production_damage_multiplier_is_one: bool,
    all_three_classes_deal_observed_damage: bool,
    party_kills_boss: bool,
    party_survives: bool,
    party_ttk_is_below_sixty_seconds: bool,
    party_is_faster_than_solo: bool,
}

#[derive(Clone)]
struct Participant {
    session: &'static str,
    name: &'static str,
    class_name: &'static str,
    class: MirClass,
    object_id: u32,
    position: Point,
    action: ParticipantAction,
    stats: ZonePlayerCombatStats,
    hp: i32,
    mp: i32,
}

#[derive(Clone, Copy)]
enum ParticipantAction {
    Melee(Spell),
    Magic(Spell, i32),
}

fn participants() -> [Participant; 3] {
    [
        Participant {
            session: "warrior",
            name: "PartyBlade",
            class_name: "Warrior",
            class: MirClass::Warrior,
            object_id: 101,
            position: Point { x: 22, y: 18 },
            action: ParticipantAction::Melee(Spell::FlamingSword),
            stats: ZonePlayerCombatStats {
                min_dc: 85,
                max_dc: 125,
                accuracy: 40,
                min_ac: 35,
                max_ac: 55,
                min_mac: 20,
                max_mac: 30,
                ..ZonePlayerCombatStats::default()
            },
            hp: 989,
            mp: 186,
        },
        Participant {
            session: "wizard",
            name: "PartyMage",
            class_name: "Wizard",
            class: MirClass::Wizard,
            object_id: 102,
            position: Point { x: 21, y: 17 },
            action: ParticipantAction::Magic(Spell::IceStorm, 18),
            stats: ZonePlayerCombatStats {
                // The shared-zone spell boundary consumes the same projected
                // attack base that the Gateway sends from the equipped player.
                min_dc: 90,
                max_dc: 120,
                min_mc: 75,
                max_mc: 110,
                accuracy: 40,
                min_ac: 18,
                max_ac: 30,
                min_mac: 35,
                max_mac: 55,
                ..ZonePlayerCombatStats::default()
            },
            hp: 270,
            mp: 1_443,
        },
        Participant {
            session: "taoist",
            name: "PartyTao",
            class_name: "Taoist",
            class: MirClass::Taoist,
            object_id: 103,
            position: Point { x: 21, y: 19 },
            action: ParticipantAction::Magic(Spell::SoulFireBall, 12),
            stats: ZonePlayerCombatStats {
                min_dc: 70,
                max_dc: 105,
                min_sc: 65,
                max_sc: 95,
                accuracy: 40,
                min_ac: 25,
                max_ac: 40,
                min_mac: 30,
                max_mac: 45,
                ..ZonePlayerCombatStats::default()
            },
            hp: 555,
            mp: 700,
        },
    ]
}

fn session(value: &str) -> SessionId {
    SessionId::new(value)
}

fn join(participant: &Participant) -> ZoneJoin {
    ZoneJoin {
        session_id: session(participant.session),
        account_id: format!("party-boss-{}", participant.session),
        character_index: 0,
        object_id: participant.object_id,
        name: participant.name.to_string(),
        class: participant.class,
        gender: MirGender::Male,
        level: 50,
        hp: participant.hp,
        max_hp: participant.hp,
        mp: participant.mp,
        map_file_name: BOSS_MAP.to_string(),
        position: participant.position.clone(),
        direction: MirDirection::Right,
        chat_profile: ZoneChatProfile {
            group_members: participants()
                .iter()
                .filter(|member| member.name != participant.name)
                .map(|member| member.name.to_string())
                .collect(),
            ..ZoneChatProfile::default()
        },
        combat_stats: participant.stats,
    }
}

fn boss_spawn() -> ZoneMonsterSpawn {
    let manifest = crystal_monster_manifest();
    let template = manifest
        .monsters
        .iter()
        .find(|monster| monster.name == BOSS_NAME)
        .expect("RedMoonEvil should exist in the Crystal monster manifest");
    ZoneMonsterSpawn {
        object_id: BOSS_OBJECT_ID,
        name: template.name.clone(),
        name_colour_argb: -1,
        image: template.image,
        ai: template.ai,
        level: template.level,
        max_hp: template.hp,
        hp: template.hp,
        experience: template.experience,
        move_speed_ms: u64::from(template.move_speed),
        attack_speed_ms: u64::from(template.attack_speed),
        friendly_guild: None,
        position: Point { x: 23, y: 18 },
        direction: MirDirection::Left,
        defense: ZoneMonsterDefense::from_crystal_template(template),
        drops: Vec::<GroundDropSnapshot>::new(),
    }
}

fn packets(outbounds: &[ZoneOutbound]) -> impl Iterator<Item = &ServerPacket> {
    outbounds.iter().flat_map(|outbound| match outbound {
        ZoneOutbound::ToSession { packets, .. }
        | ZoneOutbound::ToMany { packets, .. }
        | ZoneOutbound::ToAll { packets } => packets.as_slice(),
        _ => &[],
    })
}

fn observe_outbounds(
    outbounds: &[ZoneOutbound],
    report: &mut CombatRunReport,
    attacker: Option<&Participant>,
) {
    let observed_damage = packets(outbounds)
        .filter_map(|packet| match packet {
            ServerPacket::DamageIndicator {
                object_id, damage, ..
            } if *object_id == BOSS_OBJECT_ID => Some(*damage),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    if observed_damage > 0 {
        report.total_damage_observed = report.total_damage_observed.saturating_add(observed_damage);
        if let Some(attacker) = attacker {
            *report
                .damage_by_class
                .entry(attacker.class_name)
                .or_default() += observed_damage;
        }
    }
    report.attack_packets_observed += packets(outbounds)
        .filter(|packet| matches!(packet, ServerPacket::ObjectAttack { .. }))
        .count() as u32;
    report.magic_packets_observed += packets(outbounds)
        .filter(|packet| matches!(packet, ServerPacket::ObjectMagic { .. }))
        .count() as u32;
    for participant in participants() {
        let died = packets(outbounds).any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectDied { info } if info.object_id == participant.object_id
            )
        });
        if died && !report.player_deaths.contains(&participant.class_name) {
            report.player_deaths.push(participant.class_name);
        }
    }
    for outbound in outbounds {
        if let ZoneOutbound::MonsterKillAward { session_id, award } = outbound {
            if award.monster_object_id == BOSS_OBJECT_ID {
                report.boss_killed = true;
                report.kill_awarded_to = participants()
                    .into_iter()
                    .find(|participant| session_id == &session(participant.session))
                    .map(|participant| participant.class_name);
            }
        }
    }
}

fn run_combat(active: &[Participant]) -> CombatRunReport {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map(BOSS_MAP), ZoneCollision::unbounded());
    for participant in active {
        zone.handle(ZoneCommand::Join(join(participant)));
    }
    let owner = session(active[0].session);
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner,
        monster: boss_spawn(),
        now_ms: 0,
    });

    let mut report = CombatRunReport {
        participants: active.iter().map(|member| member.class_name).collect(),
        rounds: 0,
        elapsed_ms: 0,
        boss_killed: false,
        total_damage_observed: 0,
        damage_by_class: BTreeMap::new(),
        attack_packets_observed: 0,
        magic_packets_observed: 0,
        player_deaths: Vec::new(),
        kill_awarded_to: None,
    };

    for round in 1..=MAX_ROUNDS {
        let now_ms = u64::from(round) * ACTION_INTERVAL_MS;
        for participant in active {
            let launched = match participant.action {
                ParticipantAction::Melee(spell) => zone.handle(ZoneCommand::PlayerAttackObject {
                    session_id: session(participant.session),
                    object_id: BOSS_OBJECT_ID,
                    direction: MirDirection::Right,
                    spell: spell as u8,
                    level: 3,
                    attack_type: 0,
                    damage: participant.stats.max_dc,
                    now_ms,
                }),
                ParticipantAction::Magic(spell, mp_cost) => {
                    zone.handle(ZoneCommand::PlayerCastMagic {
                        session_id: session(participant.session),
                        object_id: BOSS_OBJECT_ID,
                        spell,
                        direction: MirDirection::Right,
                        target: Point { x: 23, y: 18 },
                        cast: true,
                        level: 3,
                        damage: participant.stats.max_dc,
                        mp_cost,
                        cooldown_ms: 500,
                        now_ms,
                    })
                }
            };
            observe_outbounds(&launched, &mut report, None);
            let resolved = zone.tick(now_ms);
            observe_outbounds(&resolved, &mut report, Some(participant));
            if report.boss_killed {
                break;
            }
        }
        report.rounds = round;
        report.elapsed_ms = now_ms;
        if report.boss_killed || !report.player_deaths.is_empty() {
            break;
        }
    }
    report
}

fn write_report_if_requested(report: &PartyBossReport) {
    let Some(output_path) = env::var_os(REPORT_ENV) else {
        return;
    };
    let output_path = Path::new(&output_path);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).expect("party Boss report directory should be created");
    }
    let mut bytes = serde_json::to_vec_pretty(report).expect("party Boss report should serialize");
    bytes.push(b'\n');
    fs::write(output_path, bytes).expect("party Boss report should be written");
}

#[test]
fn platinum_176_three_class_party_kills_real_red_moon_boss_with_bounded_ttk() {
    let profile = platinum_176_profile();
    let monster_manifest = crystal_monster_manifest();
    let monster = monster_manifest
        .monsters
        .iter()
        .find(|monster| monster.name == BOSS_NAME)
        .expect("RedMoonEvil should exist");
    let map = profile
        .map_whitelist
        .iter()
        .find(|map| map.file_name == BOSS_MAP)
        .expect("D10062 should be in the Platinum map whitelist");
    let respawn_manifest = crystal_respawn_manifest();
    let respawn = respawn_manifest
        .maps
        .iter()
        .find(|entry| entry.map_file_name == BOSS_MAP)
        .and_then(|entry| {
            entry
                .respawns
                .iter()
                .find(|respawn| respawn.monster_name == BOSS_NAME)
        })
        .expect("RedMoonEvil should have a real D10062 respawn");

    let all = participants();
    let solo = run_combat(&all[..1]);
    let party = run_combat(&all);
    let damage_multiplier_is_one = env::var("MIR2_QA_NATURAL_KILL_DAMAGE_MULTIPLIER")
        .ok()
        .is_none_or(|value| value.trim().is_empty() || value.trim() == "1");
    let assertions = PartyBossAssertions {
        boss_is_profile_whitelisted: profile.boss_monsters.iter().any(|name| name == BOSS_NAME),
        boss_uses_real_profile_respawn: respawn.location == (Point { x: 23, y: 18 }),
        map_is_level_48_to_50_boss_tier: map.tier == "red_moon_boss"
            && map.recommended_min_level == 48
            && map.recommended_max_level == 50,
        production_damage_multiplier_is_one: damage_multiplier_is_one,
        all_three_classes_deal_observed_damage: ["Warrior", "Wizard", "Taoist"]
            .into_iter()
            .all(|class| party.damage_by_class.get(class).copied().unwrap_or(0) > 0),
        party_kills_boss: party.boss_killed,
        party_survives: party.player_deaths.is_empty(),
        party_ttk_is_below_sixty_seconds: party.boss_killed && party.elapsed_ms <= 60_000,
        party_is_faster_than_solo: party.boss_killed
            && (!solo.boss_killed || party.elapsed_ms < solo.elapsed_ms),
    };
    let report = PartyBossReport {
        schema: "mir2-platinum-176-party-boss/1",
        generated_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_millis(),
        profile_id: profile.profile_id.clone(),
        profile_version: profile.version,
        fixture_notice: "Level, skills and representative Profile-era combat stats are seeded. The shared Zone owns attacks, armour, Boss AI, death and kill award; no QA damage multiplier is permitted.",
        boss: BossSourceReport {
            name: monster.name.clone(),
            map_file_name: BOSS_MAP.to_string(),
            level: monster.level,
            max_hp: monster.hp,
            min_ac: monster.min_ac,
            max_ac: monster.max_ac,
            min_mac: monster.min_mac,
            max_mac: monster.max_mac,
            min_dc: monster.min_dc,
            max_dc: monster.max_dc,
            respawn_x: respawn.location.x,
            respawn_y: respawn.location.y,
            respawn_delay_minutes: u32::from(respawn.delay_minutes),
            recommended_min_level: map.recommended_min_level,
            recommended_max_level: map.recommended_max_level,
        },
        solo,
        party,
        assertions,
    };
    write_report_if_requested(&report);
    assert!(
        serde_json::to_value(&report.assertions)
            .expect("assertions should serialize")
            .as_object()
            .expect("assertions should be an object")
            .values()
            .all(|value| value == true),
        "party Boss report assertions failed: {}",
        serde_json::to_string_pretty(&report).expect("report should serialize")
    );
}
