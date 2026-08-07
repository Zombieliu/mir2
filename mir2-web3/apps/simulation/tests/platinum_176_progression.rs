use mir2_game_data::{
    content_profile_monster_experience_multiplier, crystal_monster_manifest,
    crystal_respawn_manifest, platinum_176_profile, ContentLevelRate, ContentProfile,
};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{SimulationConfig, SimulationSession, WorldEntityKind};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const PROGRESSION_REPORT_ENV: &str = "MIR2_PLATINUM_PROGRESSION_REPORT";
const ACCEPTANCE_DIR_ENV: &str = "MIR2_PLATINUM_ACCEPTANCE_DIR";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressionCertificationReport {
    schema: &'static str,
    generated_at_unix_ms: u128,
    profile_id: String,
    profile_version: u32,
    profile_source: String,
    acceptance_level: u16,
    rate_policy_label: String,
    monster_experience_tiers: Vec<ContentLevelRate>,
    route_map_count: usize,
    route_maps: Vec<String>,
    classes: Vec<ClassProgressionReport>,
    assertions: ProgressionAssertions,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClassProgressionReport {
    class: &'static str,
    character_name: &'static str,
    level_transitions: Vec<LevelProgressionReport>,
    final_level: u16,
    final_experience: i64,
    final_max_experience: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LevelProgressionReport {
    from_level: u16,
    to_level: u16,
    map_file_name: String,
    monster_name: String,
    monster_experience: u32,
    effective_monster_experience: u32,
    representative_monster_respawn_count: u32,
    representative_monster_respawn_row_count: usize,
    representative_monster_supply_experience_per_hour: u64,
    map_non_boss_supply_experience_per_hour: u64,
    theoretical_minimum_minutes_for_level: u64,
    kill_count: i64,
    awarded_experience: i64,
    experience_before: i64,
    required_experience: i64,
    experience_after: i64,
    required_experience_after: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressionAssertions {
    all_three_classes_certified: bool,
    forty_nine_transitions_per_class: bool,
    all_four_segments_complete: bool,
    every_transition_uses_profile_map_and_real_spawn: bool,
    boss_monsters_excluded_from_leveling_route: bool,
    positive_respawn_supply_for_every_route: bool,
    every_award_is_whole_monster_kills: bool,
    natural_overflow_preserved: bool,
    final_level_is_acceptance_level: bool,
    multiple_hunting_maps_used: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SegmentProgressionSummary {
    label: &'static str,
    start_level: u16,
    end_level: u16,
    transition_count: usize,
    theoretical_minimum_minutes: u64,
    maps: Vec<String>,
    monsters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HuntingRoute {
    map_file_name: String,
    monster_name: String,
    monster_experience: u32,
    representative_monster_respawn_count: u32,
    representative_monster_respawn_row_count: usize,
    representative_monster_supply_experience_per_hour: u64,
    map_non_boss_supply_experience_per_hour: u64,
}

fn platinum_config() -> SimulationConfig {
    SimulationConfig::default().with_platinum_176_profile()
}

fn create_and_start_level_one_character(
    session: &mut SimulationSession,
    name: &str,
    class: MirClass,
) -> i32 {
    let created = session.handle_packet(ClientPacket::NewCharacter {
        name: name.to_string(),
        gender: MirGender::Male,
        class,
    });
    let character_index = match created.as_slice() {
        [ServerPacket::NewCharacterSuccess { char_info }] => char_info.index,
        packets => panic!("unexpected new-character response: {packets:?}"),
    };
    let started = session.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        started
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserInformation { .. })),
        "new character should enter the world"
    );
    character_index
}

fn self_player_level(session: &SimulationSession) -> u16 {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .and_then(|entity| entity.level)
        .expect("self player should have a level")
}

fn source_backed_hunting_routes(profile: &ContentProfile) -> BTreeMap<u16, HuntingRoute> {
    let allowed_monsters = profile
        .monster_whitelist
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let boss_monsters = profile
        .boss_monsters
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let monsters = crystal_monster_manifest()
        .monsters
        .into_iter()
        .map(|monster| (monster.name.clone(), monster))
        .collect::<BTreeMap<_, _>>();
    let respawns = crystal_respawn_manifest();

    (1..profile.acceptance_level)
        .map(|level| {
            let route = profile
                .map_whitelist
                .iter()
                .filter(|rule| {
                    rule.tier != "service"
                        && rule.recommended_min_level <= level
                        && level <= rule.recommended_max_level
                })
                .filter_map(|rule| {
                    let map = respawns
                        .maps
                        .iter()
                        .find(|map| map.map_file_name == rule.file_name)?;
                    let mut route_candidates = BTreeMap::<String, (u32, usize, u64, u32)>::new();
                    for respawn in map.respawns.iter().filter(|respawn| {
                        allowed_monsters.contains(respawn.monster_name.as_str())
                            && !boss_monsters.contains(respawn.monster_name.as_str())
                            && respawn.count > 0
                    }) {
                        let Some(monster) = monsters.get(&respawn.monster_name) else {
                            continue;
                        };
                        if monster.experience == 0 {
                            continue;
                        }
                        // Crystal respawns use a fixed delay plus a uniformly
                        // distributed random delay. Use the average cycle to
                        // calculate an upper bound on world XP supply.
                        let average_cycle_twice = u64::from(respawn.delay_minutes)
                            .saturating_mul(2)
                            .saturating_add(u64::from(respawn.random_delay_minutes))
                            .max(2);
                        let experience_per_hour = u64::from(monster.experience)
                            .saturating_mul(u64::from(respawn.count))
                            .saturating_mul(120)
                            / average_cycle_twice;
                        let candidate = route_candidates
                            .entry(respawn.monster_name.clone())
                            .or_insert((0, 0, 0, monster.experience));
                        candidate.0 = candidate.0.saturating_add(u32::from(respawn.count));
                        candidate.1 += 1;
                        candidate.2 = candidate.2.saturating_add(experience_per_hour);
                    }
                    let map_non_boss_supply_experience_per_hour = route_candidates
                        .values()
                        .map(|(_, _, experience_per_hour, _)| *experience_per_hour)
                        .sum::<u64>();
                    route_candidates
                        .into_iter()
                        .map(
                            |(
                                monster_name,
                                (
                                    representative_monster_respawn_count,
                                    representative_monster_respawn_row_count,
                                    representative_monster_supply_experience_per_hour,
                                    monster_experience,
                                ),
                            )| HuntingRoute {
                                map_file_name: rule.file_name.clone(),
                                monster_name,
                                monster_experience,
                                representative_monster_respawn_count,
                                representative_monster_respawn_row_count,
                                representative_monster_supply_experience_per_hour,
                                map_non_boss_supply_experience_per_hour,
                            },
                        )
                        .max_by_key(|route| {
                            (
                                route.representative_monster_supply_experience_per_hour,
                                route.monster_experience,
                            )
                        })
                })
                .max_by_key(|route| {
                    (
                        route.map_non_boss_supply_experience_per_hour,
                        route.representative_monster_supply_experience_per_hour,
                        route.monster_experience,
                    )
                })
                .unwrap_or_else(|| {
                    panic!("level {level} should have a source-backed hunting route")
                });
            (level, route)
        })
        .collect()
}

fn mir_class_name(class: MirClass) -> &'static str {
    match class {
        MirClass::Warrior => "Warrior",
        MirClass::Wizard => "Wizard",
        MirClass::Taoist => "Taoist",
        _ => panic!("post-1.76 class {class:?} cannot be certified"),
    }
}

fn write_progression_report_if_requested(report: &ProgressionCertificationReport) {
    let Some(output_path) = env::var_os(PROGRESSION_REPORT_ENV) else {
        return;
    };
    let output_path = Path::new(&output_path);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!(
                "failed to create progression report directory {}: {error}",
                parent.display()
            )
        });
    }
    let mut bytes =
        serde_json::to_vec_pretty(report).expect("progression report should serialize as JSON");
    bytes.push(b'\n');
    fs::write(output_path, bytes).unwrap_or_else(|error| {
        panic!(
            "failed to write progression report {}: {error}",
            output_path.display()
        )
    });
    eprintln!(
        "wrote platinum 1.76 progression certification report to {}",
        output_path.display()
    );
}

fn segment_progression_summary(
    report: &ClassProgressionReport,
    label: &'static str,
    start_level: u16,
    end_level: u16,
) -> SegmentProgressionSummary {
    let transitions = report
        .level_transitions
        .iter()
        .filter(|transition| {
            transition.from_level >= start_level && transition.to_level <= end_level
        })
        .collect::<Vec<_>>();
    SegmentProgressionSummary {
        label,
        start_level,
        end_level,
        transition_count: transitions.len(),
        theoretical_minimum_minutes: transitions
            .iter()
            .map(|transition| transition.theoretical_minimum_minutes_for_level)
            .sum(),
        maps: transitions
            .iter()
            .map(|transition| transition.map_file_name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        monsters: transitions
            .iter()
            .map(|transition| transition.monster_name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}

fn class_segment_summaries(report: &ClassProgressionReport) -> Vec<SegmentProgressionSummary> {
    vec![
        segment_progression_summary(report, "1-7", 1, 7),
        segment_progression_summary(report, "8-21", 7, 21),
        segment_progression_summary(report, "22-35", 21, 35),
        segment_progression_summary(report, "36-50", 35, 50),
    ]
}

fn write_acceptance_artifacts_if_requested(report: &ProgressionCertificationReport) {
    let Some(output_dir) = env::var_os(ACCEPTANCE_DIR_ENV) else {
        return;
    };
    let output_dir = Path::new(&output_dir);
    fs::create_dir_all(output_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create acceptance directory {}: {error}",
            output_dir.display()
        )
    });

    let mut comparison_rows = Vec::new();
    for class_report in &report.classes {
        let segments = class_segment_summaries(class_report);
        let slug = class_report.class.to_ascii_lowercase();
        let artifact = serde_json::json!({
            "schema": "mir2-platinum-176-class-acceptance/1",
            "generatedAtUnixMs": report.generated_at_unix_ms,
            "profileId": report.profile_id,
            "profileVersion": report.profile_version,
            "profileSource": report.profile_source,
            "acceptanceLevel": report.acceptance_level,
            "ratePolicyLabel": report.rate_policy_label,
            "monsterExperienceTiers": report.monster_experience_tiers,
            "class": class_report,
            "segments": segments,
            "assertions": report.assertions,
        });
        let mut bytes =
            serde_json::to_vec_pretty(&artifact).expect("class acceptance should serialize");
        bytes.push(b'\n');
        let output_path = output_dir.join(format!("{slug}-1-50.json"));
        fs::write(&output_path, bytes).unwrap_or_else(|error| {
            panic!(
                "failed to write class acceptance {}: {error}",
                output_path.display()
            )
        });
        comparison_rows.push((class_report, segments));
    }

    let rate_summary = report
        .monster_experience_tiers
        .iter()
        .map(|tier| format!("{}-{}={}x", tier.min_level, tier.max_level, tier.multiplier))
        .collect::<Vec<_>>()
        .join(", ");
    let mut comparison = format!(
        "# Platinum 1.76 class progression comparison\n\n\
         All times are source-backed theoretical supply minima under the Profile's \
         launch-candidate tiered monster XP policy ({rate_summary}). They are balance diagnostics, \
         not claimed player completion times. The historical 1x baseline remains \
         the same raw Crystal monster experience.\n\n\
         | Class | 1-7 | 8-21 | 22-35 | 36-50 | Final |\n\
         |---|---:|---:|---:|---:|---:|\n"
    );
    for (class_report, segments) in comparison_rows {
        comparison.push_str(&format!(
            "| {} | {} min | {} min | {} min | {} min | Lv.{} ({}/{}) |\n",
            class_report.class,
            segments[0].theoretical_minimum_minutes,
            segments[1].theoretical_minimum_minutes,
            segments[2].theoretical_minimum_minutes,
            segments[3].theoretical_minimum_minutes,
            class_report.final_level,
            class_report.final_experience,
            class_report.final_max_experience,
        ));
    }
    comparison.push_str(
        "\nEach class certificate contains all 49 level transitions, the selected \
         Profile map and real respawned monster, whole-kill experience arithmetic, \
         overflow, supply, and the four segment summaries.\n",
    );
    let comparison_path = output_dir.join("class-comparison.md");
    fs::write(&comparison_path, comparison).unwrap_or_else(|error| {
        panic!(
            "failed to write class comparison {}: {error}",
            comparison_path.display()
        )
    });
}

#[test]
fn platinum_profile_rejects_post_176_classes_and_heroes() {
    let mut session = SimulationSession::new(platinum_config());

    assert_eq!(
        session.handle_packet(ClientPacket::NewCharacter {
            name: "AssassinBlocked".to_string(),
            gender: MirGender::Male,
            class: MirClass::Assassin,
        }),
        vec![ServerPacket::NewCharacter { result: 1 }]
    );
    assert_eq!(
        session.handle_packet(ClientPacket::NewHero {
            name: "HeroBlocked".to_string(),
            gender: MirGender::Female,
            class: MirClass::Taoist,
        }),
        vec![ServerPacket::NewHero { result: 1 }]
    );
}

#[test]
fn monster_experience_levels_a_character_and_carries_overflow() {
    let mut session = SimulationSession::new(platinum_config());
    create_and_start_level_one_character(&mut session, "NaturalLevel", MirClass::Warrior);

    let receipt = session.commit_shared_monster_kill_award_transaction(91_001, "Hen", 300);

    assert!(receipt.committed);
    assert!(receipt
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainExperience { amount: 600 })));
    assert!(receipt.packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::LevelChanged {
            level: 4,
            experience: 0,
            max_experience: 400,
        }
    )));
    assert_eq!(self_player_level(&session), 4);
    let snapshot = session.world_snapshot();
    assert_eq!(snapshot.player_experience, 0);
    assert_eq!(snapshot.player_max_experience, 400);
    assert_eq!(snapshot.player_hp, snapshot.player_max_hp);
}

#[test]
fn one_authoritative_award_can_cross_the_full_1_to_50_acceptance_curve() {
    let mut session = SimulationSession::new(platinum_config());
    create_and_start_level_one_character(&mut session, "CurveRunner", MirClass::Wizard);
    let total_to_level_50 = platinum_176_profile()
        .experience_curve
        .iter()
        .filter(|entry| entry.level < 50)
        .map(|entry| entry.required_experience)
        .sum::<i64>();

    let receipt = session.commit_shared_monster_kill_award_transaction(
        91_002,
        "Hen",
        u32::try_from(
            total_to_level_50
                / i64::from(content_profile_monster_experience_multiplier(
                    &platinum_176_profile(),
                    1,
                )),
        )
        .expect("rate-adjusted 1-50 curve should fit protocol award"),
    );

    assert!(receipt.committed);
    assert_eq!(self_player_level(&session), 50);
    let snapshot = session.world_snapshot();
    assert_eq!(snapshot.player_experience, 0);
    assert_eq!(snapshot.player_max_experience, 350_000_000);
    assert!(receipt
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LevelChanged { level: 50, .. })));
}

#[test]
fn all_three_classes_progress_level_by_level_through_source_backed_hunting_routes() {
    let profile = platinum_176_profile();
    let routes = source_backed_hunting_routes(&profile);
    let mut route_maps = BTreeSet::new();
    let mut class_reports = Vec::new();

    for (class, character_name) in [
        (MirClass::Warrior, "RouteWarrior"),
        (MirClass::Wizard, "RouteWizard"),
        (MirClass::Taoist, "RouteTaoist"),
    ] {
        let mut session = SimulationSession::new(platinum_config());
        create_and_start_level_one_character(&mut session, character_name, class);
        let mut transaction_id = 100_000_u32 + u32::from(class as u8) * 10_000;
        let mut level_transitions = Vec::new();

        for level in 1..profile.acceptance_level {
            assert_eq!(self_player_level(&session), level);
            let route = routes
                .get(&level)
                .unwrap_or_else(|| panic!("level {level} route should be precomputed"));
            route_maps.insert(route.map_file_name.clone());
            let before = session.world_snapshot();
            let remaining = before
                .player_max_experience
                .saturating_sub(before.player_experience);
            assert!(remaining > 0);

            // Aggregate an exact whole number of real monster kills into one
            // authoritative receipt. The multiple preserves normal per-kill
            // overflow without turning the test into hundreds of thousands of
            // persistence transactions at the 40-49 level curve.
            let monster_experience_multiplier = u32::from(
                content_profile_monster_experience_multiplier(&profile, level),
            );
            let effective_monster_experience = route
                .monster_experience
                .saturating_mul(monster_experience_multiplier);
            let experience = i64::from(effective_monster_experience);
            let kill_count = remaining.saturating_add(experience - 1) / experience;
            let base_awarded_experience =
                kill_count.saturating_mul(i64::from(route.monster_experience));
            let awarded_experience =
                base_awarded_experience.saturating_mul(i64::from(monster_experience_multiplier));
            let final_kill = session.commit_shared_monster_kill_award_transaction(
                transaction_id,
                &route.monster_name,
                u32::try_from(base_awarded_experience)
                    .expect("one level's aggregate award should fit u32"),
            );
            transaction_id += 1;
            assert!(final_kill.committed);
            assert_eq!(
                self_player_level(&session),
                level + 1,
                "{character_name} should advance from {level} on {}",
                route.monster_name
            );
            assert!(final_kill.packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::LevelChanged {
                    level: changed_level,
                    ..
                } if *changed_level == level + 1
            )));
            let after = session.world_snapshot();
            level_transitions.push(LevelProgressionReport {
                from_level: level,
                to_level: level + 1,
                map_file_name: route.map_file_name.clone(),
                monster_name: route.monster_name.clone(),
                monster_experience: route.monster_experience,
                effective_monster_experience,
                representative_monster_respawn_count: route.representative_monster_respawn_count,
                representative_monster_respawn_row_count: route
                    .representative_monster_respawn_row_count,
                representative_monster_supply_experience_per_hour: route
                    .representative_monster_supply_experience_per_hour,
                map_non_boss_supply_experience_per_hour: route
                    .map_non_boss_supply_experience_per_hour,
                theoretical_minimum_minutes_for_level: u64::try_from(remaining)
                    .expect("remaining level experience should fit u64")
                    .saturating_mul(60)
                    .div_ceil(
                        route
                            .map_non_boss_supply_experience_per_hour
                            .saturating_mul(u64::from(monster_experience_multiplier)),
                    ),
                kill_count,
                awarded_experience,
                experience_before: before.player_experience,
                required_experience: before.player_max_experience,
                experience_after: after.player_experience,
                required_experience_after: after.player_max_experience,
            });
        }

        let final_snapshot = session.world_snapshot();
        assert_eq!(self_player_level(&session), profile.acceptance_level);
        assert!(
            final_snapshot.player_experience < final_snapshot.player_max_experience,
            "{character_name} overflow must remain inside level 50"
        );
        class_reports.push(ClassProgressionReport {
            class: mir_class_name(class),
            character_name,
            level_transitions,
            final_level: self_player_level(&session),
            final_experience: final_snapshot.player_experience,
            final_max_experience: final_snapshot.player_max_experience,
        });
    }

    assert!(
        route_maps.len() >= 6,
        "the certified route should use multiple distinct hunting maps, got {route_maps:?}"
    );

    let assertions = ProgressionAssertions {
        all_three_classes_certified: class_reports.len() == 3,
        forty_nine_transitions_per_class: class_reports
            .iter()
            .all(|report| report.level_transitions.len() == 49),
        all_four_segments_complete: class_reports.iter().all(|report| {
            class_segment_summaries(report)
                .iter()
                .map(|segment| segment.transition_count)
                .eq([6, 14, 14, 15])
        }),
        every_transition_uses_profile_map_and_real_spawn: class_reports.iter().all(|report| {
            report.level_transitions.iter().all(|transition| {
                routes.get(&transition.from_level).is_some_and(|route| {
                    route.map_file_name == transition.map_file_name
                        && route.monster_name == transition.monster_name
                        && route.monster_experience == transition.monster_experience
                        && route.representative_monster_respawn_count
                            == transition.representative_monster_respawn_count
                        && route.representative_monster_respawn_row_count
                            == transition.representative_monster_respawn_row_count
                        && route.representative_monster_supply_experience_per_hour
                            == transition.representative_monster_supply_experience_per_hour
                        && route.map_non_boss_supply_experience_per_hour
                            == transition.map_non_boss_supply_experience_per_hour
                })
            })
        }),
        boss_monsters_excluded_from_leveling_route: class_reports.iter().all(|report| {
            report.level_transitions.iter().all(|transition| {
                !profile
                    .boss_monsters
                    .iter()
                    .any(|boss| boss == &transition.monster_name)
            })
        }),
        positive_respawn_supply_for_every_route: class_reports.iter().all(|report| {
            report.level_transitions.iter().all(|transition| {
                transition.representative_monster_respawn_count > 0
                    && transition.representative_monster_respawn_row_count > 0
                    && transition.representative_monster_supply_experience_per_hour > 0
                    && transition.map_non_boss_supply_experience_per_hour
                        >= transition.representative_monster_supply_experience_per_hour
                    && transition.theoretical_minimum_minutes_for_level > 0
            })
        }),
        every_award_is_whole_monster_kills: class_reports.iter().all(|report| {
            report.level_transitions.iter().all(|transition| {
                transition.kill_count > 0
                    && transition.awarded_experience
                        == transition.kill_count
                            * i64::from(transition.effective_monster_experience)
            })
        }),
        natural_overflow_preserved: class_reports.iter().all(|report| {
            report.level_transitions.iter().all(|transition| {
                transition.experience_after
                    == transition.experience_before + transition.awarded_experience
                        - transition.required_experience
            })
        }),
        final_level_is_acceptance_level: class_reports
            .iter()
            .all(|report| report.final_level == profile.acceptance_level),
        multiple_hunting_maps_used: route_maps.len() >= 6,
    };
    assert!(
        [
            assertions.all_three_classes_certified,
            assertions.forty_nine_transitions_per_class,
            assertions.all_four_segments_complete,
            assertions.every_transition_uses_profile_map_and_real_spawn,
            assertions.boss_monsters_excluded_from_leveling_route,
            assertions.positive_respawn_supply_for_every_route,
            assertions.every_award_is_whole_monster_kills,
            assertions.natural_overflow_preserved,
            assertions.final_level_is_acceptance_level,
            assertions.multiple_hunting_maps_used,
        ]
        .into_iter()
        .all(|assertion| assertion),
        "all progression certification report assertions must pass"
    );
    let report = ProgressionCertificationReport {
        schema: "mir2-platinum-176-progression/1",
        generated_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_millis(),
        profile_id: profile.profile_id,
        profile_version: profile.version,
        profile_source: profile.source,
        acceptance_level: profile.acceptance_level,
        rate_policy_label: profile.rate_policy.label,
        monster_experience_tiers: profile.rate_policy.monster_experience_tiers,
        route_map_count: route_maps.len(),
        route_maps: route_maps.into_iter().collect(),
        classes: class_reports,
        assertions,
    };
    write_progression_report_if_requested(&report);
    write_acceptance_artifacts_if_requested(&report);
}

#[test]
fn natural_level_and_experience_survive_logout_and_fresh_session_reload() {
    let config = platinum_config();
    let mut first = SimulationSession::new(config.clone());
    let character_index =
        create_and_start_level_one_character(&mut first, "PersistentLevel", MirClass::Taoist);

    let receipt = first.commit_shared_monster_kill_award_transaction(91_003, "Hen", 63);
    assert!(receipt.committed);
    assert_eq!(self_player_level(&first), 2);
    assert_eq!(first.world_snapshot().player_experience, 26);
    let _ = first.handle_packet(ClientPacket::LogOut);

    let mut second = SimulationSession::new(config);
    let _ = second.handle_packet(ClientPacket::StartGame { character_index });

    assert_eq!(self_player_level(&second), 2);
    let snapshot = second.world_snapshot();
    assert_eq!(snapshot.player_experience, 26);
    assert_eq!(snapshot.player_max_experience, 200);
}

#[test]
fn platinum_176_blocks_post_176_stage5_actions_but_keeps_classic_social_endgame() {
    let mut session = SimulationSession::new(platinum_config());
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let before = session.world_snapshot();

    for (action, args) in [
        ("auction.list", vec!["WoodenSword".to_string()]),
        ("craft", vec!["WoodenSword".to_string()]),
        ("gameShop.buyGold", vec!["1".to_string()]),
        ("hero.recruit", vec!["Taoist".to_string()]),
        ("item.addSocket", vec!["WoodenSword".to_string()]),
        ("item.seal", vec!["WoodenSword".to_string()]),
        ("mail.send", vec!["Other".to_string()]),
        ("qa.advanceToLevel", vec!["50".to_string()]),
        ("shop.buyCredit", vec!["WoodenSword".to_string()]),
    ] {
        let packets = session.stage5_command(action, args);
        assert!(
            packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::Chat { message, .. }
                    if message.contains("unavailable in the active content profile")
            )),
            "{action} should be rejected by the Platinum 1.76 profile"
        );
    }

    let blocked = session.world_snapshot();
    assert_eq!(blocked.player_experience, before.player_experience);
    assert_eq!(blocked.player_max_experience, before.player_max_experience);
    assert_eq!(blocked.gold, before.gold);
    assert_eq!(blocked.credit, before.credit);
    assert_eq!(blocked.inventory_items, before.inventory_items);
    assert_eq!(blocked.stage5_systems, before.stage5_systems);

    session.stage5_command("group.create", vec!["Companion".to_string()]);
    session.stage5_command("guild.create", vec!["BichonGuard".to_string()]);
    session.stage5_command("trade.start", vec!["Trader".to_string()]);
    session.stage5_command("conquest.start", vec!["Sabuk".to_string()]);
    session.stage5_command("conquest.owner", Vec::new());
    let classic = session.world_snapshot().stage5_systems;
    assert_eq!(classic.group.members.len(), 2);
    assert_eq!(classic.guild.name, "BichonGuard");
    assert!(classic.trade.is_some());
    assert_eq!(classic.conquest.castle_owner, "BichonGuard");
}

#[test]
fn platinum_176_new_character_starts_with_source_start_items_in_the_bag() {
    let mut session = SimulationSession::new(platinum_config());
    create_and_start_level_one_character(&mut session, "SourceStarter", MirClass::Warrior);
    let snapshot = session.world_snapshot();

    assert_eq!(
        snapshot
            .inventory_items
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        vec!["WoodenSword", "BaseDress(M)", "(HP)DrugSmall", "Candle"]
    );
    assert!(snapshot
        .inventory_items
        .iter()
        .all(|item| item.key.starts_with("crystal-item-")));
    assert!(snapshot.equipment_items.is_empty());
    assert_eq!(snapshot.map_file_name.as_deref(), Some("0"));
}

#[test]
fn deeply_red_player_death_drops_two_eligible_items_and_recalculates_equipment() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session.stage5_command("qa.giveItem", vec!["red-potion".to_string()]);
    session.apply_zone_unlawful_player_kill(300);
    let before = session.world_snapshot();
    let before_items = before.inventory_items.len() + before.equipment_items.len();
    assert!(
        before.equipment_items.len() >= 2,
        "deep-red fixture should expose two droppable equipped items"
    );

    assert!(session.apply_zone_player_damage(i32::MAX));
    let packets = session.apply_zone_player_death_penalty();
    let after = session.world_snapshot();
    assert_eq!(
        after.inventory_items.len() + after.equipment_items.len(),
        before_items - 2
    );
    assert_eq!(after.ground_drops.len(), before.ground_drops.len() + 2);
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::DeleteItem { count: 1, .. }))
            .count(),
        2
    );
}

#[test]
fn pk_decay_accumulator_persists_and_reconnect_cannot_accelerate_decay() {
    let config = SimulationConfig::default();
    let mut first = SimulationSession::new(config.clone());
    first.handle_packet(ClientPacket::StartGame { character_index: 0 });
    first.apply_zone_unlawful_player_kill(2);
    for _ in 0..30 {
        first.tick();
    }
    assert_eq!(first.world_snapshot().player_pk_points, 2);
    assert_eq!(
        first.world_snapshot().stage5_systems.pk_decay_elapsed_ticks,
        30
    );
    first.save_active_character();

    let mut second = SimulationSession::new(config);
    second.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(second.world_snapshot().player_pk_points, 2);
    assert_eq!(
        second
            .world_snapshot()
            .stage5_systems
            .pk_decay_elapsed_ticks,
        30
    );
    for _ in 0..30 {
        second.tick();
    }
    assert_eq!(second.world_snapshot().player_pk_points, 1);
    assert_eq!(
        second
            .world_snapshot()
            .stage5_systems
            .pk_decay_elapsed_ticks,
        0
    );
}

#[test]
fn pk_name_colour_transitions_from_red_to_brown_to_normal_at_decay_boundaries() {
    let mut red = SimulationSession::new(SimulationConfig::default());
    red.handle_packet(ClientPacket::StartGame { character_index: 0 });
    red.apply_zone_unlawful_player_kill(200);
    assert_eq!(red.zone_player_name_colour_argb(), 0xFFFF_0000u32 as i32);
    let mut red_decay_packets = Vec::new();
    for _ in 0..60 {
        red_decay_packets.extend(red.tick());
    }
    assert_eq!(red.world_snapshot().player_pk_points, 199);
    assert_eq!(red.zone_player_name_colour_argb(), 0xFFFF_8000u32 as i32);
    assert!(red_decay_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ColourChanged { name_colour_argb }
            if *name_colour_argb == 0xFFFF_8000u32 as i32
    )));

    let mut brown = SimulationSession::new(SimulationConfig::default());
    brown.handle_packet(ClientPacket::StartGame { character_index: 0 });
    brown.apply_zone_unlawful_player_kill(100);
    let mut brown_decay_packets = Vec::new();
    for _ in 0..60 {
        brown_decay_packets.extend(brown.tick());
    }
    assert_eq!(brown.world_snapshot().player_pk_points, 99);
    assert_eq!(brown.zone_player_name_colour_argb(), -1);
    assert!(brown_decay_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ColourChanged { name_colour_argb } if *name_colour_argb == -1
    )));
}
