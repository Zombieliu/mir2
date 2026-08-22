use std::collections::BTreeSet;

use mir2_protocol::{ClientPacket, MirClass, MirGender, MirGridType, Point, ServerPacket, Spell};
use mir2_simulation::{
    CharacterSaveRecord, ItemContainer, SimulationConfig, SimulationSession, Stage5HeroState,
    Stage5SystemsState,
};

const HERO_OBJECT_ID: u32 = 1001;
const BUFF_MAGIC_BOOSTER: u8 = 21;
const BUFF_MAGIC_SHIELD: u8 = 24;
const STAT_MIN_MC: u8 = 6;
const STAT_MAX_MC: u8 = 7;
const STAT_DAMAGE_REDUCTION_PERCENT: u8 = 124;
const STAT_MANA_PENALTY_PERCENT: u8 = 127;

fn start_with_hero(session: &mut SimulationSession, class: MirClass, behaviour: u8) {
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session.handle_packet(ClientPacket::NewHero {
        name: "Aide".to_string(),
        gender: MirGender::Female,
        class,
    });
    if behaviour != 0 {
        session.handle_packet(ClientPacket::SetHeroBehaviour { behaviour });
    }
}

fn session_with_saved_hero_and_player_hp(
    class: MirClass,
    level: u16,
    behaviour: u8,
    player_hp: Option<i32>,
) -> SimulationSession {
    session_with_saved_hero_and_player_hp_in_scene(class, level, behaviour, player_hp, false)
}

fn session_with_saved_hero_and_player_hp_in_scene(
    class: MirClass,
    level: u16,
    behaviour: u8,
    player_hp: Option<i32>,
    clear_starter_monsters: bool,
) -> SimulationSession {
    session_with_saved_hero_and_player_hp_in_scene_with_learned_magics(
        class,
        level,
        behaviour,
        player_hp,
        clear_starter_monsters,
        Vec::new(),
    )
}

fn session_with_saved_hero_and_player_hp_in_scene_with_learned_magics(
    class: MirClass,
    level: u16,
    behaviour: u8,
    player_hp: Option<i32>,
    clear_starter_monsters: bool,
    learned_magics: Vec<serde_json::Value>,
) -> SimulationSession {
    let mut config = SimulationConfig::default();
    if clear_starter_monsters {
        config.visible_monsters.clear();
    }
    let mut save = CharacterSaveRecord::new(config.default_character.clone());
    if let Some(player_hp) = player_hp {
        save.hp = player_hp;
    }
    let mut systems = Stage5SystemsState::default();
    systems.hero = Some(Stage5HeroState {
        name: "Aide".to_string(),
        level,
        class,
        gender: MirGender::Female,
        behaviour,
        experience: 0,
        spawned: true,
        auto_pot: true,
        auto_hp_percent: 0,
        auto_mp_percent: 0,
        hp_item_index: 0,
        mp_item_index: 0,
    });
    let mut systems_json =
        serde_json::to_value(&systems).expect("stage5 hero state should encode as json");
    systems_json["heroLearnedMagics"] = serde_json::Value::Array(learned_magics);
    save.stage5_systems_json =
        Some(serde_json::to_string(&systems_json).expect("stage5 hero state should encode"));
    {
        let mut store = config.account_store.lock().expect("account store lock");
        let account = store
            .accounts
            .get_mut("demo")
            .expect("default demo account");
        account.saves.insert(0, save);
    }

    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
}

fn session_with_saved_hero(class: MirClass, level: u16, behaviour: u8) -> SimulationSession {
    session_with_saved_hero_and_player_hp(class, level, behaviour, None)
}

fn session_with_saved_hero_empty_scene(
    class: MirClass,
    level: u16,
    behaviour: u8,
) -> SimulationSession {
    session_with_saved_hero_and_player_hp_in_scene(class, level, behaviour, None, true)
}

fn session_with_saved_hero_empty_scene_and_learned_magics(
    class: MirClass,
    level: u16,
    behaviour: u8,
    learned_magics: Vec<serde_json::Value>,
) -> SimulationSession {
    session_with_saved_hero_and_player_hp_in_scene_with_learned_magics(
        class,
        level,
        behaviour,
        None,
        true,
        learned_magics,
    )
}

fn learned_magic(spell: Spell, level: u8) -> serde_json::Value {
    learned_magic_with(spell, level, 1, 0)
}

fn learned_magic_with(spell: Spell, level: u8, key: u8, experience: u16) -> serde_json::Value {
    serde_json::json!({
        "spell": spell,
        "level": level,
        "key": key,
        "experience": experience,
    })
}

fn hero_inventory_crystal_book_item(template_name: &str, slot: u8, unique_id: u64) -> String {
    let template = mir2_game_data::crystal_item_by_name(template_name)
        .expect("Crystal book template should exist");
    serde_json::to_string(&serde_json::json!({
        "key": format!("crystal-item-{}", template.item_index),
        "name": template.name.clone(),
        "icon": template.image,
        "slot": slot,
        "unique_id": unique_id,
        "container": ItemContainer::Bag1,
        "quantity": 1,
        "description": template.tooltip.clone().unwrap_or_default(),
        "durability_current": if template.durability > 0 {
            Some(template.durability)
        } else {
            None
        },
        "durability_max": if template.durability > 0 {
            Some(template.durability)
        } else {
            None
        },
        "weight": template.weight,
        "equip_slot": serde_json::Value::Null,
        "grade": "none",
        "added_attack": 0,
        "added_defence": 0,
        "added_stats": [],
        "cursed": false,
        "socket_slots": template.slots,
        "gem_count": 0,
        "identified": serde_json::Value::Null,
        "soul_bound_id": serde_json::Value::Null,
        "sealed_expiry_time_binary_datetime": 0,
        "sealed_next_time_binary_datetime": 0,
        "rental_binding_flags": 0,
        "rental_owner_name": "",
        "rental_expiry_binary_datetime": 0,
        "rental_locked": false,
        "attack": 0,
        "defence": 0,
        "heal_hp": 0,
        "heal_mp": 0,
    }))
    .expect("hero inventory book item should encode")
}

fn spawn_bugbat(session: &mut SimulationSession) -> u32 {
    spawn_monster(session, "BugBat")
}

fn spawn_monster(session: &mut SimulationSession, name: &str) -> u32 {
    let packets = session.stage5_command("event.spawn", vec![name.to_string(), "1".to_string()]);
    packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == name => Some(info.object_id),
            _ => None,
        })
        .or_else(|| {
            session
                .world_snapshot()
                .entities
                .iter()
                .find(|entity| entity.name == name)
                .map(|entity| entity.object_id)
        })
        .expect("event.spawn should create a visible monster")
}

fn spawn_monsters(session: &mut SimulationSession, name: &str, count: u8) -> Vec<u32> {
    let before = session
        .world_snapshot()
        .entities
        .iter()
        .map(|entity| entity.object_id)
        .collect::<BTreeSet<_>>();
    session.stage5_command("event.spawn", vec![name.to_string(), count.to_string()]);
    let mut ids = session
        .world_snapshot()
        .entities
        .iter()
        .filter(|entity| entity.name == name && !before.contains(&entity.object_id))
        .map(|entity| entity.object_id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}

fn monster_hp(session: &SimulationSession, object_id: u32) -> i32 {
    session
        .world_snapshot()
        .entities
        .iter()
        .find(|entity| entity.object_id == object_id)
        .and_then(|entity| entity.hp)
        .expect("monster hp should be present")
}

fn move_player_before_spawn(session: &mut SimulationSession, dx: i32, dy: i32) {
    let snapshot = session.world_snapshot();
    let player_id = snapshot
        .player_object_id
        .expect("player object id should be visible");
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.object_id == player_id)
        .expect("player entity should be visible");
    let _ = session.move_to_with_mode(
        Point {
            x: player.x + dx,
            y: player.y + dy,
        },
        true,
    );
}

fn first_hero_hit_damage(with_weapon: bool) -> i32 {
    let mut session = SimulationSession::new(SimulationConfig::default());
    start_with_hero(&mut session, MirClass::Warrior, 0);
    if with_weapon {
        assert_eq!(
            session.handle_packet(ClientPacket::TransferHeroItem { from: 4, to: 3 }),
            vec![ServerPacket::TransferHeroItem {
                from: 4,
                to: 3,
                success: true,
            }]
        );
    }
    let monster_id = spawn_monster(&mut session, "Scarecrow");
    let start_hp = monster_hp(&session, monster_id);

    for _ in 0..12 {
        session.tick();
        let hp = monster_hp(&session, monster_id);
        if hp < start_hp {
            return start_hp - hp;
        }
    }

    panic!("Hero should damage the spawned Scarecrow");
}

fn first_saved_hero_hit_damage(class: MirClass, level: u16) -> i32 {
    let mut session = session_with_saved_hero(class, level, 0);
    let monster_id = spawn_monster(&mut session, "OmaFighter");
    let start_hp = monster_hp(&session, monster_id);

    for _ in 0..12 {
        session.tick();
        let hp = monster_hp(&session, monster_id);
        if hp < start_hp {
            return start_hp - hp;
        }
    }

    panic!("saved Hero should damage the spawned Scarecrow");
}

#[test]
fn hero_attack_behaviour_strikes_nearby_hostile_monster() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    start_with_hero(&mut session, MirClass::Warrior, 0);
    let monster_id = spawn_bugbat(&mut session);
    let start_hp = monster_hp(&session, monster_id);

    let mut saw_hero_attack = false;
    let mut saw_hero_struck = false;
    for _ in 0..8 {
        let packets = session.tick();
        saw_hero_attack |= packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectAttack { info } if info.object_id == 1001
            )
        });
        saw_hero_struck |= packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectStruck { info }
                    if info.object_id == monster_id && info.attacker_id == 1001
            )
        });
        if monster_hp(&session, monster_id) < start_hp {
            break;
        }
    }

    assert!(
        saw_hero_attack,
        "attack behaviour should emit Hero ObjectAttack"
    );
    assert!(
        saw_hero_struck,
        "scheduled Hero damage should emit ObjectStruck against the monster"
    );
    assert!(
        monster_hp(&session, monster_id) < start_hp,
        "Hero attack should reduce monster HP"
    );
}

#[test]
fn hero_follow_behaviour_does_not_auto_attack_nearby_monster() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    start_with_hero(&mut session, MirClass::Warrior, 2);
    let monster_id = spawn_bugbat(&mut session);
    let start_hp = monster_hp(&session, monster_id);

    let mut saw_hero_attack = false;
    for _ in 0..5 {
        let packets = session.tick();
        saw_hero_attack |= packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectAttack { info } if info.object_id == 1001
            )
        });
    }

    assert!(
        !saw_hero_attack,
        "follow behaviour should suppress Hero search/attack"
    );
    assert_eq!(monster_hp(&session, monster_id), start_hp);
}

#[test]
fn archer_hero_uses_crystal_range_attack_surface() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    start_with_hero(&mut session, MirClass::Archer, 0);
    let monster_id = spawn_bugbat(&mut session);

    let mut saw_range_attack = false;
    for _ in 0..8 {
        let packets = session.tick();
        saw_range_attack |= packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectRangeAttack { info }
                    if info.object_id == 1001 && info.target_id == monster_id
            )
        });
        if saw_range_attack {
            break;
        }
    }

    assert!(
        saw_range_attack,
        "Archer Hero should use ObjectRangeAttack when the target is within range"
    );
}

#[test]
fn archer_hero_uses_concentration_and_straight_shot_at_crystal_level_gates() {
    let mut session = session_with_saved_hero(MirClass::Archer, 23, 0);
    let monster_id = spawn_monster(&mut session, "OmaFighter");

    let mut mana_updates = 0;
    let mut saw_concentration = false;
    for _ in 0..8 {
        let packets = session.tick();
        mana_updates += packets
            .iter()
            .filter(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectMana { info } if info.object_id == 1001
                )
            })
            .count();
        saw_concentration |= packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::SetConcentration {
                    object_id: 1001,
                    enabled: true,
                    interrupted: false,
                }
            )
        });
        if let Some(info) = packets.iter().find_map(|packet| match packet {
            ServerPacket::ObjectRangeAttack { info }
                if info.object_id == 1001 && info.target_id == monster_id =>
            {
                Some(info)
            }
            _ => None,
        }) {
            assert_eq!(info.spell, Spell::StraightShot as u8);
            assert_eq!(info.level, 2);
            assert!(
                saw_concentration,
                "level-gated Archer Hero should enable Concentration before shooting"
            );
            assert!(
                mana_updates >= 2,
                "Concentration and StraightShot should both spend Hero MP"
            );
            return;
        }
    }

    panic!("level-gated Archer Hero should emit StraightShot range attack");
}

#[test]
fn archer_hero_concentration_is_gated_while_active() {
    let mut session = session_with_saved_hero(MirClass::Archer, 23, 0);
    spawn_monster(&mut session, "OmaFighter");

    let mut concentration_packets = 0;
    let mut mana_updates = 0;
    for _ in 0..12 {
        let packets = session.tick();
        concentration_packets += packets
            .iter()
            .filter(|packet| {
                matches!(
                    packet,
                    ServerPacket::SetConcentration {
                        object_id: 1001,
                        enabled: true,
                        interrupted: false,
                    }
                )
            })
            .count();
        mana_updates += packets
            .iter()
            .filter(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectMana { info } if info.object_id == 1001
                )
            })
            .count();
    }

    assert_eq!(
        concentration_packets, 1,
        "Concentration should not recast while the Hero AI state marks it active"
    );
    assert!(
        mana_updates >= 2,
        "Concentration and at least one StraightShot should spend Hero MP"
    );
}

#[test]
fn wizard_hero_casts_fireball_with_mana_cooldown_and_scheduled_damage() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 7, 0);
    let monster_id = spawn_monster(&mut session, "OmaFighter");
    let start_hp = monster_hp(&session, monster_id);

    let first_cast = (0..8)
        .find_map(|_| {
            let packets = session.tick();
            packets
                .iter()
                .any(|packet| {
                    matches!(
                        packet,
                        ServerPacket::ObjectMagic {
                            object_id: 1001,
                            spell: Spell::FireBall,
                            target_id,
                            cast: true,
                            level: 0,
                            ..
                        } if *target_id == monster_id
                    )
                })
                .then_some(packets)
        })
        .expect("level-gated Wizard Hero should cast FireBall");

    assert!(first_cast.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMana { info } if info.object_id == 1001 && info.percent < 100
    )));
    assert!(!first_cast.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 1001
    )));
    assert!(!first_cast.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectRangeAttack { info } if info.object_id == 1001
    )));

    let mut followup_packets = session.tick();
    assert!(
        !followup_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                object_id: 1001,
                spell: Spell::FireBall,
                ..
            }
        )),
        "Wizard Hero FireBall should respect its Crystal cooldown gate"
    );

    for _ in 0..6 {
        if monster_hp(&session, monster_id) < start_hp {
            break;
        }
        followup_packets.extend(session.tick());
    }

    assert!(followup_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info }
            if info.object_id == monster_id && info.attacker_id == 1001
    )));
    assert!(
        monster_hp(&session, monster_id) < start_hp,
        "Wizard Hero FireBall should apply delayed monster damage"
    );
}

#[test]
fn wizard_hero_fireball_respects_crystal_level_gate() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 6, 0);
    spawn_monster(&mut session, "OmaFighter");

    let mut packets = Vec::new();
    for _ in 0..4 {
        packets.extend(session.tick());
    }

    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            object_id: 1001,
            spell: Spell::FireBall | Spell::GreatFireBall | Spell::ThunderBolt,
            ..
        }
    )));
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMana { info } if info.object_id == 1001
    )));
}

#[test]
fn wizard_hero_uses_repulsion_before_fireball_for_adjacent_lower_level_target() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 12, 0);
    let monster_ids = spawn_monsters(&mut session, "BugBat", 3);
    assert!(
        !monster_ids.is_empty(),
        "event.spawn should create BugBat targets"
    );

    for _ in 0..8 {
        let packets = session.tick();
        if packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::Repulsion,
                    cast: true,
                    level: 0,
                    ..
                }
            )
        }) {
            assert!(!packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FireBall,
                    ..
                }
            )));
            assert!(packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectPushed { object_id, .. } if monster_ids.contains(object_id)
            )));
            return;
        }
    }

    panic!("Wizard Hero should use Repulsion before FireBall against adjacent lower-level targets");
}

#[test]
fn wizard_hero_uses_fire_bang_for_crowded_target_before_single_target_spells() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 22, 0);
    let monster_ids = spawn_monsters(&mut session, "OmaFighter", 3);
    assert!(
        monster_ids.len() >= 2,
        "event.spawn should create a crowded target group"
    );

    for _ in 0..8 {
        let packets = session.tick();
        if packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FireBang,
                    cast: true,
                    level: 0,
                    ..
                }
            )
        }) {
            assert!(!packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::ThunderBolt | Spell::GreatFireBall | Spell::FireBall,
                    ..
                }
            )));
            let mut struck = BTreeSet::new();
            for _ in 0..6 {
                for packet in session.tick() {
                    if let ServerPacket::ObjectStruck { info } = packet {
                        if info.attacker_id == HERO_OBJECT_ID
                            && monster_ids.contains(&info.object_id)
                        {
                            struck.insert(info.object_id);
                        }
                    }
                }
            }
            assert!(
                struck.len() >= 2,
                "FireBang should schedule target-centered area damage"
            );
            return;
        }
    }

    panic!("Wizard Hero should use FireBang before single-target spells for crowded targets");
}

#[test]
fn wizard_hero_uses_thunder_storm_for_self_surrounded_priority() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 30, 0);
    let monster_ids = spawn_monsters(&mut session, "Tongs", 3);
    assert!(
        monster_ids.len() >= 2,
        "event.spawn should create a caster-surround group"
    );

    for _ in 0..8 {
        let packets = session.tick();
        if packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::ThunderStorm,
                    cast: true,
                    level: 0,
                    ..
                }
            )
        }) {
            assert!(!packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FireBang
                        | Spell::ThunderBolt
                        | Spell::GreatFireBall
                        | Spell::FireBall,
                    ..
                }
            )));
            let mut struck = BTreeSet::new();
            for _ in 0..6 {
                for packet in session.tick() {
                    if let ServerPacket::ObjectStruck { info } = packet {
                        if info.attacker_id == HERO_OBJECT_ID
                            && monster_ids.contains(&info.object_id)
                        {
                            struck.insert(info.object_id);
                        }
                    }
                }
            }
            assert!(
                struck.len() >= 2,
                "ThunderStorm should schedule caster-centered area damage"
            );
            return;
        }
    }

    panic!("Wizard Hero should use ThunderStorm before lower-priority target/single spells");
}

#[test]
fn wizard_hero_uses_flame_disruptor_before_vampirism_and_fallback_spells() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 38, 0);

    for _ in 0..16 {
        let packets = session.tick();
        if let Some(cast_target_id) = packets.iter().find_map(|packet| match packet {
            ServerPacket::ObjectMagic {
                object_id: HERO_OBJECT_ID,
                spell: Spell::FlameDisruptor,
                target_id,
                cast: true,
                level: 0,
                ..
            } => Some(*target_id),
            _ => None,
        }) {
            assert!(packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMana { info } if info.object_id == HERO_OBJECT_ID
            )));
            assert!(!packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::Vampirism
                        | Spell::FrostCrunch
                        | Spell::ThunderBolt
                        | Spell::GreatFireBall
                        | Spell::FireBall,
                    ..
                }
            )));

            let mut saw_struck = false;
            for _ in 0..8 {
                let followup = session.tick();
                saw_struck |= followup.iter().any(|packet| {
                    matches!(
                        packet,
                        ServerPacket::ObjectStruck { info }
                            if info.object_id == cast_target_id
                                && info.attacker_id == HERO_OBJECT_ID
                    )
                });
                if saw_struck {
                    break;
                }
            }
            assert!(saw_struck, "FlameDisruptor should schedule target damage");
            return;
        }
    }

    panic!("Wizard Hero should use FlameDisruptor before later single-target spells");
}

#[test]
fn wizard_hero_learned_magic_inventory_skips_unlearned_higher_priority_spells() {
    let mut session = session_with_saved_hero_empty_scene_and_learned_magics(
        MirClass::Wizard,
        43,
        0,
        vec![
            learned_magic(Spell::Vampirism, 0),
            learned_magic(Spell::FireBall, 2),
        ],
    );
    move_player_before_spawn(&mut session, 3, 0);
    let monster_id = spawn_monster(&mut session, "OmaWarrior");

    for _ in 0..12 {
        let packets = session.tick();
        if packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::Vampirism,
                    target_id,
                    cast: true,
                    level: 0,
                    ..
                } if *target_id == monster_id
            )
        }) {
            assert!(!packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FlameDisruptor
                        | Spell::FrostCrunch
                        | Spell::ThunderBolt
                        | Spell::GreatFireBall
                        | Spell::FireBall,
                    ..
                }
            )));
            assert!(packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMana { info } if info.object_id == HERO_OBJECT_ID
            )));
            return;
        }
    }

    panic!("Wizard Hero should skip unlearned FlameDisruptor and cast learned Vampirism");
}

#[test]
fn wizard_hero_learned_magic_still_respects_crystal_level_gates() {
    let mut session = session_with_saved_hero_empty_scene_and_learned_magics(
        MirClass::Wizard,
        37,
        0,
        vec![
            learned_magic(Spell::FlameDisruptor, 0),
            learned_magic(Spell::Vampirism, 1),
        ],
    );
    move_player_before_spawn(&mut session, 3, 0);
    let monster_id = spawn_monster(&mut session, "OmaWarrior");
    let mut saw_flame_disruptor = false;

    for _ in 0..12 {
        let packets = session.tick();
        saw_flame_disruptor |= packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FlameDisruptor,
                    ..
                }
            )
        });
        if packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::Vampirism,
                    target_id,
                    cast: true,
                    level: 1,
                    ..
                } if *target_id == monster_id
            )
        }) {
            assert!(
                !saw_flame_disruptor,
                "learned FlameDisruptor should still stay below its Crystal level gate"
            );
            return;
        }
    }

    panic!("Wizard Hero should fall through to learned Vampirism below FlameDisruptor gate");
}

#[test]
fn hero_inventory_book_magic_key_save_loop_enables_hero_ai_spell() {
    let mut config = SimulationConfig::default();
    config.visible_monsters.clear();
    let mut save = CharacterSaveRecord::new(config.default_character.clone());
    let mut systems = Stage5SystemsState::default();
    systems.hero = Some(Stage5HeroState {
        name: "Aide".to_string(),
        level: 43,
        class: MirClass::Wizard,
        gender: MirGender::Female,
        behaviour: 0,
        experience: 0,
        spawned: true,
        auto_pot: true,
        auto_hp_percent: 0,
        auto_mp_percent: 0,
        hp_item_index: 0,
        mp_item_index: 0,
    });
    save.stage5_systems_json =
        Some(serde_json::to_string(&systems).expect("stage5 hero state should encode"));
    save.hero_inventory_items_json = vec![hero_inventory_crystal_book_item("FireBall", 3, 9_001)];
    {
        let mut store = config.account_store.lock().expect("account store lock");
        let account = store
            .accounts
            .get_mut("demo")
            .expect("default demo account");
        account.saves.insert(0, save);
    }
    let saved_config = config.clone();
    let mut session = SimulationSession::new(config);
    let login_packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(login_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });

    let learn_packets = session.handle_packet(ClientPacket::UseItem {
        unique_id: 9_001,
        grid: MirGridType::HeroInventory,
    });
    assert_eq!(
        learn_packets.first(),
        Some(&ServerPacket::UseItem {
            unique_id: 9_001,
            success: true,
            grid: MirGridType::HeroInventory,
        })
    );
    assert!(learn_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::NewMagic {
            magic,
            hero: true,
        } if magic.spell == Spell::FireBall
            && magic.key == 0
            && magic.level == 0
            && magic.experience == 0
    )));

    move_player_before_spawn(&mut session, 3, 0);
    let monster_id = spawn_monster(&mut session, "OmaWarrior");
    for _ in 0..4 {
        let packets = session.tick();
        assert!(
            !packets.iter().any(|packet| {
                matches!(
                    packet,
                    ServerPacket::ObjectMagic {
                        object_id: HERO_OBJECT_ID,
                        spell: Spell::FireBall,
                        ..
                    }
                )
            }),
            "freshly learned Hero FireBall should stay gated until MagicKey assigns a key"
        );
    }

    assert!(session
        .handle_packet(ClientPacket::MagicKey {
            spell: Spell::FireBall,
            key: 17,
            old_key: 0,
        })
        .is_empty());
    session.save_active_character();
    let saved_systems_json = {
        let store = saved_config
            .account_store
            .lock()
            .expect("account store lock");
        store
            .accounts
            .get("demo")
            .and_then(|account| account.saves.get(&0))
            .and_then(|save| save.stage5_systems_json.clone())
            .expect("saved stage5 systems")
    };
    let saved_systems: serde_json::Value =
        serde_json::from_str(&saved_systems_json).expect("saved systems json should decode");
    let saved_magics = saved_systems["heroLearnedMagics"]
        .as_array()
        .expect("hero learned magics should be saved");
    assert_eq!(saved_magics.len(), 1);
    assert!(saved_magics.iter().any(|magic| {
        magic["spell"] == serde_json::to_value(Spell::FireBall).expect("spell should encode")
            && magic["level"].as_u64() == Some(0)
            && magic["key"].as_u64() == Some(17)
    }));

    for _ in 0..12 {
        let packets = session.tick();
        if packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FireBall,
                    target_id,
                    cast: true,
                    level: 0,
                    ..
                } if *target_id == monster_id
            )
        }) {
            return;
        }
    }

    panic!("Hero AI should cast the book-learned, MagicKey-enabled FireBall");
}

#[test]
fn hero_learned_magic_progression_increases_experience_after_ai_cast() {
    let mut session = session_with_saved_hero_empty_scene_and_learned_magics(
        MirClass::Wizard,
        43,
        2,
        vec![learned_magic(Spell::FireBall, 0)],
    );
    move_player_before_spawn(&mut session, 3, 0);
    let _monster_id = spawn_monster(&mut session, "Trainer");
    session.handle_packet(ClientPacket::SetHeroBehaviour { behaviour: 0 });
    let starting_experience = session
        .world_snapshot()
        .stage5_systems
        .hero_learned_magics
        .first()
        .map(|magic| magic.experience)
        .unwrap_or_default();

    for _ in 0..12 {
        let packets = session.tick();
        if !packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FireBall,
                    cast: true,
                    level: 0,
                    ..
                }
            )
        }) {
            continue;
        }

        let progress = packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::MagicLeveled {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FireBall,
                    level: 0,
                    experience,
                } => Some(*experience),
                _ => None,
            })
            .expect("Hero FireBall cast should report learned-magic experience");
        assert!(
            progress > starting_experience,
            "Crystal LevelMagic should increase practice experience from {starting_experience} to {progress}"
        );
        let snapshot = session.world_snapshot();
        let learned = snapshot
            .stage5_systems
            .hero_learned_magics
            .first()
            .expect("Hero learned magic should remain in Stage 5 state");
        assert_eq!(snapshot.stage5_systems.hero_learned_magics.len(), 1);
        assert_eq!(learned.spell, Spell::FireBall);
        assert_eq!(learned.level, 0);
        assert_eq!(learned.key, 1);
        assert_eq!(learned.experience, progress);
        return;
    }

    panic!("Hero AI should cast learned FireBall and advance practice experience");
}

#[test]
fn hero_learned_magic_progression_levels_up_and_next_cast_uses_new_level() {
    let fireball = mir2_game_data::crystal_magic_by_spell("FireBall")
        .expect("Crystal FireBall magic should exist");
    let mut session = session_with_saved_hero_empty_scene_and_learned_magics(
        MirClass::Wizard,
        43,
        2,
        vec![learned_magic_with(
            Spell::FireBall,
            0,
            1,
            fireball.need1.saturating_sub(1),
        )],
    );
    move_player_before_spawn(&mut session, 3, 0);
    let _monster_id = spawn_monster(&mut session, "Trainer");
    session.handle_packet(ClientPacket::SetHeroBehaviour { behaviour: 0 });
    let mut leveled = false;
    let mut saw_delay = false;

    for _ in 0..40 {
        let packets = session.tick();
        saw_delay |= packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::MagicDelay {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FireBall,
                    delay
                } if *delay > 0
            )
        });
        if !leveled {
            if packets.iter().any(|packet| {
                matches!(
                    packet,
                    ServerPacket::MagicLeveled {
                        object_id: HERO_OBJECT_ID,
                        spell: Spell::FireBall,
                        level: 1,
                        ..
                    }
                )
            }) {
                assert!(saw_delay);
                assert_eq!(
                    session
                        .world_snapshot()
                        .stage5_systems
                        .hero_learned_magics
                        .first()
                        .map(|magic| magic.level),
                    Some(1)
                );
                leveled = true;
            }
            continue;
        }

        if packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FireBall,
                    cast: true,
                    level: 1,
                    ..
                }
            )
        }) {
            return;
        }
    }

    panic!("Hero AI should reuse the progressed learned FireBall level on a later cast");
}

#[test]
fn wizard_hero_flame_disruptor_level_gate_falls_to_vampirism_with_heal() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 37, 0);
    let mut saw_flame_disruptor = false;

    for _ in 0..16 {
        let packets = session.tick();
        saw_flame_disruptor |= packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FlameDisruptor,
                    ..
                }
            )
        });
        if let Some(cast_target_id) = packets.iter().find_map(|packet| match packet {
            ServerPacket::ObjectMagic {
                object_id: HERO_OBJECT_ID,
                spell: Spell::Vampirism,
                target_id,
                cast: true,
                level: 1,
                ..
            } => Some(*target_id),
            _ => None,
        }) {
            assert!(
                !saw_flame_disruptor,
                "below the FlameDisruptor gate, Crystal priority should continue to Vampirism"
            );
            assert!(packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMana { info } if info.object_id == HERO_OBJECT_ID
            )));

            let mut saw_hero_heal = false;
            let mut saw_struck = false;
            for _ in 0..8 {
                let followup = session.tick();
                saw_hero_heal |= followup.iter().any(|packet| {
                    matches!(
                        packet,
                        ServerPacket::ObjectHealth { info }
                            if info.object_id == HERO_OBJECT_ID
                    )
                });
                saw_struck |= followup.iter().any(|packet| {
                    matches!(
                        packet,
                        ServerPacket::ObjectStruck { info }
                            if info.object_id == cast_target_id
                                && info.attacker_id == HERO_OBJECT_ID
                    )
                });
                if saw_hero_heal && saw_struck {
                    break;
                }
            }
            assert!(saw_struck, "Vampirism should schedule target damage");
            assert!(
                saw_hero_heal,
                "Vampirism should schedule a Hero ObjectHealth heal surface"
            );
            return;
        }
    }

    panic!("Wizard Hero should fall through to Vampirism when FlameDisruptor is below gate");
}

#[test]
fn wizard_hero_frost_crunch_roots_and_buffs_before_classic_fallbacks() {
    let mut session = session_with_saved_hero_empty_scene(MirClass::Wizard, 28, 0);
    move_player_before_spawn(&mut session, 3, 0);
    let monster_id = spawn_monster(&mut session, "OmaWarrior");
    let start_hp = monster_hp(&session, monster_id);

    for _ in 0..10 {
        let packets = session.tick();
        if packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::FrostCrunch,
                    target_id,
                    cast: true,
                    level: 0,
                    ..
                } if *target_id == monster_id
            )
        }) {
            assert!(packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMana { info } if info.object_id == HERO_OBJECT_ID
            )));
            assert!(!packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::ThunderBolt | Spell::GreatFireBall | Spell::FireBall,
                    ..
                }
            )));

            let mut saw_freeze_buff = false;
            let mut saw_struck = false;
            for _ in 0..8 {
                let followup = session.tick();
                saw_freeze_buff |= followup.iter().any(|packet| {
                    matches!(
                        packet,
                        ServerPacket::AddBuff { buff }
                            if buff.object_id == monster_id && buff.buff_type == 201
                    )
                });
                saw_struck |= followup.iter().any(|packet| {
                    matches!(
                        packet,
                        ServerPacket::ObjectStruck { info }
                            if info.object_id == monster_id
                                && info.attacker_id == HERO_OBJECT_ID
                    )
                });
                if saw_freeze_buff && monster_hp(&session, monster_id) < start_hp {
                    break;
                }
            }
            assert!(saw_freeze_buff, "FrostCrunch should publish a freeze buff");
            assert!(saw_struck, "FrostCrunch should schedule target damage");
            assert!(
                monster_hp(&session, monster_id) < start_hp,
                "FrostCrunch should damage the target"
            );
            return;
        }
    }

    panic!("Wizard Hero should cast FrostCrunch before classic single-target fallbacks");
}

#[test]
fn wizard_hero_casts_magic_shield_support_before_attack() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 31, 0);
    let monster_id = spawn_monster(&mut session, "OmaFighter");

    let support_packets = (0..8)
        .find_map(|_| {
            let packets = session.tick();
            packets
                .iter()
                .any(|packet| {
                    matches!(
                        packet,
                        ServerPacket::ObjectMagic {
                            object_id: HERO_OBJECT_ID,
                            spell: Spell::MagicShield,
                            target_id: HERO_OBJECT_ID,
                            cast: true,
                            level: 0,
                            ..
                        }
                    )
                })
                .then_some(packets)
        })
        .expect("level-gated Wizard Hero should cast MagicShield before attacking");

    assert!(support_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMana { info } if info.object_id == HERO_OBJECT_ID
    )));
    let shield = support_packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::AddBuff { buff }
                if buff.object_id == HERO_OBJECT_ID && buff.buff_type == BUFF_MAGIC_SHIELD =>
            {
                Some(buff)
            }
            _ => None,
        })
        .expect("MagicShield should publish a Hero AddBuff");
    assert!(!shield.visible);
    assert!(shield.expire_time >= 15_000);
    assert!(shield
        .stats
        .iter()
        .any(|stat| { stat.stat == STAT_DAMAGE_REDUCTION_PERCENT && stat.value == 20 }));
    assert!(!support_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == HERO_OBJECT_ID
    )));
    assert!(!support_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectRangeAttack { info } if info.object_id == HERO_OBJECT_ID
    )));
    assert!(!support_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            object_id: HERO_OBJECT_ID,
            spell: Spell::FireBall | Spell::GreatFireBall | Spell::ThunderBolt,
            target_id,
            ..
        } if *target_id == monster_id
    )));
}

#[test]
fn wizard_hero_magic_shield_respects_crystal_level_gate() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 30, 0);
    spawn_monster(&mut session, "OmaFighter");

    let mut packets = Vec::new();
    for _ in 0..8 {
        packets.extend(session.tick());
    }

    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            object_id: HERO_OBJECT_ID,
            spell: Spell::MagicShield | Spell::MagicBooster,
            ..
        }
    )));
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::AddBuff { buff }
            if buff.object_id == HERO_OBJECT_ID
                && matches!(buff.buff_type, BUFF_MAGIC_SHIELD | BUFF_MAGIC_BOOSTER)
    )));
}

#[test]
fn wizard_hero_casts_magic_booster_after_magic_shield_at_crystal_gate() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 47, 0);
    spawn_monster(&mut session, "Trainer");

    let mut saw_shield = false;
    for _ in 0..12 {
        let packets = session.tick();
        if packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::MagicShield,
                    target_id: HERO_OBJECT_ID,
                    cast: true,
                    level: 2,
                    ..
                }
            )
        }) {
            saw_shield = true;
        }
        if packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::MagicBooster,
                    target_id: HERO_OBJECT_ID,
                    cast: true,
                    level: 0,
                    ..
                }
            )
        }) {
            assert!(
                saw_shield,
                "WizardHero.ProcessFriend tries MagicShield before MagicBooster"
            );
            assert!(packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMana { info }
                    if info.object_id == HERO_OBJECT_ID && info.percent < 100
            )));
            let booster = packets
                .iter()
                .find_map(|packet| match packet {
                    ServerPacket::AddBuff { buff }
                        if buff.object_id == HERO_OBJECT_ID
                            && buff.buff_type == BUFF_MAGIC_BOOSTER =>
                    {
                        Some(buff)
                    }
                    _ => None,
                })
                .expect("MagicBooster should publish a visible Hero AddBuff");
            assert!(booster.visible);
            assert_eq!(booster.expire_time, 60_000);
            assert!(booster
                .stats
                .iter()
                .any(|stat| stat.stat == STAT_MIN_MC && stat.value == 6));
            assert!(booster
                .stats
                .iter()
                .any(|stat| stat.stat == STAT_MAX_MC && stat.value == 6));
            assert!(booster
                .stats
                .iter()
                .any(|stat| { stat.stat == STAT_MANA_PENALTY_PERCENT && stat.value == 6 }));
            return;
        }
    }

    panic!("level-gated Wizard Hero should cast MagicBooster after MagicShield");
}

#[test]
fn wizard_hero_support_does_not_recast_when_mana_is_missing() {
    let mut session = session_with_saved_hero(MirClass::Wizard, 47, 0);
    let monster_id = spawn_monster(&mut session, "Trainer");

    let mut shield_casts = 0;
    let mut booster_casts = 0;
    let mut lowest_mana_percent = 100_u8;
    for _ in 0..25 {
        for packet in session.tick() {
            match packet {
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::MagicShield,
                    ..
                } => shield_casts += 1,
                ServerPacket::ObjectMagic {
                    object_id: HERO_OBJECT_ID,
                    spell: Spell::MagicBooster,
                    ..
                } => booster_casts += 1,
                ServerPacket::ObjectMana { info } if info.object_id == HERO_OBJECT_ID => {
                    lowest_mana_percent = lowest_mana_percent.min(info.percent);
                }
                _ => {}
            }
        }
    }

    assert_eq!(shield_casts, 1);
    assert_eq!(booster_casts, 1);
    assert!(
        lowest_mana_percent < 20,
        "MagicShield plus MagicBooster should leave the Hero below the next Shield cost"
    );
    assert!(
        monster_hp(&session, monster_id) > 0,
        "the durable target should still exist while the missing-mana recast is skipped"
    );
}

#[test]
fn taoist_hero_prioritizes_owner_healing_before_melee_attack() {
    let mut session = session_with_saved_hero_and_player_hp(MirClass::Taoist, 7, 0, Some(10));
    let player_object_id = session
        .world_snapshot()
        .player_object_id
        .expect("player object id should be visible");
    let before_hp = session
        .world_snapshot()
        .player_hp
        .expect("saved low player HP should load");
    spawn_monster(&mut session, "Scarecrow");

    let first_tick = session.tick();

    assert!(first_tick.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMana { info } if info.object_id == 1001 && info.percent < 100
    )));
    assert!(first_tick.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            object_id: 1001,
            spell: Spell::Healing,
            target_id,
            cast: true,
            level: 0,
            ..
        } if *target_id == player_object_id
    )));
    assert!(first_tick.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == player_object_id && info.percent > 0
    )));
    assert!(!first_tick.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 1001
    )));
    assert!(!first_tick.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectRangeAttack { info } if info.object_id == 1001
    )));
    assert!(
        session
            .world_snapshot()
            .player_hp
            .expect("player HP should remain visible")
            > before_hp,
        "Taoist Hero Healing should restore owner HP"
    );

    let second_tick = session.tick();
    assert!(
        !second_tick.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                object_id: 1001,
                spell: Spell::Healing,
                ..
            }
        )),
        "Hero Healing should respect its Crystal cooldown gate"
    );
}

#[test]
fn taoist_hero_healing_respects_crystal_level_gate() {
    let mut session = session_with_saved_hero_and_player_hp(MirClass::Taoist, 6, 0, Some(10));
    let player_object_id = session
        .world_snapshot()
        .player_object_id
        .expect("player object id should be visible");

    let mut packets = Vec::new();
    for _ in 0..3 {
        packets.extend(session.tick());
    }

    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            object_id: 1001,
            spell: Spell::Healing,
            ..
        }
    )));
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectMana { info } if info.object_id == 1001
    )));
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == player_object_id
    )));
    assert_eq!(
        session.world_snapshot().player_hp,
        Some(10),
        "below-gate Taoist Hero should not heal the owner"
    );
}

#[test]
fn warrior_hero_uses_slaying_surface_at_crystal_level_gate() {
    let mut session = session_with_saved_hero(MirClass::Warrior, 15, 0);
    let _monster_id = spawn_bugbat(&mut session);

    for _ in 0..8 {
        let packets = session.tick();
        if let Some(info) = packets.iter().find_map(|packet| match packet {
            ServerPacket::ObjectAttack { info } if info.object_id == 1001 => Some(info),
            _ => None,
        }) {
            assert_eq!(info.spell, Spell::Slaying as u8);
            assert_eq!(info.level, 0);
            return;
        }
    }

    panic!("level-gated Warrior Hero should emit Slaying on melee attack");
}

#[test]
fn high_level_warrior_hero_uses_flaming_sword_burst_damage() {
    let mut session = session_with_saved_hero(MirClass::Warrior, 35, 0);
    let _monster_id = spawn_bugbat(&mut session);

    for _ in 0..8 {
        let packets = session.tick();
        if let Some(info) = packets.iter().find_map(|packet| match packet {
            ServerPacket::ObjectAttack { info } if info.object_id == 1001 => Some(info),
            _ => None,
        }) {
            assert_eq!(info.spell, Spell::FlamingSword as u8);
            assert_eq!(info.level, 0);
            assert!(
                first_saved_hero_hit_damage(MirClass::Warrior, 35)
                    > first_saved_hero_hit_damage(MirClass::Warrior, 34),
                "FlamingSword gate should increase the first scheduled Hero hit"
            );
            return;
        }
    }

    panic!("high-level Warrior Hero should emit FlamingSword on melee attack");
}

#[test]
fn hero_carried_weapon_stats_increase_attack_damage() {
    let unarmed = first_hero_hit_damage(false);
    let armed = first_hero_hit_damage(true);

    assert_eq!(armed, unarmed + 5);
}

#[test]
fn hero_information_projects_carried_equipment_stats() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    start_with_hero(&mut session, MirClass::Warrior, 0);
    session.stage5_command("qa.giveItem", vec!["crystal-item-18".to_string()]);
    let necklace_slot = session
        .world_snapshot()
        .inventory_items
        .iter()
        .find(|item| item.key == "crystal-item-18")
        .map(|item| item.slot)
        .expect("QA item should be in the player bag");
    assert_eq!(
        session.handle_packet(ClientPacket::TransferHeroItem {
            from: i32::from(necklace_slot),
            to: 6,
        }),
        vec![ServerPacket::TransferHeroItem {
            from: i32::from(necklace_slot),
            to: 6,
            success: true,
        }]
    );

    let packets = session.handle_packet(ClientPacket::ChangeHero { list_index: 0 });
    let info = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::HeroInformation { info } => Some(info),
            _ => None,
        })
        .expect("ChangeHero should refresh HeroInformation");
    let equipment = info
        .equipment
        .as_ref()
        .expect("HeroInformation should include equipment slots");
    let necklace = equipment[4]
        .as_ref()
        .expect("carried Crystal necklace should project into Hero neck slot");

    assert_eq!(necklace.item_index, 18);
    assert_eq!(info.hp, 91);
    assert_eq!(info.mp, 59);
}
