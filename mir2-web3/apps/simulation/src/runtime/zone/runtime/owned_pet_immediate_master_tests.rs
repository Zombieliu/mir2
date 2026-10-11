//! Source-born chains; private assertions inspect proofs, never mint a Human.
use super::*;
use mir2_protocol::MirGender;

const TARGET: u32 = 9100;
const MAP: &str = "0";

fn join(name: &str, id: u32, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(name),
        account_id: format!("snake-account-{name}"),
        character_index: 1,
        object_id: id,
        name: name.into(),
        class: MirClass::Archer,
        gender: MirGender::Male,
        level: 50,
        hp: 10000,
        max_hp: 10000,
        mp: 1000,
        map_file_name: MAP.into(),
        position,
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}

fn monster(id: u32, name: &str, ai: u8, position: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: id,
        name: name.into(),
        name_colour_argb: -1,
        image: crystal_monster_by_name(name).map_or(900, |t| t.image),
        ai,
        disposition: Some(if ai == 0 {
            crate::WorldEntityDisposition::Hostile
        } else {
            crate::WorldEntityDisposition::Friendly
        }),
        level: 4,
        max_hp: 5000,
        hp: 5000,
        experience: 6,
        move_speed_ms: 600,
        attack_speed_ms: 1200,
        friendly_guild: None,
        defense: Default::default(),
        position,
        direction: MirDirection::Up,
        respawn: None,
        drops: if ai == 0 {
            vec![GroundDropSnapshot {
                object_id: 9200,
                name: "Wasp Gold".into(),
                name_colour_argb: -1,
                icon: 0,
                x: 0,
                y: 0,
                quantity: 8,
                source_monster: String::new(),
                owner_object_id: None,
                ownership_remaining_ticks: None,
                loot: GroundDropLootSnapshot::Gold { amount: 8 },
            }]
        } else {
            Vec::new()
        },
    }
}

fn fixture() -> (ZoneRuntime, u32, u32, u64) {
    // Strict cold recovery reconstructs this ordinary map's collision identity.
    // Translate every original actor/target point by (+222,+199); the source
    // offsets, birth/impact clocks and all retired-identity checks stay intact.
    let mut z = ZoneRuntime::new(ZoneKey::for_map(MAP));
    assert!(z.has_available_collision(), "real fixture terrain must load");
    let owner = SessionId::new("owner");
    z.handle(ZoneCommand::Join(join(
        "owner",
        101,
        Point { x: 322, y: 299 },
    )));
    z.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        monster: monster(TARGET, "Field Wasp", 0, Point { x: 328, y: 299 }),
        now_ms: 0,
    });
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: owner,
        object_id: 0,
        spell: Spell::SummonSnakes,
        direction: MirDirection::Right,
        target: Point { x: 326, y: 299 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    z.tick(1310);
    let parent = *z
        .native_monsters
        .iter()
        .find(|(_, m)| m.ai == 62)
        .unwrap()
        .0;
    for now in (1610..=7310).step_by(300) {
        z.tick(now);
        if let Some(child) = z
            .native_monsters
            .iter()
            .find_map(|(&id, m)| (m.ai == 63).then_some(id))
        {
            return (z, parent, child, now);
        }
    }
    panic!("actual SummonSnakes must produce a child");
}

fn queue(
    z: &mut ZoneRuntime,
    parent: u32,
    child: u32,
    now: u64,
    damage: i32,
) -> ZoneCombatEntityRef {
    let target = z.native_entity_monster_ref(TARGET).unwrap();
    let master = z.native_entity_monster_ref(parent).unwrap();
    assert!(z.set_native_entity_target(TARGET, &master, now));
    assert!(z.queue_owned_pet_hit(child, &target, damage, EntityDefence::MAC, now + 300, now));
    target
}

#[test]
fn owned_snake_birth_binds_real_parent_child_and_causal_online_life() {
    let (z, parent, child, birth) = fixture();
    let parent_life = z.owned_pet_life(parent).expect("real direct Human pet");
    let child_life = z
        .owned_pet_life(child)
        .expect("real immediate Monster Master child");
    assert!(parent_life.monster_master.is_none());
    assert_eq!(child_life.monster_master, Some(parent_life.pet.clone()));
    assert_eq!(child_life.owner, parent_life.owner);
    assert_eq!(child_life.pet, z.native_entity_monster_ref(child).unwrap());
    assert_eq!(z.native_monsters[&child].master_object_id, parent);
    assert_eq!(z.native_monsters[&child].owner_player_object_id, 101);
    assert_eq!(z.native_monsters[&child].next_ai_ready_at_ms, birth + 2001);
    let direct = serde_json::to_value(&parent_life).unwrap();
    assert!(
        direct.get("monster_master").is_none(),
        "legacy direct-Human shape unchanged"
    );
    assert_eq!(
        serde_json::from_value::<OwnedPetLife>(direct).unwrap(),
        parent_life
    );
    assert!(
        z.capture_player_saved_pets(&SessionId::new("owner"), birth)
            .pets
            .is_empty(),
        "source Archer pets and nested children do not become durable personal pets"
    );
}

#[test]
fn owned_snake_permission_uses_totem_threat_and_modes_not_causal_human() {
    let (mut z, parent, child, birth) = fixture();
    let life = z.owned_pet_life(child).unwrap();
    let victim = z.native_entity_monster_ref(TARGET).unwrap();
    let human = life.owner.player.clone();
    z.clear_native_entity_target(TARGET);
    z.note_owned_pet_wild_target(TARGET, &human);
    let parent_life = z.owned_pet_life(parent).unwrap();
    z.owned_pet_state_mut().targets.insert(
        parent,
        OwnedPetTarget {
            life: parent_life,
            target: victim.clone(),
        },
    );
    assert!(
        !z.owned_pet_can_attack(&life, &victim, EntityTargetPurpose::Impact, birth),
        "Human threat/Target cannot replace Totem Master"
    );
    z.owned_pet_state_mut().wild_targets.remove(&TARGET);
    let master = life.monster_master.as_ref().unwrap();
    assert!(z.set_native_entity_target(TARGET, master, birth));
    assert!(z.owned_pet_can_attack(&life, &victim, EntityTargetPurpose::Impact, birth));
    for mode in 0..=4 {
        z.players
            .get_mut(&SessionId::new("owner"))
            .unwrap()
            .chat_profile
            .pet_mode = mode;
        assert!(
            z.owned_pet_can_launch_now(child),
            "Totem's own PMode=Both for Human mode {mode}"
        );
        assert!(z.owned_pet_can_move_now(child));
    }
    assert!(
        !z.native_entity_can_attack(&life.pet, &victim, EntityTargetPurpose::Impact, birth),
        "generic resolver still rejects every owned source"
    );
}

#[test]
fn owned_snake_slave_is_not_a_human_pet_for_focus_mode_or_recall() {
    let (mut z, parent, child, birth) = fixture();
    let owner = SessionId::new("owner");
    let child_life = z.owned_pet_life(child).unwrap();
    let victim = z.native_entity_monster_ref(TARGET).unwrap();
    z.owned_pet_state_mut().targets.clear();
    z.owned_pet_state_mut().targets.insert(
        child,
        OwnedPetTarget {
            life: child_life.clone(),
            target: victim.clone(),
        },
    );
    assert!(
        !z.owned_pet_has_target(&child_life.owner, &victim),
        "Human.Pets excludes the Totem's SlaveList"
    );
    let mut profile = z.player_chat_profile(&owner).unwrap();
    profile.pet_mode = 3;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: owner.clone(),
        profile,
    });
    assert_eq!(
        z.owned_pet_state().unwrap().targets[&child].life,
        child_life,
        "Human mode change retains the Monster child's assigned target"
    );
    z.owned_pet_state_mut().targets.clear();
    z.players.get_mut(&owner).unwrap().chat_profile.pet_mode = 4;
    z.focus_owned_pets(&owner, TARGET, birth);
    assert!(
        z.owned_pet_state().unwrap().targets.contains_key(&parent),
        "ordinary direct Human pets still focus"
    );
    assert!(
        !z.owned_pet_state().unwrap().targets.contains_key(&child),
        "Focus never assigns a causal Human's target to the nested child"
    );
    z.owned_pet_state_mut().targets.clear();
    let master = z.native_entity_monster_ref(parent).unwrap();
    z.set_native_entity_target(TARGET, &master, birth);
    z.note_owned_pet_owner_monster_hit(&owner, TARGET, birth);
    assert!(
        !z.owned_pet_state().unwrap().targets.contains_key(&child),
        "Human attack notification does not assign the nested child's target"
    );
    z.online_presence.get_mut(&owner).unwrap().key = ZoneKey::for_map("human-destination");
    let (recalls, _) = z.drain_online_pet_recalls(birth + 1);
    assert!(recalls
        .iter()
        .all(|recall| recall.life.pet.object_id() != child));
    assert!(
        z.native_monsters.contains_key(&child),
        "a causal Human map change cannot teleport this child away from its actual Totem"
    );
}

#[test]
fn owned_snake_pending_hit_rejects_each_retired_identity() {
    for change in [
        "parent-dead",
        "parent-hp-zero",
        "parent-death-cache",
        "child-death-cache",
        "parent-removed",
        "parent-incarnation",
        "child-incarnation",
        "owner-life",
        "owner-left",
        "cold-recovery",
    ] {
        let (mut z, parent, child, birth) = fixture();
        let now = birth + 2001;
        queue(&mut z, parent, child, now, 13);
        let before = z.native_monsters[&TARGET].hp;
        match change {
            "parent-dead" => {
                z.native_monsters.get_mut(&parent).unwrap().dead = true;
            }
            "parent-hp-zero" => {
                z.native_monsters.get_mut(&parent).unwrap().hp = 0;
            }
            "parent-death-cache" | "child-death-cache" => {
                let id = if change == "parent-death-cache" {
                    parent
                } else {
                    child
                };
                let monster = &z.native_monsters[&id];
                let packet = ServerPacket::ObjectDied {
                    info: ObjectDiedInfo {
                        object_id: id,
                        location: monster.position.clone(),
                        direction: monster.direction,
                        kind: 0,
                    },
                };
                z.apply_zone_object_packets(&[packet], now + 1);
                assert!(
                    z.dead_object_ids.contains_key(&id),
                    "genuine retained death reached the cache"
                );
                assert!(
                    z.native_monsters[&id].hp > 0,
                    "cache death independently protects an otherwise stale native snapshot"
                );
            }
            "parent-removed" => {
                z.despawn_world_event_monster(parent, now + 1);
            }
            "parent-incarnation" => {
                z.native_monsters.get_mut(&parent).unwrap().incarnation =
                    z.allocate_monster_incarnation();
                z.register_owned_pet_life(parent);
            }
            "child-incarnation" => {
                z.native_monsters.get_mut(&child).unwrap().incarnation =
                    z.allocate_monster_incarnation();
                z.register_owned_snake_child_life(child, parent);
            }
            "owner-life" => {
                z.players
                    .get_mut(&SessionId::new("owner"))
                    .unwrap()
                    .life_generation += 1;
                z.refresh_local_online_presence();
            }
            "owner-left" => {
                z.handle(ZoneCommand::Leave {
                    session_id: SessionId::new("owner"),
                });
            }
            "cold-recovery" => {
                z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            z.tick_owned_pet_hits(now + 300).is_empty(),
            "stale proof: {change}"
        );
        assert_eq!(
            z.native_monsters[&TARGET].hp, before,
            "stale proof: {change}"
        );
        assert!(
            z.owned_pet_state().unwrap().hits.is_empty(),
            "discard once: {change}"
        );
    }
}

#[test]
fn owned_snake_pending_hit_revalidates_victim_and_immediate_threat() {
    for change in ["victim-incarnation", "victim-dead", "threat-changed"] {
        let (mut z, parent, child, birth) = fixture();
        let now = birth + 2001;
        queue(&mut z, parent, child, now, 13);
        let before = z.native_monsters[&TARGET].hp;
        match change {
            "victim-incarnation" => {
                z.native_monsters.get_mut(&TARGET).unwrap().incarnation =
                    z.allocate_monster_incarnation();
            }
            "victim-dead" => {
                z.native_monsters.get_mut(&TARGET).unwrap().dead = true;
            }
            "threat-changed" => {
                z.clear_native_entity_target(TARGET);
                let human = z
                    .native_entity_player_ref(&SessionId::new("owner"))
                    .unwrap();
                z.note_owned_pet_wild_target(TARGET, &human);
            }
            _ => unreachable!(),
        }
        assert!(
            z.tick_owned_pet_hits(now + 300).is_empty(),
            "impact must revalidate {change}"
        );
        assert_eq!(z.native_monsters[&TARGET].hp, before);
    }
}

#[test]
fn owned_snake_due_hit_is_mac_delayed_and_consumed_once() {
    let (mut z, parent, child, birth) = fixture();
    let now = birth + 2001;
    let victim = z.native_monsters.get_mut(&TARGET).unwrap();
    victim.defense.min_ac = 1000;
    victim.defense.max_ac = 1000;
    let before = victim.hp;
    queue(&mut z, parent, child, now, 13);
    assert!(z.tick_owned_pet_hits(now + 299).is_empty());
    assert_eq!(z.native_monsters[&TARGET].hp, before);
    let out = z.tick_owned_pet_hits(now + 300);
    assert!(!out.is_empty());
    assert_eq!(
        z.native_monsters[&TARGET].hp,
        before - 13,
        "MAC does not use AC or agility"
    );
    assert!(z.tick_owned_pet_hits(now + 300).is_empty());
    assert_eq!(z.native_monsters[&TARGET].hp, before - 13);
}

#[test]
fn owned_snake_death_burst_uses_master_null_law_and_current_incarnations() {
    let (mut z, parent, child, birth) = fixture();
    let now = birth + 2001;
    let source = z.native_entity_monster_ref(child).unwrap();
    let victim = z.native_entity_monster_ref(TARGET).unwrap();
    let master = z.native_entity_monster_ref(parent).unwrap();
    assert!(z.set_native_entity_target(TARGET, &master, now));
    assert!(
        !z.charmed_snake_death_explosion_can_attack(&source, &victim, now),
        "a live attack is not a death burst"
    );
    let snake = z.native_monsters.get_mut(&child).unwrap();
    snake.hp = 0;
    snake.dead = true;
    assert!(
        !z.charmed_snake_death_explosion_can_attack(&source, &victim, now),
        "Master-null death cannot reuse the live Totem threat"
    );
    z.native_monsters
        .get_mut(&child)
        .unwrap()
        .hallucination_until_ms = now + 100;
    assert!(
        z.charmed_snake_death_explosion_can_attack(&source, &victim, now + 99),
        "original Hallucination permits this separate corpse burst"
    );
    assert!(
        !z.charmed_snake_death_explosion_can_attack(&source, &victim, now + 100),
        "exact deadline is inactive"
    );
    assert!(
        !z.native_entity_can_attack(&source, &victim, EntityTargetPurpose::Impact, now),
        "generic owned/dead-source rejection is unchanged"
    );
    z.native_monsters.get_mut(&TARGET).unwrap().incarnation = z.allocate_monster_incarnation();
    assert!(
        !z.charmed_snake_death_explosion_can_attack(&source, &victim, now),
        "old victim incarnation cannot receive a burst"
    );
    let fresh_victim = z.native_entity_monster_ref(TARGET).unwrap();
    assert!(z.charmed_snake_death_explosion_can_attack(&source, &fresh_victim, now));
    z.native_monsters.get_mut(&child).unwrap().incarnation = z.allocate_monster_incarnation();
    assert!(
        !z.charmed_snake_death_explosion_can_attack(&source, &fresh_victim, now),
        "old corpse incarnation cannot act as a reused source"
    );
}

#[test]
fn owned_snake_parent_id_reuse_cannot_adopt_old_child_or_its_hit() {
    let (mut z, parent, child, birth) = fixture();
    let old = z.owned_pet_life(child).unwrap();
    let now = birth + 2001;
    queue(&mut z, parent, child, now, 13);
    let position = z.native_monsters[&parent].position.clone();
    z.despawn_world_event_monster(parent, now + 1);
    assert!(!z.native_monsters.contains_key(&parent));
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("owner"),
        monster: monster(parent, "SnakeTotem", 62, position),
        now_ms: now + 1,
    });
    assert!(
        z.owned_pet_life(parent).is_none(),
        "bare trusted spawn has no Human ownership grant"
    );
    // Trusted source producer fixture binds ONLY the new parent incarnation.
    let source = z.native_monsters.get_mut(&parent).unwrap();
    source.owner_session_id = Some(SessionId::new("owner"));
    source.master_object_id = 101;
    source.owner_player_object_id = 101;
    z.register_owned_pet_life(parent);
    assert_ne!(
        z.owned_pet_life(parent).unwrap().pet,
        old.monster_master.clone().unwrap()
    );
    assert!(z.owned_pet_life(child).is_none());
    let before = z.native_monsters[&TARGET].hp;
    assert!(z.tick_owned_pet_hits(now + 300).is_empty());
    assert_eq!(z.native_monsters[&TARGET].hp, before);
}

#[test]
fn owned_snake_hit_cannot_mint_human_experience_or_custody() {
    let (mut z, parent, child, birth) = fixture();
    let now = birth + 2001;
    z.native_monsters.get_mut(&TARGET).unwrap().hp = 10;
    assert_eq!(
        z.native_monsters[&TARGET].drops.len(),
        1,
        "the victim really has a nonempty gold producer"
    );
    queue(&mut z, parent, child, now, 13);
    let out = z.tick_owned_pet_hits(now + 300);
    assert!(z.native_monsters[&TARGET].dead, "real fatal child hit");
    assert!(z.native_monsters[&TARGET].experience_owner.is_none());
    assert!(
        !out.iter().any(|out| matches!(
            out,
            ZoneOutbound::MonsterKillAward { .. } | ZoneOutbound::OwnedMonsterKillAward { .. }
        )),
        "Totem's empty WinExp must not turn into Human GainExp"
    );
    assert!(z.issued_native_monster_awards.is_empty());
    assert!(z.ground_drops.is_empty(), "no invented Human loot custody");
}

fn real_first_human_claim(
    z: &mut ZoneRuntime,
    birth: u64,
) -> super::super::super::types::ZoneExperienceOwner {
    let position = z.native_monsters[&TARGET].position.clone();
    let first_id = 90001;
    assert!(!z.object_id_in_use(first_id));
    let mut first = join(
        "first",
        first_id,
        Point {
            x: position.x + 1,
            y: position.y + 1,
        },
    );
    first.class = MirClass::Warrior;
    z.handle(ZoneCommand::Join(first));
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("first"),
        MirClass::Warrior,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    assert_eq!(
        z.players[&SessionId::new("first")].position,
        Point {
            x: position.x + 1,
            y: position.y + 1
        },
        "the first hitter really joins an adjacent vacant tile"
    );
    let attack = z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("first"),
        object_id: TARGET,
        direction: MirDirection::UpLeft,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: birth + 1,
    });
    assert!(
        attack.iter().any(|out| match out {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.iter().any(
                |p| matches!(p, ServerPacket::ObjectAttack { info } if info.object_id == first_id)
            ),
            _ => false,
        }),
        "real first Human swing: {attack:?}"
    );
    z.tick(birth + 301);
    let prior = z.native_monsters[&TARGET]
        .experience_owner
        .clone()
        .expect("actual first Human impact");
    assert_eq!(prior.object_id, first_id);
    prior
}

#[test]
fn owned_snake_hit_preserves_real_first_human_without_renewal() {
    let (mut z, parent, child, birth) = fixture();
    let prior = real_first_human_claim(&mut z, birth);
    let now = birth + 2001;
    queue(&mut z, parent, child, now, 13);
    z.tick_owned_pet_hits(now + 300);
    assert_eq!(
        z.native_monsters[&TARGET].experience_owner,
        Some(prior.clone()),
        "new child damage neither replaces nor refreshes that Human claim"
    );
    queue(&mut z, parent, child, now + 301, 6000);
    let out = z.tick_owned_pet_hits(now + 601);
    assert!(out.iter().any(|out| matches!(out, ZoneOutbound::MonsterKillAward { session_id, .. } | ZoneOutbound::OwnedMonsterKillAward { session_id, .. } if *session_id == SessionId::new("first"))));
    assert!(!out.iter().any(|out| matches!(out, ZoneOutbound::MonsterKillAward { session_id, .. } | ZoneOutbound::OwnedMonsterKillAward { session_id, .. } if *session_id == SessionId::new("owner"))));
    assert_eq!(
        z.ground_drops.len(),
        1,
        "the real first hitter still receives actual gold custody"
    );
    let drop = z.ground_drops.values().next().unwrap();
    assert_eq!(drop.drop.owner_object_id, Some(prior.object_id));
    assert_eq!(
        drop.native_owner.as_ref().unwrap().owner.object_id,
        prior.object_id
    );
}

#[test]
fn owned_snake_replaces_dead_first_human_with_totem_without_new_human_credit() {
    let (mut z, parent, child, birth) = fixture();
    real_first_human_claim(&mut z, birth);
    z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("first"),
        hp: 0,
        max_hp: 10000,
        mp: 1000,
        dead: true,
    });
    assert_eq!(z.player_is_dead(&SessionId::new("first")), Some(true));
    let now = birth + 2001;
    queue(&mut z, parent, child, now, 6000);
    let out = z.tick_owned_pet_hits(now + 300);
    assert!(
        z.native_monsters[&TARGET].dead,
        "the lawful child still applies fatal PvE damage"
    );
    assert!(
        z.native_monsters[&TARGET].experience_owner.is_none(),
        "the source's Totem replacement cannot preserve a dead Human claimant"
    );
    assert!(!out.iter().any(|out| matches!(
        out,
        ZoneOutbound::MonsterKillAward { .. } | ZoneOutbound::OwnedMonsterKillAward { .. }
    )));
    assert!(z.issued_native_monster_awards.is_empty());
    assert!(
        z.ground_drops.is_empty(),
        "neither causal Human nor previous dead Human gains Totem custody"
    );
}
