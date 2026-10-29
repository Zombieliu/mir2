use mir2_gateway::{GatewayConfig, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRuntimeFactory};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, AccountStoreTransactionFault, CharacterRecord, CharacterSaveRecord,
    Stage5FriendIdentity, WorldCommand, ZoneRuntimeHandle,
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
                level: 10 + index as u16 * 15,
                class: MirClass::Warrior,
                gender: MirGender::Male,
            });
            let mut save = CharacterSaveRecord::new(account.characters[0].clone());
            save.map_file_name = config.map.file_name.clone();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            save.position.x += index;
            save.direction = if index == 0 {
                MirDirection::Right
            } else {
                MirDirection::Left
            };
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

fn id(account: &str, index: i32) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: account.into(),
        character_index: index,
    }
}
fn command(runtime: &mut ZoneRuntimeHandle, packet: ClientPacket) -> Vec<ServerPacket> {
    runtime.execute(WorldCommand::ClientPacket(packet)).unwrap()
}

fn spouses(
    separate_zones: bool,
) -> (
    GatewayConfig,
    SharedInProcessZoneRuntimeFactory,
    ZoneRuntimeHandle,
    ZoneRuntimeHandle,
) {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut a = factory.create_runtime(config.clone(), &ZoneId::new("marriage-a"));
    let mut b = factory.create_runtime(
        config.clone(),
        &ZoneId::new(if separate_zones {
            "marriage-b"
        } else {
            "marriage-a"
        }),
    );
    login(&mut a, "demo", 0);
    login(&mut b, "peer", 1);
    command(&mut a, ClientPacket::ChangeMarriage);
    command(&mut b, ClientPacket::ChangeMarriage);
    (config, factory, a, b)
}
fn proposal(a: &mut ZoneRuntimeHandle, b: &mut ZoneRuntimeHandle, divorce: bool) {
    command(
        a,
        if divorce {
            ClientPacket::DivorceRequest
        } else {
            ClientPacket::MarriageRequest
        },
    );
    let packets = b.execute(WorldCommand::Tick).unwrap();
    assert!(
        packets.iter().any(|p| if divorce {
            matches!(p,ServerPacket::DivorceRequest{name} if name=="Scout")
        } else {
            matches!(p,ServerPacket::MarriageRequest{name} if name=="Scout")
        }),
        "{packets:?}"
    );
}
#[test]
fn marriage_requires_same_zone_facing_and_allowed_live_partner() {
    let (config, _factory, mut a, mut b) = spouses(true);
    command(&mut a, ClientPacket::MarriageRequest);
    assert!(!b
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::MarriageRequest { .. })));
    assert!(config
        .shared_marriage_profile_for(&id("demo", 0))
        .unwrap()
        .relationship
        .partner_identity
        .is_none());
    let (_config, _factory, mut a, mut b) = spouses(false);
    command(&mut b, ClientPacket::ChangeMarriage);
    command(&mut a, ClientPacket::MarriageRequest);
    assert!(!b
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::MarriageRequest { .. })));
    command(&mut b, ClientPacket::ChangeMarriage);
    command(
        &mut b,
        ClientPacket::Turn {
            direction: MirDirection::Up,
        },
    );
    command(&mut a, ClientPacket::MarriageRequest);
    assert!(!b
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::MarriageRequest { .. })));
}
#[test]
fn marriage_refusal_accept_permission_divorce_and_reload_are_bilateral() {
    let (config, _factory, mut a, mut b) = spouses(false);
    proposal(&mut a, &mut b, false);
    command(
        &mut b,
        ClientPacket::MarriageReply {
            accept_invite: false,
        },
    );
    assert!(config
        .shared_marriage_profile_for(&id("demo", 0))
        .unwrap()
        .relationship
        .partner_identity
        .is_none());
    proposal(&mut a, &mut b, false);
    let result = command(
        &mut b,
        ClientPacket::MarriageReply {
            accept_invite: true,
        },
    );
    assert!(
        result
            .iter()
            .any(|p| matches!(p,ServerPacket::LoverUpdate{name,..} if name=="Scout")),
        "{result:?}"
    );
    let update = a.execute(WorldCommand::Tick).unwrap();
    assert!(
        update
            .iter()
            .any(|p| matches!(p,ServerPacket::LoverUpdate{name,..} if name=="Peer")),
        "{update:?}"
    );
    command(&mut a, ClientPacket::ChangeMarriage);
    let state = config
        .shared_marriage_profile_for(&id("demo", 0))
        .unwrap()
        .relationship;
    assert!(state.allow_lover_recall && state.allow_marriage);
    proposal(&mut a, &mut b, true);
    command(
        &mut b,
        ClientPacket::DivorceReply {
            accept_invite: false,
        },
    );
    assert!(config
        .shared_marriage_profile_for(&id("demo", 0))
        .unwrap()
        .relationship
        .partner_identity
        .is_some());
    proposal(&mut a, &mut b, true);
    command(
        &mut b,
        ClientPacket::DivorceReply {
            accept_invite: true,
        },
    );
    for key in [id("demo", 0), id("peer", 1)] {
        let state = config
            .shared_marriage_profile_for(&key)
            .unwrap()
            .relationship;
        assert!(state.partner_identity.is_none() && state.cooldown_until_ms > 0);
    }
    command(&mut a, ClientPacket::LogOut);
    command(&mut a, ClientPacket::StartGame { character_index: 0 });
    assert!(config
        .shared_marriage_profile_for(&id("demo", 0))
        .unwrap()
        .relationship
        .partner_identity
        .is_none());
    command(&mut a, ClientPacket::MarriageRequest);
    assert!(!b
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::MarriageRequest { .. })));
}
#[test]
fn marriage_failed_commit_and_disconnected_source_do_not_create_one_sided_state() {
    let (config, _factory, mut a, mut b) = spouses(false);
    proposal(&mut a, &mut b, false);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    command(
        &mut b,
        ClientPacket::MarriageReply {
            accept_invite: true,
        },
    );
    for key in [id("demo", 0), id("peer", 1)] {
        assert!(config
            .shared_marriage_profile_for(&key)
            .unwrap()
            .relationship
            .partner_identity
            .is_none());
    }
    drop(a);
    command(
        &mut b,
        ClientPacket::MarriageReply {
            accept_invite: true,
        },
    );
    assert!(config
        .shared_marriage_profile_for(&id("peer", 1))
        .unwrap()
        .relationship
        .partner_identity
        .is_none());
}
