//! Prepared canonical Guilds, ordinary authenticated GuildWarReturn and chat/member
//! packets. These checks do not claim native, socket, public-play or colour acceptance.
use mir2_gateway::{GatewayConfig, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRuntimeFactory};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, AccountStoreTransactionFault, CharacterRecord, CharacterSaveRecord,
    SharedGuildMember, SharedGuildRank, SharedGuildRecord, Stage5FriendIdentity, WorldCommand,
    ZoneRuntimeHandle,
};
use std::collections::BTreeMap;

const OWN: &str = "3123456789abcdef0123456789abcdef";
const ENEMY: &str = "4123456789abcdef0123456789abcdef";
const NEWBIE: &str = "5123456789abcdef0123456789abcdef";

fn war_cost() -> u32 {
    let settings = mir2_game_data::crystal_guild_settings();
    // Current imported GuildSettings.ini overrides Settings.cs's 3000 fallback.
    assert_eq!(
        settings.war_cost, 30_000,
        "configured Crystal war cost changed"
    );
    assert_eq!(
        settings.war_time, 180,
        "configured Crystal war duration changed"
    );
    settings.war_cost
}

fn identity(account: &str, index: i32) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: account.into(),
        character_index: index,
    }
}
fn guild(id: &str, name: &str, account: &str, index: i32, player_name: &str) -> SharedGuildRecord {
    SharedGuildRecord {
        id: id.into(),
        name: name.into(),
        revision: 1,
        level: 0,
        experience: 0,
        spare_points: 0,
        gold: war_cost().checked_add(1_000).unwrap(),
        ranks: vec![
            SharedGuildRank {
                index: 0,
                name: "Leader".into(),
                options: 255,
            },
            SharedGuildRank {
                index: 1,
                name: "Member".into(),
                options: 0,
            },
        ],
        members: vec![SharedGuildMember {
            membership_epoch: 1,
            identity: identity(account, index),
            name: player_name.into(),
            rank_index: 0,
        }],
        notice: Vec::new(),
        storage: BTreeMap::new(),
        buffs: BTreeMap::new(),
        last_buff_tick_ms: 0,
        experience_receipts: Default::default(),
        experience_receipt_payloads: Default::default(),
        active_wars: Default::default(),
    }
}
fn fixture() -> GatewayConfig {
    let config = GatewayConfig::default().with_crystal_world_runtime();
    let mut store = config.account_store.lock().unwrap();
    store.accounts.clear();
    for (index, account, name) in [
        (0, "war-leader", "War Leader"),
        (1, "war-member", "War Member"),
        (2, "enemy-leader", "Enemy Leader"),
        (3, "outsider", "Outsider"),
        (4, "newbie-leader", "Newbie Leader"),
    ] {
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
        let mut record = AccountRecord::empty();
        record.characters.push(character);
        record.saves.insert(index, save);
        store.accounts.insert(account.into(), record);
    }
    let mut own = guild(OWN, "War Guild", "war-leader", 0, "War Leader");
    own.members.push(SharedGuildMember {
        membership_epoch: 1,
        identity: identity("war-member", 1),
        name: "War Member".into(),
        rank_index: 1,
    });
    store.shared_guilds.insert(OWN.into(), own);
    store.shared_guilds.insert(
        ENEMY.into(),
        guild(ENEMY, "Enemy Guild", "enemy-leader", 2, "Enemy Leader"),
    );
    store.shared_guilds.insert(
        NEWBIE.into(),
        guild(NEWBIE, "NewbieGuild", "newbie-leader", 4, "Newbie Leader"),
    );
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
    .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    send(
        runtime,
        ClientPacket::StartGame {
            character_index: index,
        },
    );
    assert!(runtime.active_identity().is_some());
}
fn war() -> ClientPacket {
    ClientPacket::GuildWarReturn {
        name: "Enemy Guild".into(),
    }
}
fn records(config: &GatewayConfig) -> (SharedGuildRecord, SharedGuildRecord) {
    let store = config.account_store.lock().unwrap();
    (
        store.shared_guilds[OWN].clone(),
        store.shared_guilds[ENEMY].clone(),
    )
}
fn leave() -> ClientPacket {
    ClientPacket::Chat {
        message: "@LEAVEGUILD".into(),
        linked_items: Vec::new(),
    }
}
fn has_bank_debit(packets: &[ServerPacket]) -> bool {
    packets.iter().any(|packet| matches!(packet, ServerPacket::GuildStorageGoldChange { change_type: 2, amount, name } if *amount == war_cost() && name == "War Leader"))
}

#[test]
fn ordinary_war_commits_both_guilds_debits_only_declarer_and_notifies_current_online_members() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("war-a"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("war-b"));
    let mut enemy = factory.create_runtime(config.clone(), &ZoneId::new("war-c"));
    let mut outsider = factory.create_runtime(config.clone(), &ZoneId::new("war-c"));
    login(&mut leader, "war-leader", 0);
    login(&mut member, "war-member", 1);
    login(&mut enemy, "enemy-leader", 2);
    login(&mut outsider, "outsider", 3);
    let before = records(&config);
    let saves_before = config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .iter()
        .map(|(account, record)| {
            (
                account.clone(),
                serde_json::to_value(&record.saves).unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let reply = send(&mut leader, war());
    let after = records(&config);
    assert_eq!(after.0.gold, before.0.gold - war_cost());
    assert_eq!(after.1.gold, before.1.gold);
    assert_eq!(after.0.active_wars.get(ENEMY), Some(&180));
    assert_eq!(after.1.active_wars.get(OWN), Some(&180));
    assert_eq!(
        (after.0.revision, after.1.revision),
        (before.0.revision + 1, before.1.revision + 1)
    );
    assert_eq!(
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .iter()
            .map(|(account, record)| (
                account.clone(),
                serde_json::to_value(&record.saves).unwrap()
            ))
            .collect::<BTreeMap<_, _>>(),
        saves_before
    );
    assert!(has_bank_debit(&reply));
    assert!(has_bank_debit(&member.execute(WorldCommand::Tick).unwrap()));
    assert!(enemy.execute(WorldCommand::Tick).unwrap().iter().any(|packet| matches!(packet, ServerPacket::Chat { message, .. } if message == "War Guild has started a war")));
    let outsider_packets = outsider.execute(WorldCommand::Tick).unwrap();
    assert!(!has_bank_debit(&outsider_packets));
    assert!(!outsider_packets.iter().any(|packet| matches!(packet, ServerPacket::Chat { message, .. } if message.contains(" has started a war"))));
    send(&mut enemy, ClientPacket::LogOut);
    let mut rejoined = factory.create_runtime(config.clone(), &ZoneId::new("war-d"));
    login(&mut rejoined, "enemy-leader", 2);
    assert_eq!(records(&config), after);
    let rejected = send(
        &mut rejoined,
        ClientPacket::GuildWarReturn {
            name: "War Guild".into(),
        },
    );
    assert!(rejected.iter().any(|packet| matches!(packet, ServerPacket::Chat { message, .. } if message == "Already at war with this guild.")));
    assert_eq!(records(&config), after);
}

#[test]
fn ordinary_war_requires_current_leader_existing_enemy_nonself_nonnewbie_and_bank_funds() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("war-deny"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("war-deny"));
    login(&mut leader, "war-leader", 0);
    login(&mut member, "war-member", 1);
    let before = records(&config);
    send(&mut member, war());
    assert_eq!(records(&config), before);
    for name in ["Missing Guild", "War Guild", "NewbieGuild"] {
        send(
            &mut leader,
            ClientPacket::GuildWarReturn { name: name.into() },
        );
        assert_eq!(records(&config), before);
    }
    // Prepared underfunded bank is a negative transaction fixture.
    config
        .account_store
        .lock()
        .unwrap()
        .shared_guilds
        .get_mut(OWN)
        .unwrap()
        .gold = war_cost() - 1;
    let before = records(&config);
    let rejected = send(&mut leader, war());
    assert_eq!(records(&config), before);
    assert!(!has_bank_debit(&rejected));
}

#[test]
fn war_rechecks_pending_invite_and_blocks_new_recruit_and_self_leave() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("war-invite-a"));
    let mut outsider = factory.create_runtime(config.clone(), &ZoneId::new("war-invite-b"));
    let mut member = factory.create_runtime(config.clone(), &ZoneId::new("war-invite-b"));
    login(&mut leader, "war-leader", 0);
    login(&mut outsider, "outsider", 3);
    login(&mut member, "war-member", 1);
    send(
        &mut outsider,
        ClientPacket::Chat {
            message: "@ALLOWGUILD".into(),
            linked_items: Vec::new(),
        },
    );
    let invite = || ClientPacket::EditGuildMember {
        change_type: 0,
        name: "Outsider".into(),
        rank_index: 0,
        rank_name: String::new(),
    };
    send(&mut leader, invite());
    assert!(outsider
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GuildInvite { .. })));
    send(&mut leader, war());
    let before = records(&config);
    let accepted = send(
        &mut outsider,
        ClientPacket::GuildInvite {
            accept_invite: true,
        },
    );
    assert!(accepted.iter().any(|packet| matches!(packet, ServerPacket::Chat { message, .. } if message == "Cannot recuit members whilst at war.")), "accepted={accepted:?}; guilds={:?}", records(&config));
    assert_eq!(records(&config), before);
    let rejected = send(&mut leader, invite());
    assert!(rejected.iter().any(|packet| matches!(packet, ServerPacket::Chat { message, .. } if message == "Cannot recuit members whilst at war.")));
    send(&mut member, leave());
    assert_eq!(records(&config), before);
    assert!(records(&config)
        .0
        .member(&identity("war-member", 1))
        .is_some());
}

#[test]
fn failed_war_has_no_partial_endpoint_gold_or_notification_and_retry_commits_once() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("war-fault-a"));
    let mut enemy = factory.create_runtime(config.clone(), &ZoneId::new("war-fault-b"));
    login(&mut leader, "war-leader", 0);
    login(&mut enemy, "enemy-leader", 2);
    let before = records(&config);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    let rejected = send(&mut leader, war());
    assert_eq!(records(&config), before);
    assert!(!has_bank_debit(&rejected));
    assert!(!enemy.execute(WorldCommand::Tick).unwrap().iter().any(|packet| matches!(packet, ServerPacket::Chat { message, .. } if message.contains(" has started a war"))));
    send(&mut leader, war());
    let after = records(&config);
    assert_eq!(after.0.gold, before.0.gold - war_cost());
    assert_eq!(after.1.gold, before.1.gold);
    assert_eq!(
        (after.0.revision, after.1.revision),
        (before.0.revision + 1, before.1.revision + 1)
    );
}

#[test]
fn concurrent_reverse_declarations_linearize_to_one_symmetric_war_and_one_payment() {
    let config = fixture();
    let before = records(&config);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let left_config = config.clone();
    let left_barrier = barrier.clone();
    let left = std::thread::spawn(move || {
        left_barrier.wait();
        left_config.commit_shared_guild_war(&identity("war-leader", 0), "Enemy Guild")
    });
    let right_config = config.clone();
    let right_barrier = barrier.clone();
    let right = std::thread::spawn(move || {
        right_barrier.wait();
        right_config.commit_shared_guild_war(&identity("enemy-leader", 2), "War Guild")
    });
    barrier.wait();
    let left = left.join().unwrap();
    let right = right.join().unwrap();
    assert_eq!(
        usize::from(left.is_ok()) + usize::from(right.is_ok()),
        1,
        "left={left:?}; right={right:?}"
    );
    let after = records(&config);
    assert_eq!(
        after.0.gold + after.1.gold,
        before.0.gold + before.1.gold - war_cost()
    );
    assert_eq!(after.0.active_wars.get(ENEMY), Some(&180));
    assert_eq!(after.1.active_wars.get(OWN), Some(&180));
    assert_eq!(
        (after.0.revision, after.1.revision),
        (before.0.revision + 1, before.1.revision + 1)
    );
}

#[test]
fn persisted_war_reopens_with_both_endpoints_and_does_not_debit_again() {
    let config = fixture();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mir2-guild-wars-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("accounts.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&*config.account_store.lock().unwrap()).unwrap(),
    )
    .unwrap();
    let config = config.with_account_store_path(path.clone());
    config
        .commit_shared_guild_war(&identity("war-leader", 0), "Enemy Guild")
        .unwrap();
    let after = records(&config);
    drop(config);
    let reopened = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_account_store_path(path);
    assert_eq!(records(&reopened), after);
    assert!(reopened
        .commit_shared_guild_war(&identity("enemy-leader", 2), "War Guild")
        .is_err());
    assert_eq!(records(&reopened), after);
    // Retain the isolated fixture for failure inspection.
}

#[test]
fn unauthenticated_and_pre_startgame_war_requests_cannot_mutate() {
    let config = fixture();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut runtime = factory.create_runtime(config.clone(), &ZoneId::new("war-auth"));
    let before = records(&config);
    let _ = runtime.execute(WorldCommand::ClientPacket(war()));
    assert_eq!(records(&config), before);
    send(
        &mut runtime,
        ClientPacket::Login {
            account_id: "war-leader".into(),
            password: "demo".into(),
        },
    );
    let _ = runtime.execute(WorldCommand::ClientPacket(war()));
    assert_eq!(records(&config), before);
}
