use mir2_game_data::{
    crystal_drop_table_for_monster_name, crystal_npc_info_manifest, crystal_npc_manifest,
    crystal_respawn_manifest, platinum_176_profile,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

fn better_chance(left: (u32, u32), right: (u32, u32)) -> bool {
    u64::from(left.0) * u64::from(right.1) > u64::from(right.0) * u64::from(left.1)
}

fn base_drop_chances(monster: &str, item: &str) -> Vec<(u32, u32)> {
    crystal_drop_table_for_monster_name(monster)
        .into_iter()
        .flat_map(|table| table.sections)
        .flat_map(|section| section.entries)
        .filter(|entry| entry.item_name.eq_ignore_ascii_case(item))
        .filter_map(|entry| Some((entry.chance_numerator?, entry.chance_denominator?)))
        .collect()
}

fn profile_drop_chances(monster: &str, item: &str) -> Vec<(u32, u32)> {
    let profile = platinum_176_profile();
    profile
        .drop_overrides
        .iter()
        .filter(|rule| {
            rule.monster.eq_ignore_ascii_case(monster) && rule.item.eq_ignore_ascii_case(item)
        })
        .map(|rule| (rule.chance_numerator, rule.chance_denominator))
        .collect()
}

fn all_drop_chances(monster: &str, item: &str) -> Vec<(u32, u32)> {
    base_drop_chances(monster, item)
        .into_iter()
        .chain(profile_drop_chances(monster, item))
        .collect()
}

fn assert_items_have_boss_sources(bosses: &[&str], items: &[&str]) {
    let mut missing = Vec::new();
    for item in items {
        let sources = bosses
            .iter()
            .filter(|boss| !all_drop_chances(boss, item).is_empty())
            .copied()
            .collect::<Vec<_>>();
        if sources.is_empty() {
            missing.push(*item);
        }
    }
    assert!(
        missing.is_empty(),
        "items {missing:?} need at least one Platinum 1.76 boss source from {bosses:?}"
    );
}

fn assert_profile_bosses_spawn_on_allowed_maps(bosses: &[&str]) {
    let profile = platinum_176_profile();
    let respawns = crystal_respawn_manifest();
    let allowed_maps = profile
        .map_whitelist
        .iter()
        .map(|rule| rule.file_name.as_str())
        .collect::<BTreeSet<_>>();
    for boss in bosses {
        let placements = respawns
            .maps
            .iter()
            .filter(|map| allowed_maps.contains(map.map_file_name.as_str()))
            .flat_map(|map| {
                map.respawns
                    .iter()
                    .filter(|spawn| {
                        spawn.count > 0 && spawn.monster_name.eq_ignore_ascii_case(boss)
                    })
                    .map(move |spawn| (map.map_file_name.as_str(), spawn.count))
            })
            .collect::<Vec<_>>();
        assert!(
            !placements.is_empty(),
            "{boss} needs a live respawn on an allowed Platinum 1.76 map"
        );
    }
}

fn best_chance(chances: impl IntoIterator<Item = (u32, u32)>) -> Option<(u32, u32)> {
    chances.into_iter().reduce(|best, candidate| {
        if better_chance(candidate, best) {
            candidate
        } else {
            best
        }
    })
}

#[test]
fn platinum_176_levels_8_to_21_have_a_complete_natural_product_loop() {
    let profile = platinum_176_profile();
    let respawns = crystal_respawn_manifest();
    let allowed_maps = profile
        .map_whitelist
        .iter()
        .map(|rule| rule.file_name.as_str())
        .collect::<BTreeSet<_>>();
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
    let map_file_name_by_index = respawns
        .maps
        .iter()
        .map(|map| (map.map_index, map.map_file_name.as_str()))
        .collect::<BTreeMap<_, _>>();

    let mut reachable = BTreeSet::from(["0"]);
    let mut queue = VecDeque::from(["0"]);
    while let Some(source) = queue.pop_front() {
        let map = respawns
            .maps
            .iter()
            .find(|map| map.map_file_name == source)
            .expect("reachable profile map should exist");
        for movement in &map.movements {
            let Some(destination) = map_file_name_by_index.get(&movement.map_index) else {
                continue;
            };
            if allowed_maps.contains(destination) && reachable.insert(destination) {
                queue.push_back(destination);
            }
        }
    }

    let route_rules = profile
        .map_whitelist
        .iter()
        .filter(|rule| {
            rule.tier != "service"
                && rule.recommended_min_level <= 21
                && rule.recommended_max_level >= 8
        })
        .collect::<Vec<_>>();
    for rule in &route_rules {
        assert!(
            reachable.contains(rule.file_name.as_str()),
            "{} must be naturally reachable from map 0",
            rule.file_name
        );
    }
    for level in 8..=21 {
        let usable_maps = route_rules
            .iter()
            .filter(|rule| {
                rule.recommended_min_level <= level && level <= rule.recommended_max_level
            })
            .filter(|rule| {
                respawns.maps.iter().any(|map| {
                    map.map_file_name == rule.file_name
                        && map.respawns.iter().any(|spawn| {
                            spawn.count > 0
                                && allowed_monsters.contains(spawn.monster_name.as_str())
                        })
                })
            })
            .count();
        assert!(
            usable_maps >= 2,
            "level {level} needs at least two naturally supplied hunting maps, found {usable_maps}"
        );
    }

    let route_maps = route_rules
        .iter()
        .map(|rule| rule.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let spawned_monsters = respawns
        .maps
        .iter()
        .filter(|map| route_maps.contains(map.map_file_name.as_str()))
        .flat_map(|map| map.respawns.iter())
        .filter(|spawn| spawn.count > 0 && allowed_monsters.contains(spawn.monster_name.as_str()))
        .map(|spawn| spawn.monster_name.as_str())
        .collect::<BTreeSet<_>>();
    for milestone_monster in [
        "Oma",
        "CannibalPlant",
        "ForestYeti",
        "Skeleton",
        "BoneFighter",
        "Zombie2",
        "Ghoul",
    ] {
        assert!(
            spawned_monsters.contains(milestone_monster),
            "{milestone_monster} must exist on the 8-21 route"
        );
    }

    for skill in profile
        .skills
        .iter()
        .filter(|skill| (8..=21).contains(&skill.required_level))
    {
        let ordinary_best = spawned_monsters
            .iter()
            .filter(|monster| !boss_monsters.contains(**monster))
            .flat_map(|monster| base_drop_chances(monster, &skill.spell))
            .pipe(best_chance)
            .unwrap_or_else(|| {
                panic!(
                    "{} level {} needs an ordinary-monster drop source",
                    skill.spell, skill.required_level
                )
            });

        if (18..=21).contains(&skill.required_level) {
            let ghoul_best = base_drop_chances("Ghoul", &skill.spell)
                .into_iter()
                .chain(profile_drop_chances("Ghoul", &skill.spell))
                .pipe(best_chance)
                .unwrap_or_else(|| {
                    panic!(
                        "{} level {} needs a Ghoul drop source",
                        skill.spell, skill.required_level
                    )
                });
            assert!(
                better_chance(ghoul_best, ordinary_best),
                "{} Ghoul chance {}/{} must beat ordinary chance {}/{}",
                skill.spell,
                ghoul_best.0,
                ghoul_best.1,
                ordinary_best.0,
                ordinary_best.1
            );
        }
    }

    let npc_info = crystal_npc_info_manifest();
    let npc_scripts = crystal_npc_manifest();
    let whitelisted_scripts = profile
        .npc_script_whitelist
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let placed_scripts = npc_info
        .npcs
        .iter()
        .filter(|npc| {
            whitelisted_scripts.contains(npc.script_key.as_str())
                && npc
                    .map_file_name
                    .as_deref()
                    .is_some_and(|map| allowed_maps.contains(map))
        })
        .map(|npc| npc.script_key.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        placed_scripts, whitelisted_scripts,
        "every whitelisted NPC service must have a naturally reachable profile placement"
    );

    let shop_items = npc_scripts
        .scripts
        .iter()
        .filter(|script| placed_scripts.contains(script.script_key.as_str()))
        .flat_map(|script| script.sections.iter())
        .filter(|section| section.label.eq_ignore_ascii_case("Trade"))
        .flat_map(|section| section.lines.iter())
        .map(|line| {
            line.rsplit_once(' ')
                .filter(|(_, suffix)| suffix.parse::<u32>().is_ok())
                .map_or(line.as_str(), |(item, _)| item)
        })
        .collect::<BTreeSet<_>>();
    for required_item in [
        "(HP)DrugSmall",
        "(MP)DrugSmall",
        "(HP)DrugMedium",
        "(MP)DrugMedium",
        "LightArmour(M)",
        "LightArmour(F)",
        "IronSword",
        "Scimitar",
        "BronzeHelmet",
        "Amulet",
        "GreenPoison",
        "RedPoison",
        "RandomTeleport",
        "DungeonEscape",
        "TownTeleport",
        "RepairOil",
    ] {
        assert!(
            shop_items.contains(required_item),
            "{required_item} needs a whitelisted, reachable shop source"
        );
    }
}

#[test]
fn platinum_176_levels_22_to_35_have_core_skills_gear_and_boss_sources() {
    let profile = platinum_176_profile();
    let bosses = ["WhiteBoar", "EvilCentipede"];
    assert_profile_bosses_spawn_on_allowed_maps(&bosses);

    let expected_skills = profile
        .skills
        .iter()
        .filter(|skill| (22..=35).contains(&skill.required_level))
        .map(|skill| skill.spell.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        expected_skills.len(),
        17,
        "22-35 should expose the complete three-class core skill set"
    );
    assert_items_have_boss_sources(
        &bosses,
        &expected_skills.iter().copied().collect::<Vec<_>>(),
    );

    assert_items_have_boss_sources(
        &bosses,
        &[
            "GreatAxe",
            "MageStaff",
            "SerpentSword",
            "HeavyArmour(M)",
            "HeavyArmour(F)",
            "MagicRobe(M)",
            "MagicRobe(F)",
            "SoulArmour(M)",
            "SoulArmour(F)",
        ],
    );
}

#[test]
fn platinum_176_levels_36_to_50_have_endgame_boss_and_equipment_sources() {
    let profile = platinum_176_profile();
    assert!(
        profile
            .skills
            .iter()
            .all(|skill| skill.required_level <= 35),
        "Platinum 1.76 should finish its three-class skill ladder at level 35"
    );

    let bosses = ["WoomaTaurus", "ZumaTaurus", "EvilBigApe", "RedMoonEvil"];
    assert_profile_bosses_spawn_on_allowed_maps(&bosses);
    assert_items_have_boss_sources(
        &bosses,
        &[
            "JudgementMace",
            "DragonSword",
            "WarSpiritBlade",
            "DragonSlayer",
            "WarMageStaff",
            "DragonStaff",
            "MagicScythe",
            "SoulSpringWand",
            "StoneBambooFan",
            "SoulSabre",
            "IronArmour(M)",
            "IronArmour(F)",
            "WizardRobe(M)",
            "WitchRobe(F)",
            "PearlArmour(M)",
            "PearlArmour(F)",
            "SteelArmour(M)",
            "SteelArmour(F)",
            "DragonRobe(M)",
            "DragonRobe(F)",
            "TitanArmour(M)",
            "TitanArmour(F)",
        ],
    );
}

trait Pipe: Sized {
    fn pipe<T>(self, apply: impl FnOnce(Self) -> T) -> T {
        apply(self)
    }
}

impl<T> Pipe for T {}
