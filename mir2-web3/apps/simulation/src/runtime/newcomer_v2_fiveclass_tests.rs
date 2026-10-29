//! Custom newcomer V2 exercises for the two additional source professions.
//! Typed outcome projection is tested here; real impact authority, damage and
//! life fences are exercised separately in zone/runtime/five_class_combat_tests.
use super::*;
use crate::config::{EquipmentSlot, ItemContainer, QuestStage};
use crate::runtime::components::{player_entity, CharacterBody};
use crate::runtime::resources::{
    set_runtime_tick, InventoryResource, MapRuntimeResource, SkillResource,
};
use crate::runtime::zone::{
    ZoneJourneyEventKind as Event, ZoneJourneyEventReceipt,
    ZoneJourneyPhysicalTechnique as Technique, ZoneKey,
};
use crate::{SimulationConfig, SimulationSession};
use mir2_protocol::{ClientPacket, Point, ServerPacket, Spell};

const PRACTICE: [i32; 8] = [
    2_110_004, 2_110_008, 2_110_011, 2_110_012, 2_110_015, 2_110_016, 2_110_018, 2_110_021,
];

fn session(class: MirClass, gender: MirGender) -> (SimulationSession, SimulationConfig, i32) {
    let config = SimulationConfig::default().with_platinum_fiveclass_profile();
    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    let packets = session.handle_packet(ClientPacket::NewCharacter {
        name: if class == MirClass::Assassin {
            "PracticeA"
        } else {
            "PracticeR"
        }
        .into(),
        gender,
        class,
    });
    let index = packets
        .iter()
        .find_map(|p| match p {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .expect("fiveclass profile admits actual NewCharacter");
    session.handle_packet(ClientPacket::StartGame {
        character_index: index,
    });
    assert_eq!(session.active_identity().unwrap().character_index, index);
    let world = session.app.world_mut();
    world.resource_mut::<QuestResource>().quests.clear();
    world.resource_mut::<QuestResource>().newcomer_v1_cadence = false;
    world.resource_mut::<QuestResource>().newcomer_v2_cadence = true;
    world.resource_mut::<SkillResource>().skills.clear();
    world
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 50;
    let player = player_entity(world).unwrap();
    world
        .entity_mut(player)
        .get_mut::<CharacterBody>()
        .unwrap()
        .level = 50;
    (session, config, index)
}

fn learn(world: &mut World, spell: &str, level: u8) {
    let skill = crate::runtime::skills::crystal_skill_state(spell, level).unwrap();
    let mut skills = world.resource_mut::<SkillResource>();
    skills.skills.retain(|old| old.key != skill.key);
    skills.skills.push(skill);
}

fn learn_core(world: &mut World, class: MirClass) {
    for spell in if class == MirClass::Assassin {
        &["FatalSword", "DoubleSlash"][..]
    } else {
        &["Focus", "StraightShot", "DoubleShot"][..]
    } {
        learn(world, spell, 0);
    }
}

fn equip_source_weapon(world: &mut World, class: MirClass) {
    let name = if class == MirClass::Assassin {
        "HoaSword"
    } else {
        "WoodenBow"
    };
    let template = crystal_item_by_name(name).unwrap();
    let item =
        crate::runtime::items::embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    let equipment =
        crate::runtime::equipment::equipment_state_from_item_state(&item, EquipmentSlot::Weapon);
    let mut inventory = world.resource_mut::<InventoryResource>();
    inventory.equipment_items.clear();
    inventory.equipment_items.push(equipment);
}

fn begin(session: &mut SimulationSession, id: i32) -> (i32, u64) {
    let map = definition(id).unwrap().maps.first().unwrap().clone();
    session
        .app
        .world_mut()
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .file_name = map;
    assert_eq!(
        super::super::begin_quest(session.app.world_mut(), id),
        if id == 2_110_018 {
            // This exercise has only learned-skill/equipment and arrival
            // conditions, which this fixture establishes before acceptance.
            QuestStage::ReadyToTurnIn
        } else {
            QuestStage::InProgress
        },
        "practice {id} must enter its exact authored initial stage"
    );
    let world = session.app.world();
    let flag = definition(id)
        .unwrap()
        .flags
        .iter()
        .find(|flag| flag.kind == FlagKind::ClassPractice)
        .unwrap()
        .number;
    let stamp = world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|q| q.quest_id == id)
        .unwrap()
        .task_progress
        .keys()
        .filter_map(|key| key.strip_prefix("v2:accepted_at:")?.parse().ok())
        .max()
        .unwrap();
    (flag, stamp)
}

fn condition(world: &World, id: i32, flag: i32, condition: &str) -> bool {
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == id)
        .unwrap()
        .task_progress
        .get(&format!("v2:condition:{flag}:{condition}"))
        == Some(&1)
}

fn flag_complete(world: &World, id: i32, flag: i32) -> bool {
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == id)
        .unwrap()
        .task_progress
        .get(&super::super::crystal_flag_task_key(flag))
        == Some(&1)
}

fn receipt(
    session: &SimulationSession,
    kind: Event,
    source_at: u64,
    damage: i32,
) -> ZoneJourneyEventReceipt {
    let identity = session.active_identity().unwrap();
    let map = session
        .app
        .world()
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    ZoneJourneyEventReceipt {
        kind,
        session_id: "fiveclass-projection".into(),
        account_id: identity.account_id,
        character_index: identity.character_index,
        object_id: 1_000,
        life_generation: 1,
        zone_key: ZoneKey::for_map(map),
        source_action_at_ms: source_at,
        committed_at_ms: source_at + 600,
        event_sequence: 1,
        source_object_id: 1_000,
        target_object_id: Some(9_001),
        target_location: Some(Point { x: 11, y: 10 }),
        effect_object_id: None,
        damage: Some(damage),
        material_consumed: false,
    }
}

fn deliver(session: &mut SimulationSession, kind: Event, source_at: u64) {
    let event = receipt(session, kind, source_at, 1);
    session.commit_zone_journey_event(event);
}

#[test]
fn fiveclass_newcomer_has_eight_distinct_source_practices_for_each_added_class() {
    let authored: Config = serde_json::from_str(CONFIG_JSON).unwrap();
    let ids = authored
        .quests
        .iter()
        .filter(|quest| {
            quest
                .flags
                .iter()
                .any(|flag| flag.kind == FlagKind::ClassPractice)
        })
        .map(|quest| quest.id)
        .collect::<Vec<_>>();
    assert_eq!(ids, PRACTICE);
    for class in [MirClass::Assassin, MirClass::Archer] {
        for id in PRACTICE {
            let flag = definition(id)
                .unwrap()
                .flags
                .iter()
                .find(|flag| flag.kind == FlagKind::ClassPractice)
                .unwrap();
            let conditions = flag.requirements.as_ref().unwrap().for_class(class);
            assert!(!conditions.is_empty(), "{class:?} {id}");
            assert!(
                conditions.iter().all(|condition| match class {
                    MirClass::Assassin => [
                        "FatalSword learned",
                        "normal attack landed with FatalSword learned",
                        "DoubleSlash learned",
                        "DoubleSlash attack damage committed",
                        "legal reposition between attacks",
                        "level-eligible weapon equipped"
                    ]
                    .contains(&condition.as_str()),
                    _ => [
                        "Focus learned",
                        "normal ranged attack landed with Focus learned",
                        "StraightShot learned",
                        "StraightShot damage committed",
                        "DoubleShot learned",
                        "DoubleShot damage committed",
                        "legal reposition between attacks",
                        "level-eligible weapon equipped"
                    ]
                    .contains(&condition.as_str()),
                }),
                "no unsupported or higher-level spell can strand {class:?} node {id}"
            );
        }
    }
}

#[test]
fn fiveclass_all_sixteen_practice_flags_accept_only_their_authored_outcome_sequence() {
    for class in [MirClass::Assassin, MirClass::Archer] {
        for id in PRACTICE {
            let (mut session, _, _) = session(class, MirGender::Male);
            learn_core(session.app.world_mut(), class);
            equip_source_weapon(session.app.world_mut(), class);
            let (flag, stamp) = begin(&mut session, id);
            let conditions = definition(id)
                .unwrap()
                .flags
                .iter()
                .find(|f| f.number == flag)
                .unwrap()
                .requirements
                .as_ref()
                .unwrap()
                .for_class(class)
                .to_vec();
            if id == 2_110_018 {
                assert!(
                    flag_complete(session.app.world(), id, flag),
                    "learned + actual eligible source weapon"
                );
                continue;
            }
            assert!(
                !flag_complete(session.app.world(), id, flag),
                "learning is not a damage outcome"
            );
            let event_for = |condition: &str| match condition {
                "normal attack landed with FatalSword learned" => Some(Event::PhysicalDamage {
                    technique: Technique::Normal,
                }),
                "normal ranged attack landed with Focus learned" => Some(Event::RangedDamage),
                "DoubleSlash attack damage committed" => Some(Event::PhysicalDamage {
                    technique: Technique::DoubleSlash,
                }),
                "StraightShot damage committed" => Some(Event::StraightShotDamage),
                "DoubleShot damage committed" => Some(Event::DoubleShotDamage),
                _ => None,
            };
            let events = conditions
                .iter()
                .filter_map(|condition| event_for(condition))
                .collect::<Vec<_>>();
            assert!(!events.is_empty());
            for event in &events {
                deliver(&mut session, *event, stamp + 1);
            }
            if conditions
                .iter()
                .any(|condition| condition == "legal reposition between attacks")
            {
                assert!(
                    !flag_complete(session.app.world(), id, flag),
                    "damage/damage has no legal move"
                );
                session.commit_zone_journey_reposition();
                assert!(
                    !flag_complete(session.app.world(), id, flag),
                    "damage/move still needs subsequent damage"
                );
                deliver(&mut session, events[0], stamp + 2);
            }
            for authored in &conditions {
                assert!(
                    condition(session.app.world(), id, flag, authored),
                    "{class:?} {id}: {authored}"
                );
            }
            assert!(
                flag_complete(session.app.world(), id, flag),
                "{class:?} {id}"
            );
            // Monster counts belong to separate objectives and never become
            // inferred kills merely because the class practice flag completed.
            assert!(session
                .app
                .world()
                .resource::<QuestResource>()
                .quests
                .iter()
                .find(|q| q.quest_id == id)
                .unwrap()
                .task_progress
                .iter()
                .filter(|(key, _)| key.starts_with("kill:"))
                .all(|(_, value)| *value == 0));
        }
    }
}

#[test]
fn fiveclass_training_rejects_zero_damage_pre_acceptance_wrong_map_and_wrong_owner() {
    for class in [MirClass::Assassin, MirClass::Archer] {
        let (mut session, _, _) = session(class, MirGender::Female);
        learn_core(session.app.world_mut(), class);
        let (flag, stamp) = begin(&mut session, 2_110_004);
        let kind = if class == MirClass::Assassin {
            Event::PhysicalDamage {
                technique: Technique::Normal,
            }
        } else {
            Event::RangedDamage
        };
        for boundary in 0..5 {
            let mut event = receipt(&session, kind, stamp + 1, 1);
            match boundary {
                0 => event.damage = Some(0),
                1 => event.source_action_at_ms = stamp - 1,
                2 => event.zone_key = ZoneKey::for_map("D401"),
                3 => event.account_id = "other-account".into(),
                _ => event.character_index += 1,
            }
            session.commit_zone_journey_event(event);
            assert!(
                !flag_complete(session.app.world(), 2_110_004, flag),
                "{class:?} boundary {boundary}"
            );
        }
        deliver(&mut session, kind, stamp + 1);
        assert!(flag_complete(session.app.world(), 2_110_004, flag));
    }
}

#[test]
fn fiveclass_reward_items_keep_source_class_gender_levels_and_spell_book_shape() {
    let config: Config = serde_json::from_str(CONFIG_JSON).unwrap();
    let profile = mir2_game_data::platinum_fiveclass_profile();
    for (rewards, award_level, early_book) in config
        .quests
        .iter()
        .map(|quest| (&quest.rewards, quest.min_level, quest.id == 2_110_003))
        .chain(
            config
                .growth_rewards
                .iter()
                .map(|growth| (&growth.rewards, growth.level, false)),
        )
    {
        for (class, class_flag) in [(MirClass::Assassin, 8), (MirClass::Archer, 16)] {
            for item in rewards.class_items(class) {
                assert!(item.valid());
                for (gender, gender_flag) in [(MirGender::Male, 1), (MirGender::Female, 2)] {
                    let name = item.name_for(gender).unwrap();
                    let source = crystal_item_by_name(name).unwrap();
                    assert!(
                        profile.item_whitelist.contains(&name.to_string()),
                        "admitted source reward {name}"
                    );
                    assert_ne!(source.required_class & class_flag, 0, "{class:?} {name}");
                    assert_ne!(source.required_gender & gender_flag, 0, "{gender:?} {name}");
                    assert_eq!(
                        source.required_type, 0,
                        "reward has a real source level gate"
                    );
                    if !(early_book && source.item_type == 20) {
                        assert!(
                            i32::from(source.required_amount) <= award_level,
                            "{name} requires {} but reward is level {award_level}",
                            source.required_amount
                        );
                    } else {
                        assert_eq!(
                            source.required_amount, 7,
                            "starter books intentionally delivered before learn gate"
                        );
                    }
                    if source.item_type == 20 {
                        let magic = mir2_game_data::crystal_magic_for_book(&source).unwrap();
                        let rule = profile
                            .skills
                            .iter()
                            .find(|rule| rule.spell == magic.spell)
                            .unwrap();
                        assert_eq!(rule.class, class);
                        assert_eq!(rule.required_level, u16::from(source.required_amount));
                    }
                    assert!(reward_packet(item, gender).is_some());
                }
            }
        }
    }
    // The book is a level-11 source item; the growth bundle's level 15 weapon
    // must not accidentally rewrite that book to MagicInfo.Level1 (14).
    let double = crystal_item_by_name("DoubleShot").unwrap();
    assert_eq!(double.required_amount, 11);
    assert_eq!(
        mir2_game_data::crystal_magic_for_book(&double)
            .unwrap()
            .level1,
        14
    );
}

#[test]
fn fiveclass_actual_new_character_progress_and_skills_survive_save_and_new_session_login() {
    for class in [MirClass::Assassin, MirClass::Archer] {
        let (mut original, config, index) = session(class, MirGender::Female);
        let primary = if class == MirClass::Assassin {
            "DoubleSlash"
        } else {
            "DoubleShot"
        };
        learn(original.app.world_mut(), primary, 2);
        let skill_key = original.app.world().resource::<SkillResource>().skills[0]
            .key
            .clone();
        original
            .app
            .world_mut()
            .resource_mut::<SkillResource>()
            .skills[0]
            .experience = 321;
        original
            .app
            .world_mut()
            .resource_mut::<SkillResource>()
            .skills[0]
            .hotkey = 4;
        let (flag, stamp) = begin(&mut original, 2_110_011);
        let event = if class == MirClass::Assassin {
            Event::PhysicalDamage {
                technique: Technique::DoubleSlash,
            }
        } else {
            Event::DoubleShotDamage
        };
        deliver(&mut original, event, stamp + 1);
        assert!(!flag_complete(original.app.world(), 2_110_011, flag));
        let before = original
            .app
            .world()
            .resource::<QuestResource>()
            .quests
            .clone();
        original.force_authoritative_player_transform(
            Point { x: 151, y: 362 },
            mir2_protocol::MirDirection::Right,
        );
        original
            .save_active_character()
            .expect("real source class private save");
        drop(original);
        let mut restored = SimulationSession::new(config);
        assert!(restored.handle_packet(ClientPacket::Login { account_id: "demo".into(),
            password: "demo".into() }).iter().any(|p| matches!(p, ServerPacket::LoginSuccess { characters }
                if characters.iter().any(|character| character.index == index && character.class == class))));
        restored.handle_packet(ClientPacket::StartGame {
            character_index: index,
        });
        // Cadence is cached from deployment policy, not a personal save field.
        // Keep this test local rather than mutating process-wide environment.
        restored
            .app
            .world_mut()
            .resource_mut::<QuestResource>()
            .newcomer_v1_cadence = false;
        restored
            .app
            .world_mut()
            .resource_mut::<QuestResource>()
            .newcomer_v2_cadence = true;
        let selected = restored
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character
            .as_ref()
            .unwrap();
        assert_eq!(
            (selected.class, selected.gender),
            (class, MirGender::Female)
        );
        assert_eq!(
            restored
                .app
                .world()
                .resource::<QuestResource>()
                .quests
                .iter()
                .find(|q| q.quest_id == 2_110_011)
                .unwrap()
                .task_progress,
            before
                .iter()
                .find(|q| q.quest_id == 2_110_011)
                .unwrap()
                .task_progress
        );
        let skill = restored
            .app
            .world()
            .resource::<SkillResource>()
            .skills
            .iter()
            .find(|skill| skill.key == skill_key)
            .unwrap();
        assert_eq!((skill.level, skill.experience, skill.hotkey), (2, 321, 4));
        assert!(!flag_complete(restored.app.world(), 2_110_011, flag));
        restored.commit_zone_journey_reposition();
        deliver(&mut restored, event, stamp + 2);
        assert!(
            flag_complete(restored.app.world(), 2_110_011, flag),
            "saved damage/move/new damage resumes same exercise"
        );
    }
}

#[test]
fn fiveclass_focus_proc_probability_and_melee_requests_are_server_owned() {
    let (mut archer, _, _) = session(MirClass::Archer, MirGender::Male);
    assert_eq!(
        archer.zone_range_attack_profile().0,
        Spell::None,
        "unlearned level zero is not learned"
    );
    let object_id =
        crate::runtime::components::current_player_object_id(archer.app.world()).unwrap();
    for level in 0..=3 {
        learn(archer.app.world_mut(), "Focus", level);
        let mut seen = [false; 5];
        for tick in 0..100 {
            set_runtime_tick(archer.app.world_mut(), tick);
            let roll = crate::runtime::combat::crystal_accuracy_roll(
                tick,
                object_id,
                Spell::Focus as u32,
                5,
            );
            seen[roll as usize] = true;
            let (spell, source_level, _) = archer.zone_range_attack_profile();
            assert_eq!(
                spell == Spell::Focus,
                roll <= u64::from(level),
                "source 1/5..4/5 passive proc; no client spell controls this profile"
            );
            if spell == Spell::Focus {
                assert_eq!(source_level, level);
            }
        }
        assert!(seen.into_iter().all(|seen| seen));
    }
    let (mut assassin, _, _) = session(MirClass::Assassin, MirGender::Male);
    learn(assassin.app.world_mut(), "FatalSword", 0);
    learn(assassin.app.world_mut(), "DoubleSlash", 0);
    for requested in [
        Spell::FatalSword,
        Spell::StraightShot,
        Spell::DoubleShot,
        Spell::FireBall,
    ] {
        assert_eq!(
            assassin.zone_melee_attack_profile(requested).0,
            Spell::None,
            "raw client request cannot turn normal melee into {requested:?}"
        );
    }
    assert_eq!(
        assassin.zone_melee_attack_profile(Spell::DoubleSlash).0,
        Spell::None,
        "learned but unarmed DoubleSlash is not admitted"
    );
}

#[test]
fn fiveclass_legacy_classic_profile_still_rejects_extra_character_creation() {
    let config = SimulationConfig::default().with_platinum_176_profile();
    for class in [MirClass::Assassin, MirClass::Archer] {
        assert!(!config.class_is_allowed(class));
        let mut session = SimulationSession::new(config.clone());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        let packets = session.handle_packet(ClientPacket::NewCharacter {
            name: "MustReject".into(),
            gender: MirGender::Male,
            class,
        });
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewCharacterSuccess { .. })));
    }
    assert!([MirClass::Warrior, MirClass::Wizard, MirClass::Taoist]
        .into_iter()
        .all(|class| config.class_is_allowed(class)));
}
