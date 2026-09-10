use super::*;
use crate::SimulationConfig;
use mir2_protocol::ClientPacket;

fn session_with(spell: &str, level: u8) -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut skills = session.app.world_mut().resource_mut::<SkillResource>();
    skills.skills.clear();
    skills.spell_toggles.clear();
    skills.slaying_armed = false;
    skills.skills.push(super::super::skills::crystal_skill_state(spell, level).unwrap());
    drop(skills);
    session
}

fn mp(session: &SimulationSession) -> i32 {
    session.app.world().get::<PlayerVitals>(player_entity(session.app.world()).unwrap()).unwrap().mp
}

fn set_mp(session: &mut SimulationSession, value: i32) {
    let player = player_entity(session.app.world()).unwrap();
    session.app.world_mut().entity_mut(player).get_mut::<PlayerVitals>().unwrap().mp = value;
}

#[test]
fn warrior_preparation_slaying_exact_twelve_outcomes_at_every_level() {
    for level in 0..=3 {
        let mut count = 0;
        for roll in 0..12 {
            let mut session = session_with("Slaying", level);
            let packets = prepare_slaying_after_attack(session.app.world_mut(), roll);
            let expected = roll <= u64::from(level);
            assert_eq!(!packets.is_empty(), expected, "level={level}, roll={roll}");
            assert_eq!(skill_toggle_state(session.app.world(), Spell::Slaying), expected);
            count += usize::from(expected);
            if expected {
                assert!(prepare_slaying_after_attack(session.app.world_mut(), 0).is_empty());
            }
        }
        assert_eq!(count, usize::from(level) + 1);
        assert!(!slaying_roll_arms(level, 12));
    }
    let mut unlearned = session_with("Fencing", 3);
    assert!(prepare_slaying_after_attack(unlearned.app.world_mut(), 0).is_empty());
}

#[test]
fn warrior_preparation_twin_drake_hotkey_requires_strict_mana_and_consumes_once() {
    for level in 0..=3 {
        let mut session = session_with("TwinDrakeBlade", level);
        let (magic, _) = crystal_skill_magic(session.app.world(), "TwinDrakeBlade").unwrap();
        let cost = i32::from(magic.base_cost) + i32::from(magic.level_cost) * i32::from(level);
        let skill = &session.app.world().resource::<SkillResource>().skills[0];
        assert_eq!(skill.snapshot(0, mir2_game_data::LanguageCode::English).cast_kind, "toggle");
        assert_eq!(session.zone_melee_attack_profile(Spell::None).0, Spell::None);
        assert_eq!(session.zone_melee_attack_profile(Spell::TwinDrakeBlade).0, Spell::None);
        set_mp(&mut session, cost);
        let toggle = ClientPacket::SpellToggle { spell: Spell::TwinDrakeBlade, toggle_state: 1 };
        assert!(session.handle_packet(toggle.clone()).is_empty());
        assert_eq!(mp(&session), cost);
        set_mp(&mut session, cost * 2);
        // Original SpellToggle ignores the requested enable/disable state.
        let prepared = session.handle_packet(ClientPacket::SpellToggle { spell: Spell::TwinDrakeBlade, toggle_state: 0 });
        assert!(prepared.iter().any(|p| matches!(p, ServerPacket::ObjectMagic { spell: Spell::TwinDrakeBlade, target_id: 0, .. })));
        assert_eq!(mp(&session), cost);
        assert!(session.handle_packet(toggle).is_empty());
        assert_eq!(mp(&session), cost);
        set_mp(&mut session, cost - 1);
        assert_eq!(session.zone_melee_attack_profile(Spell::None).0, Spell::None);
        assert!(skill_toggle_state(session.app.world(), Spell::TwinDrakeBlade));
        set_mp(&mut session, cost);
        assert_eq!(session.zone_melee_attack_profile(Spell::None).0, Spell::TwinDrakeBlade);
        let consumed = session.commit_zone_melee_attack_spell(Spell::TwinDrakeBlade);
        assert_eq!(mp(&session), 0);
        assert!(consumed.iter().any(|p| matches!(p, ServerPacket::SpellToggle { spell: Spell::TwinDrakeBlade, can_use: false, .. })));
        assert_eq!(session.zone_melee_attack_profile(Spell::None).0, Spell::None);
        assert!(session.commit_zone_melee_attack_spell(Spell::None).is_empty());
        assert!(session.commit_zone_melee_attack_spell(Spell::TwinDrakeBlade).is_empty());
        assert_eq!(mp(&session), 0);
    }
}

#[test]
fn warrior_preparation_shared_plain_swing_arms_only_the_next_slaying_attack() {
    let mut session = session_with("Slaying", 0);
    for tick in 0..1000 {
        session.app.world_mut().resource_mut::<RuntimeClockResource>().tick = tick;
        if slaying_attack_roll(session.app.world()) == 0 { break; }
    }
    assert_eq!(slaying_attack_roll(session.app.world()), 0);
    assert_eq!(session.zone_melee_attack_profile(Spell::None).0, Spell::None);
    assert!(session.commit_zone_melee_attack_spell(Spell::None).iter().any(|p| matches!(p,
        ServerPacket::SpellToggle { spell: Spell::Slaying, can_use: true, .. }
    )));
    assert_eq!(session.zone_melee_attack_profile(Spell::None).0, Spell::Slaying);
    for tick in 0..1000 {
        session.app.world_mut().resource_mut::<RuntimeClockResource>().tick = tick;
        if slaying_attack_roll(session.app.world()) > 0 { break; }
    }
    assert!(slaying_attack_roll(session.app.world()) > 0);
    session.commit_zone_melee_attack_spell(Spell::Slaying);
    assert!(!session.app.world().resource::<SkillResource>().slaying_armed);
    assert_eq!(session.zone_melee_attack_profile(Spell::None).0, Spell::None);
}
