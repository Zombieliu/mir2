use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, SimulationConfig, SimulationSession,
};

fn ranked_character(index: i32, name: &str, level: u16, class: MirClass) -> CharacterRecord {
    CharacterRecord {
        index,
        name: name.to_string(),
        level,
        class,
        gender: MirGender::Male,
    }
}

fn save_for_character(
    config: &SimulationConfig,
    character: CharacterRecord,
    experience: i64,
) -> CharacterSaveRecord {
    let mut save = CharacterSaveRecord::new(character);
    save.map_file_name = config.map.file_name.clone();
    save.map_title = config.map.title.clone();
    save.position = config.spawn.clone();
    save.direction = MirDirection::Down;
    save.experience = experience;
    save
}

fn account_for_character(
    config: &SimulationConfig,
    character: CharacterRecord,
    experience: i64,
) -> AccountRecord {
    let mut account = AccountRecord::empty();
    account.characters.push(character.clone());
    account.saves.insert(
        character.index,
        save_for_character(config, character, experience),
    );
    account
}

fn ranking_packet(packets: &[ServerPacket]) -> &ServerPacket {
    packets
        .iter()
        .find(|packet| matches!(packet, ServerPacket::Rankings { .. }))
        .expect("ranking request should emit Rankings")
}

#[test]
fn get_ranking_returns_crystal_rankings_from_character_saves() {
    let config = SimulationConfig::default();
    let scout = ranked_character(0, "Scout", 12, MirClass::Warrior);
    let blade = ranked_character(1, "Blade", 40, MirClass::Warrior);
    let mage = ranked_character(2, "Mage", 40, MirClass::Wizard);
    let arrow = ranked_character(3, "Arrow", 18, MirClass::Archer);

    {
        let mut store = config.account_store.lock().expect("account store lock");
        store.accounts.clear();
        store.next_character_index = 4;
        store.accounts.insert(
            "demo".to_string(),
            account_for_character(&config, scout.clone(), 50),
        );
        store.accounts.insert(
            "blade".to_string(),
            account_for_character(&config, blade.clone(), 500),
        );
        store.accounts.insert(
            "mage".to_string(),
            account_for_character(&config, mage.clone(), 900),
        );
        store.accounts.insert(
            "arrow".to_string(),
            account_for_character(&config, arrow.clone(), 1),
        );
    }

    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let _ = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

    let overall = session.handle_packet(ClientPacket::GetRanking {
        rank_type: 0,
        rank_index: 0,
        online_only: false,
    });
    match ranking_packet(&overall) {
        ServerPacket::Rankings {
            rank_type,
            my_rank,
            listing_details,
            listings,
            count,
        } => {
            assert_eq!(*rank_type, 0);
            assert_eq!(*count, 4);
            assert_eq!(*my_rank, 4);
            assert_eq!(
                listing_details
                    .iter()
                    .map(|entry| entry.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["Mage", "Blade", "Arrow", "Scout"]
            );
            assert_eq!(*listings, vec![2, 1, 3, 0]);
        }
        _ => unreachable!("ranking_packet only returns Rankings"),
    }

    let warriors = session.handle_packet(ClientPacket::GetRanking {
        rank_type: 1,
        rank_index: 0,
        online_only: false,
    });
    match ranking_packet(&warriors) {
        ServerPacket::Rankings {
            my_rank,
            listing_details,
            count,
            ..
        } => {
            assert_eq!(*count, 2);
            assert_eq!(*my_rank, 2);
            assert_eq!(
                listing_details
                    .iter()
                    .map(|entry| entry.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["Blade", "Scout"]
            );
        }
        _ => unreachable!("ranking_packet only returns Rankings"),
    }

    let online = session.handle_packet(ClientPacket::GetRanking {
        rank_type: 0,
        rank_index: 0,
        online_only: true,
    });
    match ranking_packet(&online) {
        ServerPacket::Rankings {
            my_rank,
            listing_details,
            count,
            ..
        } => {
            assert_eq!(*count, 1);
            assert_eq!(*my_rank, 4);
            assert_eq!(listing_details[0].name, "Scout");
        }
        _ => unreachable!("ranking_packet only returns Rankings"),
    }
}
