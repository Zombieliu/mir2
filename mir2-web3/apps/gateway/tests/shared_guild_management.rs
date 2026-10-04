//! Prepared guilds exercise ordinary packets; this is not NPC creation or native UI acceptance.
use mir2_gateway::{GatewayConfig, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRuntimeFactory};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, AccountStoreTransactionFault, CharacterRecord, CharacterSaveRecord,
    SharedGuildMember, SharedGuildRank, SharedGuildRecord, Stage5FriendIdentity, WorldCommand,
    ZoneRuntimeHandle,
};
use std::collections::BTreeMap;

const GUILD_ID: &str = "1123456789abcdef0123456789abcdef";

fn identity(account_id: &str, character_index: i32) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: account_id.into(),
        character_index,
    }
}

fn fixture() -> GatewayConfig {
    let config = GatewayConfig::default().with_crystal_world_runtime();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for (index, account_id, name) in [
            (0, "leader", "Leader"),
            (1, "member", "Member"),
            (2, "outsider", "Outsider"),
        ] {
            let mut account = AccountRecord::empty();
            let character = CharacterRecord {
                index,
                name: name.into(),
                level: 22,
                class: MirClass::Warrior,
                gender: MirGender::Male,
            };
            let mut save = CharacterSaveRecord::new(character.clone());
            save.map_file_name = config.map.file_name.clone();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            account.characters.push(character);
            account.saves.insert(index, save);
            store.accounts.insert(account_id.into(), account);
        }
        store.shared_guilds.insert(
            GUILD_ID.into(),
            SharedGuildRecord {
                id: GUILD_ID.into(),
                name: "Notice Guild".into(),
                revision: 1,
                level: 0,
                experience: 0,
                spare_points: 0,
                gold: 0,
                ranks: vec![
                    SharedGuildRank {
                        index: 0,
                        name: "Leader".into(),
                        options: 255,
                    },
                    SharedGuildRank {
                        index: 1,
                        name: "Members".into(),
                        options: 0,
                    },
                ],
                members: vec![
                    SharedGuildMember {
                        membership_epoch: 1,
                        identity: identity("leader", 0),
                        name: "Leader".into(),
                        rank_index: 0,
                    },
                    SharedGuildMember {
                        membership_epoch: 1,
                        identity: identity("member", 1),
                        name: "Member".into(),
                        rank_index: 1,
                    },
                ],
                notice: vec!["old notice".into()],
                storage: BTreeMap::new(),
                buffs: BTreeMap::new(),
                last_buff_tick_ms: 0,
                experience_receipts: Default::default(),
                experience_receipt_payloads: Default::default(),
            },
        );
    }
    config
}

fn send(runtime: &mut ZoneRuntimeHandle, packet: ClientPacket) -> Vec<ServerPacket> {
    runtime.execute(WorldCommand::ClientPacket(packet)).unwrap()
}

fn login(runtime: &mut ZoneRuntimeHandle, account: &str, index: i32) {
    assert!(send(
        runtime,
        ClientPacket::Login {
            account_id: account.into(),
            password: "demo".into()
        }
    )
    .iter()
    .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    send(
        runtime,
        ClientPacket::StartGame {
            character_index: index,
        },
    );
    assert!(runtime.active_identity().is_some());
}

fn persisted(config: &GatewayConfig) -> SharedGuildRecord {
    config
        .shared_guild_for_identity(&identity("leader", 0))
        .unwrap()
        .unwrap()
}

fn has_invalidation(packets: &[ServerPacket]) -> bool {
    packets.iter().any(|p| matches!(p, ServerPacket::GuildNoticeChange { update: -1, notice } if notice.is_empty()))
}

fn has_notice(packets: &[ServerPacket], expected: &[String]) -> bool {
    packets.iter().any(|p| matches!(p, ServerPacket::GuildNoticeChange { update, notice } if *update >= 0 && notice == expected))
}

#[test]
fn ordinary_notice_edit_is_authoritative_cross_zone_and_survives_relogin() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("notice-a"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("notice-b"));
    let mut outsider = factory.create_runtime(config.clone(), &ZoneId::new("notice-b"));
    login(&mut leader, "leader", 0);
    login(&mut member, "member", 1);
    login(&mut outsider, "outsider", 2);
    let notice = vec![
        "行會公告 — 今日集合".into(),
        "Weekly hunt: sábado".into(),
        String::new(),
    ];
    let reply = send(
        &mut leader,
        ClientPacket::EditGuildNotice {
            notice: notice.clone(),
        },
    );
    assert!(
        has_invalidation(&reply),
        "source terminal reply missing: {reply:?}"
    );
    assert!(
        has_notice(&reply, &notice),
        "native authoritative content missing: {reply:?}"
    );
    let seen = member.execute(WorldCommand::Tick).unwrap();
    assert!(has_invalidation(&seen));
    assert!(has_notice(&seen, &notice));
    assert!(!outsider
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::GuildNoticeChange { .. })));
    let after = persisted(&config);
    assert_eq!(after.notice, notice);
    assert_eq!(after.revision, 2);
    send(&mut leader, ClientPacket::LogOut);
    send(&mut member, ClientPacket::LogOut);
    send(&mut outsider, ClientPacket::LogOut);
    drop(leader);
    drop(member);
    drop(outsider);
    let mut rejoined = factory.create_runtime(config.clone(), &ZoneId::new("notice-c"));
    login(&mut rejoined, "member", 1);
    let fresh = send(
        &mut rejoined,
        ClientPacket::RequestGuildInfo { info_type: 0 },
    );
    assert!(has_notice(&fresh, &notice));
    assert_eq!(persisted(&config), after);
}

#[test]
fn notice_permission_membership_and_200_line_boundary_are_checked_without_mutation() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("notice-permissions"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("notice-permissions"));
    let mut outsider = factory.create_runtime(config.clone(), &ZoneId::new("notice-permissions"));
    login(&mut leader, "leader", 0);
    login(&mut member, "member", 1);
    login(&mut outsider, "outsider", 2);
    let before = persisted(&config);
    for runtime in [&mut member, &mut outsider] {
        assert!(!has_invalidation(&send(
            runtime,
            ClientPacket::EditGuildNotice {
                notice: vec!["unauthorized".into()]
            }
        )));
        assert_eq!(persisted(&config), before);
    }
    let excessive = vec!["too many".into(); 201];
    assert!(!has_invalidation(&send(
        &mut leader,
        ClientPacket::EditGuildNotice { notice: excessive }
    )));
    assert_eq!(persisted(&config), before);
    let limit = vec!["boundary".into(); 200];
    assert!(has_invalidation(&send(
        &mut leader,
        ClientPacket::EditGuildNotice {
            notice: limit.clone()
        }
    )));
    assert_eq!(persisted(&config).notice, limit);
    assert!(has_invalidation(&send(
        &mut leader,
        ClientPacket::EditGuildNotice { notice: vec![] }
    )));
    assert!(persisted(&config).notice.is_empty());
}

#[test]
fn notice_known_save_failure_has_no_success_or_broadcast_and_retry_commits_once() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("notice-failure-a"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("notice-failure-b"));
    login(&mut leader, "leader", 0);
    login(&mut member, "member", 1);
    let before = persisted(&config);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    let edit = ClientPacket::EditGuildNotice {
        notice: vec!["committed only".into()],
    };
    let denied = send(&mut leader, edit.clone());
    assert!(!has_invalidation(&denied));
    assert!(!has_notice(&denied, &["committed only".into()]));
    assert_eq!(persisted(&config), before);
    assert!(!member
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::GuildNoticeChange { .. })));
    let accepted = send(&mut leader, edit);
    assert!(has_invalidation(&accepted));
    let after = persisted(&config);
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.notice, vec!["committed only"]);
    assert!(has_notice(
        &member.execute(WorldCommand::Tick).unwrap(),
        &after.notice
    ));
}

#[test]
fn notice_file_commit_reopens_with_exact_content_and_revision() {
    let config = fixture();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("mir2-guild-notice-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("accounts.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&*config.account_store.lock().unwrap()).unwrap(),
    )
    .unwrap();
    let config = config.with_account_store_path(path.clone());
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("notice-file"));
    login(&mut leader, "leader", 0);
    let notice = vec![
        "繁體中文".into(),
        "Português brasileiro".into(),
        "Русский".into(),
        String::new(),
    ];
    assert!(has_invalidation(&send(
        &mut leader,
        ClientPacket::EditGuildNotice {
            notice: notice.clone()
        }
    )));
    let after = persisted(&config);
    send(&mut leader, ClientPacket::LogOut);
    drop(leader);
    drop(config);
    drop(factory);
    let reopened = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_account_store_path(path);
    assert_eq!(persisted(&reopened), after);
    assert_eq!(after.notice, notice);
}
