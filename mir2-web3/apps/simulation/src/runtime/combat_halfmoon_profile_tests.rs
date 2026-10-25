use super::*;
use crate::SimulationConfig;
use mir2_protocol::ClientPacket;

fn session_with_weapon_toggles() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut skills = session.app.world_mut().resource_mut::<SkillResource>();
    skills.skills.clear();
    skills.spell_toggles.clear();
    for spell in [
        Spell::Slaying,
        Spell::Thrusting,
        Spell::HalfMoon,
        Spell::CrossHalfMoon,
        Spell::TwinDrakeBlade,
        Spell::FlamingSword,
        Spell::DoubleSlash,
    ] {
        skills.skills.push(
            super::super::skills::crystal_skill_state(&format!("{spell:?}"), 0)
                .expect("weapon skill"),
        );
    }
    drop(skills);
    let player = player_entity(session.app.world()).expect("player");
    session
        .app
        .world_mut()
        .entity_mut(player)
        .get_mut::<PlayerVitals>()
        .expect("player vitals")
        .mp = 500;
    session
}

#[test]
fn halfmoon_stays_selected_when_thrusting_and_slaying_are_armed() {
    let mut session = session_with_weapon_toggles();
    for spell in [Spell::Slaying, Spell::Thrusting, Spell::HalfMoon] {
        let packets = session.handle_packet(ClientPacket::SpellToggle {
            spell,
            toggle_state: 1,
        });
        assert!(packets.iter().any(|packet| matches!(packet,
            ServerPacket::SpellToggle { spell: actual, can_use: true, .. } if *actual == spell
        )));
    }
    assert_eq!(
        session.zone_melee_attack_profile(Spell::None).0,
        Spell::HalfMoon
    );
    assert_eq!(
        session.zone_melee_attack_profile(Spell::Thrusting).0,
        Spell::Thrusting
    );
    session.handle_packet(ClientPacket::SpellToggle {
        spell: Spell::HalfMoon,
        toggle_state: 0,
    });
    assert_eq!(
        session.zone_melee_attack_profile(Spell::None).0,
        Spell::Thrusting
    );
}

#[test]
fn automatic_weapon_profile_uses_crystal_override_order() {
    let mut session = session_with_weapon_toggles();
    // Crystal's attack selection overwrites the earlier weapon technique with
    // every later active technique, ending with prepared FlamingSword.
    for spell in [
        Spell::Slaying,
        Spell::Thrusting,
        Spell::HalfMoon,
        Spell::CrossHalfMoon,
        Spell::TwinDrakeBlade,
        Spell::FlamingSword,
    ] {
        set_skill_toggle_state(session.app.world_mut(), spell, true);
        assert_eq!(session.zone_melee_attack_profile(Spell::None).0, spell);
    }
}

#[test]
fn automatic_weapon_profile_preserves_assassin_prepared_double_slash_priority() {
    let mut session = session_with_weapon_toggles();
    for spell in [
        Spell::Slaying,
        Spell::Thrusting,
        Spell::HalfMoon,
        Spell::CrossHalfMoon,
        Spell::TwinDrakeBlade,
        Spell::DoubleSlash,
    ] {
        set_skill_toggle_state(session.app.world_mut(), spell, true);
    }
    assert_eq!(
        session.zone_melee_attack_profile(Spell::None).0,
        Spell::DoubleSlash
    );
    set_skill_toggle_state(session.app.world_mut(), Spell::FlamingSword, true);
    assert_eq!(
        session.zone_melee_attack_profile(Spell::None).0,
        Spell::FlamingSword
    );
}
