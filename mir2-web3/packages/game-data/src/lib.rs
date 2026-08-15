use mir2_protocol::{MapInformation, MirClass, MirDirection, MirGender, Point};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentProfile {
    pub profile_id: String,
    pub version: u32,
    pub source: String,
    pub allowed_classes: Vec<MirClass>,
    pub acceptance_level: u16,
    pub rate_policy: ContentRatePolicy,
    pub experience_curve: Vec<ExperienceLevel>,
    pub map_whitelist: Vec<ContentMapRule>,
    pub monster_whitelist: Vec<String>,
    pub boss_monsters: Vec<String>,
    pub boss_respawn_jitter_minutes: u16,
    #[serde(default)]
    pub respawn_overrides: Vec<ContentMonsterRespawnRule>,
    #[serde(default)]
    pub drop_overrides: Vec<ContentMonsterDropRule>,
    #[serde(default)]
    pub quest_prerequisite_overrides: Vec<ContentQuestPrerequisiteRule>,
    #[serde(default)]
    pub quest_reward_overrides: Vec<ContentQuestRewardRule>,
    pub item_whitelist: Vec<String>,
    pub skills: Vec<ContentSkillRule>,
    pub npc_script_whitelist: Vec<String>,
    #[serde(default)]
    pub disabled_stage5_action_prefixes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRatePolicy {
    pub label: String,
    pub monster_experience_tiers: Vec<ContentLevelRate>,
    pub gold_multiplier: u16,
    pub drop_multiplier: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentLevelRate {
    pub min_level: u16,
    pub max_level: u16,
    pub multiplier: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceLevel {
    /// The current character level.
    pub level: u16,
    /// Experience required to advance from `level` to `level + 1`.
    pub required_experience: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentMapRule {
    pub file_name: String,
    pub tier: String,
    pub recommended_min_level: u16,
    pub recommended_max_level: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSkillRule {
    pub spell: String,
    pub class: MirClass,
    pub required_level: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentMonsterRespawnRule {
    pub monster: String,
    pub map_file_name: String,
    pub position: Point,
    pub count: u16,
    pub spread: u16,
    pub delay_minutes: u16,
    pub source_quest_id: i32,
    pub source_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentMonsterDropRule {
    pub monster: String,
    #[serde(default)]
    pub map_file_name: Option<String>,
    pub item: String,
    pub chance_numerator: u32,
    pub chance_denominator: u32,
    #[serde(default)]
    pub quest_required: bool,
    #[serde(default)]
    pub source_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentQuestPrerequisiteRule {
    pub quest_id: i32,
    pub required_quest_id: i32,
    pub source_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentQuestRewardRule {
    pub quest_id: i32,
    pub item: String,
    pub count: u16,
    pub source_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentProfileBundle {
    pub schema: String,
    pub profile_id: String,
    pub profile_version: u32,
    pub acceptance_level: u16,
    pub source: String,
    pub source_data: ContentProfileBundleSourceData,
    pub built_at: String,
    pub hash_algorithm: String,
    pub content_hash: String,
    pub files: Vec<ContentProfileBundleFile>,
    pub summary: ContentProfileBundleSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentProfileBundleSourceData {
    pub crystal_database_version: i32,
    pub crystal_database_custom_version: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentProfileBundleFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentProfileBundleSummary {
    pub maps: usize,
    pub monsters: usize,
    pub items: usize,
    pub skills: usize,
    pub npc_scripts: usize,
    pub respawn_overrides: usize,
    pub drop_overrides: usize,
    pub quest_prerequisite_overrides: usize,
    pub quest_reward_overrides: usize,
}

pub fn platinum_176_profile() -> ContentProfile {
    static PROFILE: OnceLock<ContentProfile> = OnceLock::new();
    PROFILE
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/content_profiles/platinum_176.json"))
                .expect("platinum_176 content profile json should be valid")
        })
        .clone()
}

pub fn platinum_176_profile_bundle() -> ContentProfileBundle {
    static BUNDLE: OnceLock<ContentProfileBundle> = OnceLock::new();
    BUNDLE
        .get_or_init(|| {
            let bundle: ContentProfileBundle =
                serde_json::from_str(include_str!("../data/generated/platinum_176_bundle.json"))
                    .expect("platinum_176 content profile bundle json should be valid");
            let profile = platinum_176_profile();
            assert_eq!(
                (bundle.profile_id.as_str(), bundle.profile_version),
                (profile.profile_id.as_str(), profile.version),
                "platinum_176 bundle must describe the compiled content profile"
            );
            bundle
        })
        .clone()
}

pub fn content_profile_experience_required(profile: &ContentProfile, level: u16) -> Option<i64> {
    profile
        .experience_curve
        .iter()
        .find(|entry| entry.level == level)
        .map(|entry| entry.required_experience)
}

pub fn content_profile_monster_experience_multiplier(profile: &ContentProfile, level: u16) -> u16 {
    profile
        .rate_policy
        .monster_experience_tiers
        .iter()
        .find(|tier| tier.min_level <= level && level <= tier.max_level)
        .map(|tier| tier.multiplier)
        .unwrap_or(1)
        .max(1)
}

pub fn content_profile_monster_is_boss(profile: &ContentProfile, monster_name: &str) -> bool {
    profile
        .boss_monsters
        .iter()
        .any(|boss| boss.eq_ignore_ascii_case(monster_name))
}

pub fn platinum_176_monster_is_boss(monster_name: &str) -> bool {
    content_profile_monster_is_boss(&platinum_176_profile(), monster_name)
}

pub fn content_profile_respawn_overrides_for_map(
    profile: &ContentProfile,
    map_file_name: &str,
) -> Vec<CrystalRespawnTemplate> {
    profile
        .respawn_overrides
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.map_file_name.eq_ignore_ascii_case(map_file_name))
        .filter_map(|(rule_index, rule)| {
            let monster = crystal_monster_by_name(&rule.monster)?;
            Some(CrystalRespawnTemplate {
                monster_index: monster.monster_index,
                location: rule.position.clone(),
                count: rule.count,
                spread: rule.spread,
                delay_minutes: rule.delay_minutes,
                direction: MirDirection::Up,
                route_path: None,
                random_delay_minutes: 0,
                respawn_index: 10_000 + i32::try_from(rule_index).unwrap_or(i32::MAX - 10_000),
                save_respawn_time: false,
                respawn_ticks: 0,
                monster_name: monster.name,
                monster_image: monster.image,
                monster_ai: monster.ai,
                monster_view_range: monster.view_range,
                monster_hp: monster.hp,
                monster_attack_speed: monster.attack_speed,
                monster_move_speed: monster.move_speed,
                monster_can_push: monster.can_push,
                monster_can_tame: monster.can_tame,
                monster_auto_rev: monster.auto_rev,
                monster_undead: monster.undead,
                monster_agility: monster.agility,
                route: Vec::new(),
            })
        })
        .collect()
}

pub fn validate_content_profile(profile: &ContentProfile) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if profile.profile_id.trim().is_empty() {
        errors.push("profileId must not be empty".to_string());
    }
    if profile.version == 0 {
        errors.push("version must be greater than zero".to_string());
    }
    if profile.acceptance_level < 2 {
        errors.push("acceptanceLevel must be at least 2".to_string());
    }
    if profile.boss_respawn_jitter_minutes == 0 {
        errors.push("bossRespawnJitterMinutes must be greater than zero".to_string());
    }
    if profile.rate_policy.label.trim().is_empty() {
        errors.push("ratePolicy.label must not be empty".to_string());
    }
    for (name, multiplier) in [
        (
            "ratePolicy.goldMultiplier",
            profile.rate_policy.gold_multiplier,
        ),
        (
            "ratePolicy.dropMultiplier",
            profile.rate_policy.drop_multiplier,
        ),
    ] {
        if !(1..=100).contains(&multiplier) {
            errors.push(format!("{name} must be between 1 and 100"));
        }
    }
    let mut expected_rate_level = 1_u16;
    for tier in &profile.rate_policy.monster_experience_tiers {
        if tier.min_level != expected_rate_level {
            errors.push(format!(
                "ratePolicy.monsterExperienceTiers must be contiguous: expected level {expected_rate_level}, found {}",
                tier.min_level
            ));
        }
        if tier.max_level < tier.min_level {
            errors.push(format!(
                "ratePolicy.monsterExperienceTiers {}-{} has an invalid range",
                tier.min_level, tier.max_level
            ));
        }
        if !(1..=100).contains(&tier.multiplier) {
            errors.push(format!(
                "ratePolicy.monsterExperienceTiers {}-{} multiplier must be between 1 and 100",
                tier.min_level, tier.max_level
            ));
        }
        expected_rate_level = tier.max_level.saturating_add(1);
    }
    if profile
        .rate_policy
        .monster_experience_tiers
        .last()
        .is_none_or(|tier| tier.max_level < profile.acceptance_level)
    {
        errors.push(format!(
            "ratePolicy.monsterExperienceTiers must cover acceptance level {}",
            profile.acceptance_level
        ));
    }

    let expected_classes = [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist];
    if profile.allowed_classes.as_slice() != expected_classes {
        errors.push(
            "allowedClasses must contain exactly Warrior, Wizard, Taoist in canonical order"
                .to_string(),
        );
    }

    let mut expected_level = 1_u16;
    for entry in &profile.experience_curve {
        if entry.level != expected_level {
            errors.push(format!(
                "experienceCurve must be contiguous: expected level {expected_level}, found {}",
                entry.level
            ));
            expected_level = entry.level;
        }
        if entry.required_experience <= 0 {
            errors.push(format!(
                "experienceCurve level {} must require positive experience",
                entry.level
            ));
        }
        expected_level = expected_level.saturating_add(1);
    }
    if profile
        .experience_curve
        .last()
        .is_none_or(|entry| entry.level < profile.acceptance_level)
    {
        errors.push(format!(
            "experienceCurve must cover acceptance level {}",
            profile.acceptance_level
        ));
    }

    validate_unique_strings(
        "mapWhitelist.fileName",
        profile
            .map_whitelist
            .iter()
            .map(|entry| entry.file_name.as_str()),
        &mut errors,
    );
    validate_unique_strings(
        "monsterWhitelist",
        profile.monster_whitelist.iter().map(String::as_str),
        &mut errors,
    );
    validate_unique_strings(
        "bossMonsters",
        profile.boss_monsters.iter().map(String::as_str),
        &mut errors,
    );
    let mut respawn_override_keys = BTreeSet::new();
    for entry in &profile.respawn_overrides {
        let key = format!("{}/{}", entry.map_file_name, entry.monster);
        if !respawn_override_keys.insert(key.clone()) {
            errors.push(format!(
                "respawnOverrides.map+monster contains duplicate value {key}"
            ));
        }
    }
    let mut drop_override_keys = BTreeSet::new();
    for entry in &profile.drop_overrides {
        let key = format!(
            "{}/{}/{}",
            entry.map_file_name.as_deref().unwrap_or("*"),
            entry.monster,
            entry.item
        );
        if !drop_override_keys.insert(key.clone()) {
            errors.push(format!(
                "dropOverrides.monster+item contains duplicate value {key}"
            ));
        }
    }
    let mut quest_prerequisite_override_ids = BTreeSet::new();
    for entry in &profile.quest_prerequisite_overrides {
        if !quest_prerequisite_override_ids.insert(entry.quest_id) {
            errors.push(format!(
                "questPrerequisiteOverrides.questId contains duplicate value {}",
                entry.quest_id
            ));
        }
    }
    let mut quest_reward_override_keys = BTreeSet::new();
    for entry in &profile.quest_reward_overrides {
        let key = format!("{}/{}", entry.quest_id, entry.item);
        if !quest_reward_override_keys.insert(key.clone()) {
            errors.push(format!(
                "questRewardOverrides.questId+item contains duplicate value {key}"
            ));
        }
    }
    validate_unique_strings(
        "itemWhitelist",
        profile.item_whitelist.iter().map(String::as_str),
        &mut errors,
    );
    validate_unique_strings(
        "skills.spell",
        profile.skills.iter().map(|entry| entry.spell.as_str()),
        &mut errors,
    );
    validate_unique_strings(
        "npcScriptWhitelist",
        profile.npc_script_whitelist.iter().map(String::as_str),
        &mut errors,
    );
    validate_unique_strings(
        "disabledStage5ActionPrefixes",
        profile
            .disabled_stage5_action_prefixes
            .iter()
            .map(String::as_str),
        &mut errors,
    );

    let crystal_map_manifest = crystal_respawn_manifest();
    let crystal_maps: BTreeSet<_> = crystal_map_manifest
        .maps
        .iter()
        .map(|map| map.map_file_name.clone())
        .collect();
    for map in &profile.map_whitelist {
        if map.recommended_min_level == 0 || map.recommended_min_level > map.recommended_max_level {
            errors.push(format!(
                "map {} has invalid recommended level range {}-{}",
                map.file_name, map.recommended_min_level, map.recommended_max_level
            ));
        }
        if !crystal_maps.contains(&map.file_name) {
            errors.push(format!(
                "mapWhitelist references missing Crystal map {}",
                map.file_name
            ));
        }
    }

    let crystal_monsters: BTreeSet<_> = crystal_monster_manifest()
        .monsters
        .into_iter()
        .map(|monster| monster.name)
        .collect();
    let crystal_quest_ids = crystal_quest_packet_manifest()
        .quests
        .into_iter()
        .map(|quest| quest.index)
        .collect::<BTreeSet<_>>();
    for monster in &profile.monster_whitelist {
        if !crystal_monsters.contains(monster) {
            errors.push(format!(
                "monsterWhitelist references missing Crystal monster {monster}"
            ));
        }
    }
    for boss in &profile.boss_monsters {
        if !crystal_monsters.contains(boss) {
            errors.push(format!(
                "bossMonsters references missing Crystal monster {boss}"
            ));
        }
        if !profile
            .monster_whitelist
            .iter()
            .any(|monster| monster.eq_ignore_ascii_case(boss))
        {
            errors.push(format!(
                "bossMonsters entry {boss} must also appear in monsterWhitelist"
            ));
        }
        let has_allowed_spawn = crystal_map_manifest.maps.iter().any(|map| {
            profile
                .map_whitelist
                .iter()
                .any(|allowed| allowed.file_name == map.map_file_name)
                && map
                    .respawns
                    .iter()
                    .any(|respawn| respawn.monster_name.eq_ignore_ascii_case(boss))
        });
        if !has_allowed_spawn {
            errors.push(format!(
                "bossMonsters entry {boss} has no spawn on an allowed map"
            ));
        }
    }
    for respawn_rule in &profile.respawn_overrides {
        if !profile
            .monster_whitelist
            .iter()
            .any(|monster| monster.eq_ignore_ascii_case(&respawn_rule.monster))
        {
            errors.push(format!(
                "respawnOverrides monster {} must appear in monsterWhitelist",
                respawn_rule.monster
            ));
        }
        if !crystal_monsters.contains(&respawn_rule.monster) {
            errors.push(format!(
                "respawnOverrides references missing Crystal monster {}",
                respawn_rule.monster
            ));
        }
        if !profile.map_whitelist.iter().any(|map| {
            map.file_name
                .eq_ignore_ascii_case(&respawn_rule.map_file_name)
        }) {
            errors.push(format!(
                "respawnOverrides map {} must appear in mapWhitelist",
                respawn_rule.map_file_name
            ));
        }
        let has_imported_spawn = crystal_map_manifest.maps.iter().any(|map| {
            map.map_file_name
                .eq_ignore_ascii_case(&respawn_rule.map_file_name)
                && map.respawns.iter().any(|respawn| {
                    respawn
                        .monster_name
                        .eq_ignore_ascii_case(&respawn_rule.monster)
                })
        });
        if has_imported_spawn {
            errors.push(format!(
                "respawnOverrides {}/{} duplicates an existing Crystal respawn",
                respawn_rule.map_file_name, respawn_rule.monster
            ));
        }
        if respawn_rule.position.x < 0 || respawn_rule.position.y < 0 {
            errors.push(format!(
                "respawnOverrides {}/{} has a negative position",
                respawn_rule.map_file_name, respawn_rule.monster
            ));
        }
        if respawn_rule.count == 0 || respawn_rule.delay_minutes == 0 {
            errors.push(format!(
                "respawnOverrides {}/{} must have positive count and delayMinutes",
                respawn_rule.map_file_name, respawn_rule.monster
            ));
        }
        if respawn_rule.source_quest_id <= 0
            || !crystal_quest_ids.contains(&respawn_rule.source_quest_id)
        {
            errors.push(format!(
                "respawnOverrides {}/{} references missing source quest {}",
                respawn_rule.map_file_name, respawn_rule.monster, respawn_rule.source_quest_id
            ));
        }
        if respawn_rule.source_note.trim().is_empty() {
            errors.push(format!(
                "respawnOverrides {}/{} must include a sourceNote",
                respawn_rule.map_file_name, respawn_rule.monster
            ));
        }
    }
    for drop_rule in &profile.drop_overrides {
        if !profile
            .monster_whitelist
            .iter()
            .any(|monster| monster.eq_ignore_ascii_case(&drop_rule.monster))
        {
            errors.push(format!(
                "dropOverrides monster {} must appear in monsterWhitelist",
                drop_rule.monster
            ));
        }
        if let Some(map_file_name) = drop_rule.map_file_name.as_deref() {
            if !profile
                .map_whitelist
                .iter()
                .any(|map| map.file_name.eq_ignore_ascii_case(map_file_name))
            {
                errors.push(format!(
                    "dropOverrides map {map_file_name} must appear in mapWhitelist"
                ));
            }
            let has_matching_spawn = crystal_map_manifest.maps.iter().any(|map| {
                map.map_file_name.eq_ignore_ascii_case(map_file_name)
                    && map.respawns.iter().any(|respawn| {
                        respawn
                            .monster_name
                            .eq_ignore_ascii_case(&drop_rule.monster)
                    })
            }) || profile.respawn_overrides.iter().any(|respawn| {
                respawn.map_file_name.eq_ignore_ascii_case(map_file_name)
                    && respawn.monster.eq_ignore_ascii_case(&drop_rule.monster)
            });
            if !has_matching_spawn {
                errors.push(format!(
                    "dropOverrides {map_file_name}/{} has no matching Crystal or profile respawn",
                    drop_rule.monster
                ));
            }
        }
        if drop_rule.quest_required
            && drop_rule
                .source_note
                .as_deref()
                .is_none_or(|note| note.trim().is_empty())
        {
            errors.push(format!(
                "quest-required dropOverrides {}/{} must include a sourceNote",
                drop_rule.monster, drop_rule.item
            ));
        }
        if !profile
            .item_whitelist
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&drop_rule.item))
        {
            errors.push(format!(
                "dropOverrides item {} must appear in itemWhitelist",
                drop_rule.item
            ));
        }
        if drop_rule.chance_numerator == 0
            || drop_rule.chance_denominator == 0
            || drop_rule.chance_numerator > drop_rule.chance_denominator
        {
            errors.push(format!(
                "dropOverrides {}/{} has invalid chance {}/{}",
                drop_rule.monster,
                drop_rule.item,
                drop_rule.chance_numerator,
                drop_rule.chance_denominator
            ));
        }
    }

    for rule in &profile.quest_prerequisite_overrides {
        if !crystal_quest_ids.contains(&rule.quest_id) {
            errors.push(format!(
                "questPrerequisiteOverrides references missing Crystal quest {}",
                rule.quest_id
            ));
        }
        if rule.required_quest_id < 0 {
            errors.push(format!(
                "questPrerequisiteOverrides q{} has negative requiredQuestId {}",
                rule.quest_id, rule.required_quest_id
            ));
        } else if rule.required_quest_id > 0 && !crystal_quest_ids.contains(&rule.required_quest_id)
        {
            errors.push(format!(
                "questPrerequisiteOverrides q{} references missing prerequisite q{}",
                rule.quest_id, rule.required_quest_id
            ));
        }
        if rule.source_note.trim().is_empty() {
            errors.push(format!(
                "questPrerequisiteOverrides q{} must include a sourceNote",
                rule.quest_id
            ));
        }
    }

    let allowed_maps: BTreeMap<_, _> = profile
        .map_whitelist
        .iter()
        .map(|map| (map.file_name.to_ascii_lowercase(), map.file_name.as_str()))
        .collect();
    let allowed_monsters: BTreeSet<_> = profile
        .monster_whitelist
        .iter()
        .map(String::as_str)
        .collect();
    for level in 1..=profile.acceptance_level {
        let has_hunting_map = profile.map_whitelist.iter().any(|rule| {
            rule.tier != "service"
                && rule.recommended_min_level <= level
                && level <= rule.recommended_max_level
                && crystal_map_manifest.maps.iter().any(|map| {
                    map.map_file_name == rule.file_name
                        && map
                            .respawns
                            .iter()
                            .any(|respawn| allowed_monsters.contains(respawn.monster_name.as_str()))
                })
        });
        if !has_hunting_map {
            errors.push(format!(
                "level {level} has no recommended map with an allowed monster spawn"
            ));
        }
    }

    let map_file_name_by_index: BTreeMap<_, _> = crystal_map_manifest
        .maps
        .iter()
        .map(|map| (map.map_index, map.map_file_name.as_str()))
        .collect();
    let scripted_map_transfers = content_profile_visible_npc_script_map_transfers(profile);
    let mut reachable_maps = BTreeSet::from(["0"]);
    loop {
        let before = reachable_maps.len();
        for map in &crystal_map_manifest.maps {
            if !allowed_maps.contains_key(map.map_file_name.to_ascii_lowercase().as_str())
                || !reachable_maps.contains(map.map_file_name.as_str())
            {
                continue;
            }
            for movement in &map.movements {
                if let Some(destination) = map_file_name_by_index.get(&movement.map_index) {
                    if let Some(canonical_destination) =
                        allowed_maps.get(destination.to_ascii_lowercase().as_str())
                    {
                        reachable_maps.insert(*canonical_destination);
                    }
                }
            }
        }
        for (source, destination) in &scripted_map_transfers {
            if reachable_maps.contains(source.as_str())
                && allowed_maps.contains_key(destination.to_ascii_lowercase().as_str())
            {
                reachable_maps.insert(destination.as_str());
            }
        }
        if reachable_maps.len() == before {
            break;
        }
    }
    for map in &profile.map_whitelist {
        if !reachable_maps.contains(map.file_name.as_str()) {
            errors.push(format!(
                "mapWhitelist map {} is not reachable from map 0 through whitelisted movements or visible NPC scripts",
                map.file_name
            ));
        }
    }

    let crystal_items: BTreeSet<_> = crystal_item_manifest()
        .items
        .into_iter()
        .map(|item| item.name)
        .collect();
    for item in &profile.item_whitelist {
        if !crystal_items.contains(item) {
            errors.push(format!(
                "itemWhitelist references missing Crystal item {item}"
            ));
        }
    }
    for rule in &profile.quest_reward_overrides {
        if !crystal_quest_ids.contains(&rule.quest_id) {
            errors.push(format!(
                "questRewardOverrides references missing Crystal quest {}",
                rule.quest_id
            ));
        }
        if rule.count == 0 {
            errors.push(format!(
                "questRewardOverrides q{}/{} must have positive count",
                rule.quest_id, rule.item
            ));
        }
        if !crystal_items.contains(&rule.item) {
            errors.push(format!(
                "questRewardOverrides q{} references missing Crystal item {}",
                rule.quest_id, rule.item
            ));
        }
        if !profile
            .item_whitelist
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&rule.item))
        {
            errors.push(format!(
                "questRewardOverrides q{}/{} item must appear in itemWhitelist",
                rule.quest_id, rule.item
            ));
        }
        if rule.source_note.trim().is_empty() {
            errors.push(format!(
                "questRewardOverrides q{}/{} must include a sourceNote",
                rule.quest_id, rule.item
            ));
        }
    }

    let crystal_spells: BTreeSet<_> = crystal_magic_manifest()
        .magics
        .into_iter()
        .map(|magic| magic.spell)
        .collect();
    for skill in &profile.skills {
        if !expected_classes.contains(&skill.class) {
            errors.push(format!(
                "skill {} references a class outside platinum_176",
                skill.spell
            ));
        }
        if skill.required_level == 0 || skill.required_level > profile.acceptance_level {
            errors.push(format!(
                "skill {} has invalid required level {}",
                skill.spell, skill.required_level
            ));
        }
        if !crystal_spells.contains(&skill.spell) {
            errors.push(format!(
                "skills references missing Crystal spell {}",
                skill.spell
            ));
        }
        if !profile.item_whitelist.contains(&skill.spell) {
            errors.push(format!(
                "skill {} has no matching book in itemWhitelist",
                skill.spell
            ));
        }
    }

    let crystal_npc_scripts: BTreeSet<_> = crystal_npc_info_manifest()
        .npcs
        .into_iter()
        .map(|npc| npc.script_key)
        .collect();
    for script in &profile.npc_script_whitelist {
        if !crystal_npc_scripts.contains(script) {
            errors.push(format!(
                "npcScriptWhitelist references missing Crystal NPC script {script}"
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Return only map transfers that a normal client can reach from an enabled
/// NPC's visible main dialog. This is content-profile validation metadata: the
/// runtime and autonomous client still execute the actual Crystal dialog and
/// `MOVE` action; this helper never relocates a player.
pub fn content_profile_visible_npc_script_map_transfers(
    profile: &ContentProfile,
) -> Vec<(String, String)> {
    let allowed_maps: BTreeMap<_, _> = profile
        .map_whitelist
        .iter()
        .map(|map| (map.file_name.to_ascii_lowercase(), map.file_name.as_str()))
        .collect();
    let allowed_scripts: BTreeSet<_> = profile
        .npc_script_whitelist
        .iter()
        .map(String::as_str)
        .collect();
    let script_manifest = crystal_npc_manifest();
    let npc_manifest = crystal_npc_info_manifest();
    let mut transfers = BTreeSet::new();

    for npc in &npc_manifest.npcs {
        let Some(source_map) = npc.map_file_name.as_deref() else {
            continue;
        };
        let Some(canonical_source_map) = allowed_maps.get(source_map.to_ascii_lowercase().as_str())
        else {
            continue;
        };
        if !allowed_scripts.contains(npc.script_key.as_str()) {
            continue;
        }
        let Some(script) = script_manifest
            .scripts
            .iter()
            .find(|script| script.script_key.eq_ignore_ascii_case(&npc.script_key))
        else {
            continue;
        };
        let reachable_sections = visible_crystal_npc_script_sections(script);
        for section in &script.sections {
            let label = normalize_crystal_npc_script_label(&section.label);
            if !reachable_sections.contains(&label) {
                continue;
            }
            for line in &section.lines {
                let Some(destination_map) = crystal_npc_script_move_destination(line) else {
                    continue;
                };
                if let Some(canonical_destination_map) =
                    allowed_maps.get(destination_map.to_ascii_lowercase().as_str())
                {
                    transfers.insert((
                        (*canonical_source_map).to_string(),
                        (*canonical_destination_map).to_string(),
                    ));
                }
            }
        }
    }

    transfers.into_iter().collect()
}

fn visible_crystal_npc_script_sections(script: &CrystalNpcScript) -> BTreeSet<String> {
    let sections_by_label: BTreeMap<_, _> = script
        .sections
        .iter()
        .map(|section| (normalize_crystal_npc_script_label(&section.label), section))
        .collect();
    let mut reachable: BTreeSet<_> = ["@main", "main"]
        .into_iter()
        .map(normalize_crystal_npc_script_label)
        .filter(|label| sections_by_label.contains_key(label))
        .collect();

    loop {
        let before = reachable.len();
        let current: Vec<_> = reachable.iter().cloned().collect();
        for label in current {
            let Some(section) = sections_by_label.get(&label) else {
                continue;
            };
            for target in crystal_npc_script_section_targets(section) {
                if sections_by_label.contains_key(&target) {
                    reachable.insert(target);
                }
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    reachable
}

fn crystal_npc_script_section_targets(section: &CrystalNpcSection) -> BTreeSet<String> {
    let mut targets = BTreeSet::new();
    for line in &section.lines {
        let trimmed = line.trim();
        let parts: Vec<_> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 && parts[0].eq_ignore_ascii_case("GOTO") {
            targets.insert(normalize_crystal_npc_script_label(parts[1]));
        }

        let mut remainder = trimmed;
        while let Some(offset) = remainder.find("/@") {
            let target = &remainder[offset + 1..];
            let end = target
                .find(|character: char| {
                    character.is_whitespace() || character == '>' || character == '/'
                })
                .unwrap_or(target.len());
            if end > 1 {
                targets.insert(normalize_crystal_npc_script_label(&target[..end]));
            }
            remainder = &target[end..];
        }
    }
    targets
}

fn crystal_npc_script_move_destination(line: &str) -> Option<String> {
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.len() != 4 || !parts[0].eq_ignore_ascii_case("MOVE") {
        return None;
    }
    parts[2].parse::<i32>().ok()?;
    parts[3].parse::<i32>().ok()?;
    Some(parts[1].to_string())
}

fn normalize_crystal_npc_script_label(label: &str) -> String {
    label.trim().trim_end_matches('>').to_ascii_lowercase()
}

fn validate_unique_strings<'a>(
    field: &str,
    values: impl Iterator<Item = &'a str>,
    errors: &mut Vec<String>,
) {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() {
            errors.push(format!("{field} must not contain empty values"));
        } else if !seen.insert(value) {
            errors.push(format!("{field} contains duplicate value {value}"));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneView {
    pub center: Point,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapBounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainKind {
    Grass,
    Dirt,
    Road,
    Water,
    Stone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainPatchTemplate {
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
    pub kind: TerrainKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecorKind {
    Lantern,
    Banner,
    Tree,
    Rock,
    Campfire,
    Stump,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecorObjectTemplate {
    pub id: String,
    pub kind: DecorKind,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterTemplate {
    pub name: String,
    pub level: u16,
    pub class: MirClass,
    pub gender: MirGender,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisiblePlayerTemplate {
    pub object_id: u32,
    pub name: String,
    pub class: MirClass,
    pub gender: MirGender,
    pub level: u16,
    pub position: Point,
    pub direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibleMonsterTemplate {
    pub object_id: u32,
    pub name: String,
    pub image: u16,
    pub position: Point,
    pub direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibleNpcTemplate {
    pub object_id: u32,
    pub name: String,
    pub image: u16,
    pub colour_argb: i32,
    pub position: Point,
    pub direction: MirDirection,
    pub quest_ids: Vec<i32>,
    #[serde(default)]
    pub script_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneBootstrap {
    pub map: MapInformation,
    pub spawn: Point,
    pub scene_view: SceneView,
    pub terrain_patches: Vec<TerrainPatchTemplate>,
    pub decor_objects: Vec<DecorObjectTemplate>,
    pub default_character: CharacterTemplate,
    pub object_id: u32,
    pub real_id: u32,
    pub visible_players: Vec<VisiblePlayerTemplate>,
    pub visible_monsters: Vec<VisibleMonsterTemplate>,
    pub visible_npcs: Vec<VisibleNpcTemplate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapCellAttribute {
    LowWall,
    HighWall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedMapCellTemplate {
    pub x: i32,
    pub y: i32,
    pub attribute: MapCellAttribute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoorMapCellTemplate {
    pub x: i32,
    pub y: i32,
    pub index: u8,
    pub closed: bool,
}

/// A cell flagged as fishable in the `.map` file (Crystal `Cell.FishingAttribute`,
/// derived from a light byte in 100..=119 → attribute 0..=19).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FishingCellTemplate {
    pub x: i32,
    pub y: i32,
    pub attribute: i8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StarterMapCollision {
    pub map_file_name: String,
    pub map_width: u16,
    pub map_height: u16,
    pub region_bounds: MapBounds,
    pub play_bounds: MapBounds,
    pub blocked_cells: Vec<BlockedMapCellTemplate>,
    pub doors: Vec<DoorMapCellTemplate>,
    #[serde(default)]
    pub fishing_cells: Vec<FishingCellTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterServerData {
    pub monster_spawns: Vec<MonsterSpawnTemplate>,
    pub skills: Vec<SkillTemplate>,
    pub monster_drops: Vec<MonsterDropTable>,
    pub quests: Vec<QuestTemplate>,
    pub npc_scripts: Vec<NpcScriptTemplate>,
    pub buffs: Vec<BuffTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonsterSpawnTemplate {
    pub object_id: u32,
    pub key: String,
    pub name: String,
    pub image: u16,
    pub position: Point,
    pub direction: MirDirection,
    pub count: u16,
    pub spread: u16,
    pub respawn_delay_ticks: u64,
    pub random_delay_ticks: u64,
    pub can_wander: bool,
    pub max_hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillTemplate {
    pub key: String,
    pub crystal_spell: Option<String>,
    pub name: String,
    pub description: String,
    pub cooldown_ticks: u32,
    pub mana_cost: i32,
    pub effect: SkillEffectTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkillEffectTemplate {
    Heal {
        hp: i32,
    },
    Buff {
        buff_key: String,
        buff_name: String,
        buff_description: String,
        duration_ticks: u64,
        attack_bonus: i32,
        defence_bonus: i32,
    },
    Summon {
        spell: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonsterDropTable {
    pub monster_object_id: u32,
    pub drops: Vec<DropTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DropTemplate {
    Gold {
        name: String,
        amount: u32,
        quantity: u32,
    },
    Item {
        key: String,
        name: String,
        description: String,
        weight: u16,
        quantity: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestTemplate {
    pub quest_id: i32,
    pub title: String,
    pub summary: String,
    pub reward_preview: String,
    pub required: u32,
    pub stages: QuestStageTextTemplate,
    pub quest_item: ItemTemplate,
    pub completion_rewards: QuestRewardTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestStageTextTemplate {
    pub available: QuestStageCopy,
    pub in_progress: QuestStageCopy,
    pub ready_to_turn_in: QuestStageCopy,
    pub completed: QuestStageCopy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestStageCopy {
    pub objective: String,
    pub progress_label: String,
    pub tracker: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestRewardTemplate {
    pub gold: u32,
    pub items: Vec<ItemTemplate>,
    pub equipment: Vec<EquipmentTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcScriptTemplate {
    pub npc_object_id: u32,
    pub stages: Vec<NpcStageTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcStageTemplate {
    pub quest_stage: String,
    pub title: String,
    pub body: Vec<String>,
    pub footer: String,
    pub object_chat: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuffTemplate {
    pub key: String,
    pub crystal_buff_type: Option<String>,
    pub name: String,
    pub description: String,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemTemplate {
    pub key: String,
    pub name: String,
    pub description: String,
    pub weight: u16,
    pub quantity: u32,
    pub preferred_slot: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentTemplate {
    pub slot: String,
    pub name: String,
    pub shape: Option<u16>,
    pub description: String,
    pub durability_current: u16,
    pub durability_max: u16,
    pub attack: i32,
    pub defence: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalItemManifest {
    pub generated_at: String,
    pub source_file: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_items: usize,
    pub items: Vec<CrystalItemTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalItemTemplate {
    pub item_index: i32,
    pub name: String,
    pub item_type: u8,
    pub grade: u8,
    pub required_type: u8,
    pub required_class: u8,
    pub required_gender: u8,
    pub item_set: u8,
    pub shape: i16,
    pub weight: u8,
    pub light: u8,
    pub required_amount: u8,
    pub image: u16,
    pub durability: u16,
    pub stack_size: u16,
    pub price: u32,
    pub start_item: bool,
    pub effect: u8,
    pub need_identify: bool,
    pub show_group_pickup: bool,
    pub class_based: bool,
    pub level_based: bool,
    pub can_mine: bool,
    pub global_drop_notify: bool,
    pub bind: i16,
    pub unique: i16,
    pub random_stats_id: u8,
    pub can_fast_run: bool,
    pub can_awakening: bool,
    pub slots: u8,
    pub stats: Vec<CrystalItemStat>,
    pub tooltip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalItemStat {
    pub stat: u8,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRandomItemStatsManifest {
    pub generated_at: String,
    pub source_file: String,
    pub total_profiles: usize,
    pub profiles: Vec<CrystalRandomItemStatProfile>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRandomStatRoll {
    pub chance: u8,
    pub stat_chance: u8,
    pub max_stat: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRandomItemStatProfile {
    pub id: u8,
    pub max_dura: CrystalRandomStatRoll,
    pub max_ac: CrystalRandomStatRoll,
    pub max_mac: CrystalRandomStatRoll,
    pub max_dc: CrystalRandomStatRoll,
    pub max_mc: CrystalRandomStatRoll,
    pub max_sc: CrystalRandomStatRoll,
    pub accuracy: CrystalRandomStatRoll,
    pub agility: CrystalRandomStatRoll,
    pub hp: CrystalRandomStatRoll,
    pub mp: CrystalRandomStatRoll,
    pub strong: CrystalRandomStatRoll,
    pub magic_resist: CrystalRandomStatRoll,
    pub poison_resist: CrystalRandomStatRoll,
    pub hp_recovery: CrystalRandomStatRoll,
    pub mp_recovery: CrystalRandomStatRoll,
    pub poison_recovery: CrystalRandomStatRoll,
    pub critical_rate: CrystalRandomStatRoll,
    pub critical_damage: CrystalRandomStatRoll,
    pub freezing: CrystalRandomStatRoll,
    pub poison_attack: CrystalRandomStatRoll,
    pub attack_speed: CrystalRandomStatRoll,
    pub luck: CrystalRandomStatRoll,
    pub curse_chance: u8,
    pub slot: CrystalRandomStatRoll,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalMagicManifest {
    pub generated_at: String,
    pub source_file: String,
    pub total_magics: usize,
    pub magics: Vec<CrystalMagicTemplate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalMagicTemplate {
    pub name: String,
    pub spell: String,
    pub base_cost: u8,
    pub level_cost: u8,
    pub icon: u8,
    pub level1: u8,
    pub level2: u8,
    pub level3: u8,
    pub need1: u16,
    pub need2: u16,
    pub need3: u16,
    pub delay_base: u32,
    pub delay_reduction: u32,
    pub power_base: u16,
    pub power_bonus: u16,
    pub mpower_base: u16,
    pub mpower_bonus: u16,
    pub multiplier_base: f32,
    pub multiplier_bonus: f32,
    pub range: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalBuffManifest {
    pub generated_at: String,
    pub source_file: String,
    pub total_buffs: usize,
    pub buffs: Vec<CrystalBuffTemplate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalBuffTemplate {
    pub buff_type: String,
    pub stack_type: String,
    pub properties: Vec<String>,
    pub visible: Option<bool>,
    pub visible_expression: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalDropManifest {
    pub generated_at: String,
    pub source_dir: String,
    pub total_tables: usize,
    pub total_entries: usize,
    pub tables: Vec<CrystalDropTable>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalDropTable {
    pub table_key: String,
    pub relative_path: String,
    pub total_entries: usize,
    pub sections: Vec<CrystalDropSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalDropSection {
    pub name: String,
    pub entries: Vec<CrystalDropEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalDropEntry {
    pub raw_line: String,
    pub chance_raw: String,
    pub chance_numerator: Option<u32>,
    pub chance_denominator: Option<u32>,
    pub item_name: String,
    pub amount: Option<u32>,
    pub modifiers: Vec<String>,
    pub group: Option<CrystalDropGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalDropGroup {
    pub random: bool,
    pub first: bool,
    #[serde(default)]
    pub entries: Vec<CrystalDropEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcManifest {
    pub generated_at: String,
    pub source_dir: String,
    pub total_scripts: usize,
    pub total_labels: usize,
    pub total_inserts: usize,
    pub scripts: Vec<CrystalNpcScript>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcInfoManifest {
    pub generated_at: String,
    pub source_file: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_npcs: usize,
    pub npcs: Vec<CrystalNpcInfoTemplate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcInfoTemplate {
    pub npc_index: i32,
    #[serde(default)]
    pub loaded_object_id: Option<u32>,
    pub map_index: i32,
    pub map_file_name: Option<String>,
    pub file_name: String,
    pub script_key: String,
    pub name: String,
    pub location: Point,
    pub image: u16,
    pub rate: u16,
    pub price_rate: f32,
    pub collect_quest_indexes: Vec<i32>,
    pub finish_quest_indexes: Vec<i32>,
    pub time_visible: bool,
    pub hour_start: u8,
    pub minute_start: u8,
    pub hour_end: u8,
    pub minute_end: u8,
    pub min_level: i16,
    pub max_level: i16,
    pub day_of_week: String,
    pub class_required: String,
    pub conquest: i32,
    pub flag_needed: i32,
    pub show_on_big_map: bool,
    pub big_map_icon: i32,
    pub can_teleport_to: bool,
    pub conquest_visible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalQuestPacketManifest {
    pub generated_at: String,
    pub source_file: String,
    pub source_quests_dir: String,
    pub source_npcs_dir: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_quests: usize,
    pub quests: Vec<CrystalQuestPacketTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalQuestPacketTemplate {
    pub index: i32,
    pub name: String,
    pub group: String,
    pub file_name: String,
    #[serde(default)]
    pub required_min_level: i32,
    #[serde(default)]
    pub required_max_level: i32,
    #[serde(default)]
    pub required_quest: i32,
    #[serde(default)]
    pub required_class: u8,
    #[serde(default)]
    pub quest_type: u8,
    #[serde(default)]
    pub goto_message: String,
    #[serde(default)]
    pub kill_message: String,
    #[serde(default)]
    pub item_message: String,
    #[serde(default)]
    pub flag_message: String,
    #[serde(default)]
    pub time_limit_in_seconds: i32,
    pub npc_index: u32,
    pub finish_npc_index: u32,
    #[serde(default)]
    pub carry_items: Vec<CrystalQuestItemTaskTemplate>,
    #[serde(default)]
    pub kill_tasks: Vec<CrystalQuestKillTaskTemplate>,
    #[serde(default)]
    pub item_tasks: Vec<CrystalQuestItemTaskTemplate>,
    #[serde(default)]
    pub flag_tasks: Vec<CrystalQuestFlagTaskTemplate>,
    pub payload_len: usize,
    pub payload_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalQuestKillTaskTemplate {
    pub monster_index: i32,
    pub monster_name: String,
    pub count: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalQuestItemTaskTemplate {
    pub item_index: i32,
    pub item_name: String,
    pub count: u16,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalQuestFlagTaskTemplate {
    pub number: i32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalRecipePacketManifest {
    pub generated_at: String,
    pub source_file: String,
    pub source_recipe_dir: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_recipes: usize,
    pub recipes: Vec<CrystalRecipePacketTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRecipePacketTemplate {
    pub name: String,
    pub item_index: i32,
    pub item_name: String,
    pub gold: u32,
    pub chance: u8,
    pub item_info_indices: Vec<i32>,
    pub tool_count: usize,
    pub ingredient_count: usize,
    pub payload_len: usize,
    pub payload_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrystalRecipeBootstrapPacket {
    pub item_info_indices: Vec<i32>,
    pub payload: Vec<u8>,
}

/// One item slot inside a decoded Crystal recipe (the output, a tool, or an
/// ingredient). Decoded from the `UserItem` records embedded in the captured
/// `NewRecipeInfo` payload, so the values match exactly what Crystal serializes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalRecipeItem {
    pub item_index: i32,
    /// `UserItem.Count`: required quantity for tools/ingredients, or the
    /// produced stack size for the recipe output (`goods.Count`).
    pub count: u16,
    /// `UserItem.CurrentDura`: for ingredients this is the minimum durability the
    /// supplied item must meet when `current_dura < max_dura` (0 means no check).
    pub current_dura: u16,
    pub max_dura: u16,
}

/// A Crystal crafting recipe decoded from the captured `NewRecipeInfo` packet
/// (`ClientRecipeInfo`). Mirrors the server-side `RecipeInfo` fields that reach
/// the client: gold cost, success chance, the produced item, required tools and
/// required ingredients (each with its per-item count). The criteria fields
/// (level/class/gender/quest/flag) live only in the server's `RecipeInfo` and are
/// not transmitted, so they are intentionally absent here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrystalRecipe {
    pub name: String,
    /// `recipe.Item.UniqueID`: the id the client echoes back when crafting.
    pub output_unique_id: u64,
    pub output: CrystalRecipeItem,
    pub gold: u32,
    pub chance: u8,
    pub tools: Vec<CrystalRecipeItem>,
    pub ingredients: Vec<CrystalRecipeItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalGameShopPacketManifest {
    pub generated_at: String,
    pub source_file: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_items: usize,
    pub items: Vec<CrystalGameShopPacketTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalGameShopPacketTemplate {
    pub item_index: i32,
    pub game_shop_index: i32,
    pub item_name: String,
    pub gold_price: u32,
    pub credit_price: u32,
    pub count: u16,
    pub class: String,
    pub category: String,
    pub stock: i32,
    pub stock_level: i32,
    pub payload_len: usize,
    pub payload_hex: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalBaseStatsPacketManifest {
    pub generated_at: String,
    pub source_configs_dir: String,
    pub total_classes: usize,
    pub classes: Vec<CrystalBaseStatsPacketTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalBaseStatsPacketTemplate {
    pub class: String,
    pub class_id: u8,
    pub stat_count: usize,
    pub cap_count: usize,
    pub payload_len: usize,
    pub payload_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalGuildBuffPacketManifest {
    pub generated_at: String,
    pub source_config_file: String,
    pub payload_len: usize,
    pub payload_hex: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcScript {
    pub script_key: String,
    pub relative_path: String,
    pub raw_text: String,
    pub lines: Vec<String>,
    pub line_count: usize,
    pub non_empty_line_count: usize,
    pub label_count: usize,
    pub insert_count: usize,
    pub command_directives: Vec<String>,
    pub labels: Vec<CrystalNpcLabel>,
    pub sections: Vec<CrystalNpcSection>,
    pub inserts: Vec<CrystalNpcInsert>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcLabel {
    pub label: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcSection {
    pub label: String,
    pub line_number: usize,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcInsert {
    pub line_number: usize,
    pub target_path: String,
    pub target_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcCommandSummary {
    pub generated_at: String,
    pub source_file: String,
    pub total_scripts: usize,
    pub total_commands: usize,
    pub implemented_commands: usize,
    pub unimplemented_commands: usize,
    pub implemented_occurrences: usize,
    pub unimplemented_occurrences: usize,
    pub commands: Vec<CrystalNpcCommandSummaryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalNpcCommandSummaryEntry {
    pub kind: String,
    pub command: String,
    pub count: usize,
    pub script_count: usize,
    pub runtime_status: String,
    pub examples: Vec<CrystalNpcCommandExample>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalNpcCommandExample {
    pub script_key: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalMonsterManifest {
    pub generated_at: String,
    pub source_file: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_monsters: usize,
    pub monsters: Vec<CrystalMonsterTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMonsterTemplate {
    pub monster_index: i32,
    pub name: String,
    pub image: u16,
    pub ai: u8,
    pub effect: u8,
    pub level: u16,
    pub view_range: u8,
    pub cool_eye: u8,
    pub hp: i32,
    pub min_ac: i32,
    pub max_ac: i32,
    pub min_mac: i32,
    pub max_mac: i32,
    pub min_dc: i32,
    pub max_dc: i32,
    pub min_mc: i32,
    pub max_mc: i32,
    pub min_sc: i32,
    pub max_sc: i32,
    #[serde(default)]
    pub agility: i32,
    pub light: u8,
    pub attack_speed: u16,
    pub move_speed: u16,
    pub experience: u32,
    pub can_push: bool,
    pub can_tame: bool,
    pub auto_rev: bool,
    pub undead: bool,
    pub drop_path: Option<String>,
    pub can_recall: bool,
    pub is_boss: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMonsterAiSummary {
    pub generated_at: String,
    pub source_files: BTreeMap<String, String>,
    pub total_monsters: usize,
    pub total_ai_families: usize,
    pub spawned_ai_families: usize,
    pub implemented_runtime_families: usize,
    pub generic_runtime_families: usize,
    pub data_only_families: usize,
    pub unknown_source_ai_values: Vec<u8>,
    #[serde(default)]
    pub remaining_runtime_priorities: Vec<CrystalMonsterAiPrioritySummary>,
    pub families: Vec<CrystalMonsterAiFamilySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMonsterAiPrioritySummary {
    pub rank: usize,
    pub ai: u8,
    pub crystal_class: String,
    pub runtime_status: String,
    pub priority_score: usize,
    pub respawn_entity_count: usize,
    pub respawn_rule_count: usize,
    pub map_count: usize,
    pub monster_count: usize,
    #[serde(default)]
    pub boss_monster_count: usize,
    #[serde(default)]
    pub example_monsters: Vec<String>,
    #[serde(default)]
    pub example_maps: Vec<String>,
    #[serde(default)]
    pub priority_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMonsterAiFamilySummary {
    pub ai: u8,
    pub crystal_class: String,
    #[serde(default)]
    pub crystal_notes: Vec<String>,
    pub crystal_todo: bool,
    pub runtime_status: String,
    #[serde(default)]
    pub runtime_notes: Vec<String>,
    pub monster_count: usize,
    pub respawn_rule_count: usize,
    pub respawn_entity_count: usize,
    pub map_count: usize,
    #[serde(default)]
    pub boss_monster_count: usize,
    #[serde(default)]
    pub example_monsters: Vec<String>,
    #[serde(default)]
    pub example_maps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalRespawnManifest {
    pub generated_at: String,
    pub source_file: String,
    pub source_routes_dir: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_maps: usize,
    pub total_respawns: usize,
    pub maps: Vec<CrystalRespawnMap>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalRespawnMap {
    pub map_index: i32,
    pub map_file_name: String,
    pub map_title: String,
    #[serde(default)]
    pub mini_map: u16,
    #[serde(default)]
    pub big_map: u16,
    #[serde(default)]
    pub light: u8,
    #[serde(default)]
    pub map_dark_light: u8,
    #[serde(default)]
    pub weather_particles: u16,
    #[serde(default)]
    pub no_throw_item: bool,
    #[serde(default)]
    pub no_drop_player: bool,
    #[serde(default)]
    pub no_drop_monster: bool,
    #[serde(default)]
    pub no_mount: bool,
    #[serde(default)]
    pub no_hero: bool,
    #[serde(default)]
    pub need_bridle: bool,
    #[serde(default)]
    pub safe_zones: Vec<CrystalSafeZoneTemplate>,
    #[serde(default)]
    pub safe_zone_spells: Vec<CrystalSafeZoneSpellTemplate>,
    #[serde(default)]
    pub movement_count: usize,
    #[serde(default)]
    pub movements: Vec<CrystalMovementTemplate>,
    pub respawn_count: usize,
    pub respawns: Vec<CrystalRespawnTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalSafeZoneTemplate {
    pub location: Point,
    pub size: u16,
    pub start_point: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalSafeZoneSpellTemplate {
    pub object_id: u32,
    pub location: Point,
    pub spell: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMovementTemplate {
    pub map_index: i32,
    pub source: Point,
    pub destination: Point,
    pub need_hole: bool,
    pub need_move: bool,
    pub conquest_index: i32,
    pub show_on_big_map: bool,
    pub icon: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRespawnTemplate {
    pub monster_index: i32,
    pub location: Point,
    pub count: u16,
    pub spread: u16,
    pub delay_minutes: u16,
    pub direction: MirDirection,
    pub route_path: Option<String>,
    pub random_delay_minutes: u16,
    pub respawn_index: i32,
    pub save_respawn_time: bool,
    pub respawn_ticks: u16,
    pub monster_name: String,
    pub monster_image: u16,
    pub monster_ai: u8,
    pub monster_view_range: u8,
    pub monster_hp: i32,
    pub monster_attack_speed: u16,
    pub monster_move_speed: u16,
    pub monster_can_push: bool,
    pub monster_can_tame: bool,
    pub monster_auto_rev: bool,
    pub monster_undead: bool,
    #[serde(default)]
    pub monster_agility: i32,
    pub route: Vec<CrystalRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRoutePoint {
    pub x: i32,
    pub y: i32,
    pub delay: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageCode {
    English,
    ChineseSimplified,
    Spanish,
    Portuguese,
}

impl LanguageCode {
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::ChineseSimplified => "zh-CN",
            Self::Spanish => "es",
            Self::Portuguese => "pt-BR",
        }
    }

    pub fn locale(self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::ChineseSimplified => "zh-CN",
            Self::Spanish => "es-ES",
            Self::Portuguese => "pt-BR",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "en" | "en-us" | "english" => Some(Self::English),
            "zh" | "zh-cn" | "zh-hans" | "chinese" | "chinese-simplified" => {
                Some(Self::ChineseSimplified)
            }
            "es" | "es-es" | "spanish" => Some(Self::Spanish),
            "pt" | "pt-br" | "pt-pt" | "portuguese" | "brazilian" => Some(Self::Portuguese),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizationBundle {
    pub default_language: String,
    pub generated_at: String,
    pub sources: BTreeMap<String, String>,
    pub languages: BTreeMap<String, LocalizationLanguage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizationLanguage {
    pub native_name: String,
    pub locale: String,
    pub texts: BTreeMap<String, String>,
}

pub fn localization_bundle() -> &'static LocalizationBundle {
    static LOCALIZATION_BUNDLE: OnceLock<LocalizationBundle> = OnceLock::new();
    LOCALIZATION_BUNDLE.get_or_init(|| {
        serde_json::from_str(include_str!("../data/generated/localization_bundle.json"))
            .expect("localization bundle json should be valid")
    })
}

pub fn localized_text(language: LanguageCode, key: &str) -> Option<String> {
    let bundle = localization_bundle();
    bundle
        .languages
        .get(language.code())
        .and_then(|entry| entry.texts.get(key))
        .cloned()
        .or_else(|| {
            bundle
                .languages
                .get(bundle.default_language.as_str())
                .and_then(|entry| entry.texts.get(key))
                .cloned()
        })
}

pub fn localized_text_or_key(language: LanguageCode, key: &str) -> String {
    localized_text(language, key).unwrap_or_else(|| key.to_string())
}

pub fn localized_text_or_fallback(language: LanguageCode, key: &str, fallback: &str) -> String {
    localized_text(language, key).unwrap_or_else(|| fallback.to_string())
}

pub fn format_localized_text<I, S>(language: LanguageCode, key: &str, args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    format_localized_text_or_fallback(language, key, key, args)
}

pub fn format_localized_text_or_fallback<I, S>(
    language: LanguageCode,
    key: &str,
    fallback: &str,
    args: I,
) -> String
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    let template = localized_text_or_fallback(language, key, fallback);
    let values = args
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    apply_localized_args(&template, &values)
}

fn apply_localized_args(template: &str, args: &[String]) -> String {
    let mut rendered = template.to_string();
    for (index, value) in args.iter().enumerate() {
        rendered = rendered.replace(&format!("{{{index}}}"), value);
        let format_prefix = format!("{{{index}:");
        while let Some(start) = rendered.find(&format_prefix) {
            let Some(end_offset) = rendered[start..].find('}') else {
                break;
            };
            let end = start + end_offset + 1;
            rendered.replace_range(start..end, value);
        }
    }
    rendered
}

pub fn starter_scene() -> SceneBootstrap {
    static STARTER_SCENE: OnceLock<SceneBootstrap> = OnceLock::new();
    STARTER_SCENE
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/starter_scene.json"))
                .expect("starter scene json should be valid")
        })
        .clone()
}

/// Borrow the process-wide starter-map collision template without cloning it.
/// Prefer this over [`starter_map_collision`] on hot paths — the owned variant
/// deep-clones the whole `StarterMapCollision` (large blocked-cell vectors) on
/// every call.
pub fn starter_map_collision_ref() -> &'static StarterMapCollision {
    static STARTER_MAP_COLLISION: OnceLock<StarterMapCollision> = OnceLock::new();
    STARTER_MAP_COLLISION.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../data/generated/crystal_starter_map_collision.json"
        ))
        .expect("starter map collision json should be valid")
    })
}

pub fn starter_map_collision() -> StarterMapCollision {
    starter_map_collision_ref().clone()
}

pub fn starter_server_data() -> StarterServerData {
    static STARTER_SERVER_DATA: OnceLock<StarterServerData> = OnceLock::new();
    STARTER_SERVER_DATA
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/starter_server_data.json"))
                .expect("starter server data json should be valid")
        })
        .clone()
}

pub fn crystal_item_manifest() -> CrystalItemManifest {
    static CRYSTAL_ITEM_MANIFEST: OnceLock<CrystalItemManifest> = OnceLock::new();
    CRYSTAL_ITEM_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/generated/crystal_item_manifest.json"))
                .expect("crystal item manifest json should be valid")
        })
        .clone()
}

pub fn crystal_random_item_stats_manifest() -> CrystalRandomItemStatsManifest {
    static CRYSTAL_RANDOM_ITEM_STATS_MANIFEST: OnceLock<CrystalRandomItemStatsManifest> =
        OnceLock::new();
    CRYSTAL_RANDOM_ITEM_STATS_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_random_item_stats_manifest.json"
            ))
            .expect("crystal random item stats manifest json should be valid")
        })
        .clone()
}

pub fn crystal_magic_manifest() -> CrystalMagicManifest {
    static CRYSTAL_MAGIC_MANIFEST: OnceLock<CrystalMagicManifest> = OnceLock::new();
    CRYSTAL_MAGIC_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_magic_manifest.json"
            ))
            .expect("crystal magic manifest json should be valid")
        })
        .clone()
}

pub fn crystal_buff_manifest() -> CrystalBuffManifest {
    static CRYSTAL_BUFF_MANIFEST: OnceLock<CrystalBuffManifest> = OnceLock::new();
    CRYSTAL_BUFF_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/generated/crystal_buff_manifest.json"))
                .expect("crystal buff manifest json should be valid")
        })
        .clone()
}

pub fn crystal_drop_manifest() -> CrystalDropManifest {
    static CRYSTAL_DROP_MANIFEST: OnceLock<CrystalDropManifest> = OnceLock::new();
    CRYSTAL_DROP_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/generated/crystal_drop_manifest.json"))
                .expect("crystal drop manifest json should be valid")
        })
        .clone()
}

pub fn crystal_npc_manifest() -> CrystalNpcManifest {
    static CRYSTAL_NPC_MANIFEST: OnceLock<CrystalNpcManifest> = OnceLock::new();
    CRYSTAL_NPC_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/generated/crystal_npc_manifest.json"))
                .expect("crystal npc manifest json should be valid")
        })
        .clone()
}

pub fn crystal_npc_info_manifest() -> CrystalNpcInfoManifest {
    static CRYSTAL_NPC_INFO_MANIFEST: OnceLock<CrystalNpcInfoManifest> = OnceLock::new();
    CRYSTAL_NPC_INFO_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_npc_info_manifest.json"
            ))
            .expect("crystal npc info manifest json should be valid")
        })
        .clone()
}

pub fn crystal_quest_packet_manifest() -> CrystalQuestPacketManifest {
    static CRYSTAL_QUEST_PACKET_MANIFEST: OnceLock<CrystalQuestPacketManifest> = OnceLock::new();
    CRYSTAL_QUEST_PACKET_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_quest_packet_manifest.json"
            ))
            .expect("crystal quest packet manifest json should be valid")
        })
        .clone()
}

pub fn crystal_quest_packet_payloads() -> Vec<Vec<u8>> {
    crystal_quest_packet_manifest()
        .quests
        .into_iter()
        .map(|quest| {
            decode_hex(&quest.payload_hex)
                .expect("crystal quest packet payload hex should be valid")
        })
        .collect()
}

pub fn crystal_recipe_packet_manifest() -> CrystalRecipePacketManifest {
    static CRYSTAL_RECIPE_PACKET_MANIFEST: OnceLock<CrystalRecipePacketManifest> = OnceLock::new();
    CRYSTAL_RECIPE_PACKET_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_recipe_packet_manifest.json"
            ))
            .expect("crystal recipe packet manifest json should be valid")
        })
        .clone()
}

pub fn crystal_recipe_bootstrap_packets() -> Vec<CrystalRecipeBootstrapPacket> {
    crystal_recipe_packet_manifest()
        .recipes
        .into_iter()
        .map(|recipe| CrystalRecipeBootstrapPacket {
            item_info_indices: recipe.item_info_indices,
            payload: decode_hex(&recipe.payload_hex)
                .expect("crystal recipe packet payload hex should be valid"),
        })
        .collect()
}

/// Sequential little-endian reader over a captured recipe payload, mirroring the
/// `BinaryReader` traversal Crystal uses to deserialize `ClientRecipeInfo`.
struct RecipePayloadReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> RecipePayloadReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| "recipe payload length overflow".to_string())?;
        if end > self.bytes.len() {
            return Err(format!(
                "recipe payload truncated: need {len} bytes at offset {} of {}",
                self.offset,
                self.bytes.len()
            ));
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn read_bool(&mut self) -> Result<bool, String> {
        Ok(self.read_u8()? != 0)
    }

    fn read_u16(&mut self) -> Result<u16, String> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        let bytes = self.take(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, String> {
        let bytes = self.take(8)?;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(buf))
    }

    /// Skips a .NET `BinaryWriter`-style length-prefixed UTF-8 string (7-bit
    /// encoded length prefix followed by the raw bytes).
    fn skip_string(&mut self) -> Result<(), String> {
        let mut len: usize = 0;
        let mut shift: u32 = 0;
        loop {
            let byte = self.read_u8()?;
            len |= ((byte & 0x7f) as usize) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        self.take(len)?;
        Ok(())
    }
}

/// Reads a single `UserItem` record, returning its unique id and the recipe-level
/// view (index + count + durability). Follows `UserItem.Save` field order exactly
/// (UniqueID, ItemIndex, CurrentDura, MaxDura, Count, SoulBoundId, bool flags,
/// nested slots, GemCount, AddedStats, Awake, refine fields, WeddingRing, and the
/// optional Expire/Rental/Sealed blocks) so the cursor lands precisely on the
/// next record.
fn read_recipe_user_item(
    reader: &mut RecipePayloadReader,
) -> Result<(u64, CrystalRecipeItem), String> {
    let unique_id = reader.read_u64()?;
    let item_index = reader.read_i32()?;
    let current_dura = reader.read_u16()?;
    let max_dura = reader.read_u16()?;
    let count = reader.read_u16()?;
    let _soul_bound_id = reader.read_i32()?;
    let _bools = reader.read_u8()?;

    let slot_count = reader.read_i32()?;
    for _ in 0..slot_count.max(0) {
        // `UserItem.Save` writes true for empty slots and recurses otherwise.
        let is_null = reader.read_bool()?;
        if !is_null {
            read_recipe_user_item(reader)?;
        }
    }

    let _gem_count = reader.read_u16()?;

    let added_stat_count = reader.read_i32()?;
    for _ in 0..added_stat_count.max(0) {
        let _stat = reader.read_u8()?;
        let _value = reader.read_i32()?;
    }

    let _awake_type = reader.read_u8()?;
    let awake_count = reader.read_i32()?;
    for _ in 0..awake_count.max(0) {
        let _value = reader.read_u8()?;
    }

    let _refined_value = reader.read_u8()?;
    let _refine_added = reader.read_u8()?;
    let _refine_success_chance = reader.read_i32()?;
    let _wedding_ring = reader.read_i32()?;

    if reader.read_bool()? {
        // ExpireInfo: ExpiryDate (i64 binary).
        reader.take(8)?;
    }
    if reader.read_bool()? {
        // RentalInformation: OwnerName, BindingFlags (i16), ExpiryDate (i64), RentalLocked (bool).
        reader.skip_string()?;
        reader.take(2)?;
        reader.take(8)?;
        reader.read_bool()?;
    }
    let _is_shop_item = reader.read_bool()?;
    if reader.read_bool()? {
        // SealedInfo: ExpiryDate + NextSealDate (both i64 binary).
        reader.take(16)?;
    }
    let _gm_made = reader.read_bool()?;

    Ok((
        unique_id,
        CrystalRecipeItem {
            item_index,
            count,
            current_dura,
            max_dura,
        },
    ))
}

fn decode_crystal_recipe(template: &CrystalRecipePacketTemplate) -> Result<CrystalRecipe, String> {
    let bytes = decode_hex(&template.payload_hex)?;
    let mut reader = RecipePayloadReader::new(&bytes);

    let gold = reader.read_u32()?;
    let chance = reader.read_u8()?;
    let (output_unique_id, output) = read_recipe_user_item(&mut reader)?;

    let tool_count = reader.read_i32()?;
    let mut tools = Vec::with_capacity(tool_count.max(0) as usize);
    for _ in 0..tool_count.max(0) {
        tools.push(read_recipe_user_item(&mut reader)?.1);
    }

    let ingredient_count = reader.read_i32()?;
    let mut ingredients = Vec::with_capacity(ingredient_count.max(0) as usize);
    for _ in 0..ingredient_count.max(0) {
        ingredients.push(read_recipe_user_item(&mut reader)?.1);
    }

    if reader.offset != bytes.len() {
        return Err(format!(
            "recipe {} decoded with {} trailing bytes",
            template.name,
            bytes.len() - reader.offset
        ));
    }

    Ok(CrystalRecipe {
        name: template.name.clone(),
        output_unique_id,
        output,
        gold,
        chance,
        tools,
        ingredients,
    })
}

/// Every Crystal crafting recipe, decoded once from the captured `NewRecipeInfo`
/// payloads. The decoded ingredient/tool counts come straight from the bytes
/// Crystal ships to clients, so the crafting transaction stays 1:1 with the
/// original server.
pub fn crystal_recipes() -> Vec<CrystalRecipe> {
    static CRYSTAL_RECIPES: OnceLock<Vec<CrystalRecipe>> = OnceLock::new();
    CRYSTAL_RECIPES
        .get_or_init(|| {
            crystal_recipe_packet_manifest()
                .recipes
                .iter()
                .map(|template| {
                    decode_crystal_recipe(template).unwrap_or_else(|error| {
                        panic!("crystal recipe {} should decode: {error}", template.name)
                    })
                })
                .collect()
        })
        .clone()
}

pub fn crystal_game_shop_packet_manifest() -> CrystalGameShopPacketManifest {
    static CRYSTAL_GAME_SHOP_PACKET_MANIFEST: OnceLock<CrystalGameShopPacketManifest> =
        OnceLock::new();
    CRYSTAL_GAME_SHOP_PACKET_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_game_shop_packet_manifest.json"
            ))
            .expect("crystal game shop packet manifest json should be valid")
        })
        .clone()
}

pub fn crystal_game_shop_info_packet_payloads() -> Vec<Vec<u8>> {
    crystal_game_shop_packet_manifest()
        .items
        .into_iter()
        .map(|item| {
            decode_hex(&item.payload_hex)
                .expect("crystal game shop packet payload hex should be valid")
        })
        .collect()
}

pub fn crystal_base_stats_packet_manifest() -> CrystalBaseStatsPacketManifest {
    static CRYSTAL_BASE_STATS_PACKET_MANIFEST: OnceLock<CrystalBaseStatsPacketManifest> =
        OnceLock::new();
    CRYSTAL_BASE_STATS_PACKET_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_base_stats_packet_manifest.json"
            ))
            .expect("crystal base stats packet manifest json should be valid")
        })
        .clone()
}

pub fn crystal_base_stats_info_packet_payload(class: MirClass) -> Option<Vec<u8>> {
    let class_name = format!("{class:?}");
    crystal_base_stats_packet_manifest()
        .classes
        .into_iter()
        .find(|packet| packet.class == class_name)
        .map(|packet| {
            decode_hex(&packet.payload_hex)
                .expect("crystal base stats packet payload hex should be valid")
        })
}

pub fn crystal_guild_buff_packet_manifest() -> CrystalGuildBuffPacketManifest {
    static CRYSTAL_GUILD_BUFF_PACKET_MANIFEST: OnceLock<CrystalGuildBuffPacketManifest> =
        OnceLock::new();
    CRYSTAL_GUILD_BUFF_PACKET_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_guild_buff_packet_manifest.json"
            ))
            .expect("crystal guild buff packet manifest json should be valid")
        })
        .clone()
}

pub fn crystal_guild_buff_list_packet_payload() -> Vec<u8> {
    let packet = crystal_guild_buff_packet_manifest();
    decode_hex(&packet.payload_hex).expect("crystal guild buff packet payload hex should be valid")
}

pub fn crystal_npc_command_summary() -> CrystalNpcCommandSummary {
    static CRYSTAL_NPC_COMMAND_SUMMARY: OnceLock<CrystalNpcCommandSummary> = OnceLock::new();
    CRYSTAL_NPC_COMMAND_SUMMARY
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_npc_command_summary.json"
            ))
            .expect("crystal npc command summary json should be valid")
        })
        .clone()
}

pub fn crystal_monster_manifest() -> CrystalMonsterManifest {
    static CRYSTAL_MONSTER_MANIFEST: OnceLock<CrystalMonsterManifest> = OnceLock::new();
    CRYSTAL_MONSTER_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_monster_manifest.json"
            ))
            .expect("crystal monster manifest json should be valid")
        })
        .clone()
}

pub fn crystal_monster_ai_summary() -> CrystalMonsterAiSummary {
    static CRYSTAL_MONSTER_AI_SUMMARY: OnceLock<CrystalMonsterAiSummary> = OnceLock::new();
    CRYSTAL_MONSTER_AI_SUMMARY
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_monster_ai_summary.json"
            ))
            .expect("crystal monster ai summary json should be valid")
        })
        .clone()
}

/// Borrow the process-wide respawn manifest without cloning it. The owned
/// [`crystal_respawn_manifest`] deep-clones every map's respawn list on each
/// call, so single-map lookups and per-tick safe-zone checks must use this.
pub fn crystal_respawn_manifest_ref() -> &'static CrystalRespawnManifest {
    static CRYSTAL_RESPAWN_MANIFEST: OnceLock<CrystalRespawnManifest> = OnceLock::new();
    CRYSTAL_RESPAWN_MANIFEST.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../data/generated/crystal_respawn_manifest.json"
        ))
        .expect("crystal respawn manifest json should be valid")
    })
}

pub fn crystal_respawn_manifest() -> CrystalRespawnManifest {
    crystal_respawn_manifest_ref().clone()
}

pub fn crystal_magic_by_spell(spell: &str) -> Option<CrystalMagicTemplate> {
    crystal_magic_manifest()
        .magics
        .into_iter()
        .find(|magic| magic.spell == spell)
}

pub fn crystal_buff_by_type(buff_type: &str) -> Option<CrystalBuffTemplate> {
    crystal_buff_manifest()
        .buffs
        .into_iter()
        .find(|buff| buff.buff_type == buff_type)
}

pub fn crystal_drop_table_by_key(table_key: &str) -> Option<CrystalDropTable> {
    crystal_drop_manifest()
        .tables
        .into_iter()
        .find(|table| table.table_key == table_key)
}

pub fn crystal_drop_table_for_monster_name(monster_name: &str) -> Option<CrystalDropTable> {
    let monster = crystal_monster_by_name(monster_name)?;
    let drop_path = monster.drop_path?;
    let table_key = drop_path.replace('\\', "/");
    crystal_drop_table_by_key(&table_key)
}

pub fn crystal_item_by_name(name: &str) -> Option<CrystalItemTemplate> {
    crystal_item_manifest()
        .items
        .into_iter()
        .find(|item| item.name.eq_ignore_ascii_case(name))
}

pub fn crystal_item_by_index(item_index: i32) -> Option<CrystalItemTemplate> {
    crystal_item_manifest()
        .items
        .into_iter()
        .find(|item| item.item_index == item_index)
}

pub fn crystal_random_item_stat_profile(id: u8) -> Option<CrystalRandomItemStatProfile> {
    crystal_random_item_stats_manifest()
        .profiles
        .into_iter()
        .find(|profile| profile.id == id)
}

pub fn crystal_npc_script_by_key(script_key: &str) -> Option<CrystalNpcScript> {
    crystal_npc_manifest()
        .scripts
        .into_iter()
        .find(|script| script.script_key == script_key)
}

pub fn crystal_npc_info_by_script_key(script_key: &str) -> Option<CrystalNpcInfoTemplate> {
    crystal_npc_info_manifest()
        .npcs
        .into_iter()
        .find(|npc| npc.script_key.eq_ignore_ascii_case(script_key))
}

pub fn crystal_monster_by_name(name: &str) -> Option<CrystalMonsterTemplate> {
    crystal_monster_manifest()
        .monsters
        .into_iter()
        .find(|monster| monster.name.eq_ignore_ascii_case(name))
}

pub fn crystal_monster_by_index(monster_index: i32) -> Option<CrystalMonsterTemplate> {
    crystal_monster_manifest()
        .monsters
        .into_iter()
        .find(|monster| monster.monster_index == monster_index)
}

/// Borrow the respawn record for `file_name` from the process-wide manifest
/// without cloning anything. Read-only hot paths (safe-zone checks run per
/// incoming hit, per-map flag lookups) must use this; the owned
/// [`crystal_map_respawns_by_file_name`] clones the matched map.
pub fn crystal_map_respawns_ref(file_name: &str) -> Option<&'static CrystalRespawnMap> {
    let normalized = normalize_crystal_map_file_name(file_name);
    crystal_respawn_manifest_ref()
        .maps
        .iter()
        .find(|map| normalize_crystal_map_file_name(&map.map_file_name) == normalized)
}

pub fn crystal_map_respawns_by_file_name(file_name: &str) -> Option<CrystalRespawnMap> {
    crystal_map_respawns_ref(file_name).cloned()
}

pub fn crystal_map_respawns_by_index(map_index: i32) -> Option<CrystalRespawnMap> {
    crystal_respawn_manifest_ref()
        .maps
        .iter()
        .find(|map| map.map_index == map_index)
        .cloned()
}

pub fn crystal_starter_region_respawns() -> Vec<CrystalRespawnTemplate> {
    let collision = starter_map_collision_ref();
    let map_file_name = normalize_crystal_map_file_name(&collision.map_file_name);

    crystal_map_respawns_ref(&map_file_name)
        .map(|map| {
            map.respawns
                .iter()
                .filter(|respawn| respawn_overlaps_bounds(respawn, collision.region_bounds))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_crystal_map_file_name(file_name: &str) -> String {
    file_name
        .trim()
        .trim_end_matches(".map")
        .trim_end_matches(".MAP")
        .to_ascii_lowercase()
}

fn respawn_overlaps_bounds(respawn: &CrystalRespawnTemplate, bounds: MapBounds) -> bool {
    let spread = i32::from(respawn.spread);
    respawn.location.x + spread >= bounds.min_x
        && respawn.location.x - spread <= bounds.max_x
        && respawn.location.y + spread >= bounds.min_y
        && respawn.location.y - spread <= bounds.max_y
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 {
        return Err("hex string has odd length".to_string());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let raw = value.as_bytes();
    for index in (0..raw.len()).step_by(2) {
        let high = hex_nibble(raw[index])?;
        let low = hex_nibble(raw[index + 1])?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex byte {byte}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        content_profile_experience_required, content_profile_respawn_overrides_for_map,
        content_profile_visible_npc_script_map_transfers, crystal_base_stats_info_packet_payload,
        crystal_base_stats_packet_manifest, crystal_buff_by_type, crystal_buff_manifest,
        crystal_drop_manifest, crystal_drop_table_by_key, crystal_drop_table_for_monster_name,
        crystal_game_shop_info_packet_payloads, crystal_game_shop_packet_manifest,
        crystal_guild_buff_list_packet_payload, crystal_guild_buff_packet_manifest,
        crystal_item_by_index, crystal_item_by_name, crystal_item_manifest, crystal_magic_by_spell,
        crystal_magic_manifest, crystal_map_respawns_by_file_name, crystal_map_respawns_by_index,
        crystal_monster_ai_summary, crystal_monster_by_index, crystal_monster_by_name,
        crystal_monster_manifest, crystal_npc_command_summary, crystal_npc_info_by_script_key,
        crystal_npc_info_manifest, crystal_npc_manifest, crystal_npc_script_by_key,
        crystal_quest_packet_manifest, crystal_quest_packet_payloads,
        crystal_random_item_stat_profile, crystal_random_item_stats_manifest,
        crystal_recipe_bootstrap_packets, crystal_recipe_packet_manifest, crystal_recipes,
        crystal_respawn_manifest, crystal_starter_region_respawns, format_localized_text,
        localization_bundle, localized_text, platinum_176_profile, platinum_176_profile_bundle,
        starter_map_collision, starter_scene, starter_server_data, validate_content_profile,
        ContentLevelRate, ContentRatePolicy, DropTemplate, LanguageCode, MapCellAttribute,
        SkillEffectTemplate,
    };
    use mir2_protocol::{MirClass, Point};

    #[test]
    fn starter_scene_loads() {
        let scene = starter_scene();

        assert_eq!(scene.default_character.name, "Scout");
        assert_eq!(scene.scene_view.width, 24);
        assert_eq!(scene.terrain_patches.len(), 6);
        assert_eq!(scene.decor_objects.len(), 8);
        assert_eq!(scene.visible_players.len(), 1);
        assert_eq!(scene.visible_monsters.len(), 2);
        assert_eq!(scene.visible_npcs.len(), 1);
    }

    #[test]
    fn platinum_176_profile_is_reference_complete_and_has_a_1_to_50_curve() {
        let profile = platinum_176_profile();

        assert_eq!(profile.profile_id, "platinum_176");
        assert_eq!(profile.version, 24);
        assert_eq!(
            profile.allowed_classes,
            [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist,]
        );
        assert_eq!(profile.acceptance_level, 50);
        assert_eq!(
            profile.rate_policy,
            ContentRatePolicy {
                label: "launch_candidate_tiered_xp_1x_economy".to_string(),
                monster_experience_tiers: vec![
                    ContentLevelRate {
                        min_level: 1,
                        max_level: 21,
                        multiplier: 2,
                    },
                    ContentLevelRate {
                        min_level: 22,
                        max_level: 35,
                        multiplier: 3,
                    },
                    ContentLevelRate {
                        min_level: 36,
                        max_level: 50,
                        multiplier: 4,
                    },
                ],
                gold_multiplier: 1,
                drop_multiplier: 1,
            }
        );
        assert_eq!(profile.boss_monsters.len(), 7);
        assert_eq!(profile.boss_respawn_jitter_minutes, 30);
        assert_eq!(profile.respawn_overrides.len(), 2);
        assert_eq!(profile.drop_overrides.len(), 25);
        assert_eq!(profile.quest_prerequisite_overrides.len(), 1);
        assert_eq!(profile.quest_reward_overrides.len(), 1);
        assert_eq!(
            profile.disabled_stage5_action_prefixes,
            [
                "auction.",
                "craft",
                "gameShop.",
                "hero.",
                "item.addSocket",
                "item.seal",
                "mail.",
                "qa.",
                "shop.buyCredit",
            ]
        );
        assert!(profile.drop_overrides.iter().any(|rule| {
            rule.monster == "WhiteBoar"
                && rule.item == "GreatFireBall"
                && rule.chance_numerator == 1
                && rule.chance_denominator == 3
        }));
        assert!(profile.respawn_overrides.iter().any(|rule| {
            rule.monster == "ChainGhoul"
                && rule.map_file_name == "D421"
                && rule.source_quest_id == 75
                && rule.count == 15
                && rule.source_note.contains("no ChainGhoul respawn")
        }));
        assert!(profile.respawn_overrides.iter().any(|rule| {
            rule.monster == "RotNdZombie"
                && rule.map_file_name == "D422"
                && rule.source_quest_id == 78
                && rule.source_note.contains("no RotNdZombie respawn")
        }));
        let d421_overrides = content_profile_respawn_overrides_for_map(&profile, "D421");
        assert_eq!(d421_overrides.len(), 1);
        assert_eq!(d421_overrides[0].monster_name, "ChainGhoul");
        assert_eq!(d421_overrides[0].respawn_index, 10_000);
        let d422_overrides = content_profile_respawn_overrides_for_map(&profile, "D422");
        assert_eq!(d422_overrides.len(), 1);
        assert_eq!(d422_overrides[0].monster_name, "RotNdZombie");
        assert_eq!(d422_overrides[0].respawn_index, 10_001);
        assert!(content_profile_visible_npc_script_map_transfers(&profile)
            .contains(&("0".to_string(), "WhiteVillage".to_string(),)));
        assert!(profile.drop_overrides.iter().any(|rule| {
            rule.monster == "RotNdZombie"
                && rule.map_file_name.as_deref() == Some("D422")
                && rule.item == "StolenGold"
                && rule.quest_required
        }));
        assert!(profile.drop_overrides.iter().any(|rule| {
            rule.monster == "BloodyLureSpider"
                && rule.map_file_name.as_deref() == Some("12")
                && rule.item == "WornAxe"
                && rule.quest_required
                && rule
                    .source_note
                    .as_deref()
                    .is_some_and(|note| note.contains("q91"))
        }));
        assert!(profile.drop_overrides.iter().any(|rule| {
            rule.monster == "RedEvilApe"
                && rule.map_file_name.as_deref() == Some("D10053")
                && rule.item == "RedMoonChip"
                && rule.chance_numerator == 1
                && rule.chance_denominator == 1
                && rule.quest_required
                && rule
                    .source_note
                    .as_deref()
                    .is_some_and(|note| note.contains("q148") && note.contains("RedMoonEvil1"))
        }));
        assert!(profile
            .item_whitelist
            .iter()
            .any(|item| item == "GoldChestnut"));
        for monster in ["ChestnutTree", "ChestnutTree1", "ChestnutTree2"] {
            assert!(profile
                .monster_whitelist
                .iter()
                .any(|candidate| candidate == monster));
        }
        for script in [
            "MongchonProvince/WierdPillar",
            "MongchonProvince/StrangePillar",
            "MongchonProvince/MysteriousPillar",
        ] {
            assert!(profile
                .npc_script_whitelist
                .iter()
                .any(|candidate| candidate == script));
        }
        for monster in [
            "HungryZombie",
            "RoninGhoul",
            "ToxicGhoul",
            "BoneArcher",
            "BoneSpearman",
            "BoneBlademan",
        ] {
            assert!(profile
                .monster_whitelist
                .iter()
                .any(|candidate| candidate == monster));
        }
        for map in [
            "1006", "B354", "D2070", "D2071", "D2072", "D2073", "D2074", "D2075",
        ] {
            assert!(profile
                .map_whitelist
                .iter()
                .any(|candidate| candidate.file_name == map));
        }
        for map in [
            "D10053", "D10054", "D10061", "HELL00", "R01", "R02", "R03", "R04", "R05", "R06",
            "R07", "R08", "R09", "R10", "R11", "R12", "RCK",
        ] {
            assert!(profile
                .map_whitelist
                .iter()
                .any(|candidate| candidate.file_name == map));
        }
        for monster in [
            "RedEvilApe",
            "GreyEvilApe",
            "GhastlyLeecher",
            "CyanoGhast",
            "MutatedManworm",
            "CrazyManworm",
            "DreamDevourer",
        ] {
            assert!(profile
                .monster_whitelist
                .iter()
                .any(|candidate| candidate == monster));
        }
        assert!(profile.quest_prerequisite_overrides.iter().any(|rule| {
            rule.quest_id == 58 && rule.required_quest_id == 0 && rule.source_note.contains("q57")
        }));
        assert!(profile.quest_reward_overrides.iter().any(|rule| {
            rule.quest_id == 135
                && rule.item == "StoneHeart"
                && rule.count == 1
                && rule.source_note.contains("MysteriousStone")
        }));
        assert!(profile.drop_overrides.iter().any(|rule| {
            rule.monster == "Skeleton"
                && rule.map_file_name.as_deref() == Some("D001")
                && rule.item == "OliviasRing"
                && rule.chance_numerator == 1
                && rule.chance_denominator == 4
                && rule.quest_required
                && rule
                    .source_note
                    .as_deref()
                    .is_some_and(|note| !note.is_empty())
        }));
        assert!(profile.drop_overrides.iter().any(|rule| {
            rule.monster == "EvilCentipede"
                && rule.item == "SummonShinsu"
                && rule.chance_numerator == 1
                && rule.chance_denominator == 3
        }));
        assert_eq!(profile.experience_curve.len(), 50);
        assert_eq!(content_profile_experience_required(&profile, 1), Some(100));
        assert_eq!(
            content_profile_experience_required(&profile, 49),
            Some(300_000_000)
        );
        assert_eq!(
            content_profile_experience_required(&profile, 50),
            Some(350_000_000)
        );
        assert_eq!(validate_content_profile(&profile), Ok(()));
    }

    #[test]
    fn platinum_176_prajna_island_requires_the_visible_round_trip_sailor_scripts() {
        let mut profile = platinum_176_profile();
        assert!(profile
            .npc_script_whitelist
            .iter()
            .any(|script| script == "BichonProvince/Sailor"));
        assert!(profile
            .npc_script_whitelist
            .iter()
            .any(|script| script == "PrajnaIsland/Sailor"));

        profile
            .npc_script_whitelist
            .retain(|script| script != "BichonProvince/Sailor" && script != "PrajnaIsland/Sailor");
        let errors = validate_content_profile(&profile)
            .expect_err("map 5 must not be reachable after removing its visible boats");
        assert!(errors.iter().any(|error| {
            error.contains("mapWhitelist map 5 is not reachable")
                && error.contains("visible NPC scripts")
        }));
    }

    #[test]
    fn platinum_176_profile_bundle_matches_the_compiled_profile() {
        let profile = platinum_176_profile();
        let bundle = platinum_176_profile_bundle();

        assert_eq!(bundle.schema, "mir2-content-profile-bundle/1");
        assert_eq!(bundle.profile_id, profile.profile_id);
        assert_eq!(bundle.profile_version, profile.version);
        assert_eq!(bundle.acceptance_level, profile.acceptance_level);
        assert_eq!(bundle.source, profile.source);
        assert_eq!(bundle.hash_algorithm, "sha256");
        assert_eq!(bundle.content_hash.len(), 64);
        assert!(bundle
            .content_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(bundle.source_data.crystal_database_version, 117);
        assert_eq!(bundle.summary.maps, profile.map_whitelist.len());
        assert_eq!(bundle.summary.monsters, profile.monster_whitelist.len());
        assert_eq!(bundle.summary.items, profile.item_whitelist.len());
        assert_eq!(bundle.summary.skills, profile.skills.len());
        assert_eq!(
            bundle.summary.npc_scripts,
            profile.npc_script_whitelist.len()
        );
        assert_eq!(bundle.summary.drop_overrides, profile.drop_overrides.len());
        assert_eq!(
            bundle.summary.respawn_overrides,
            profile.respawn_overrides.len()
        );
        assert_eq!(
            bundle.summary.quest_reward_overrides,
            profile.quest_reward_overrides.len()
        );
        assert_eq!(bundle.files.len(), 13);
    }

    #[test]
    fn starter_server_data_loads() {
        let data = starter_server_data();

        assert_eq!(data.monster_spawns.len(), 2);
        assert_eq!(data.skills.len(), 7);
        assert_eq!(data.monster_drops.len(), 2);
        assert_eq!(data.quests.len(), 1);
        assert_eq!(data.npc_scripts.len(), 1);
        assert_eq!(data.buffs.len(), 1);
        assert_eq!(data.monster_spawns[0].count, 1);
        assert_eq!(data.skills[0].mana_cost, 6);
        assert!(matches!(
            data.skills[1].effect,
            SkillEffectTemplate::Buff {
                attack_bonus: 4,
                defence_bonus: 1,
                ..
            }
        ));
        assert!(matches!(
            data.skills[2].effect,
            SkillEffectTemplate::Summon { ref spell } if spell == "SummonShinsu"
        ));
        assert!(matches!(
            data.monster_drops[0].drops[0],
            DropTemplate::Gold { amount: 45, .. }
        ));
        assert_eq!(data.quests[0].completion_rewards.gold, 300);
        assert_eq!(data.npc_scripts[0].npc_object_id, 4001);
        assert_eq!(data.buffs[0].key, "battle-focus");
        assert_eq!(data.skills[0].crystal_spell.as_deref(), Some("Healing"));
        assert_eq!(data.buffs[0].crystal_buff_type.as_deref(), Some("Fury"));
    }

    #[test]
    fn starter_map_collision_loads() {
        let collision = starter_map_collision();

        assert_eq!(collision.map_file_name, "0.map");
        assert_eq!(collision.map_width, 700);
        assert_eq!(collision.map_height, 700);
        assert_eq!(collision.region_bounds.min_x, 302);
        assert_eq!(collision.region_bounds.max_y, 313);
        assert_eq!(collision.blocked_cells.len(), 744);
        assert_eq!(collision.doors.len(), 7);
        assert!(collision.blocked_cells.iter().any(|cell| {
            cell.x == 327 && cell.y == 265 && cell.attribute == MapCellAttribute::HighWall
        }));
    }

    #[test]
    fn localization_bundle_loads() {
        let bundle = localization_bundle();

        assert_eq!(bundle.default_language, "en");
        assert!(bundle.languages.contains_key("zh-CN"));
        assert!(bundle.languages.contains_key("es"));
        assert!(bundle.languages.contains_key("pt-BR"));
    }

    #[test]
    fn localization_lookup_uses_selected_language() {
        let value = localized_text(LanguageCode::ChineseSimplified, "client.GameName")
            .expect("client.GameName should exist");

        assert!(value.contains("传奇"));
    }

    #[test]
    fn portuguese_language_is_wired() {
        assert_eq!(LanguageCode::Portuguese.code(), "pt-BR");
        assert_eq!(LanguageCode::parse("pt-BR"), Some(LanguageCode::Portuguese));
        assert_eq!(LanguageCode::parse("pt"), Some(LanguageCode::Portuguese));
        // A key present in the pt-BR bundle resolves to a non-English string.
        let pt = localized_text(LanguageCode::Portuguese, "client.Warrior")
            .expect("client.Warrior should exist in pt-BR");
        let en = localized_text(LanguageCode::English, "client.Warrior")
            .expect("client.Warrior should exist in en");
        assert!(!pt.is_empty());
        assert_ne!(pt, en);
    }

    #[test]
    fn localization_formatter_replaces_placeholders() {
        let value = format_localized_text(
            LanguageCode::English,
            "sim.youHitTargetForDamage",
            ["Field Wasp", "18"],
        );

        assert_eq!(value, "You hit Field Wasp for 18.");
    }

    #[test]
    fn crystal_magic_manifest_loads() {
        let manifest = crystal_magic_manifest();

        assert!(manifest.total_magics > 50);
        assert!(manifest
            .magics
            .iter()
            .any(|magic| magic.spell == "FireBall"));
        assert!(manifest
            .magics
            .iter()
            .any(|magic| magic.spell == "Thrusting" && magic.multiplier_base > 0.0));
    }

    #[test]
    fn crystal_buff_manifest_loads() {
        let manifest = crystal_buff_manifest();

        assert!(manifest.total_buffs > 40);
        assert!(manifest
            .buffs
            .iter()
            .any(|buff| buff.buff_type == "Curse"
                && buff.properties.iter().any(|prop| prop == "Debuff")));
        assert!(manifest
            .buffs
            .iter()
            .any(|buff| buff.buff_type == "SwiftFeet" && buff.visible == Some(true)));
    }

    #[test]
    fn crystal_drop_manifest_loads() {
        let manifest = crystal_drop_manifest();

        assert!(manifest.total_tables > 1000);
        assert!(manifest.total_entries > 70_000);

        let fishing = manifest
            .tables
            .iter()
            .find(|table| table.table_key == "00Fishing")
            .expect("00Fishing table should exist");
        assert!(fishing
            .sections
            .iter()
            .any(|section| section.name == "Fish Market"
                && section.entries.iter().any(|entry| {
                    entry.item_name == "Trout"
                        && entry.chance_numerator == Some(1)
                        && entry.chance_denominator == Some(500)
                })));

        let bone_fighter = manifest
            .tables
            .iter()
            .find(|table| table.table_key == "BichonProvince/OmaCave/BoneFighter")
            .expect("nested bone fighter table should exist");
        assert!(bone_fighter.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.item_name == "Gold"
                    && entry.amount == Some(200)
                    && entry.chance_numerator == Some(1)
                    && entry.chance_denominator == Some(10)
            })
        }));
    }

    #[test]
    fn crystal_drop_lookup_preserves_special_entry_shapes() {
        let dice = crystal_drop_table_by_key("00DiceItem").expect("00DiceItem should exist");
        assert!(dice.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.chance_raw == "0"
                    && entry.item_name == "Gold"
                    && entry.amount == Some(1000)
                    && entry.modifiers.is_empty()
            })
        }));

        let mir_king = crystal_drop_table_by_key("MirKing").expect("MirKing should exist");
        assert!(mir_king.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.item_name == "Gold"
                    && entry.amount == Some(15_000)
                    && entry.modifiers.iter().any(|modifier| modifier == "LV1")
            })
        }));

        let hi_great_ghoul =
            crystal_drop_table_by_key("HiGreatGhoul").expect("HiGreatGhoul should exist");
        assert!(hi_great_ghoul.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.raw_line == "1/1RedDagger Q"
                    && entry.item_name == "RedDagger"
                    && entry.modifiers.iter().any(|modifier| modifier == "Q")
            })
        }));
    }

    #[test]
    fn crystal_drop_entry_deserializes_group_shape() {
        let entry: super::CrystalDropEntry = serde_json::from_value(serde_json::json!({
            "raw_line": "0 GROUP*",
            "chance_raw": "0",
            "chance_numerator": null,
            "chance_denominator": null,
            "item_name": "GROUP",
            "amount": null,
            "modifiers": [],
            "group": {
                "random": true,
                "first": false,
                "entries": [
                    {
                        "raw_line": "0 Gold 1000",
                        "chance_raw": "0",
                        "chance_numerator": null,
                        "chance_denominator": null,
                        "item_name": "Gold",
                        "amount": 1000,
                        "modifiers": []
                    }
                ]
            }
        }))
        .expect("group-shaped drop entry should deserialize");

        let group = entry.group.expect("group metadata");
        assert!(group.random);
        assert!(!group.first);
        assert_eq!(group.entries.len(), 1);
        assert_eq!(group.entries[0].item_name, "Gold");
    }

    #[test]
    fn crystal_monster_drop_path_resolves_imported_drop_table() {
        let hen = crystal_drop_table_for_monster_name("Hen").expect("Hen drops should resolve");
        assert_eq!(hen.table_key, "Provinces/Hen");
        assert!(hen.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.item_name == "Chicken"
                    && entry.chance_numerator == Some(1)
                    && entry.chance_denominator == Some(1)
            })
        }));

        let deer = crystal_drop_table_for_monster_name("Deer").expect("Deer drops should resolve");
        assert_eq!(deer.table_key, "Provinces/Deer");
        assert!(deer.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.item_name == "Venison"
                    && entry.chance_numerator == Some(1)
                    && entry.chance_denominator == Some(1)
            })
        }));
    }

    #[test]
    fn crystal_item_manifest_loads() {
        let manifest = crystal_item_manifest();

        assert!(manifest.total_items > 1_000);

        let bronze_helmet =
            crystal_item_by_name("BronzeHelmet").expect("BronzeHelmet should exist");
        assert_eq!(bronze_helmet.item_index, 595);
        assert_eq!(bronze_helmet.item_type, 4);
        assert_eq!(bronze_helmet.image, 100);
        assert!(bronze_helmet
            .stats
            .iter()
            .any(|stat| stat.stat == 1 && stat.value == 1));

        let hp_drug = crystal_item_by_index(658).expect("(HP)DrugSmall should exist");
        assert_eq!(hp_drug.name, "(HP)DrugSmall");
        assert_eq!(hp_drug.stack_size, 20);
        assert!(hp_drug
            .stats
            .iter()
            .any(|stat| stat.stat == 12 && stat.value == 30));
    }

    #[test]
    fn crystal_random_item_stats_manifest_loads() {
        let manifest = crystal_random_item_stats_manifest();

        assert_eq!(manifest.total_profiles, 11);

        let weapon = crystal_random_item_stat_profile(1).expect("profile 1 should exist");
        assert_eq!(weapon.max_dura.chance, 2);
        assert_eq!(weapon.max_dc.stat_chance, 15);
        assert_eq!(weapon.max_mc.max_stat, 13);
        assert_eq!(weapon.accuracy.max_stat, 2);
        assert_eq!(weapon.attack_speed.chance, 130);

        let cursed = crystal_random_item_stat_profile(8).expect("profile 8 should exist");
        assert_eq!(cursed.curse_chance, 5);
        assert_eq!(cursed.max_ac.chance, 3);
        assert!(crystal_random_item_stat_profile(11).is_none());
    }

    #[test]
    fn crystal_npc_manifest_loads() {
        let manifest = crystal_npc_manifest();

        assert!(manifest.total_scripts > 300);
        assert!(manifest.total_labels > 1_000);
        assert!(manifest.total_inserts >= 10);

        let default_script =
            crystal_npc_script_by_key("00Default").expect("00Default script should exist");
        assert_eq!(default_script.insert_count, 10);
        assert!(default_script
            .raw_text
            .contains("SystemScripts\\00Default\\Login.txt"));
        assert_eq!(default_script.lines.len(), default_script.line_count);
        assert!(default_script.inserts.iter().any(|insert| {
            insert.target_path == "SystemScripts/00Default/Login.txt"
                && insert.target_label.as_deref() == Some("@Main")
        }));

        let test_script = crystal_npc_script_by_key("Test").expect("Test script should exist");
        assert!(test_script
            .command_directives
            .iter()
            .any(|directive| directive == "#SAY"));
        assert!(test_script
            .command_directives
            .iter()
            .any(|directive| directive == "#ACT"));
        assert!(test_script
            .labels
            .iter()
            .any(|label| label.label == "@Warrior"));
        assert!(test_script
            .labels
            .iter()
            .any(|label| label.label == "@Level"));
        assert!(test_script.lines.iter().any(|line| line.trim() == "#SAY"));
        assert!(test_script.sections.iter().any(|section| {
            section.label.eq_ignore_ascii_case("@main")
                && section.lines.iter().any(|line| line.trim() == "#SAY")
        }));

        let administrator = crystal_npc_script_by_key("BichonProvince/BichonWall/Administrator")
            .expect("Administrator script should exist");
        assert!(administrator
            .labels
            .iter()
            .any(|label| label.label == "@requestcastlewar"));
    }

    #[test]
    fn crystal_npc_info_manifest_loads() {
        let manifest = crystal_npc_info_manifest();

        assert!(manifest.total_npcs > 300);

        let wicked_trader =
            crystal_npc_info_by_script_key("BichonProvince/NaturalCave/WickedTrader")
                .expect("natural cave wicked trader NPC info should exist");
        assert_eq!(wicked_trader.map_file_name.as_deref(), Some("DM001"));
        assert_eq!(wicked_trader.rate, 200);
        assert_eq!(wicked_trader.price_rate, 2.0);
        assert_eq!(wicked_trader.location, Point { x: 4, y: 6 });
        assert!(wicked_trader.loaded_object_id.is_some());
    }

    #[test]
    fn crystal_quest_packet_manifest_loads() {
        let manifest = crystal_quest_packet_manifest();

        assert_eq!(manifest.total_quests, 154);
        assert_eq!(manifest.total_quests, manifest.quests.len());

        let first = manifest.quests.first().expect("first quest packet");
        assert_eq!(first.index, 1);
        assert_eq!(first.name, "Assistant's Request");
        assert_eq!(first.payload_len, first.payload_hex.len() / 2);

        let payloads = crystal_quest_packet_payloads();
        assert_eq!(payloads.len(), manifest.total_quests);
        assert_eq!(payloads[0].len(), first.payload_len);
    }

    #[test]
    fn crystal_recipe_packet_manifest_loads() {
        let manifest = crystal_recipe_packet_manifest();

        assert!(manifest.total_recipes >= 70);
        assert_eq!(manifest.total_recipes, manifest.recipes.len());

        let first = manifest.recipes.first().expect("first recipe packet");
        assert_eq!(first.name, "(HP)DrugXL");
        assert_eq!(first.item_index, 664);
        assert_eq!(first.payload_len, first.payload_hex.len() / 2);
        assert!(first.item_info_indices.contains(&664));

        let packets = crystal_recipe_bootstrap_packets();
        assert_eq!(packets.len(), manifest.total_recipes);
        assert_eq!(packets[0].payload.len(), first.payload_len);
    }

    #[test]
    fn crystal_recipes_decode_every_payload() {
        let manifest = crystal_recipe_packet_manifest();
        let recipes = crystal_recipes();
        assert_eq!(recipes.len(), manifest.total_recipes);

        // Each decoded recipe must agree with the pre-extracted manifest fields:
        // gold, chance, tool/ingredient counts, and the ordered item-info indices
        // (output, then tools, then ingredients). This proves the binary cursor
        // lands exactly on every record across all recipes.
        for (recipe, template) in recipes.iter().zip(manifest.recipes.iter()) {
            assert_eq!(recipe.gold, template.gold, "{} gold", recipe.name);
            assert_eq!(recipe.chance, template.chance, "{} chance", recipe.name);
            assert_eq!(
                recipe.tools.len(),
                template.tool_count,
                "{} tools",
                recipe.name
            );
            assert_eq!(
                recipe.ingredients.len(),
                template.ingredient_count,
                "{} ingredients",
                recipe.name
            );

            let mut indices = vec![recipe.output.item_index];
            indices.extend(recipe.tools.iter().map(|tool| tool.item_index));
            indices.extend(
                recipe
                    .ingredients
                    .iter()
                    .map(|ingredient| ingredient.item_index),
            );
            assert_eq!(
                indices, template.item_info_indices,
                "{} indices",
                recipe.name
            );
        }
    }

    #[test]
    fn crystal_recipes_decode_per_ingredient_counts() {
        let recipes = crystal_recipes();

        // BraveryOrb: a tool plus a multi-count ingredient at a non-100 chance.
        let bravery = recipes
            .iter()
            .find(|recipe| recipe.name == "BraveryOrb")
            .expect("BraveryOrb recipe");
        assert_eq!(bravery.gold, 10_000);
        assert_eq!(bravery.chance, 20);
        assert_eq!(bravery.output_unique_id, 17);
        assert_eq!(bravery.output.item_index, 747);
        assert_eq!(bravery.output.count, 1);
        assert_eq!(bravery.tools.len(), 1);
        assert_eq!(bravery.tools[0].item_index, 1348);
        assert_eq!(bravery.tools[0].count, 1);
        assert_eq!(
            bravery
                .ingredients
                .iter()
                .map(|ingredient| (ingredient.item_index, ingredient.count))
                .collect::<Vec<_>>(),
            vec![(646, 1), (677, 1), (664, 2)]
        );

        // GreenPoison: produces a stack of 4 and consumes multi-count ingredients.
        let poison = recipes
            .iter()
            .find(|recipe| recipe.name == "GreenPoison")
            .expect("GreenPoison recipe");
        assert_eq!(poison.output.item_index, 710);
        assert_eq!(poison.output.count, 4);
        assert_eq!(
            poison
                .ingredients
                .iter()
                .map(|ingredient| (ingredient.item_index, ingredient.count))
                .collect::<Vec<_>>(),
            vec![(864, 1), (868, 2), (866, 4)]
        );
    }

    #[test]
    fn crystal_game_shop_packet_manifest_loads() {
        let manifest = crystal_game_shop_packet_manifest();

        assert_eq!(manifest.total_items, 105);
        assert_eq!(manifest.total_items, manifest.items.len());

        let first = manifest.items.first().expect("first game shop packet");
        assert_eq!(first.item_index, 1268);
        assert_eq!(first.item_name, "DestructionLiquor1");
        assert_eq!(first.payload_len, first.payload_hex.len() / 2);

        let payloads = crystal_game_shop_info_packet_payloads();
        assert_eq!(payloads.len(), manifest.total_items);
        assert_eq!(payloads[0].len(), first.payload_len);
    }

    #[test]
    fn crystal_base_stats_packet_manifest_loads() {
        let manifest = crystal_base_stats_packet_manifest();

        assert_eq!(manifest.total_classes, 5);
        assert_eq!(manifest.total_classes, manifest.classes.len());

        let warrior = manifest
            .classes
            .iter()
            .find(|packet| packet.class == "Warrior")
            .expect("Warrior base stats packet");
        assert_eq!(warrior.class_id, 0);
        assert_eq!(warrior.stat_count, 11);
        assert_eq!(warrior.cap_count, 9);
        assert_eq!(warrior.payload_len, warrior.payload_hex.len() / 2);

        let payload =
            crystal_base_stats_info_packet_payload(MirClass::Warrior).expect("Warrior payload");
        assert_eq!(payload.len(), warrior.payload_len);
    }

    #[test]
    fn crystal_guild_buff_packet_manifest_loads() {
        let manifest = crystal_guild_buff_packet_manifest();

        assert_eq!(manifest.payload_len, 660);
        assert_eq!(manifest.payload_len, manifest.payload_hex.len() / 2);

        let payload = crystal_guild_buff_list_packet_payload();
        assert_eq!(payload.len(), manifest.payload_len);
    }

    #[test]
    fn crystal_npc_command_summary_classifies_runtime_coverage() {
        let summary = crystal_npc_command_summary();

        assert!(summary.total_scripts > 300);
        assert!(summary.total_commands >= 80);
        assert_eq!(summary.unimplemented_commands, 0);
        assert_eq!(summary.unimplemented_occurrences, 0);
        assert!(summary.implemented_commands >= 45);
        assert!(summary.implemented_occurrences > summary.unimplemented_occurrences);
        let simplified: Vec<_> = summary
            .commands
            .iter()
            .filter(|entry| entry.runtime_status == "simplified")
            .collect();
        assert!(
            simplified.is_empty(),
            "unexpected simplified NPC command coverage entries: {:?}",
            simplified
        );
        let missing: Vec<_> = summary
            .commands
            .iter()
            .filter(|entry| {
                entry.runtime_status == "missing" || entry.runtime_status == "unimplemented"
            })
            .collect();
        assert!(
            missing.is_empty(),
            "unexpected missing NPC command coverage entries: {:?}",
            missing
        );
        assert!(summary.commands.iter().any(|entry| {
            entry.kind == "action"
                && entry.command == "GOTO"
                && entry.runtime_status == "implemented"
                && entry.count > 1_000
        }));
        assert!(summary.commands.iter().any(|entry| {
            entry.kind == "condition"
                && entry.command == "LEVEL"
                && entry.runtime_status == "implemented"
        }));
        assert!(summary.commands.iter().any(|entry| {
            entry.command == "CONQUESTGUARD" && entry.runtime_status == "implemented"
        }));
    }

    #[test]
    fn crystal_monster_manifest_loads() {
        let manifest = crystal_monster_manifest();

        assert!(manifest.total_monsters > 200);

        let bug_bat = crystal_monster_by_name("BugBat").expect("BugBat should exist");
        assert_eq!(bug_bat.image, 42);
        assert!(bug_bat.attack_speed > 0);
        assert!(bug_bat.move_speed > 0);
        assert!(bug_bat.max_dc >= bug_bat.min_dc);
        assert!(bug_bat.agility >= 0);
        assert_eq!(
            crystal_monster_by_index(bug_bat.monster_index)
                .expect("BugBat should resolve by index")
                .name,
            "BugBat"
        );

        let bomb_spider = crystal_monster_by_name("BombSpider").expect("BombSpider should exist");
        assert!(bomb_spider.max_dc > 0);
        assert!(bomb_spider.max_sc >= bomb_spider.min_sc);
        assert!(bomb_spider.agility >= 0);
    }

    #[test]
    fn crystal_monster_ai_summary_classifies_manifest_families() {
        let manifest = crystal_monster_manifest();
        let summary = crystal_monster_ai_summary();

        assert_eq!(summary.total_monsters, manifest.total_monsters);
        assert!(summary.total_ai_families > 100);
        assert!(summary.spawned_ai_families > 80);
        assert!(summary.implemented_runtime_families >= 95);
        assert!(summary.unknown_source_ai_values.is_empty());
        assert!(summary.remaining_runtime_priorities.is_empty());
        if !summary.remaining_runtime_priorities.is_empty() {
            assert!(summary
                .remaining_runtime_priorities
                .windows(2)
                .all(|entries| {
                    let left = &entries[0];
                    let right = &entries[1];
                    left.priority_score > right.priority_score
                        || (left.priority_score == right.priority_score
                            && left.respawn_entity_count >= right.respawn_entity_count)
                }));
            assert!(summary
                .remaining_runtime_priorities
                .iter()
                .all(|entry| entry.rank > 0
                    && entry.respawn_entity_count > 0
                    && matches!(
                        entry.runtime_status.as_str(),
                        "generic_baseline" | "wildlife_partial"
                    )
                    && !entry.priority_reasons.is_empty()));
        }

        let by_ai: std::collections::BTreeMap<u8, _> = summary
            .families
            .iter()
            .map(|family| (family.ai, family))
            .collect();
        assert!(manifest
            .monsters
            .iter()
            .all(|monster| by_ai.contains_key(&monster.ai)));

        let default_monster = by_ai.get(&0).expect("AI 0 family should be classified");
        assert_eq!(default_monster.crystal_class, "MonsterObject");
        assert_eq!(default_monster.runtime_status, "implemented_special");
        assert!(default_monster.respawn_entity_count > 40_000);

        let spider = by_ai.get(&4).expect("AI 4 family should be classified");
        assert_eq!(spider.crystal_class, "SpittingSpider");
        assert_eq!(spider.runtime_status, "implemented_special");
        assert!(spider.respawn_entity_count > 1_000);

        let dig_out_zombie = by_ai.get(&24).expect("AI 24 family should be classified");
        assert_eq!(dig_out_zombie.crystal_class, "DigOutZombie");
        assert_eq!(dig_out_zombie.runtime_status, "implemented_special");

        let reviving_zombie = by_ai.get(&25).expect("AI 25 family should be classified");
        assert_eq!(reviving_zombie.crystal_class, "RevivingZombie");
        assert_eq!(reviving_zombie.runtime_status, "implemented_special");

        let shaman = by_ai.get(&26).expect("AI 26 family should be classified");
        assert_eq!(shaman.crystal_class, "ShamanZombie");
        assert_eq!(shaman.runtime_status, "implemented_special");

        let guard = by_ai.get(&6).expect("AI 6 family should be classified");
        assert_eq!(guard.crystal_class, "Guard");
        assert_eq!(guard.runtime_status, "neutral_guard_baseline");

        let black_foxman = by_ai.get(&44).expect("AI 44 family should be classified");
        assert_eq!(black_foxman.crystal_class, "BlackFoxman");
        assert_eq!(black_foxman.runtime_status, "implemented_special");

        let yin_devil_node = by_ai.get(&42).expect("AI 42 family should be classified");
        assert_eq!(yin_devil_node.crystal_class, "YinDevilNode");
        assert_eq!(yin_devil_node.runtime_status, "implemented_special");

        let red_foxman = by_ai.get(&45).expect("AI 45 family should be classified");
        assert_eq!(red_foxman.crystal_class, "RedFoxman");
        assert_eq!(red_foxman.runtime_status, "implemented_special");

        let white_foxman = by_ai.get(&46).expect("AI 46 family should be classified");
        assert_eq!(white_foxman.crystal_class, "WhiteFoxman");
        assert_eq!(white_foxman.runtime_status, "implemented_special");

        let black_hammer_cat = by_ai.get(&116).expect("AI 116 family should be classified");
        assert_eq!(black_hammer_cat.crystal_class, "BlackHammerCat");
        assert_eq!(black_hammer_cat.runtime_status, "implemented_special");

        let stray_cat = by_ai.get(&117).expect("AI 117 family should be classified");
        assert_eq!(stray_cat.crystal_class, "StrayCat");
        assert_eq!(stray_cat.runtime_status, "implemented_special");

        let cat_shaman = by_ai.get(&118).expect("AI 118 family should be classified");
        assert_eq!(cat_shaman.crystal_class, "CatShaman");
        assert_eq!(cat_shaman.runtime_status, "implemented_special");

        let water_dragon = by_ai.get(&181).expect("AI 181 family should be classified");
        assert_eq!(water_dragon.crystal_class, "WaterDragon");
        assert_eq!(water_dragon.runtime_status, "implemented_special");

        let black_tortoise = by_ai.get(&182).expect("AI 182 family should be classified");
        assert_eq!(black_tortoise.crystal_class, "BlackTortoise");
        assert_eq!(black_tortoise.runtime_status, "implemented_special");

        let hell_knight = by_ai.get(&97).expect("AI 97 family should be classified");
        assert_eq!(hell_knight.crystal_class, "HellKnight");
        assert_eq!(hell_knight.runtime_status, "implemented_special");
        assert!(hell_knight.respawn_entity_count > 1_000);

        let hell_lord = by_ai.get(&98).expect("AI 98 family should be classified");
        assert_eq!(hell_lord.crystal_class, "HellLord");
        assert_eq!(hell_lord.runtime_status, "implemented_special");

        let hell_bomb = by_ai.get(&99).expect("AI 99 family should be classified");
        assert_eq!(hell_bomb.crystal_class, "HellBomb");
        assert_eq!(hell_bomb.runtime_status, "implemented_special");

        let stone_trap = by_ai.get(&255).expect("AI 255 family should be classified");
        assert_eq!(stone_trap.crystal_class, "StoneTrap");
        assert_eq!(stone_trap.runtime_status, "implemented_special");
    }

    #[test]
    fn crystal_respawn_manifest_loads() {
        let manifest = crystal_respawn_manifest();

        assert!(manifest.total_maps > 400);
        assert!(manifest.total_respawns > 6_000);
        assert_eq!(manifest.total_maps, manifest.maps.len());
        assert!(manifest.maps.iter().any(|map| map.respawn_count == 0));
        assert!(manifest.maps.iter().all(|map| {
            map.respawn_count == map.respawns.len() && map.movement_count == map.movements.len()
        }));
        let map_indices: std::collections::BTreeSet<i32> =
            manifest.maps.iter().map(|map| map.map_index).collect();
        let missing_movement_targets: Vec<_> = manifest
            .maps
            .iter()
            .flat_map(|map| {
                let map_indices = &map_indices;
                map.movements.iter().filter_map(move |movement| {
                    (!map_indices.contains(&movement.map_index)).then_some((
                        map.map_file_name.as_str(),
                        movement.map_index,
                        movement.source.clone(),
                        movement.destination.clone(),
                    ))
                })
            })
            .collect();
        assert_eq!(
            missing_movement_targets,
            vec![
                ("4", 388, Point { x: 70, y: 191 }, Point { x: 77, y: 74 }),
                ("4", 388, Point { x: 71, y: 190 }, Point { x: 77, y: 74 }),
            ]
        );

        let bichon = crystal_map_respawns_by_file_name("0.map").expect("Bichon map should exist");
        assert_eq!(bichon.map_title, "BichonProvince");
        assert_eq!(bichon.mini_map, 101);
        assert_eq!(bichon.big_map, 101);
        assert!(bichon.safe_zones.iter().any(|safe_zone| {
            safe_zone.location == (Point { x: 288, y: 616 })
                && safe_zone.size == 10
                && safe_zone.start_point
        }));
        assert!(bichon.safe_zone_spells.iter().any(|spell| {
            spell.object_id == 46 && spell.location == (Point { x: 278, y: 606 })
        }));
        assert!(bichon.movements.iter().any(|movement| {
            movement.map_index == 2
                && movement.source == (Point { x: 347, y: 188 })
                && movement.destination == (Point { x: 13, y: 40 })
                && !movement.need_hole
                && !movement.need_move
        }));
        assert!(bichon
            .respawns
            .iter()
            .any(|respawn| respawn.monster_name == "Hen"));
        assert!(bichon
            .respawns
            .iter()
            .any(|respawn| respawn.monster_name == "Royal_Guard"));
        assert!(bichon.respawns.iter().any(|respawn| {
            respawn.monster_name == "Royal_Guard"
                && respawn.monster_ai == 6
                && respawn.monster_view_range > 0
                && respawn.monster_move_speed >= 400
        }));
        assert_eq!(
            crystal_map_respawns_by_index(bichon.map_index)
                .expect("Bichon should resolve by index")
                .map_file_name,
            "0"
        );

        let penal_cavern =
            crystal_map_respawns_by_file_name("D1801").expect("Penal Cavern should exist");
        assert_eq!(penal_cavern.map_title, "PenalCavern");
        assert_eq!(penal_cavern.mini_map, 0);
        assert_eq!(penal_cavern.big_map, 0);
        assert_eq!(penal_cavern.light, 4);
        assert!(!penal_cavern.movements.is_empty());

        let dog_yo_hyun = crystal_map_respawns_by_file_name("DogYoHyun")
            .expect("DogYoHyun weather map should exist");
        assert_eq!(dog_yo_hyun.weather_particles, 3);
        assert_eq!(dog_yo_hyun.map_dark_light, 0);
    }

    #[test]
    fn crystal_starter_region_respawns_include_expected_monsters() {
        let respawns = crystal_starter_region_respawns();

        assert!(respawns.len() >= 13);
        assert!(respawns.iter().any(|respawn| {
            respawn.monster_name == "Hen" && respawn.spread > 0 && respawn.count >= 5
        }));
        assert!(respawns
            .iter()
            .any(|respawn| respawn.monster_name == "Royal_Guard"));
        assert!(respawns
            .iter()
            .any(|respawn| respawn.monster_name == "Royal_Archer"));
        assert!(respawns.iter().any(|respawn| {
            respawn.monster_name == "Royal_Archer"
                && respawn.monster_ai == 57
                && respawn.monster_attack_speed >= 400
        }));
    }

    #[test]
    fn starter_runtime_data_maps_into_crystal_manifests() {
        let data = starter_server_data();

        assert!(data.skills.iter().all(|skill| {
            skill
                .crystal_spell
                .as_deref()
                .and_then(crystal_magic_by_spell)
                .is_some()
        }));
        assert!(data.buffs.iter().all(|buff| {
            buff.crystal_buff_type
                .as_deref()
                .and_then(crystal_buff_by_type)
                .is_some()
        }));
    }
}
