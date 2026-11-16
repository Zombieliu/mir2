use super::*;
use crate::{SimulationConfig, SimulationSession};
use mir2_protocol::ClientPacket;
fn session() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
}
#[test]
fn magic_key_preflight_unknown_spell_does_not_clear_a_learned_binding() {
    let mut session = session();
    session
        .app
        .world_mut()
        .resource_mut::<SkillResource>()
        .skills
        .retain(|skill| skill.key == "minor-heal");
    assert!(session.supports_magic_key_assignment(Spell::Healing, 5, 0));
    assign_magic_key(session.app.world_mut(), Spell::Healing, 5, 0);
    assert!(!session.supports_magic_key_assignment(Spell::FireBall, 5, 0));
    assign_magic_key(session.app.world_mut(), Spell::FireBall, 5, 0);
    assert_eq!(
        session.app.world().resource::<SkillResource>().skills[0].hotkey,
        5
    );
    session.handle_packet(ClientPacket::LogOut);
    assert!(!session.supports_magic_key_assignment(Spell::Healing, 5, 0));
}
#[test]
fn magic_key_preflight_requires_spawned_living_hero_actor_and_learned_spell() {
    let mut session = session();
    {
        let mut stage5 = session
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>();
        stage5.stage5_systems.hero = Some(
            serde_json::from_value(
                serde_json::json!({"name":"Hero","level":20,"behaviour":0,"spawned":true}),
            )
            .unwrap(),
        );
        stage5.stage5_systems.hero_learned_magics = vec![crate::config::Stage5HeroMagicState {
            spell: Spell::Healing,
            level: 0,
            key: 20,
            experience: 0,
        }];
    }
    assert!(
        !session.supports_magic_key_assignment(Spell::Healing, 20, 20),
        "a matching old key is not evidence that a request can execute"
    );
    let entity = session
        .app
        .world_mut()
        .spawn((
            super::super::components::Hero {
                owner_name: "Scout".into(),
                next_attack_tick: 0,
                next_move_tick: 0,
            },
            PlayerVitals {
                hp: 1,
                max_hp: 10,
                mp: 1,
                max_mp: 10,
            },
        ))
        .id();
    assert!(session.supports_magic_key_assignment(Spell::Healing, 20, 20));
    assert!(!session.supports_magic_key_assignment(Spell::FireBall, 20, 20));
    session
        .app
        .world_mut()
        .entity_mut(entity)
        .get_mut::<PlayerVitals>()
        .unwrap()
        .hp = 0;
    assert!(!session.supports_magic_key_assignment(Spell::Healing, 20, 20));
    assign_magic_key(session.app.world_mut(), Spell::Healing, 21, 20);
    assert_eq!(
        session
            .app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .hero_learned_magics[0]
            .key,
        20
    );
    session
        .app
        .world_mut()
        .entity_mut(entity)
        .get_mut::<PlayerVitals>()
        .unwrap()
        .hp = 1;
    session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .as_mut()
        .unwrap()
        .spawned = false;
    assert!(!session.supports_magic_key_assignment(Spell::Healing, 20, 20));
}
