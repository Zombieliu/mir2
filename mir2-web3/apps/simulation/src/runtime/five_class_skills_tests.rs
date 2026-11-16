use super::*;
use crate::config::ItemGrade;
use crate::runtime::components::{Facing, MonsterCombatStats, ObjectId, WorldObject};
use crate::runtime::equipment::EquipmentState;
use crate::runtime::items::{crystal_item_key_for_template, item_icon_for_key};
use crate::SimulationConfig;
use mir2_protocol::{ClientPacket, MirGender};

fn assassin_session() -> (SimulationSession, Entity, Point) {
    let mut session = SimulationSession::new(SimulationConfig::default());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let world = session.app.world_mut();
    let player = player_entity(world).expect("started player");
    let selected = world.resource_mut::<SessionResource>();
    let mut selected = selected;
    let character = selected.selected_character.as_mut().expect("character");
    character.class = MirClass::Assassin;
    character.gender = MirGender::Male;
    character.level = 50;
    drop(selected);
    {
        let mut body = world.entity_mut(player);
        let mut body = body.get_mut::<CharacterBody>().expect("body");
        body.class = MirClass::Assassin;
        body.level = 50;
    }
    let origin = Point { x: 333, y: 300 };
    world
        .entity_mut(player)
        .insert((Position(origin.clone()), Facing(MirDirection::Right)));
    let vitals = PlayerVitals {
        hp: 100,
        max_hp: 100,
        mp: 500,
        max_mp: 500,
    };
    world.entity_mut(player).insert(vitals);
    world
        .resource_mut::<PlayerRuntimeResource>()
        .player_position = origin.clone();
    world.resource_mut::<PlayerRuntimeResource>().player_vitals = vitals;
    (session, player, origin)
}

fn equip_powder(world: &mut World, name: &str) {
    let template = mir2_game_data::crystal_item_by_name(name).expect("source powder");
    let key = crystal_item_key_for_template(&template);
    world
        .resource_mut::<InventoryResource>()
        .equipment_items
        .push(EquipmentState {
            key: key.clone(),
            slot: EquipmentSlot::BraceletRight,
            quantity: 2,
            awake_type: 0,
            awake_values: Vec::new(),
            name: template.name.clone(),
            icon: item_icon_for_key(&key),
            shape: u16::try_from(template.shape).ok(),
            description: template.tooltip.clone().unwrap_or_default(),
            durability_current: template.durability.max(1),
            durability_max: template.durability.max(1),
            grade: ItemGrade::None,
            added_attack: 0,
            added_defence: 0,
            added_luck: 0,
            added_stats: Vec::new(),
            socketed: Vec::new(),
            user_item_metadata: None,
            user_item_unique_id: None,
            cursed: false,
            socket_slots: template.slots,
            gem_count: 0,
            identified: None,
            soul_bound_id: None,
            sealed_expiry_time_binary_datetime: 0,
            sealed_next_time_binary_datetime: 0,
            rental_binding_flags: 0,
            rental_owner_name: String::new(),
            rental_expiry_binary_datetime: 0,
            rental_locked: false,
            attack: 0,
            defence: 0,
        });
}

fn target(world: &mut World, id: u32, position: Point, name: &str) -> Entity {
    world
        .spawn((
            WorldObject,
            ObjectId(id),
            DisplayName::literal(name),
            Position(position.clone()),
            Monster,
            MonsterVitals {
                hp: 500,
                max_hp: 500,
            },
            MonsterCombatStats::default(),
            MonsterAgent {
                image: 1,
                dead: false,
                patrol_origin: position,
                ai: 0,
                disposition: crate::config::WorldEntityDisposition::Hostile,
                hostile_to_player: true,
                tracking_player: false,
                view_range: 0,
                can_wander: false,
                move_interval_ticks: u64::MAX,
                attack_interval_ticks: u64::MAX,
                next_move_tick: u64::MAX,
                next_attack_tick: u64::MAX,
                route: Vec::new(),
                route_index: 0,
                route_waiting: false,
                next_route_tick: u64::MAX,
            },
        ))
        .id()
}

#[test]
fn assassin_source_active_direction_and_locked_monster_metadata() {
    assert_eq!(
        crystal_spell_cast_kind(Spell::PoisonSword),
        SkillCastKind::Direction
    );
    assert_eq!(crystal_spell_cast_kind(Spell::Trap), SkillCastKind::Target);
    assert_eq!(
        crystal_spell_cast_kind(Spell::FatalSword),
        SkillCastKind::Passive
    );
    assert_eq!(
        crystal_spell_cast_kind(Spell::Haste),
        SkillCastKind::SelfOnly
    );
}

#[test]
fn five_class_source_traps_and_special_arrows_do_not_require_taoist_materials() {
    let (session, _, _) = assassin_session();
    for spell in [
        Spell::Trap,
        Spell::DelayedExplosion,
        Spell::ExplosiveTrap,
        Spell::BindingShot,
        Spell::PoisonShot,
        Spell::CrippleShot,
    ] {
        assert!(
            crystal_spell_required_items_available(session.app.world(), spell),
            "{spell:?}"
        );
        assert_eq!(
            zone_magic_inventory_components(session.app.world(), spell),
            Some(Vec::new()),
            "{spell:?}"
        );
    }
    assert!(!crystal_spell_required_items_available(
        session.app.world(),
        Spell::PoisonSword
    ));
    // Old Taoist casting still requires its actual source amulet.
    assert!(!crystal_spell_required_items_available(
        session.app.world(),
        Spell::SoulFireBall
    ));
}

#[test]
fn poison_sword_missing_material_does_not_spend_mana_or_enter_cooldown() {
    let (mut session, player, _) = assassin_session();
    let skill = crystal_skill_state("PoisonSword", 2).expect("source skill");
    let key = skill.key.clone();
    session
        .app
        .world_mut()
        .resource_mut::<SkillResource>()
        .skills
        .push(skill);
    let packets = cast_skill(session.app.world_mut(), &key);
    assert!(packets.is_empty());
    assert_eq!(
        entity_player_vitals(session.app.world(), player)
            .unwrap()
            .mp,
        500
    );
    assert_eq!(
        session
            .app
            .world()
            .resource::<SkillResource>()
            .skills
            .iter()
            .find(|s| s.key == key)
            .unwrap()
            .cooldown_ends_at,
        0
    );
}

#[test]
fn poison_sword_accepts_each_source_powder_without_a_target_and_consumes_once() {
    for powder in ["GreenPoison", "RedPoison"] {
        let (mut session, _, _) = assassin_session();
        equip_powder(session.app.world_mut(), powder);
        let skill = crystal_skill_state("PoisonSword", 2).expect("source skill");
        let key = skill.key.clone();
        session
            .app
            .world_mut()
            .resource_mut::<SkillResource>()
            .skills
            .push(skill);
        assert!(skill_cast_preflight_for_key(
            session.app.world(),
            &key,
            None
        ));
        let packets = cast_skill(session.app.world_mut(), &key);
        assert!(
            packets.iter().any(|p| matches!(
                p,
                ServerPacket::Magic {
                    spell: Spell::PoisonSword,
                    cast: true,
                    target_id: 0,
                    ..
                }
            )),
            "{powder}"
        );
        assert_eq!(
            session
                .app
                .world()
                .resource::<InventoryResource>()
                .equipment_items
                .iter()
                .find(|i| i.name == powder)
                .unwrap()
                .quantity,
            1,
            "{powder}"
        );
    }
}

#[test]
fn trap_requires_a_live_hostile_monster_before_mana_or_cooldown_mutation() {
    let (mut session, player, origin) = assassin_session();
    let skill = crystal_skill_state("Trap", 2).expect("source skill");
    let key = skill.key.clone();
    session
        .app
        .world_mut()
        .resource_mut::<SkillResource>()
        .skills
        .push(skill);
    // The source Trap has no material requirement; this tests the target gate.
    assert!(!skill_cast_preflight_for_key(
        session.app.world(),
        &key,
        None
    ));
    let position = Point {
        x: origin.x + 1,
        y: origin.y,
    };
    let entity = target(session.app.world_mut(), 98_775, position.clone(), "Yob");
    let context = SkillCastContext {
        direction: MirDirection::Right,
        target_id: 98_775,
        target: position,
    };
    assert!(skill_cast_preflight_for_key(
        session.app.world(),
        &key,
        Some(&context)
    ));
    session
        .app
        .world_mut()
        .entity_mut(entity)
        .get_mut::<MonsterVitals>()
        .unwrap()
        .hp = 0;
    assert!(!skill_cast_preflight_for_key(
        session.app.world(),
        &key,
        Some(&context)
    ));
    assert!(cast_skill_with_context(session.app.world_mut(), &key, Some(context)).is_empty());
    assert_eq!(
        entity_player_vitals(session.app.world(), player)
            .unwrap()
            .mp,
        500
    );
    assert_eq!(
        session
            .app
            .world()
            .resource::<SkillResource>()
            .skills
            .iter()
            .find(|s| s.key == key)
            .unwrap()
            .cooldown_ends_at,
        0
    );
}

#[test]
fn self_buff_fallback_preserves_source_duration_stats_and_cooldown() {
    for (name, level, expected_ms, stat, value) in [
        ("Haste", 2, 55_000, CRYSTAL_STAT_ATTACK_SPEED, 6),
        ("LightBody", 2, 90_000, CRYSTAL_STAT_AGILITY, 6),
    ] {
        let (mut session, player, _) = assassin_session();
        let skill = crystal_skill_state(name, level).expect("source skill");
        let key = skill.key.clone();
        session
            .app
            .world_mut()
            .resource_mut::<SkillResource>()
            .skills
            .push(skill);
        let packets = cast_skill(session.app.world_mut(), &key);
        let buff = packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::AddBuff { buff } => Some(buff),
                _ => None,
            })
            .expect("real client buff");
        assert_eq!(buff.expire_time, expected_ms, "{name}");
        assert!(
            buff.stats
                .iter()
                .any(|s| s.stat == stat && s.value == value),
            "{name}"
        );
        let buff_key = if name == "LightBody" {
            "light-body"
        } else {
            "haste"
        };
        assert!(session
            .app
            .world()
            .resource::<BuffResource>()
            .buffs
            .iter()
            .any(|buff| buff.key == buff_key
                && buff
                    .stats
                    .iter()
                    .any(|s| s.stat == stat && s.value == value)));
        let mp = entity_player_vitals(session.app.world(), player)
            .unwrap()
            .mp;
        assert!(mp < 500);
        assert!(cast_skill(session.app.world_mut(), &key).is_empty());
        assert_eq!(
            entity_player_vitals(session.app.world(), player)
                .unwrap()
                .mp,
            mp
        );
    }
}

#[test]
fn moonlight_duration_uses_actual_defence_range_and_luck() {
    let (mut session, _, _) = assassin_session();
    let skill = crystal_skill_state("MoonLight", 2).expect("source skill");
    let magic = crystal_magic_by_spell("MoonLight").expect("source metadata");
    let stats = crate::runtime::stats::player_stats(session.app.world());
    let mut durations = std::collections::BTreeSet::new();
    for tick in 0..64 {
        // AddBuff encodes remaining time from the same authoritative clock
        // that admitted the cast, rather than from the fixture's login tick.
        crate::runtime::resources::set_runtime_tick(session.app.world_mut(), tick);
        let packets =
            apply_crystal_self_buff_spell(session.app.world_mut(), &skill, &magic, tick).unwrap();
        let duration = packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::AddBuff { buff } => Some(buff.expire_time),
                _ => None,
            })
            .unwrap();
        assert!(duration >= i64::from(stats.min_ac() + 15) * 500);
        assert!(duration <= i64::from(stats.max_ac() + 15) * 500);
        durations.insert(duration);
    }
    if stats.min_ac() < stats.max_ac() {
        assert!(
            durations.len() > 1,
            "variable defence must not always give MaxAC duration"
        );
    }
}

#[test]
fn assassin_stealth_half_second_lifetime_is_precise_persisted_and_expires_without_a_coarse_tick() {
    for name in ["MoonLight", "DarkBody"] {
        let (mut session, _, _) = assassin_session();
        let skill = crystal_skill_state(name, 2).unwrap();
        let magic = crystal_magic_by_spell(name).unwrap();
        let mut half_second = None;
        for tick in 0..64 {
            crate::runtime::resources::set_runtime_tick(session.app.world_mut(), tick);
            let packets =
                apply_crystal_self_buff_spell(session.app.world_mut(), &skill, &magic, tick)
                    .unwrap();
            let duration = packets
                .iter()
                .find_map(|p| match p {
                    ServerPacket::AddBuff { buff } => Some(buff.expire_time as u64),
                    _ => None,
                })
                .unwrap();
            if duration % 1000 == 500 {
                half_second = Some((tick, duration));
                break;
            }
        }
        let (tick, duration_ms) =
            half_second.expect("actual Source defence roll produces an odd half-second");
        let key = if name == "MoonLight" {
            "moon-light"
        } else {
            "dark-body"
        };
        let mut buff = session
            .app
            .world()
            .resource::<BuffResource>()
            .buffs
            .iter()
            .find(|buff| buff.key == key)
            .unwrap()
            .clone();
        assert!(
            buff.real_time_duration.is_some(),
            "subtick lifetime is not rounded up"
        );
        assert!(!buff.infinite());
        assert!(!buff.real_time_duration.as_ref().unwrap().is_paused());
        assert!(buff.remaining_ms(tick) <= duration_ms);
        let encoded = serde_json::to_value(&buff).unwrap();
        let remaining = encoded["real_time_duration"].as_u64().unwrap();
        assert!(remaining > 0 && remaining <= duration_ms);
        let restored: BuffState = serde_json::from_value(encoded).unwrap();
        assert!(restored.real_time_duration.is_some());
        assert!(restored.remaining_ms(tick) <= remaining);
        assert!(!buff.expired(tick));
        buff.real_time_duration
            .as_mut()
            .unwrap()
            .elapse_for_test(u64::MAX);
        assert!(
            buff.expired(tick),
            "real expiry does not wait for the rounded tick deadline"
        );
    }
}

#[test]
fn shared_assassin_and_taoist_mirrors_keep_subtick_duration_and_serialized_clock() {
    let (mut session, _, _) = assassin_session();
    for (buff_type, key) in [(13, "moon-light"), (14, "dark-body"), (2, "hiding")] {
        session.apply_zone_player_buff_packets(
            &[ServerPacket::AddBuff {
                buff: mir2_protocol::ClientBuff {
                    buff_type,
                    visible: false,
                    object_id: 1,
                    expire_time: 7500,
                    values: vec![],
                    infinite: false,
                    paused: false,
                    stats: vec![],
                },
            }],
            1,
        );
        let tick = crate::runtime::session::runtime_tick(session.app.world());
        let buff = session
            .app
            .world()
            .resource::<BuffResource>()
            .buffs
            .iter()
            .find(|buff| buff.key == key)
            .unwrap();
        // R25 projects every shared finite buff with its real millisecond
        // lifetime, including original Taoist Hiding. Personal tick count must
        // not retire any of these three leases early.
        assert!(buff.real_time_duration.is_some());
        assert!(!buff.infinite());
        assert!(!buff.real_time_duration.as_ref().unwrap().is_paused());
        assert!(buff.remaining_ms(tick) > 0 && buff.remaining_ms(tick) <= 7500);
        let saved = serde_json::to_value(buff).unwrap();
        let remaining = saved["real_time_duration"].as_u64().unwrap();
        assert!(remaining > 0 && remaining <= 7500);
        let mut restored: BuffState = serde_json::from_value(saved).unwrap();
        assert!(restored.real_time_duration.is_some());
        assert!(!restored.expired(tick));
        restored
            .real_time_duration
            .as_mut()
            .unwrap()
            .elapse_for_test(7500);
        assert!(restored.expired(tick));
    }
}
