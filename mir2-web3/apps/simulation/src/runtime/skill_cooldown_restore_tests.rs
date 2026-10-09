use super::*;

use mir2_protocol::{ClientPacket, Spell};

fn learned_fireball(cooldown_ends_at: u64, cast_time_ms: i64) -> SkillState {
    SkillState {
        key: "FireBall".to_string(),
        name: "FireBall".to_string(),
        description: "A learned ranged spell.".to_string(),
        level: 2,
        experience: 321,
        hotkey: 4,
        cooldown_ticks: 2,
        delay_ms: 1_800,
        cooldown_ends_at,
        cast_time_ms,
    }
}

fn enter_demo_world(session: &mut SimulationSession) {
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(super::super::resources::is_in_world(session.app.world()));
}

#[test]
fn new_session_restore_rebases_remaining_skill_cooldown_and_keeps_progression() {
    let config = SimulationConfig::default();
    let mut original = SimulationSession::new(config.clone());
    enter_demo_world(&mut original);
    let fireball_key = super::super::skills::skill_key_for_crystal_spell(Spell::FireBall)
        .expect("source FireBall skill key");
    let mut fireball = learned_fireball(0, 0);
    fireball.key = fireball_key.clone();
    original.app.world_mut().resource_mut::<SkillResource>().skills = vec![fireball];
    set_runtime_tick(original.app.world_mut(), 3_655);
    original.apply_zone_player_magic_spend(Spell::FireBall, 0, 1_800);
    let cast_skill = original
        .world_snapshot()
        .known_skills
        .into_iter()
        .find(|skill| skill.key == fireball_key)
        .expect("cast FireBall");
    assert_eq!(cast_skill.cooldown_remaining_ticks, 1);
    original
        .save_active_character()
        .expect("late-tick cast should save");
    drop(original);

    let mut reloaded = SimulationSession::new(config);
    enter_demo_world(&mut reloaded);

    let skill = reloaded
        .world_snapshot()
        .known_skills
        .into_iter()
        .find(|skill| skill.key == fireball_key)
        .expect("restored FireBall");
    assert!(skill.cooldown_remaining_ticks <= 1);
    assert_eq!(skill.cast_time_ms, 0);
    assert_eq!(skill.level, 2);
    assert_eq!(skill.experience, 321);
    assert_eq!(skill.hotkey, 4);
    assert_eq!(skill.delay_ms, 1_800);
}

#[test]
fn durable_cooldown_rebase_uses_elapsed_wall_time_and_authoritative_cap() {
    let clock = DurableSkillClock {
        schema: DURABLE_SKILL_CLOCK_SCHEMA,
        saved_at_unix_ms: 10_000,
        remaining_ms: 2_000,
    };
    assert_eq!(rebased_durable_cooldown_ticks(2, Some(clock), 10_500), 2);
    assert_eq!(rebased_durable_cooldown_ticks(2, Some(clock), 12_500), 0);
    assert_eq!(
        rebased_durable_cooldown_ticks(
            2,
            Some(DurableSkillClock {
                remaining_ms: 3_657_000,
                ..clock
            }),
            10_000,
        ),
        2
    );
    assert_eq!(rebased_durable_cooldown_ticks(2, None, 10_000), 0);
}

#[test]
fn inactive_skill_encoding_is_deterministic_across_wall_clock_samples() {
    let skill = learned_fireball(9_000, 8_000_000);
    let first = encode_durable_skill_states(std::slice::from_ref(&skill), 9_000, 10_000);
    let later = encode_durable_skill_states(std::slice::from_ref(&skill), 9_000, 99_000);
    assert_eq!(later, first);

    let value: serde_json::Value = serde_json::from_str(&first[0]).unwrap();
    assert_eq!(
        value[SKILL_CLOCK_METADATA_KEY]["savedAtUnixMs"],
        serde_json::json!(0)
    );
    assert_eq!(
        value[SKILL_CLOCK_METADATA_KEY]["remainingMs"],
        serde_json::json!(0)
    );
}

#[test]
fn legacy_absolute_skill_deadline_expires_instead_of_becoming_an_hour_cooldown() {
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .and_then(|account| account.saves.get_mut(&0))
            .expect("demo character save");
        save.skill_states_json = encode_state_vec(&vec![learned_fireball(3_657, 3_655_000)]);
    }

    let mut reloaded = SimulationSession::new(config);
    enter_demo_world(&mut reloaded);
    let skill = reloaded
        .world_snapshot()
        .known_skills
        .into_iter()
        .find(|skill| skill.key == "FireBall")
        .expect("legacy FireBall");
    assert_eq!(skill.cooldown_remaining_ticks, 0);
    assert_eq!(skill.cast_time_ms, 0);
    assert_eq!(skill.level, 2);
    assert_eq!(skill.experience, 321);
}

#[test]
fn same_session_checkpoint_restore_preserves_active_skill_cooldown() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    enter_demo_world(&mut session);
    set_runtime_tick(session.app.world_mut(), 10_000);
    session.app.world_mut().resource_mut::<SkillResource>().skills =
        vec![learned_fireball(10_005, 10_000_000)];
    let checkpoint = session
        .active_character_checkpoint()
        .expect("active character checkpoint");

    set_runtime_tick(session.app.world_mut(), 10_001);
    session
        .restore_active_character_checkpoint(&checkpoint)
        .expect("same-session rollback");

    assert_eq!(runtime_tick(session.app.world()), 10_001);
    let skill = session
        .world_snapshot()
        .known_skills
        .into_iter()
        .find(|skill| skill.key == "FireBall")
        .expect("restored FireBall");
    assert_eq!(skill.cooldown_remaining_ticks, 4);
    assert_eq!(skill.cast_time_ms, 10_000_000);
    assert_eq!(skill.level, 2);
    assert_eq!(skill.experience, 321);
    assert_eq!(skill.hotkey, 4);
}
