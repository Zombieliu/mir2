use super::*;
use crate::SimulationConfig;
use mir2_protocol::ClientPacket;

#[test]
fn slaying_is_passive_in_snapshot_but_still_bindable_and_grants_melee_bonus() {
    // Crystal GameScene's hotkey switch returns for Slaying; its server alone
    // arms Slaying through SpellToggle after an attack. Assigning a key is
    // still permitted, just like the other passive skills.
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(is_in_world(session.app.world()));
    let key = skill_key_for_crystal_spell(Spell::Slaying).expect("source Slaying key");
    session
        .app
        .world_mut()
        .resource_mut::<SkillResource>()
        .skills
        .retain(|s| s.key != key);
    let before_damage = super::super::combat::crystal_player_melee_damage(session.app.world());
    session
        .app
        .world_mut()
        .resource_mut::<SkillResource>()
        .skills
        .push(SkillState {
            key: key.clone(),
            name: "Slaying".into(),
            description: String::new(),
            level: 3,
            experience: 0,
            hotkey: 0,
            cooldown_ticks: 0,
            delay_ms: 0,
            cooldown_ends_at: 0,
            cast_time_ms: 0,
        });
    assert!(session.supports_magic_key_assignment(Spell::Slaying, 2, 0));
    assign_magic_key(session.app.world_mut(), Spell::Slaying, 2, 0);
    let state = session
        .app
        .world()
        .resource::<SkillResource>()
        .skills
        .iter()
        .find(|s| s.key == key)
        .unwrap();
    let snapshot = state.snapshot(0, LanguageCode::English);
    assert_eq!(snapshot.cast_kind, "passive");
    assert_eq!(snapshot.hotkey, 2);
    assert_eq!(snapshot.spell.as_deref(), Some("Slaying"));
    assert_eq!(
        super::super::combat::crystal_player_melee_damage(session.app.world()),
        before_damage + 8
    );
    let player = player_entity(session.app.world()).unwrap();
    let before_mp = entity_player_vitals(session.app.world(), player)
        .unwrap()
        .mp;
    assert!(!skill_cast_preflight_for_key(
        session.app.world(),
        &key,
        None
    ));
    let packets = session.cast_skill(&key);
    assert!(!packets.iter().any(|p| matches!(
        p,
        ServerPacket::ObjectMagic { .. } | ServerPacket::SpellToggle { .. }
    )));
    assert_eq!(
        entity_player_vitals(session.app.world(), player)
            .unwrap()
            .mp,
        before_mp
    );
    assert!(
        !session
            .app
            .world()
            .resource::<SkillResource>()
            .slaying_armed
    );
    assert_eq!(
        crystal_spell_cast_kind(Spell::Thrusting),
        SkillCastKind::Toggle
    );
    assert_eq!(
        crystal_spell_cast_kind(Spell::HalfMoon),
        SkillCastKind::Toggle
    );
}
