//! Original NPC compatibility semantics in the existing personal script runtime.
//! These tests do not establish shared-Zone NPC world-effect delivery.

use super::*;
use crate::SimulationConfig;

fn authenticated_session() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(player_entity(session.app.world()).is_some());
    session
}

fn action(session: &mut SimulationSession, line: &str) {
    let mut packets = Vec::new();
    assert_eq!(
        execute_crystal_npc_action_line(
            session.app.world_mut(),
            line,
            &mut packets,
            &mut CrystalNpcExecutionState::default(),
        ),
        CrystalNpcActionControl::Continue,
    );
    assert!(
        packets.is_empty(),
        "unexpected packets for {line}: {packets:?}"
    );
}

fn condition(session: &mut SimulationSession, line: &str) -> bool {
    evaluate_crystal_npc_condition(
        session.app.world_mut(),
        line,
        &CrystalNpcContextState {
            npc_position: Point { x: 0, y: 0 },
        },
    )
}

fn section(session: &mut SimulationSession, lines: &[&str]) -> CrystalNpcSectionOutcome {
    execute_crystal_npc_section(
        session.app.world_mut(),
        &CrystalNpcSection {
            label: "@source-semantics".into(),
            line_number: 1,
            lines: lines.iter().map(|line| (*line).into()).collect(),
        },
        &CrystalNpcContextState {
            npc_position: Point { x: 0, y: 0 },
        },
        &mut CrystalNpcExecutionState::default(),
    )
}

#[test]
fn givepet_zero_is_legal_and_does_not_mutate_existing_pet_or_world() {
    let mut session = authenticated_session();
    action(&mut session, "GIVEPET Hen 1 2");
    let old = crystal_npc_pet_entities(session.app.world())[0];
    let old_agent = session
        .app
        .world()
        .entity(old)
        .get::<MonsterAgent>()
        .unwrap()
        .clone();
    let before = serde_json::to_value(session.world_snapshot()).unwrap();
    action(&mut session, "GIVEPET Hen 0 7");
    assert_eq!(crystal_npc_pet_entities(session.app.world()), vec![old]);
    assert_eq!(crystal_npc_pet_level_value(session.app.world(), old), 2);
    assert!(
        session
            .app
            .world()
            .entity(old)
            .get::<MonsterAgent>()
            .unwrap()
            == &old_agent
    );
    assert_eq!(
        serde_json::to_value(session.world_snapshot()).unwrap(),
        before
    );
}

#[test]
fn givepet_byte_arguments_keep_original_defaults_and_caps() {
    let mut session = authenticated_session();
    // NPCSegment.cs:533–539 supplies 1/0; :3429–3433 parses bytes then caps 5/7.
    for (line, count, level) in [
        ("GIVEPET Hen", 1, 0),
        ("GIVEPET Hen 0 7", 0, 7),
        ("GIVEPET Hen 5 7", 5, 7),
        ("GIVEPET Hen 6 8", 5, 7),
        ("GIVEPET Hen 255 255", 5, 7),
        ("GIVEPET Hen 256 256", 1, 0),
        ("GIVEPET Hen -1 -1", 1, 0),
        ("GIVEPET Hen invalid invalid", 1, 0),
        ("GIVEPET Hen +2 +3 ignored", 2, 3),
    ] {
        action(&mut session, "CLEARPETS");
        action(&mut session, line);
        let pets = crystal_npc_pet_entities(session.app.world());
        assert_eq!(pets.len(), count, "{line}");
        for pet in pets {
            assert_eq!(
                crystal_npc_pet_level_value(session.app.world(), pet),
                level,
                "{line}"
            );
        }
    }
}

#[test]
fn givepet_sets_only_new_pets_and_preserves_same_name_old_foreign_and_dead_pets() {
    let mut session = authenticated_session();
    action(&mut session, "GIVEPET Hen 3 2");
    let old_pets = crystal_npc_pet_entities(session.app.world());
    let old = old_pets[0];
    let foreign = old_pets[1];
    let dead = old_pets[2];
    let mut foreign_owner = *session
        .app
        .world()
        .entity(foreign)
        .get::<SummonedMonster>()
        .unwrap();
    foreign_owner.summoner_object_id += 1;
    session
        .app
        .world_mut()
        .entity_mut(foreign)
        .insert(foreign_owner);
    session
        .app
        .world_mut()
        .entity_mut(dead)
        .get_mut::<MonsterAgent>()
        .unwrap()
        .dead = true;
    session
        .app
        .world_mut()
        .entity_mut(dead)
        .get_mut::<MonsterVitals>()
        .unwrap()
        .hp = 0;
    let before: Vec<_> = old_pets
        .iter()
        .map(|pet| {
            let entry = session.app.world().entity(*pet);
            (
                entry.get::<MonsterAgent>().unwrap().clone(),
                *entry.get::<SummonedMonster>().unwrap(),
                entity_position(session.app.world(), *pet).unwrap(),
                entity_facing(session.app.world(), *pet).unwrap(),
            )
        })
        .collect();

    action(&mut session, "GIVEPET Hen 2 7");
    for (pet, (agent, owner, position, facing)) in old_pets.iter().zip(before) {
        let entry = session.app.world().entity(*pet);
        assert_eq!(crystal_npc_pet_level_value(session.app.world(), *pet), 2);
        assert!(
            entry.get::<MonsterAgent>().unwrap() == &agent,
            "old AI state changed"
        );
        assert_eq!(*entry.get::<SummonedMonster>().unwrap(), owner);
        assert_eq!(entity_position(session.app.world(), *pet), Some(position));
        assert_eq!(entity_facing(session.app.world(), *pet), Some(facing));
    }
    let pets = crystal_npc_pet_entities(session.app.world());
    assert_eq!(pets.len(), 3);
    assert!(pets.contains(&old));
    for pet in pets.into_iter().filter(|pet| !old_pets.contains(pet)) {
        assert_eq!(crystal_npc_pet_level_value(session.app.world(), pet), 7);
    }
}

#[test]
fn givepet_birth_uses_exact_owner_tile_direction_and_initial_action_delay() {
    let mut session = authenticated_session();
    let player = player_entity(session.app.world()).unwrap();
    let owner_position = entity_position(session.app.world(), player).unwrap();
    session
        .app
        .world_mut()
        .entity_mut(player)
        .insert(Facing(MirDirection::DownLeft));
    set_runtime_tick(session.app.world_mut(), 41);
    action(&mut session, "GIVEPET Hen 5 7");
    let owner_id = current_player_object_id(session.app.world()).unwrap();
    let pets = crystal_npc_pet_entities(session.app.world());
    assert_eq!(pets.len(), 5);
    for pet in pets {
        let entry = session.app.world().entity(pet);
        assert_eq!(
            entity_position(session.app.world(), pet),
            Some(owner_position.clone())
        );
        assert_eq!(
            entity_facing(session.app.world(), pet),
            Some(MirDirection::DownLeft)
        );
        assert_eq!(
            entry.get::<SummonedMonster>().unwrap().summoner_object_id,
            owner_id
        );
        let agent = entry.get::<MonsterAgent>().unwrap();
        // The existing personal monster clock is in one-second ticks. Preserve
        // that ABI and project the original Envir.Time + 1000 birth action time.
        assert_eq!(agent.next_move_tick, 42);
        assert_eq!(agent.next_attack_tick, 42);
        assert_eq!(agent.next_route_tick, 42);
        assert_eq!(agent.patrol_origin, owner_position);
        let vitals = entry.get::<MonsterVitals>().unwrap();
        assert_eq!(vitals.hp, vitals.max_hp);
        assert!(vitals.hp > 0);
    }
}

#[test]
fn givepet_cap_is_per_action_and_clearpets_still_removes_owner_pets() {
    let mut session = authenticated_session();
    action(&mut session, "GIVEPET Hen 255 2");
    let first = crystal_npc_pet_entities(session.app.world());
    action(&mut session, "GIVEPET Hen 255 7");
    assert_eq!(crystal_npc_pet_entities(session.app.world()).len(), 10);
    for old in first {
        assert_eq!(crystal_npc_pet_level_value(session.app.world(), old), 2);
    }
    assert!(condition(&mut session, "CHECKPET Hen"));
    action(&mut session, "CLEARPETS");
    assert!(!condition(&mut session, "CHECKPET Hen"));
    assert!(crystal_npc_pet_entities(session.app.world()).is_empty());
}

#[test]
fn givepet_missing_or_unknown_monster_leaves_existing_state_unchanged() {
    let mut session = authenticated_session();
    action(&mut session, "GIVEPET Hen 1 2");
    let before = serde_json::to_value(session.world_snapshot()).unwrap();
    for line in ["GIVEPET", "GIVEPET nonexistent-source-monster 5 7"] {
        action(&mut session, line);
        assert_eq!(
            serde_json::to_value(session.world_snapshot()).unwrap(),
            before
        );
    }
    assert!(condition(&mut session, "PETLEVEL = 2"));
}

#[test]
fn checkhum_short_lines_are_omitted_instead_of_becoming_conditions() {
    let mut session = authenticated_session();
    for short in [
        "CHECKHUM",
        "CHECKHUM 1",
        "CHECKHUM 2 0",
        "checkhum 1 D10071",
    ] {
        let outcome = section(
            &mut session,
            &["#IF", short, "#SAY", "accepted", "#ELSESAY", "rejected"],
        );
        assert_eq!(outcome.body, ["accepted"], "{short}");
        assert!(outcome.packets.is_empty());
    }
    assert!(session
        .app
        .world()
        .resource::<NpcStateResource>()
        .npc_script_diagnostics
        .is_empty());
}

#[test]
fn checkhum_omission_preserves_other_conditions_and_valid_checkhum_checks() {
    let mut session = authenticated_session();
    for (real_condition, expected) in [
        ("LEVEL >= 1", "accepted"),
        ("LEVEL < 0", "rejected"),
        ("CHECKHUM >= 1 0", "accepted"),
        ("CHECKHUM >= 2 0", "rejected"),
        ("CHECKHUM >= 1 D10071", "rejected"),
    ] {
        let outcome = section(
            &mut session,
            &[
                "#IF",
                "CHECKHUM 2 0",
                real_condition,
                "#SAY",
                "accepted",
                "#ELSESAY",
                "rejected",
            ],
        );
        assert_eq!(outcome.body, [expected], "{real_condition}");
    }
}

#[test]
fn actual_default_accept_quest_short_checkhum_keeps_original_omission() {
    let mut session = authenticated_session();
    let script = super::super::default_npc_events::expanded_script()
        .expect("actual expanded and source-verified DefaultNPC script");
    let source_section =
        crystal_npc_section(&script, "@_OnAcceptQuest(149)").expect("actual source quest hook");
    assert!(source_section
        .lines
        .iter()
        .any(|line| line.trim() == "CHECKHUM 1 D10071"));
    let before = serde_json::to_value(session.world_snapshot()).unwrap();
    let outcome = execute_crystal_npc_section(
        session.app.world_mut(),
        source_section,
        &CrystalNpcContextState {
            npc_position: Point { x: 0, y: 0 },
        },
        &mut CrystalNpcExecutionState::default(),
    );
    assert!(
        !outcome.body.is_empty(),
        "omitted condition executes the original #SAY"
    );
    assert!(outcome.packets.is_empty());
    assert_eq!(
        serde_json::to_value(session.world_snapshot()).unwrap(),
        before
    );
}
