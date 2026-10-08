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
        active_wars: Default::default(),
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


// Ordinary typed packets through the production wrapper; no injected success.
#[test]
fn guild_rank_actor_receives_original_status7_after_canonical_refresh() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("terminal-a"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("terminal-b"));
    let mut outsider = factory.create_runtime(config.clone(), &ZoneId::new("terminal-b"));
    login(&mut leader, "leader", 0);
    login(&mut member, "member", 2);
    login(&mut outsider, "outsider", 3);
    let before = persisted(&config);
    let reply = send(&mut leader, edit(2, "Veterans"));
    assert_only_rename(&before, &persisted(&config), 2, "Veterans");
    let terminals = reply.iter().filter(|p| matches!(p,
        ServerPacket::GuildMemberChange { status: 7, .. })).collect::<Vec<_>>();
    assert_eq!(terminals.len(), 1, "full projection is not the source terminal: {reply:?}");
    assert!(matches!(terminals[0], ServerPacket::GuildMemberChange {
        name, rank_index: 0, status: 7, ranks
    } if name == "Leader" && ranks.len() == 1 && ranks[0].index == 2
        && ranks[0].name == "Veterans" && ranks[0].options == 0
        && ranks[0].members.len() == 1 && ranks[0].members[0].name == "Member"));
    let terminal_index = reply.iter().position(|p| matches!(p,
        ServerPacket::GuildMemberChange { status: 7, .. })).unwrap();
    let canonical_index = reply.iter().position(|p| matches!(p,
        ServerPacket::GuildMemberChange { status: 255, .. })).unwrap();
    assert!(terminal_index < canonical_index,
        "source terminal must enter the actual native FIFO before full metadata: {reply:?}");
    assert!(source_rank_event(&member.execute(WorldCommand::Tick).unwrap(), 2, "Veterans"));
    assert!(!has_rank(&outsider.execute(WorldCommand::Tick).unwrap(), 2, "Veterans"));
}

#[test]
fn guild_rank_repeated_same_name_has_source_terminal_for_each_valid_request() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("terminal-repeat"));
    login(&mut leader, "leader", 0);
    let before = persisted(&config);
    for offset in 1..=2 {
        let reply = send(&mut leader, edit(2, "Members"));
        assert_eq!(persisted(&config).revision, before.revision + offset);
        assert!(source_rank_event(&reply, 2, "Members"), "same name is still a source reply: {reply:?}");
    }
}

#[test]
fn guild_rank_denied_request_has_no_success_terminal_or_durable_change() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("terminal-denied"));
    login(&mut member, "member", 2);
    let before = persisted(&config);
    let reply = send(&mut member, edit(2, "Forbidden"));
    assert_eq!(persisted(&config), before);
    assert!(!reply.iter().any(|p| matches!(p, ServerPacket::GuildMemberChange { status: 7, .. })));
}
