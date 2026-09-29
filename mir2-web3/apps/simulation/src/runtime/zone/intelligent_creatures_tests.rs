use super::*;

fn owner() -> CreatureOwnerPose {
    CreatureOwnerPose {
        identity: CreatureOwner {
            session_id: SessionId::new("session"),
            account_id: "account".into(),
            character_index: 7,
            object_id: 100,
        },
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        may_operate: true,
    }
}
fn record() -> ClientIntelligentCreature {
    ClientIntelligentCreature {
        pet_type: 0,
        icon: 500,
        custom_name: "Pig".into(),
        fullness: 7500,
        slot_index: 0,
        expire_binary_datetime: 0,
        blackstone_time: 0,
        pet_mode: 1,
        creature_rules: mir2_protocol::IntelligentCreatureRules {
            minimal_fullness: 4000,
            mouse_pickup_enabled: true,
            mouse_pickup_range: 7,
            auto_pickup_enabled: true,
            auto_pickup_range: 7,
            semi_auto_pickup_enabled: true,
            semi_auto_pickup_range: 3,
            can_produce_blackstone: false,
        },
        filter: mir2_protocol::IntelligentCreatureItemFilter {
            pet_pickup_all: true,
            pet_pickup_gold: false,
            pet_pickup_weapons: false,
            pet_pickup_armours: false,
            pet_pickup_helmets: false,
            pet_pickup_boots: false,
            pet_pickup_belts: false,
            pet_pickup_accessories: false,
            pet_pickup_others: false,
        },
        pickup_grade: 0,
        maintain_food_time: 0,
    }
}
fn monster() -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: 200,
        name: "Pig (Owner)".into(),
        name_colour_argb: -1,
        image: 100,
        ai: 64,
        disposition: None,
        level: 1,
        max_hp: 1,
        hp: 1,
        experience: 0,
        move_speed_ms: 200,
        attack_speed_ms: 900,
        friendly_guild: None,
        position: Point { x: 0, y: 0 },
        direction: MirDirection::Up,
        defense: Default::default(),
        respawn: None,
        drops: vec![],
    }
}
fn actor() -> ZoneIntelligentCreature {
    ZoneIntelligentCreature::summon(&owner(), record(), monster(), 0, 0, |_| true)
        .unwrap()
        .0
}
fn target(x: i32, y: i32) -> CreatureTarget {
    CreatureTarget {
        object_id: 300,
        position: Point { x, y },
    }
}

#[test]
fn validates_catalog_and_accepts_all_fifteen_types_including_zero() {
    for ty in 0..15 {
        let mut pet = record();
        pet.pet_type = ty;
        let (actor, event) =
            ZoneIntelligentCreature::summon(&owner(), pet, monster(), ty, 0, |_| true).unwrap();
        assert_eq!(actor.position(), &Point { x: 11, y: 10 });
        assert!(!actor.blocking());
        assert!(matches!(event, CreatureEvent::Appear { pet_type, .. } if pet_type == ty));
    }
    let mut pet = record();
    pet.pet_type = 99;
    assert_eq!(
        ZoneIntelligentCreature::summon(&owner(), pet, monster(), 99, 0, |_| true).unwrap_err(),
        CreatureSpawnError::InvalidType
    );
    assert_eq!(
        ZoneIntelligentCreature::summon(&owner(), record(), monster(), 1, 0, |_| true).unwrap_err(),
        CreatureSpawnError::InvalidCatalog
    );
    assert_eq!(
        ZoneIntelligentCreature::summon(&owner(), record(), monster(), 0, 0, |_| false)
            .unwrap_err(),
        CreatureSpawnError::InvalidPosition
    );
}

#[test]
fn attack_precedes_pickup_by_500ms_and_transaction_is_not_reissued() {
    let mut actor = actor();
    let drop = target(11, 10);
    assert!(actor.request_pickup(
        &owner().identity,
        false,
        Point { x: 0, y: 0 },
        &[drop.clone()]
    ));
    assert!(actor
        .tick(999, Some(&owner()), &[drop.clone()], |_| true)
        .is_empty());
    assert!(matches!(
        actor
            .tick(1000, Some(&owner()), &[drop.clone()], |_| true)
            .as_slice(),
        [CreatureEvent::Attack { .. }]
    ));
    assert!(actor
        .tick(1499, Some(&owner()), &[drop.clone()], |_| true)
        .is_empty());
    let events = actor.tick(1500, Some(&owner()), &[drop.clone()], |_| true);
    assert!(
        matches!(events.as_slice(), [CreatureEvent::PickupIntent(intent)] if intent.creature_object_id == 200 && intent.owner.character_index == 7 && intent.target.object_id == 300)
    );
    assert!(actor
        .tick(10_000, Some(&owner()), &[drop.clone()], |_| true)
        .is_empty());
    assert!(!actor.settle_pickup(301));
    assert!(actor.settle_pickup(300));
    assert!(!actor.settle_pickup(300));
}

#[test]
fn maintenance_sync_preserves_pending_manual_pickup_but_policy_and_hunger_cancel() {
    for change in 0..3 {
        let mut actor = actor();
        let drop = target(11, 10);
        assert!(actor.request_pickup(
            &owner().identity,
            false,
            drop.position.clone(),
            &[drop.clone()]
        ));
        let started = actor.tick(1000, Some(&owner()), &[drop.clone()], |_| true);
        assert!(started
            .iter()
            .any(|e| matches!(e, CreatureEvent::Attack { .. })));
        let mut pet = record();
        pet.blackstone_time = 23;
        pet.maintain_food_time = 3599;
        pet.fullness = 7499;
        pet.custom_name = "Renamed".into();
        pet.slot_index = 3;
        match change {
            1 => pet.pickup_grade = 3,
            2 => pet.fullness = 3999,
            _ => {}
        }
        assert!(actor.synchronize(&owner().identity, pet));
        assert!(actor
            .tick(1499, Some(&owner()), &[drop.clone()], |_| true)
            .is_empty());
        let completed = actor.tick(1500, Some(&owner()), &[drop], |_| true);
        assert_eq!(
            completed
                .iter()
                .filter(|e| matches!(e, CreatureEvent::PickupIntent(_)))
                .count(),
            usize::from(change == 0)
        );
    }
}

#[test]
fn moves_on_cadence_and_never_catches_up_a_stale_queue() {
    let mut actor = actor();
    let drop = target(14, 10);
    actor.request_pickup(
        &owner().identity,
        false,
        Point { x: 0, y: 0 },
        &[drop.clone()],
    );
    assert!(matches!(
        actor
            .tick(1000, Some(&owner()), &[drop.clone()], |_| true)
            .as_slice(),
        [CreatureEvent::Move { .. }]
    ));
    assert_eq!(actor.position().x, 12);
    assert!(actor
        .tick(1100, Some(&owner()), &[drop.clone()], |_| true)
        .is_empty());
    assert_eq!(
        actor
            .tick(100_000, Some(&owner()), &[drop.clone()], |_| true)
            .len(),
        1
    );
    assert_eq!(actor.position().x, 13);
}

#[test]
fn rejects_foreign_owner_distant_mouse_and_starved_requests() {
    let mut actor = actor();
    let mut other = owner().identity;
    other.character_index += 1;
    assert!(!actor.request_pickup(&other, false, Point { x: 11, y: 10 }, &[target(11, 10)]));
    assert!(!actor.request_pickup(
        &owner().identity,
        true,
        Point { x: 999, y: 10 },
        &[target(999, 10)]
    ));
    let mut pet = record();
    pet.fullness = 3999;
    assert!(actor.synchronize(&owner().identity, pet));
    assert!(!actor.request_pickup(
        &owner().identity,
        false,
        Point { x: 11, y: 10 },
        &[target(11, 10)]
    ));
}

#[test]
fn disappeared_or_no_longer_eligible_target_cancels_pending_attack() {
    let mut actor = actor();
    let drop = target(11, 10);
    actor.request_pickup(
        &owner().identity,
        false,
        drop.position.clone(),
        &[drop.clone()],
    );
    actor.tick(1000, Some(&owner()), &[drop], |_| true);
    assert!(actor.tick(1500, Some(&owner()), &[], |_| true).is_empty());
}

#[test]
fn lifecycle_removal_is_once_and_stale_session_cannot_keep_actor_alive() {
    let mut actor = actor();
    let mut owner = owner();
    owner.identity.session_id = SessionId::new("replacement");
    assert_eq!(
        actor.tick(1000, Some(&owner), &[], |_| true),
        vec![CreatureEvent::Remove { object_id: 200 }]
    );
    assert!(actor.tick(2000, None, &[], |_| true).is_empty());
    assert!(actor.appearance().is_none());
}

#[test]
fn rename_updates_aoi_appearance_and_rejects_foreign_owner() {
    let mut actor = actor();
    let mut other = owner().identity;
    other.account_id = "other".into();
    assert!(actor.set_display_name(&other, "Other".into()).is_none());
    assert!(matches!(
        actor.set_display_name(&owner().identity, "New (Owner)".into()),
        Some(CreatureEvent::Name { object_id: 200, .. })
    ));
    assert!(
        matches!(actor.appearance(), Some(CreatureEvent::Appear { monster, .. }) if monster.name == "New (Owner)")
    );
}

#[test]
fn follows_owner_and_recalls_using_remove_appear_without_blocking() {
    let mut actor = actor();
    let mut owner = owner();
    owner.position = Point { x: 15, y: 10 };
    assert!(matches!(
        actor.tick(1000, Some(&owner), &[], |_| true).as_slice(),
        [CreatureEvent::Move { .. }]
    ));
    owner.position = Point { x: 100, y: 100 };
    let events = actor.tick(2000, Some(&owner), &[], |_| true);
    assert!(matches!(
        events.as_slice(),
        [CreatureEvent::Remove { .. }, CreatureEvent::Appear { .. }]
    ));
    assert_eq!(actor.position(), &Point { x: 101, y: 100 });
}

#[test]
fn automatic_search_origin_is_pet_and_blocked_adjacent_drop_can_be_collected() {
    let mut actor = actor();
    let mut pet = record();
    pet.pet_mode = 0;
    actor.synchronize(&owner().identity, pet);
    let drop = target(12, 10);
    let events = actor.tick(1000, Some(&owner()), &[drop.clone()], |point| {
        point != &drop.position
    });
    assert!(events
        .iter()
        .any(|e| matches!(e, CreatureEvent::Attack { object_id: 200, .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, CreatureEvent::PickupIntent(_))));
    assert!(actor
        .tick(1500, Some(&owner()), &[drop], |_| false)
        .is_empty());
}

#[test]
fn mouse_command_remains_delayed_even_when_pet_is_in_automatic_mode() {
    let mut actor = actor();
    let mut pet = record();
    pet.pet_mode = 0;
    actor.synchronize(&owner().identity, pet);
    let drop = target(11, 10);
    actor.request_pickup(
        &owner().identity,
        true,
        drop.position.clone(),
        &[drop.clone()],
    );
    let events = actor.tick(1000, Some(&owner()), &[drop.clone()], |_| true);
    assert!(events
        .iter()
        .any(|e| matches!(e, CreatureEvent::Attack { .. })));
    assert!(!events
        .iter()
        .any(|e| matches!(e, CreatureEvent::PickupIntent(_))));
    assert!(actor
        .tick(1499, Some(&owner()), &[drop.clone()], |_| true)
        .is_empty());
    assert!(matches!(
        actor
            .tick(1500, Some(&owner()), &[drop], |_| true)
            .as_slice(),
        [CreatureEvent::PickupIntent(_)]
    ));
}

#[test]
fn injected_seed_replays_obstacle_rotation_and_supports_both_directions() {
    let mut positions = std::collections::BTreeSet::new();
    for seed in 0..64 {
        let mut first = actor();
        first.set_replay_seed(seed);
        let mut second = actor();
        second.set_replay_seed(seed);
        let drop = target(13, 10);
        for pet in [&mut first, &mut second] {
            pet.request_pickup(
                &owner().identity,
                false,
                drop.position.clone(),
                &[drop.clone()],
            );
        }
        let blocked = |p: &Point| *p != Point { x: 12, y: 10 };
        assert_eq!(
            first.tick(1000, Some(&owner()), &[drop.clone()], blocked),
            second.tick(1000, Some(&owner()), &[drop], blocked)
        );
        positions.insert((first.position().x, first.position().y));
    }
    assert_eq!(
        positions,
        std::collections::BTreeSet::from([(12, 9), (12, 11)])
    );
}

#[test]
fn idle_variants_follow_source_types_and_ten_second_cooldown() {
    let mut observed = std::collections::BTreeSet::new();
    for ty in [0, 7, 8, 9] {
        for seed in 0..256 {
            let mut pet = record();
            pet.pet_type = ty;
            let mut actor =
                ZoneIntelligentCreature::summon(&owner(), pet, monster(), ty, 0, |_| true)
                    .unwrap()
                    .0;
            actor.set_replay_seed(seed);
            assert!(actor.tick(10_000, Some(&owner()), &[], |_| true).is_empty());
            let events = actor.tick(10_500, Some(&owner()), &[], |_| true);
            for event in events {
                match event {
                    CreatureEvent::Attack { attack_type, .. } => {
                        match ty {
                            7 | 8 => assert!([1, 2].contains(&attack_type)),
                            9 => assert_eq!(attack_type, 1),
                            _ => assert!(attack_type <= 3),
                        }
                        observed.insert((ty, attack_type));
                        for now in (11_000..20_500).step_by(500) {
                            assert!(actor.tick(now, Some(&owner()), &[], |_| true).is_empty());
                        }
                    }
                    _ => panic!("idle must never emit a pickup intent"),
                }
            }
        }
    }
    assert_eq!(
        observed,
        std::collections::BTreeSet::from([
            (0, 0),
            (0, 1),
            (0, 2),
            (0, 3),
            (7, 1),
            (7, 2),
            (8, 1),
            (8, 2),
            (9, 1)
        ])
    );
}
