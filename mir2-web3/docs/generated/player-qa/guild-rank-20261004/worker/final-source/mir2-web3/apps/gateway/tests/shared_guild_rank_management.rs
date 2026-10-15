//! Isolated prepared guilds plus ordinary authenticated typed packets.
//! These API checks do not accept native pending/status, AOI ranks, TCP/WSS or guild creation.
use mir2_gateway::{GatewayConfig, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRuntimeFactory};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, AccountStoreTransactionFault, CharacterRecord, CharacterSaveRecord,
    SharedGuildMember, SharedGuildRank, SharedGuildRecord, Stage5FriendIdentity, WorldCommand,
    ZoneRuntimeHandle,
};
use std::collections::BTreeMap;

const GUILD_ID: &str = "1123456789abcdef0123456789abcdef";
const OTHER_GUILD_ID: &str = "2123456789abcdef0123456789abcdef";

fn identity(account_id: &str, character_index: i32) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: account_id.into(),
        character_index,
    }
}

fn fixture() -> GatewayConfig {
    let config = GatewayConfig::default().with_crystal_world_runtime();
    let mut store = config.account_store.lock().unwrap();
    store.accounts.clear();
    for (index, account_id, name) in [
        (0, "leader", "Leader"),
        (1, "officer", "Officer"),
        (2, "member", "Member"),
        (3, "outsider", "Outsider"),
        (4, "other-leader", "Other Leader"),
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
    let guild = SharedGuildRecord {
        id: GUILD_ID.into(),
        name: "Rank Guild".into(),
        revision: 7,
        level: 0,
        experience: 41,
        spare_points: 2,
        gold: 99_123,
        ranks: vec![
            SharedGuildRank {
                index: 0,
                name: "Leader".into(),
                options: 255,
            },
            SharedGuildRank {
                index: 1,
                name: "Officer".into(),
                options: 1,
            },
            SharedGuildRank {
                index: 2,
                name: "Members".into(),
                options: 0,
            },
        ],
        members: vec![
            SharedGuildMember {
                membership_epoch: 4,
                identity: identity("leader", 0),
                name: "Leader".into(),
                rank_index: 0,
            },
            SharedGuildMember {
                membership_epoch: 5,
                identity: identity("officer", 1),
                name: "Officer".into(),
                rank_index: 1,
            },
            SharedGuildMember {
                membership_epoch: 6,
                identity: identity("member", 2),
                name: "Member".into(),
                rank_index: 2,
            },
        ],
        notice: vec!["unchanged notice".into()],
        storage: BTreeMap::new(),
        buffs: BTreeMap::new(),
        last_buff_tick_ms: 0,
        experience_receipts: Default::default(),
        experience_receipt_payloads: Default::default(),
    };
    let mut other = guild.clone();
    other.id = OTHER_GUILD_ID.into();
    other.name = "Other Guild".into();
    other.members = vec![SharedGuildMember {
        membership_epoch: 2,
        identity: identity("other-leader", 4),
        name: "Other Leader".into(),
        rank_index: 0,
    }];
    store.shared_guilds.insert(GUILD_ID.into(), guild);
    store.shared_guilds.insert(OTHER_GUILD_ID.into(), other);
    drop(store);
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

fn edit(rank_index: u8, rank_name: &str) -> ClientPacket {
    ClientPacket::EditGuildMember {
        change_type: 3,
        rank_index,
        // This field must never supply the actor identity or display name.
        name: "Other Leader".into(),
        rank_name: rank_name.into(),
    }
}

fn persisted(config: &GatewayConfig) -> SharedGuildRecord {
    config.account_store.lock().unwrap().shared_guilds[GUILD_ID].clone()
}

fn has_rank(packets: &[ServerPacket], index: u8, name: &str) -> bool {
    packets.iter().any(|p| {
        matches!(p, ServerPacket::GuildMemberChange { ranks, .. }
        if ranks.iter().any(|rank| rank.index == i32::from(index) && rank.name == name))
    })
}

fn source_rank_event(packets: &[ServerPacket], index: u8, name: &str) -> bool {
    packets.iter().any(|p| matches!(p, ServerPacket::GuildMemberChange { status: 7, name: actor, ranks, .. }
        if actor == "Leader" && ranks.len() == 1 && ranks[0].index == i32::from(index) && ranks[0].name == name))
}

fn assert_only_rename(
    before: &SharedGuildRecord,
    after: &SharedGuildRecord,
    index: u8,
    name: &str,
) {
    let mut expected = before.clone();
    expected
        .ranks
        .iter_mut()
        .find(|rank| rank.index == index)
        .unwrap()
        .name = name.into();
    expected.revision += 1;
    assert_eq!(after, &expected);
}

#[test]
fn ordinary_rank_rename_commits_and_reaches_only_members_across_zones_then_relogin() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("rank-a"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("rank-b"));
    let mut outsider = factory.create_runtime(config.clone(), &ZoneId::new("rank-b"));
    let mut foreign = factory.create_runtime(config.clone(), &ZoneId::new("rank-b"));
    login(&mut leader, "leader", 0);
    login(&mut member, "member", 2);
    login(&mut outsider, "outsider", 3);
    login(&mut foreign, "other-leader", 4);
    let before = persisted(&config);
    let other_before = config.account_store.lock().unwrap().shared_guilds[OTHER_GUILD_ID].clone();
    let reply = send(&mut leader, edit(2, "Veterans"));
    assert_only_rename(&before, &persisted(&config), 2, "Veterans");
    // Existing routing replaces the actor terminal event with a full canonical roster.
    // This assertion deliberately does not claim that native pending completion is fixed.
    assert!(
        has_rank(&reply, 2, "Veterans"),
        "committed actor projection missing: {reply:?}"
    );
    let seen = member.execute(WorldCommand::Tick).unwrap();
    assert!(
        source_rank_event(&seen, 2, "Veterans"),
        "committed member event missing: {seen:?}"
    );
    assert!(seen.iter().any(|p| matches!(p, ServerPacket::GuildStatus { guild_rank_name, .. } if guild_rank_name == "Veterans")));
    assert!(!has_rank(
        &outsider.execute(WorldCommand::Tick).unwrap(),
        2,
        "Veterans"
    ));
    assert!(!has_rank(
        &foreign.execute(WorldCommand::Tick).unwrap(),
        2,
        "Veterans"
    ));
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[OTHER_GUILD_ID],
        other_before
    );
    send(&mut member, ClientPacket::LogOut);
    drop(member);
    let mut rejoined = factory.create_runtime(config.clone(), &ZoneId::new("rank-c"));
    login(&mut rejoined, "member", 2);
    let packets = send(
        &mut rejoined,
        ClientPacket::RequestGuildInfo { info_type: 1 },
    );
    assert!(has_rank(&packets, 2, "Veterans"));
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::GuildStatus { guild_rank_name, .. } if guild_rank_name == "Veterans")));
    assert_only_rename(&before, &persisted(&config), 2, "Veterans");
}

#[test]
fn officer_can_rename_equal_and_lower_ranks_but_not_superior() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut officer = factory.create_runtime(config.clone(), &ZoneId::new("rank-hierarchy"));
    login(&mut officer, "officer", 1);
    let before = persisted(&config);
    send(&mut officer, edit(1, "Sergeants"));
    assert_only_rename(&before, &persisted(&config), 1, "Sergeants");
    let before = persisted(&config);
    send(&mut officer, edit(2, "Recruits"));
    assert_only_rename(&before, &persisted(&config), 2, "Recruits");
    let before = persisted(&config);
    let denied = send(&mut officer, edit(0, "Usurpers"));
    assert_eq!(persisted(&config), before);
    assert!(!has_rank(&denied, 0, "Usurpers"));
}

#[test]
fn permissions_and_utf16_boundaries_are_authoritative_without_other_mutations() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("rank-limits"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("rank-limits"));
    let mut outsider = factory.create_runtime(config.clone(), &ZoneId::new("rank-limits"));
    login(&mut leader, "leader", 0);
    login(&mut member, "member", 2);
    login(&mut outsider, "outsider", 3);
    let before = persisted(&config);
    for actor in [&mut member, &mut outsider] {
        assert!(!has_rank(
            &send(actor, edit(2, "Forbidden")),
            2,
            "Forbidden"
        ));
        assert_eq!(persisted(&config), before);
    }
    for invalid in [
        String::new(),
        "ab".into(),
        "a\\b".into(),
        "x".repeat(21),
        "😀".repeat(11),
    ] {
        send(&mut leader, edit(2, &invalid));
        assert_eq!(
            persisted(&config),
            before,
            "invalid UTF-16/backslash value was accepted: {invalid:?}"
        );
    }
    send(&mut leader, edit(255, "Unknown"));
    assert_eq!(persisted(&config), before);
    for valid in ["abc".to_string(), "😀a".to_string(), "😀".repeat(10)] {
        let before = persisted(&config);
        send(&mut leader, edit(2, &valid));
        assert_only_rename(&before, &persisted(&config), 2, &valid);
    }
}

#[test]
fn unauthenticated_and_pre_startgame_rank_packets_cannot_mutate() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut runtime = factory.create_runtime(config.clone(), &ZoneId::new("rank-auth"));
    let before = persisted(&config);
    let _ = runtime.execute(WorldCommand::ClientPacket(edit(2, "Anonymous")));
    assert_eq!(persisted(&config), before);
    send(
        &mut runtime,
        ClientPacket::Login {
            account_id: "leader".into(),
            password: "demo".into(),
        },
    );
    let _ = runtime.execute(WorldCommand::ClientPacket(edit(2, "TooEarly")));
    assert_eq!(persisted(&config), before);
}

#[test]
fn every_effective_repeated_name_and_a_b_a_request_advances_revision() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("rank-repeat-a"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("rank-repeat-b"));
    login(&mut leader, "leader", 0);
    login(&mut member, "member", 2);
    for name in ["Alpha", "Alpha", "Bravo", "Alpha"] {
        let before = persisted(&config);
        send(&mut leader, edit(2, name));
        assert_only_rename(&before, &persisted(&config), 2, name);
        assert!(source_rank_event(
            &member.execute(WorldCommand::Tick).unwrap(),
            2,
            name
        ));
    }
}

#[test]
fn known_commit_failure_has_no_success_or_peer_event_and_retry_commits_once() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("rank-failure-a"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("rank-failure-b"));
    login(&mut leader, "leader", 0);
    login(&mut member, "member", 2);
    let before = persisted(&config);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    let rejected = send(&mut leader, edit(2, "Committed"));
    assert_eq!(persisted(&config), before);
    assert!(!has_rank(&rejected, 2, "Committed"));
    assert!(!source_rank_event(
        &member.execute(WorldCommand::Tick).unwrap(),
        2,
        "Committed"
    ));
    send(&mut leader, edit(2, "Committed"));
    assert_only_rename(&before, &persisted(&config), 2, "Committed");
    assert!(source_rank_event(
        &member.execute(WorldCommand::Tick).unwrap(),
        2,
        "Committed"
    ));
}

#[test]
fn durable_current_rank_and_membership_override_cached_login_permission() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut officer = factory.create_runtime(config.clone(), &ZoneId::new("rank-recheck"));
    login(&mut officer, "officer", 1);
    // Fixture-only canonical changes stand in for a separately owned management operation.
    // This is permission revalidation evidence, not ordinary promotion/leave acceptance.
    config
        .account_store
        .lock()
        .unwrap()
        .shared_guilds
        .get_mut(GUILD_ID)
        .unwrap()
        .ranks[1]
        .options = 0;
    let before = persisted(&config);
    send(&mut officer, edit(2, "OldPermission"));
    assert_eq!(persisted(&config), before);
    {
        let mut store = config.account_store.lock().unwrap();
        let guild = store.shared_guilds.get_mut(GUILD_ID).unwrap();
        guild.ranks[1].options = 1;
        guild
            .members
            .retain(|member| member.identity != identity("officer", 1));
    }
    let before = persisted(&config);
    send(&mut officer, edit(2, "OldMembership"));
    assert_eq!(persisted(&config), before);
}

#[test]
fn file_reopen_and_new_factory_keep_exact_rank_revision_and_membership_epochs() {
    let config = fixture();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mir2-rank-rename-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("accounts.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&*config.account_store.lock().unwrap()).unwrap(),
    )
    .unwrap();
    let config = config.with_account_store_path(path.clone());
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("rank-file-a"));
    login(&mut leader, "leader", 0);
    let before = persisted(&config);
    let desired = "資深😀隊員";
    send(&mut leader, edit(2, desired));
    assert_only_rename(&before, &persisted(&config), 2, desired);
    send(&mut leader, ClientPacket::LogOut);
    let after = persisted(&config);
    drop(leader);
    drop(factory);
    drop(config);
    let reopened = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_account_store_path(path);
    assert_eq!(persisted(&reopened), after);
    let fresh_factory = SharedInProcessZoneRuntimeFactory::new();
    let mut member = fresh_factory.create_runtime(reopened.clone(), &ZoneId::new("rank-file-b"));
    login(&mut member, "member", 2);
    assert!(has_rank(
        &send(&mut member, ClientPacket::RequestGuildInfo { info_type: 1 }),
        2,
        desired
    ));
    assert_eq!(persisted(&reopened), after);
    // The isolated file is retained for failure inspection; no user store is used.
}
