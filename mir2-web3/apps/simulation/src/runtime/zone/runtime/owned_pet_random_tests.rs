use super::*;
use crate::runtime::zone::runtime::combat_random::AuthoritativeCombatRandom;

#[test]
fn real_fatal_pet_hit_consumes_one_replayable_curse_draw_and_duplicate_consumes_none() {
    let (mut original, pet, victim) = fixture();
    threaten_owner(&mut original);
    original
        .players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .hp = 10;
    original
        .owned_pet_state_mut()
        .players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .brown_until_ms = 0;
    assert!(original.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
    original.combat_random = AuthoritativeCombatRandom::from_seed([17; 32]);
    let mut expected_random = original.combat_random.clone();
    let expected_roll = (expected_random.next_u32().unwrap() & 3) as u8;
    let mut reference = original.transaction_fork();
    let actual = original.tick_owned_pet_hits(900);
    assert_eq!(actual, reference.tick_owned_pet_hits(900));
    let receipt = actual
        .iter()
        .find_map(|out| match out {
            ZoneOutbound::OwnedPetPlayerKill { receipt } => Some(receipt),
            _ => None,
        })
        .expect("actual lethal damage emits the causal economic receipt");
    assert_eq!(receipt.curse_roll, expected_roll);
    assert!(original.players[&SessionId::new("victim")].dead);
    assert_eq!(original.combat_random, expected_random);
    assert_eq!(
        original.checkpoint_bytes().unwrap(),
        reference.checkpoint_bytes().unwrap()
    );
    assert!(original.tick_owned_pet_hits(901).is_empty());
    assert_eq!(original.combat_random.draws(), 1);
    assert_eq!(
        original.issue_native_player_death_receipt(&SessionId::new("owner"), 902),
        None
    );
    assert_eq!(
        original.combat_random.draws(),
        1,
        "a living victim cannot consume a curse roll"
    );
}

#[test]
fn real_electric_shock_own_pet_preserves_first_roll_and_short_circuit_draw_count() {
    let (mut base, pet, _) = fixture();
    owned_pet_test_map_flags(&mut base, false, false);
    let mut saw_rejected_roll = false;
    let mut saw_shock = false;
    for seed in 0..16 {
        let mut original = base.transaction_fork();
        original.combat_random = AuthoritativeCombatRandom::from_seed([seed; 32]);
        original.queue_owned_human_monster_control(
            &SessionId::new("owner"),
            pet,
            Spell::ElectricShock,
            0,
            1820,
        );
        let mut reference = original.transaction_fork();
        let mut expected_random = original.combat_random.clone();
        let expected_shock = expected_random.next_u32().unwrap() % 4 == 0;
        let actual = original.tick_owned_human_monster_controls(2320);
        assert_eq!(actual, reference.tick_owned_human_monster_controls(2320));
        assert_eq!(original.owned_monster_shocked(pet, 2320), expected_shock);
        assert_eq!(
            original.combat_random, expected_random,
            "own-pet short circuit must not draw the foreign-target coin"
        );
        assert_eq!(
            original.checkpoint_bytes().unwrap(),
            reference.checkpoint_bytes().unwrap()
        );
        saw_shock |= expected_shock;
        saw_rejected_roll |= !expected_shock;
    }
    assert!(saw_shock && saw_rejected_roll);
}

#[test]
fn real_electric_shock_wild_target_replays_shock_rage_tame_and_failed_tame_branches() {
    let (mut base, _, _) = fixture();
    owned_pet_test_map_flags(&mut base, false, false);
    assert!(crystal_monster_by_name("Yob").unwrap().can_tame);
    base.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("owner"),
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 9900,
            name: "Yob".into(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(crate::config::WorldEntityDisposition::Hostile),
            level: 12,
            max_hp: 50,
            hp: 50,
            experience: 0,
            move_speed_ms: 600,
            attack_speed_ms: 1200,
            friendly_guild: None,
            position: point(13, 20),
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
        now_ms: 1000,
    });
    assert!(base.native_monsters.contains_key(&9900));
    let mut outcomes = std::collections::BTreeSet::new();
    for seed in 0..64 {
        let mut original = base.transaction_fork();
        original.combat_random = AuthoritativeCombatRandom::from_seed([seed; 32]);
        original.queue_owned_human_monster_control(
            &SessionId::new("owner"),
            9900,
            Spell::ElectricShock,
            3,
            1820,
        );
        let mut reference = original.transaction_fork();
        let mut expected_random = original.combat_random.clone();
        assert_eq!(expected_random.next_u32().unwrap() % 1, 0);
        let mut rage_until = 0;
        let outcome = if expected_random.next_u32().unwrap() % 2 > 0 {
            "shock"
        } else if expected_random.next_u32().unwrap() % 75 <= 22 {
            if expected_random.next_u32().unwrap() % 5 > 0 {
                rage_until =
                    2320 + (u64::from(expected_random.next_u32().unwrap() % 20) + 10) * 1000;
                "rage"
            } else {
                "failed-level"
            }
        } else if expected_random.next_u32().unwrap() % 2 == 0 {
            "tame"
        } else {
            "failed-hp"
        };
        let actual = original.tick_owned_human_monster_controls(2320);
        assert_eq!(
            actual,
            reference.tick_owned_human_monster_controls(2320),
            "seed {seed}: {outcome}"
        );
        assert_eq!(
            original.combat_random, expected_random,
            "seed {seed}: {outcome}"
        );
        assert_eq!(
            original.owned_monster_shocked(9900, 2320),
            outcome == "shock"
        );
        assert_eq!(
            original.native_monsters[&9900].owner_session_id.is_some(),
            outcome == "tame"
        );
        assert_eq!(
            original
                .owned_monster_clock(9900)
                .map_or(0, |clock| clock.rage_until_ms),
            rage_until
        );
        assert_eq!(
            original.checkpoint_bytes().unwrap(),
            reference.checkpoint_bytes().unwrap()
        );
        outcomes.insert(outcome);
    }
    assert_eq!(
        outcomes,
        ["shock", "rage", "tame", "failed-level", "failed-hp"]
            .into_iter()
            .collect()
    );
}

#[test]
fn cold_restore_keeps_random_suffix_but_revokes_pending_online_shock_authority() {
    let (mut original, pet, _) = fixture();
    owned_pet_test_map_flags(&mut original, false, false);
    original.combat_random = AuthoritativeCombatRandom::from_seed([23; 32]);
    original.combat_random.next_u32().unwrap();
    original.queue_owned_human_monster_control(
        &SessionId::new("owner"),
        pet,
        Spell::ElectricShock,
        3,
        1820,
    );
    let mut live = original.transaction_fork();
    let mut cold = ZoneRuntime::restore_checkpoint(&original.checkpoint_bytes().unwrap()).unwrap();
    let before = cold.combat_random.clone();
    assert!(cold.online_presence.is_empty());
    assert!(cold.tick_owned_human_monster_controls(2320).is_empty());
    assert!(!cold.owned_monster_shocked(pet, 2320));
    assert_eq!(
        cold.combat_random, before,
        "stale online work cannot draw randomness after cold restore"
    );
    live.tick_owned_human_monster_controls(2320);
    assert!(live.owned_monster_shocked(pet, 2320));
    assert_eq!(live.combat_random.draws(), 2);
    assert_eq!(
        cold.combat_random.next_u32(),
        original.combat_random.next_u32(),
        "cold recovery preserves the stream, not old actor permission"
    );
}
