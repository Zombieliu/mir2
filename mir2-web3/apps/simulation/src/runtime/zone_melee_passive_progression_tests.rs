use super::*;
use crate::SimulationConfig;
use mir2_protocol::ClientPacket;

fn session_with_passives() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .expect("character should be active")
        .level = 55;

    let mut skills = session.app.world_mut().resource_mut::<SkillResource>();
    skills.skills.clear();
    skills
        .skills
        .push(super::super::skills::crystal_skill_state("Fencing", 2).expect("Fencing skill"));
    skills.skills.push(
        super::super::skills::crystal_skill_state("SpiritSword", 2).expect("SpiritSword skill"),
    );
    drop(skills);
    session
}

fn experience(session: &SimulationSession, key: &str) -> u16 {
    session
        .app
        .world()
        .resource::<SkillResource>()
        .skills
        .iter()
        .find(|skill| skill.key == normalize_crystal_skill_key(key))
        .expect("learned passive skill")
        .experience
}

#[test]
fn shared_zone_committed_melee_advances_each_learned_passive_once() {
    let mut session = session_with_passives();
    let fencing_before = experience(&session, "Fencing");
    let spirit_before = experience(&session, "SpiritSword");

    // Reading the attack profile is admission preparation, not a committed hit.
    session.zone_melee_attack_profile(Spell::None);
    assert_eq!(experience(&session, "Fencing"), fencing_before);
    assert_eq!(experience(&session, "SpiritSword"), spirit_before);

    let packets = session.commit_zone_melee_attack_spell(Spell::None);
    let fencing_after = experience(&session, "Fencing");
    let spirit_after = experience(&session, "SpiritSword");

    assert!(fencing_after > fencing_before);
    assert!(spirit_after > spirit_before);
    assert_eq!(
        packets
            .iter()
            .filter(|packet| {
                matches!(
                    packet,
                    ServerPacket::MagicLeveled {
                        spell: Spell::Fencing,
                        ..
                    }
                )
            })
            .count(),
        1
    );
    assert_eq!(
        packets
            .iter()
            .filter(|packet| {
                matches!(
                    packet,
                    ServerPacket::MagicLeveled {
                        spell: Spell::SpiritSword,
                        ..
                    }
                )
            })
            .count(),
        1
    );
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MagicLeveled {
            spell: Spell::Fencing,
            experience,
            ..
        } if *experience == fencing_after
    )));
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MagicLeveled {
            spell: Spell::SpiritSword,
            experience,
            ..
        } if *experience == spirit_after
    )));
}

#[test]
fn shared_zone_commit_only_progresses_passives_the_player_has_learned() {
    let mut session = session_with_passives();
    session
        .app
        .world_mut()
        .resource_mut::<SkillResource>()
        .skills
        .retain(|skill| skill.key == normalize_crystal_skill_key("Fencing"));

    let packets = session.commit_zone_melee_attack_spell(Spell::None);

    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MagicLeveled {
            spell: Spell::Fencing,
            ..
        }
    )));
    assert!(packets.iter().all(|packet| !matches!(
        packet,
        ServerPacket::MagicLeveled {
            spell: Spell::SpiritSword,
            ..
        }
    )));
}
