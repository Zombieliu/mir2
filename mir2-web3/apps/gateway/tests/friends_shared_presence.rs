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

fn friends(
    runtime: &mut ZoneRuntimeHandle,
    packet: ClientPacket,
) -> Vec<mir2_protocol::ClientFriend> {
    runtime
        .execute(WorldCommand::ClientPacket(packet))
        .unwrap()
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::FriendUpdate { friends } => Some(friends),
            _ => None,
        })
        .expect("friend update")
}

#[test]
fn friend_online_state_tracks_other_zone_login_logout_and_blocked_identity() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut owner = factory.create_runtime(config.clone(), &ZoneId::new("friend-zone-a"));
    let mut peer = factory.create_runtime(config.clone(), &ZoneId::new("friend-zone-b"));
    login(&mut owner, "demo", 0);
    let entries = friends(
        &mut owner,
        ClientPacket::AddFriend {
            name: "Peer".into(),
            blocked: true,
        },
    );
    assert_eq!(entries[0].index, 1);
    assert!(entries[0].blocked && !entries[0].online);
    login(&mut peer, "peer", 1);
    let entries = friends(&mut owner, ClientPacket::RefreshFriends);
    assert!(entries[0].blocked && entries[0].online);
    friends(
        &mut owner,
        ClientPacket::AddMemo {
            character_index: 1,
            memo: "Cross zone".into(),
        },
    );
    peer.execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    assert!(!friends(&mut owner, ClientPacket::RefreshFriends)[0].online);
    peer.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
        character_index: 1,
    }))
    .unwrap();
    assert!(friends(&mut owner, ClientPacket::RefreshFriends)[0].online);
    owner
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    let entries = friends(&mut owner, ClientPacket::StartGame { character_index: 0 });
    assert!(entries[0].online && entries[0].blocked);
    assert_eq!(entries[0].memo, "Cross zone");
    drop(peer);
    assert!(!friends(&mut owner, ClientPacket::RefreshFriends)[0].online);
    assert!(friends(
        &mut owner,
        ClientPacket::RemoveFriend { character_index: 1 }
    )
    .is_empty());
}
