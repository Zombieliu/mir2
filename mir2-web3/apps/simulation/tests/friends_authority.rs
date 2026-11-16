use mir2_protocol::{ClientFriend, ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{AccountRecord, CharacterRecord, SimulationConfig, SimulationSession};
use std::collections::BTreeSet;

fn fixture() -> (SimulationConfig, SimulationSession) {
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        for (index, account, name) in [(12, "peer", "Peer"), (34, "blocked", "Blocked")] {
            store.accounts.insert(
                account.into(),
                AccountRecord::new(CharacterRecord {
                    index,
                    name: name.into(),
                    level: 10,
                    class: MirClass::Warrior,
                    gender: MirGender::Male,
                }),
            );
        }
    }
    let session = SimulationSession::new(config.clone());
    (config, session)
}

fn enter(session: &mut SimulationSession) {
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
}

fn add(session: &mut SimulationSession, name: &str, blocked: bool) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::AddFriend {
        name: name.into(),
        blocked,
    })
}

fn friends(session: &mut SimulationSession) -> Vec<ClientFriend> {
    match session
        .friends_with_online_characters(&BTreeSet::new())
        .unwrap()
    {
        ServerPacket::FriendUpdate { friends } => friends,
        _ => unreachable!(),
    }
}

#[test]
fn friends_require_real_other_character_and_duplicates_do_not_toggle_blocked() {
    let (_, mut session) = fixture();
    assert!(add(&mut session, "Peer", false).is_empty());
    enter(&mut session);
    add(&mut session, "Missing", false);
    add(&mut session, "Scout", false);
    assert!(friends(&mut session).is_empty());
    add(&mut session, "pEeR", false);
    add(&mut session, "Peer", true);
    let entries = friends(&mut session);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        (
            entries[0].index,
            entries[0].name.as_str(),
            entries[0].blocked
        ),
        (12, "Peer", false)
    );
}

#[test]
fn removal_preserves_other_character_ids_and_blocked_entries_have_real_online_status() {
    let (_, mut session) = fixture();
    enter(&mut session);
    add(&mut session, "Peer", false);
    add(&mut session, "Blocked", true);
    session.handle_packet(ClientPacket::RemoveFriend {
        character_index: 12,
    });
    let online = [("blocked".into(), 34)].into_iter().collect();
    let ServerPacket::FriendUpdate { friends } =
        session.friends_with_online_characters(&online).unwrap()
    else {
        panic!()
    };
    assert_eq!(friends.len(), 1);
    assert_eq!(friends[0].index, 34);
    assert!(friends[0].blocked && friends[0].online);
    session.handle_packet(ClientPacket::RemoveFriend { character_index: 0 });
    assert_eq!(
        session.world_snapshot().stage5_systems.social.blocked,
        ["Blocked"]
    );
}

#[test]
fn memo_bounds_use_crystal_utf16_length_and_valid_changes_survive_logout() {
    let (_, mut session) = fixture();
    enter(&mut session);
    add(&mut session, "Peer", false);
    let memo = "😀".repeat(100);
    session.handle_packet(ClientPacket::AddMemo {
        character_index: 12,
        memo: memo.clone(),
    });
    for invalid in [String::new(), "😀".repeat(101)] {
        assert!(session
            .handle_packet(ClientPacket::AddMemo {
                character_index: 12,
                memo: invalid
            })
            .is_empty());
    }
    assert_eq!(friends(&mut session)[0].memo, memo);
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(friends(&mut session)[0].memo, memo);
    session.handle_packet(ClientPacket::RemoveFriend {
        character_index: 12,
    });
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(friends(&mut session).is_empty());
}

#[test]
fn identity_survives_rename_and_does_not_rebind_to_recreated_name() {
    let (config, mut session) = fixture();
    enter(&mut session);
    add(&mut session, "Peer", true);
    session.handle_packet(ClientPacket::AddMemo {
        character_index: 12,
        memo: "Known player".into(),
    });
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("peer")
        .unwrap()
        .characters[0]
        .name = "Renamed".into();
    let entries = friends(&mut session);
    assert_eq!(
        (
            entries[0].index,
            entries[0].name.as_str(),
            entries[0].memo.as_str()
        ),
        (12, "Renamed", "Known player")
    );
    assert_eq!(
        session.world_snapshot().stage5_systems.social.blocked,
        ["Renamed"]
    );
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("peer")
        .unwrap()
        .characters[0]
        .index = 99;
    assert!(friends(&mut session).is_empty());
}

#[test]
fn legacy_name_only_save_is_hydrated_to_real_ids_and_bootstrap_restores_friend_list() {
    let (config, _) = fixture();
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap();
        save.stage5_systems_json = Some(r#"{"social":{"friends":["Peer","Missing"],"blocked":["Blocked"],"memos":{"Peer":"Legacy"}}}"#.into());
    }
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(packets.iter().any(
        |packet| matches!(packet, ServerPacket::FriendUpdate { friends }
        if friends.len() == 2 && friends[0].index == 12 && friends[1].index == 34)
    ));
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(friends(&mut session)[0].memo, "Legacy");
}

#[test]
fn legacy_index_collision_is_hidden_and_index_only_mutations_cannot_choose_a_person() {
    let (config, _) = fixture();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.get_mut("blocked").unwrap().characters[0].index = 12;
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap();
        save.stage5_systems_json = Some(r#"{"social":{"friends":["Peer"],"blocked":["Blocked"],"memos":{"Peer":"First","Blocked":"Second"}}}"#.into());
    }
    let mut session = SimulationSession::new(config);
    enter(&mut session);
    assert!(friends(&mut session).is_empty());
    let before = session.world_snapshot().stage5_systems.social;
    assert!(session
        .handle_packet(ClientPacket::AddMemo {
            character_index: 12,
            memo: "Wrong".into()
        })
        .is_empty());
    assert!(session
        .handle_packet(ClientPacket::RemoveFriend {
            character_index: 12
        })
        .is_empty());
    let after = session.world_snapshot().stage5_systems.social;
    assert_eq!(after.friends, before.friends);
    assert_eq!(after.blocked, before.blocked);
    assert_eq!(after.memos, before.memos);
    session.handle_packet(ClientPacket::LogOut);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(friends(&mut session).is_empty());
    assert_eq!(
        session.world_snapshot().stage5_systems.social.memos,
        before.memos
    );
}
