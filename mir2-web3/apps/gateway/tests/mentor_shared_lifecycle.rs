use mir2_gateway::{GatewayConfig, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRuntimeFactory};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
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
fn pair() -> (
    GatewayConfig,
    SharedInProcessZoneRuntimeFactory,
    ZoneRuntimeHandle,
    ZoneRuntimeHandle,
) {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut pupil = factory.create_runtime(config.clone(), &ZoneId::new("mentor-a"));
    let mut teacher = factory.create_runtime(config.clone(), &ZoneId::new("mentor-b"));
    login(&mut pupil, "demo", 0);
    login(&mut teacher, "peer", 1);
    command(&mut teacher, ClientPacket::AllowMentor);
    (config, factory, pupil, teacher)
}
fn request(pupil: &mut ZoneRuntimeHandle, teacher: &mut ZoneRuntimeHandle) {
    let sent = command(
        pupil,
        ClientPacket::AddMentor {
            name: "Peer".into(),
        },
    );
    assert!(
        !sent
            .iter()
            .any(|p| matches!(p, ServerPacket::MentorRequest { .. })),
        "request belongs to recipient"
    );
    let packets = teacher.execute(WorldCommand::Tick).unwrap();
    assert!(
        packets
            .iter()
            .any(|p| matches!(p,ServerPacket::MentorRequest{name,level:10} if name=="Scout")),
        "{packets:?}"
    );
}
#[test]
fn mentorship_cross_zone_refusal_accept_cancel_and_cooldown() {
    let (config, _factory, mut pupil, mut teacher) = pair();
    request(&mut pupil, &mut teacher);
    command(
        &mut teacher,
        ClientPacket::MentorReply {
            accept_invite: false,
        },
    );
    assert!(config
        .shared_mentor_profile_for(&id("demo", 0))
        .unwrap()
        .mentor
        .partner_identity
        .is_none());
    request(&mut pupil, &mut teacher);
    let accepted = command(
        &mut teacher,
        ClientPacket::MentorReply {
            accept_invite: true,
        },
    );
    assert!(accepted.iter().any(|p|matches!(p,ServerPacket::MentorUpdate{name,level:10,online:true,..} if name=="Scout")),"{accepted:?}");
    let update = pupil.execute(WorldCommand::Tick).unwrap();
    assert!(
        update.iter().any(
            |p| matches!(p,ServerPacket::MentorUpdate{name,level:25,online:true,..} if name=="Peer")
        ),
        "{update:?}"
    );
    assert!(
        config
            .shared_mentor_profile_for(&id("peer", 1))
            .unwrap()
            .mentor
            .is_mentor
    );
    command(&mut pupil, ClientPacket::CancelMentor);
    let student = config
        .shared_mentor_profile_for(&id("demo", 0))
        .unwrap()
        .mentor;
    let mentor = config
        .shared_mentor_profile_for(&id("peer", 1))
        .unwrap()
        .mentor;
    assert!(student.partner_identity.is_none() && mentor.partner_identity.is_none());
    assert!(student.cooldown_until_ms > 0);
    assert_eq!(mentor.cooldown_until_ms, 0);
    command(
        &mut pupil,
        ClientPacket::AddMentor {
            name: "Peer".into(),
        },
    );
    assert!(!teacher
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::MentorRequest { .. })));
}
#[test]
fn mentorship_failed_commit_rolls_back_both_and_ticket_can_retry() {
    let (config, _factory, mut pupil, mut teacher) = pair();
    request(&mut pupil, &mut teacher);
    let before_pupil = config.shared_mentor_profile_for(&id("demo", 0)).unwrap();
    let before_teacher = config.shared_mentor_profile_for(&id("peer", 1)).unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    let failed = command(
        &mut teacher,
        ClientPacket::MentorReply {
            accept_invite: true,
        },
    );
    assert!(!failed
        .iter()
        .any(|p| matches!(p,ServerPacket::MentorUpdate{name,..} if !name.is_empty())));
    assert_eq!(
        config.shared_mentor_profile_for(&id("demo", 0)).unwrap(),
        before_pupil
    );
    assert_eq!(
        config.shared_mentor_profile_for(&id("peer", 1)).unwrap(),
        before_teacher
    );
    command(
        &mut teacher,
        ClientPacket::MentorReply {
            accept_invite: true,
        },
    );
    assert_eq!(
        config
            .shared_mentor_profile_for(&id("demo", 0))
            .unwrap()
            .mentor
            .partner_identity,
        Some(id("peer", 1))
    );
}
#[test]
fn mentorship_permission_and_disconnect_invalidate_old_ticket() {
    let (config, factory, mut pupil, mut teacher) = pair();
    command(&mut teacher, ClientPacket::AllowMentor);
    command(
        &mut pupil,
        ClientPacket::AddMentor {
            name: "Peer".into(),
        },
    );
    assert!(!teacher
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::MentorRequest { .. })));
    command(&mut teacher, ClientPacket::AllowMentor);
    request(&mut pupil, &mut teacher);
    drop(pupil);
    let mut replacement = factory.create_runtime(config.clone(), &ZoneId::new("mentor-c"));
    login(&mut replacement, "demo", 0);
    command(
        &mut teacher,
        ClientPacket::MentorReply {
            accept_invite: true,
        },
    );
    assert!(config
        .shared_mentor_profile_for(&id("peer", 1))
        .unwrap()
        .mentor
        .partner_identity
        .is_none());
    request(&mut replacement, &mut teacher);
    command(
        &mut teacher,
        ClientPacket::MentorReply {
            accept_invite: true,
        },
    );
    command(&mut replacement, ClientPacket::LogOut);
    let update = teacher.execute(WorldCommand::Tick).unwrap();
    assert!(
        update
            .iter()
            .any(|p| matches!(p,ServerPacket::MentorUpdate{online:false,name,..} if name=="Scout")),
        "{update:?}"
    );
    let packets = command(
        &mut replacement,
        ClientPacket::StartGame { character_index: 0 },
    );
    assert!(
        packets
            .iter()
            .any(|p| matches!(p,ServerPacket::MentorUpdate{online:true,name,..} if name=="Peer")),
        "{packets:?}"
    );
}

#[test]
fn mentorship_uses_live_unsaved_teacher_level_for_commit_and_projection() {
    let (config, _factory, mut pupil, mut teacher) = pair();
    // The online teacher has level 25; durable state still predates the level-up.
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("peer").unwrap();
        account.characters[0].level = 10;
        account.saves.get_mut(&1).unwrap().character.level = 10;
    }
    request(&mut pupil, &mut teacher);
    command(
        &mut teacher,
        ClientPacket::MentorReply {
            accept_invite: true,
        },
    );
    let update = pupil.execute(WorldCommand::Tick).unwrap();
    assert!(
        update.iter().any(
            |p| matches!(p,ServerPacket::MentorUpdate{name,level:25,online:true,..} if name=="Peer")
        ),
        "{update:?}"
    );
    assert_eq!(
        config
            .shared_mentor_profile_for(&id("demo", 0))
            .unwrap()
            .mentor
            .level,
        25
    );
}

#[test]
fn mentorship_pending_dialog_cannot_be_replaced_by_another_student() {
    let (config, factory, mut pupil, mut teacher) = pair();
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("offline").unwrap();
        account.characters[0].level = 8;
        account.saves.get_mut(&2).unwrap().character.level = 8;
    }
    let mut other = factory.create_runtime(config.clone(), &ZoneId::new("mentor-other"));
    login(&mut other, "offline", 2);
    request(&mut pupil, &mut teacher);
    command(
        &mut other,
        ClientPacket::AddMentor {
            name: "Peer".into(),
        },
    );
    assert!(!teacher
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::MentorRequest { .. })));
    drop(pupil);
    // Source disconnect leaves a tombstone until the recipient consumes the old dialog.
    command(
        &mut other,
        ClientPacket::AddMentor {
            name: "Peer".into(),
        },
    );
    command(
        &mut teacher,
        ClientPacket::MentorReply {
            accept_invite: true,
        },
    );
    assert!(config
        .shared_mentor_profile_for(&id("peer", 1))
        .unwrap()
        .mentor
        .partner_identity
        .is_none());
    command(
        &mut other,
        ClientPacket::AddMentor {
            name: "Peer".into(),
        },
    );
    let pending = teacher.execute(WorldCommand::Tick).unwrap();
    assert!(
        pending
            .iter()
            .any(|p| matches!(p,ServerPacket::MentorRequest{name,..} if name=="Offline")),
        "{pending:?}"
    );
    command(
        &mut teacher,
        ClientPacket::MentorReply {
            accept_invite: true,
        },
    );
    assert_eq!(
        config
            .shared_mentor_profile_for(&id("peer", 1))
            .unwrap()
            .mentor
            .partner_identity,
        Some(id("offline", 2))
    );
}
