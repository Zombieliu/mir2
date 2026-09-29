use super::*;
use crate::config::WorldEntityDisposition;
use crate::runtime::zone::types::ZonePlayerCombatStats;
use mir2_protocol::{MirClass, MirGender};

fn joined_zone(hp: i32, mp: i32) -> (ZoneRuntime, SessionId) {
    joined_zone_with_evidence(MirClass::Taoist, 19, hp, mp, true)
}

fn joined_zone_with_evidence(
    class: MirClass,
    level: u16,
    hp: i32,
    mp: i32,
    journey_evidence_enabled: bool,
) -> (ZoneRuntime, SessionId) {
    let session_id = SessionId::new("journey-owner");
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    zone.set_journey_evidence_enabled_for_test(journey_evidence_enabled);
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: session_id.clone(),
        account_id: "journey-account".to_string(),
        character_index: 7,
        object_id: 101,
        name: "JourneyOwner".to_string(),
        class,
        gender: MirGender::Male,
        level,
        hp,
        max_hp: 100,
        mp,
        map_file_name: "0".to_string(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    (zone, session_id)
}

fn spawn_target(zone: &mut ZoneRuntime, owner: &SessionId, object_id: u32) {
    spawn_target_at(zone, owner, object_id, Point { x: 11, y: 10 }, 100);
}

fn spawn_target_at(
    zone: &mut ZoneRuntime,
    owner: &SessionId,
    object_id: u32,
    position: Point,
    hp: i32,
) {
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id,
            name: "Scarecrow".to_string(),
            name_colour_argb: -1,
            image: 5,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 1,
            max_hp: hp,
            hp,
            experience: 1,
            move_speed_ms: 600,
            attack_speed_ms: 1_200,
            friendly_guild: None,
            position,
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
        now_ms: 0,
    });
}

fn admit_combat(zone: &mut ZoneRuntime, owner: &SessionId, class: MirClass) {
    zone.handle(ZoneCommand::sync_player_combat_state(
        owner.clone(),
        class,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
}

fn authoritative_warrior_stats() -> ZonePlayerCombatStats {
    ZonePlayerCombatStats {
        min_dc: 100,
        max_dc: 100,
        accuracy: 100,
        ..ZonePlayerCombatStats::default()
    }
}

fn journey_receipts(outbounds: &[ZoneOutbound]) -> Vec<ZoneJourneyEventReceipt> {
    outbounds
        .iter()
        .filter_map(|outbound| match outbound {
            ZoneOutbound::JourneyEvent { receipt } => Some(receipt.clone()),
            _ => None,
        })
        .collect()
}

fn physical_draft(
    zone: &ZoneRuntime,
    session_id: &SessionId,
    source_action_at_ms: u64,
    source_object_id: u32,
) -> ZoneJourneyEventReceipt {
    let player = zone.players.get(session_id).expect("joined owner");
    zone.journey_event_draft(
        player,
        ZoneJourneyEventKind::PhysicalDamage {
            technique: ZoneJourneyPhysicalTechnique::Normal,
        },
        source_action_at_ms,
        source_object_id,
        Some(9001),
        Some(Point { x: 11, y: 10 }),
        None,
        false,
    )
}

#[test]
fn journey_damage_receipt_requires_positive_committed_hit_and_preserves_delayed_owner_epoch() {
    let (mut zone, owner) = joined_zone(100, 100);
    spawn_target(&mut zone, &owner, 9001);

    // A pending/launch record is not an outbound receipt. Its eventual zero
    // damage result is also not proof of an exercise hit.
    let launch = PendingNativeMonsterHit {
        ready_at_ms: 20,
        session_id: owner.clone(),
        attacker_object_id: 101,
        object_id: 9001,
        damage: 0,
        soulfire_practice: None,
        journey_event: Some(physical_draft(&zone, &owner, 10, 101)),
        fire_bounce: None,
    };
    assert!(journey_receipts(&zone.resolve_pending_native_monster_hit(launch, 20)).is_empty());

    // A delayed positive HP mutation creates a receipt whose source action is
    // earlier than its commit, and its sequence is assigned at commit time.
    let mut skeleton_draft = physical_draft(&zone, &owner, 30, 777);
    skeleton_draft.kind = ZoneJourneyEventKind::SummonSkeletonDamage;
    let hit = PendingNativeMonsterHit {
        ready_at_ms: 40,
        session_id: owner.clone(),
        attacker_object_id: 777,
        object_id: 9001,
        damage: 9,
        soulfire_practice: None,
        journey_event: Some(skeleton_draft),
        fire_bounce: None,
    };
    let receipts = journey_receipts(&zone.resolve_pending_native_monster_hit(hit, 40));
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].source_action_at_ms, 30);
    assert_eq!(receipts[0].committed_at_ms, 40);
    assert_eq!(receipts[0].event_sequence, 1);
    assert_eq!(receipts[0].source_object_id, 777);
    assert_eq!(receipts[0].damage, Some(9));
    assert_eq!(receipts[0].kind, ZoneJourneyEventKind::SummonSkeletonDamage);

    // A delayed hit captured for a previous owner incarnation cannot become a
    // receipt after that owner has revived/rejoined.
    let stale = PendingNativeMonsterHit {
        ready_at_ms: 60,
        session_id: owner.clone(),
        attacker_object_id: 101,
        object_id: 9001,
        damage: 9,
        soulfire_practice: None,
        journey_event: Some(physical_draft(&zone, &owner, 50, 101)),
        fire_bounce: None,
    };
    zone.players.get_mut(&owner).unwrap().life_generation += 1;
    assert!(journey_receipts(&zone.resolve_pending_native_monster_hit(stale, 60)).is_empty());
}

#[test]
fn journey_healing_records_accepted_full_hp_cast_but_rejected_cast_is_quiet() {
    let (mut zone, owner) = joined_zone(100, 10);
    let accepted = zone.player_cast_native_self_magic(
        &owner,
        0,
        Spell::Healing,
        MirDirection::Right,
        Point { x: 10, y: 10 },
        true,
        0,
        1,
        1,
        100,
        10,
    );
    let receipts = journey_receipts(&accepted);
    assert!(matches!(
        receipts.as_slice(),
        [ZoneJourneyEventReceipt {
            kind: ZoneJourneyEventKind::HealingAccepted {
                full_hp_exercise: true
            },
            ..
        }]
    ));

    let (mut rejected_zone, rejected_owner) = joined_zone(100, 0);
    let rejected = rejected_zone.player_cast_native_self_magic(
        &rejected_owner,
        0,
        Spell::Healing,
        MirDirection::Right,
        Point { x: 10, y: 10 },
        true,
        0,
        1,
        1,
        100,
        10,
    );
    assert!(journey_receipts(&rejected).is_empty());
}

#[test]
fn journey_public_healing_with_explicit_self_id_uses_self_route_at_full_and_partial_hp() {
    for hp in [100, 50] {
        let (mut zone, owner) = joined_zone(hp, 10);
        let accepted = zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: owner.clone(),
            object_id: 101,
            spell: Spell::Healing,
            direction: MirDirection::Right,
            target: Point { x: 10, y: 10 },
            cast: true,
            level: 0,
            damage: 1,
            mp_cost: 1,
            cooldown_ms: 100,
            now_ms: 10,
        });
        let receipts = journey_receipts(&accepted);
        assert_eq!(receipts.len(), 1, "explicit self target at {hp} HP must retain journey evidence");
        assert_eq!(receipts[0].kind, ZoneJourneyEventKind::HealingAccepted { full_hp_exercise: hp == 100 });
        assert_eq!(receipts[0].target_object_id, Some(101));
        assert_eq!(zone.players[&owner].mp, 9);
        zone.tick(510);
        if hp < 100 {
            assert!(zone.players[&owner].hp > hp, "normal self healing still restores damaged HP");
        } else {
            assert_eq!(zone.players[&owner].hp, 100);
        }
    }
}

#[test]
fn journey_physical_receipts_follow_committed_native_skill_hits() {
    for (raw_spell, technique, target_position) in [
        (
            Spell::None as u8,
            ZoneJourneyPhysicalTechnique::Normal,
            Point { x: 11, y: 10 },
        ),
        (
            Spell::Fencing as u8,
            ZoneJourneyPhysicalTechnique::Fencing,
            Point { x: 11, y: 10 },
        ),
        (
            Spell::Slaying as u8,
            ZoneJourneyPhysicalTechnique::Slaying,
            Point { x: 11, y: 10 },
        ),
        (
            Spell::Thrusting as u8,
            ZoneJourneyPhysicalTechnique::Thrusting,
            Point { x: 12, y: 10 },
        ),
        (
            Spell::HalfMoon as u8,
            ZoneJourneyPhysicalTechnique::HalfMoon,
            Point { x: 11, y: 10 },
        ),
    ] {
        let (mut zone, owner) = joined_zone_with_evidence(MirClass::Warrior, 50, 100, 100, true);
        zone.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: owner.clone(),
            stats: authoritative_warrior_stats(),
        });
        admit_combat(&mut zone, &owner, MirClass::Warrior);
        spawn_target_at(&mut zone, &owner, 9001, target_position, 500);

        let launched = zone.handle(ZoneCommand::PlayerAttackObject {
            session_id: owner.clone(),
            object_id: 9001,
            direction: MirDirection::Right,
            spell: raw_spell,
            level: 3,
            attack_type: 0,
            damage: 1,
            now_ms: 20,
        });
        assert!(
            journey_receipts(&launched).is_empty(),
            "{technique:?} launch must not be journey evidence"
        );

        let receipts = journey_receipts(&zone.tick(20));
        assert!(matches!(
            receipts.as_slice(),
            [ZoneJourneyEventReceipt {
                kind: ZoneJourneyEventKind::PhysicalDamage { technique: received },
                session_id,
                object_id: 101,
                source_object_id: 101,
                target_object_id: Some(9001),
                damage: Some(damage),
                source_action_at_ms: 20,
                committed_at_ms: 20,
                event_sequence: 1,
                ..
            }] if *received == technique && session_id == &owner && *damage > 0
        ));
    }
}

#[test]
fn journey_lightning_and_firewall_receipts_wait_for_positive_effects_and_respect_gate() {
    let (mut lightning_zone, owner) =
        joined_zone_with_evidence(MirClass::Wizard, 50, 100, 100, true);
    spawn_target_at(
        &mut lightning_zone,
        &owner,
        9001,
        Point { x: 15, y: 10 },
        100,
    );
    let launch = lightning_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: owner.clone(),
        object_id: 0,
        spell: Spell::Lightning,
        direction: MirDirection::Right,
        target: Point { x: 10, y: 10 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    assert!(journey_receipts(&launch).is_empty());
    let lightning = journey_receipts(&lightning_zone.tick(520));
    assert!(matches!(
        lightning.as_slice(),
        [ZoneJourneyEventReceipt {
            kind: ZoneJourneyEventKind::LightningDamage,
            session_id,
            source_object_id: 101,
            target_object_id: Some(9001),
            damage: Some(damage),
            ..
        }] if session_id == &owner && *damage > 0
    ));

    let (mut firewall_zone, firewall_owner) =
        joined_zone_with_evidence(MirClass::Wizard, 50, 100, 100, true);
    let firewall_target = Point { x: 14, y: 10 };
    spawn_target_at(
        &mut firewall_zone,
        &firewall_owner,
        9001,
        firewall_target.clone(),
        100,
    );
    let launch = firewall_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: firewall_owner.clone(),
        object_id: 0,
        spell: Spell::FireWall,
        direction: MirDirection::Right,
        target: firewall_target.clone(),
        cast: true,
        level: 2,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    assert!(journey_receipts(&launch).is_empty());
    let firewall = journey_receipts(&firewall_zone.tick(520));
    assert!(matches!(
        firewall.as_slice(),
        [ZoneJourneyEventReceipt {
            kind: ZoneJourneyEventKind::FireWallDamage,
            session_id,
            target_object_id: Some(9001),
            target_location: Some(target),
            effect_object_id: Some(_),
            damage: Some(damage),
            source_action_at_ms: 20,
            committed_at_ms: 520,
            ..
        }] if session_id == &firewall_owner && target == &firewall_target && *damage > 0
    ));

    let (mut disabled_zone, disabled_owner) =
        joined_zone_with_evidence(MirClass::Wizard, 50, 100, 100, false);
    spawn_target_at(
        &mut disabled_zone,
        &disabled_owner,
        9001,
        Point { x: 15, y: 10 },
        100,
    );
    disabled_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: disabled_owner,
        object_id: 0,
        spell: Spell::Lightning,
        direction: MirDirection::Right,
        target: Point { x: 10, y: 10 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    assert!(journey_receipts(&disabled_zone.tick(520)).is_empty());
    assert!(disabled_zone.native_monsters[&9001].hp < 100);
}

#[test]
fn journey_poison_receipt_requires_material_effect_commit() {
    let (mut zone, owner) = joined_zone_with_evidence(MirClass::Taoist, 50, 100, 100, true);
    let target = Point { x: 14, y: 10 };
    spawn_target_at(&mut zone, &owner, 9001, target.clone(), 100);

    let preview = zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: owner.clone(),
        object_id: 9001,
        spell: Spell::Poisoning,
        direction: MirDirection::Right,
        target: target.clone(),
        cast: false,
        level: 3,
        damage: 12,
        mp_cost: 0,
        cooldown_ms: 500,
        item_param: 1,
        now_ms: 10,
    });
    assert!(journey_receipts(&preview).is_empty());

    let committed = zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: owner.clone(),
        object_id: 9001,
        spell: Spell::Poisoning,
        direction: MirDirection::Right,
        target: target.clone(),
        cast: true,
        level: 3,
        damage: 12,
        mp_cost: 0,
        cooldown_ms: 500,
        item_param: 1,
        now_ms: 20,
    });
    assert!(matches!(
        journey_receipts(&committed).as_slice(),
        [ZoneJourneyEventReceipt {
            kind: ZoneJourneyEventKind::PoisoningApplied,
            session_id,
            source_object_id: 101,
            target_object_id: Some(9001),
            target_location: Some(location),
            material_consumed: true,
            damage: None,
            ..
        }] if session_id == &owner && location == &target
    ));
    assert_ne!(zone.native_monsters[&9001].damage_poison, 0);
}

#[test]
fn journey_summon_receipts_require_real_spawn_and_owned_skeleton_damage() {
    let (mut zone, owner) = joined_zone_with_evidence(MirClass::Taoist, 50, 100, 100, true);
    let summon_location = Point { x: 11, y: 10 };
    let launched = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: owner.clone(),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target: summon_location.clone(),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    assert!(journey_receipts(&launched).is_empty());
    assert!(journey_receipts(&zone.tick(509)).is_empty());

    let spawned = zone.tick(510);
    let spawn_receipt = journey_receipts(&spawned)
        .into_iter()
        .find(|receipt| receipt.kind == ZoneJourneyEventKind::SummonSkeletonSpawn)
        .expect("spawned BoneFamiliar must produce committed journey evidence");
    let skeleton_id = spawn_receipt
        .effect_object_id
        .expect("real summoned object identifies the receipt");
    assert_eq!(spawn_receipt.session_id, owner);
    assert_eq!(spawn_receipt.source_object_id, 101);
    assert_eq!(spawn_receipt.target_location, Some(summon_location.clone()));
    let skeleton = zone
        .native_monsters
        .get(&skeleton_id)
        .expect("spawned skeleton");
    assert_eq!(skeleton.name, "BoneFamiliar");
    assert_eq!(skeleton.owner_session_id.as_ref(), Some(&owner));
    assert_eq!(skeleton.master_object_id, 101);

    let target_location = Point { x: 12, y: 10 };
    spawn_target_at(&mut zone, &owner, 9001, target_location.clone(), 500);
    let launch = zone.launch_native_summon_monster_attack(
        skeleton_id,
        &owner,
        NativeSummonTarget {
            object_id: 9001,
            position: target_location.clone(),
        },
        MirDirection::Right,
        600,
    );
    assert!(journey_receipts(&launch).is_empty());

    let damage = journey_receipts(&zone.tick(1_200));
    assert!(damage.iter().any(|receipt| {
        receipt.kind == ZoneJourneyEventKind::SummonSkeletonDamage
            && receipt.session_id == owner
            && receipt.source_object_id == skeleton_id
            && receipt.target_object_id == Some(9001)
            && receipt.damage.is_some_and(|value| value > 0)
            && receipt.source_action_at_ms == 600
            && receipt.committed_at_ms == 1_200
    }));
}
