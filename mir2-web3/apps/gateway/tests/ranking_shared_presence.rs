use mir2_gateway::{GatewayConfig, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRuntimeFactory};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, WorldCommand, ZoneRuntimeHandle,
};

fn fixture() -> GatewayConfig {
    let config = GatewayConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for (index, account_id, name) in [
            (0, "demo", "Scout"),
            (1, "peer", "Peer"),
            (2, "offline", "Offline"),
        ] {
            let mut account = AccountRecord::empty();
            account.characters.push(CharacterRecord {
                index,
                name: name.into(),
                level: 10 + index as u16,
                class: MirClass::Warrior,
                gender: MirGender::Male,
            });
            let mut save = CharacterSaveRecord::new(account.characters[0].clone());
            save.map_file_name = config.map.file_name.clone();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            account.saves.insert(index, save);
            store.accounts.insert(account_id.into(), account);
        }
    }
    config
}

fn login(runtime: &mut ZoneRuntimeHandle, account: &str, index: i32) {
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: account.into(),
            password: "demo".into(),
        }))
        .unwrap();
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap();
    assert!(runtime.active_identity().is_some());
}

fn names(runtime: &mut ZoneRuntimeHandle, online_only: bool) -> Vec<String> {
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::GetRanking {
            rank_type: 0,
            rank_index: 0,
            online_only,
        }))
        .unwrap()
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::Rankings {
                listing_details, ..
            } => Some(
                listing_details
                    .into_iter()
                    .map(|entry| entry.name)
                    .collect(),
            ),
            _ => None,
        })
        .expect("ranking reply")
}

#[test]
fn ranking_tracks_other_zone_join_logout_and_disconnect_without_hiding_offline_rankings() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut owner = factory.create_runtime(config.clone(), &ZoneId::new("ranking-zone-a"));
    let mut peer = factory.create_runtime(config.clone(), &ZoneId::new("ranking-zone-b"));
    login(&mut owner, "demo", 0);
    assert_eq!(names(&mut owner, true), ["Scout"]);
    login(&mut peer, "peer", 1);
    assert_eq!(names(&mut owner, true), ["Peer", "Scout"]);
    assert_eq!(names(&mut owner, false), ["Offline", "Peer", "Scout"]);
    peer.execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    assert_eq!(names(&mut owner, true), ["Scout"]);
    peer.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
        character_index: 1,
    }))
    .unwrap();
    assert_eq!(names(&mut owner, true), ["Peer", "Scout"]);
    drop(peer);
    assert_eq!(names(&mut owner, true), ["Scout"]);
    assert_eq!(names(&mut owner, false), ["Offline", "Peer", "Scout"]);
}
