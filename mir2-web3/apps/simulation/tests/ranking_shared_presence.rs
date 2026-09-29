use std::collections::BTreeSet;

use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, SimulationConfig, SimulationSession,
};

fn fixture() -> SimulationSession {
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for index in 0..25 {
            let mut account = AccountRecord::empty();
            account.characters.push(CharacterRecord {
                index,
                name: format!("Rank{index:02}"),
                level: 30 - index as u16,
                class: if index == 1 {
                    MirClass::Wizard
                } else {
                    MirClass::Warrior
                },
                gender: MirGender::Male,
            });
            let mut save = CharacterSaveRecord::new(account.characters[0].clone());
            save.map_file_name = config.map.file_name.clone();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            account.saves.insert(index, save);
            store.accounts.insert(
                if index == 0 {
                    "demo".into()
                } else {
                    format!("account{index}")
                },
                account,
            );
        }
    }
    SimulationSession::new(config)
}

fn online() -> BTreeSet<(String, i32)> {
    [
        ("demo".into(), 0),
        ("account1".into(), 1),
        ("account24".into(), 24),
        // A character index under a different account is not the same identity.
        ("wrong-account".into(), 2),
    ]
    .into_iter()
    .collect()
}

fn ranking(packets: Vec<ServerPacket>) -> (i32, i32, Vec<i64>) {
    packets
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::Rankings {
                my_rank,
                count,
                listings,
                ..
            } => Some((my_rank, count, listings)),
            _ => None,
        })
        .expect("ranking response")
}

#[test]
fn shared_online_filter_uses_exact_identity_before_pagination_and_preserves_global_rank() {
    let mut session = fixture();
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let (rank, count, ids) = ranking(session.ranking_with_online_characters(0, 1, true, &online()));
    assert_eq!((rank, count), (1, 3));
    assert_eq!(ids, [1, 24]);
    let (_, count, ids) = ranking(session.ranking_with_online_characters(1, 0, true, &online()));
    assert_eq!(count, 2);
    assert_eq!(ids, [0, 24]);
    let (_, count, ids) =
        ranking(session.ranking_with_online_characters(0, 20, false, &BTreeSet::new()));
    assert_eq!(count, 25);
    assert_eq!(ids, [20, 21, 22, 23, 24]);
    let (_, count, ids) =
        ranking(session.ranking_with_online_characters(0, 0, true, &BTreeSet::new()));
    assert_eq!(count, 0);
    assert!(ids.is_empty());
}

#[test]
fn shared_ranking_does_not_bypass_login_or_start_game() {
    let mut session = fixture();
    assert!(session
        .ranking_with_online_characters(0, 0, true, &online())
        .is_empty());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    assert!(session
        .ranking_with_online_characters(0, 0, false, &online())
        .is_empty());
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session
        .ranking_with_online_characters(6, 0, false, &online())
        .is_empty());
    assert!(session
        .ranking_with_online_characters(0, -1, false, &online())
        .is_empty());
    session.handle_packet(ClientPacket::LogOut);
    assert!(session
        .ranking_with_online_characters(0, 0, false, &online())
        .is_empty());
}
