use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    AccountRecord, SimulationConfig, SimulationSession, WorldEntityKind, WorldEntitySnapshot,
};

fn start_fresh_bichon_warrior() -> SimulationSession {
    let account_id = "quest-icon-fresh-bichon";
    let config = SimulationConfig::default().with_crystal_world_runtime();
    config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned")
        .accounts
        .insert(account_id.to_owned(), AccountRecord::empty());

    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: account_id.to_owned(),
        password: "demo".to_owned(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let created = session.handle_packet(ClientPacket::NewCharacter {
        name: "QuestIconBlade".to_owned(),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let character_index = created
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .expect("fresh character should be created");
    let started = session.handle_packet(ClientPacket::StartGame { character_index });
    assert!(started
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    session
}

fn npc(session: &SimulationSession, object_id: u32) -> WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::Npc && entity.object_id == object_id)
        .unwrap_or_else(|| panic!("NPC {object_id} should be visible"))
}

fn interact_at(
    session: &mut SimulationSession,
    npc_object_id: u32,
    player_position: Point,
    target: &str,
) {
    session.force_authoritative_player_transform(player_position, MirDirection::Right);
    let _ = session.interact(npc_object_id);
    let dialog = session
        .world_snapshot()
        .active_npc_dialog
        .expect("NPC interaction should open a dialog");
    assert!(
        dialog.links.iter().any(|link| link.target == target),
        "NPC {npc_object_id} should expose {target}: {dialog:?}"
    );
}

#[test]
fn authoritative_npc_quest_icon_tracks_original_q1_accept_and_finish_roles() {
    let mut session = start_fresh_bichon_warrior();

    assert_eq!(
        npc(&session, 3).quest_icon,
        Some(2),
        "Assistant Jane should offer q1 with Crystal ExclamationYellow"
    );
    assert_eq!(
        npc(&session, 4).quest_icon,
        None,
        "CraftsLady Jude must not advertise the prerequisite-gated q2 yet"
    );

    interact_at(&mut session, 3, Point { x: 283, y: 606 }, "@quest:accept:1");
    let accepted = session.select_npc_dialog_target("@quest:accept:1");
    assert!(accepted.iter().any(|packet| matches!(
        packet,
        ServerPacket::ChangeQuest {
            quest_id: 1,
            taken: true,
            completed: true,
            ..
        }
    )));
    session.force_authoritative_player_transform(Point { x: 288, y: 616 }, MirDirection::Right);

    assert_eq!(
        npc(&session, 3).quest_icon,
        None,
        "the start NPC must stop offering q1 once it is current"
    );
    assert_eq!(
        npc(&session, 4).quest_icon,
        Some(3),
        "the finish NPC should show Crystal QuestionYellow when q1 is ready"
    );

    interact_at(&mut session, 4, Point { x: 293, y: 619 }, "@quest:finish:1");
    let finished = session.select_npc_dialog_target("@quest:finish:1");
    assert!(finished.iter().any(|packet| matches!(
        packet,
        ServerPacket::CompleteQuest { completed_quests } if completed_quests.contains(&1)
    )));
    session.force_authoritative_player_transform(Point { x: 288, y: 616 }, MirDirection::Right);

    assert_eq!(
        npc(&session, 4).quest_icon,
        Some(2),
        "after q1 hand-in the same NPC should offer q2 with ExclamationYellow"
    );
}
