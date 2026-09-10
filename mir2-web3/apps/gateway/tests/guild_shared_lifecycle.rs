use mir2_gateway::{GatewayConfig, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRuntimeFactory};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{
    AccountRecord, AccountStoreTransactionFault, CharacterRecord, CharacterSaveRecord,
    SharedGuildMember, SharedGuildRank, SharedGuildRecord, Stage5FriendIdentity, WorldCommand,
    ZoneRuntimeHandle,
};
use std::collections::{BTreeMap, BTreeSet};
const ID: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn guild_shared_bank_ordinary_packets_cross_zone_broadcast_and_atomic_failure() {
    let config = fixture(true);
    {
        let mut store = config.account_store.lock().unwrap();
        let guild = store.shared_guilds.get_mut(ID).unwrap();
        guild.spare_points = 1;
        guild.ranks.push(SharedGuildRank {
            index: 1,
            name: "Banker".into(),
            options: 24,
        });
        guild.members.push(SharedGuildMember { membership_epoch: 0,
            identity: id("peer", 1),
            name: "Peer".into(),
            rank_index: 1,
        });
    }
    let mut seed = mir2_simulation::SimulationSession::new(config.clone());
    seed.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    seed.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let horn = mir2_game_data::crystal_item_by_name("WoomaHorn").unwrap();
    seed.stage5_command(
        "qa.giveItem",
        vec![format!("crystal-item-{}", horn.item_index), "1".into()],
    );
    seed.save_active_character().unwrap();
    drop(seed);
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("bank-a"));
    let mut peer = factory.create_runtime(config.clone(), &ZoneId::new("bank-b"));
    let mut outsider = factory.create_runtime(config.clone(), &ZoneId::new("bank-b"));
    login(&mut leader, "demo", 0);
    login(&mut peer, "peer", 1);
    login(&mut outsider, "other", 2);
    assert!(leader.world_snapshot().in_safe_zone);
    let buff = mir2_game_data::crystal_guild_buff_definitions()
        .iter()
        .find(|buff| {
            buff.level_requirement == 0
                && buff.points_requirement == 1
                && buff.activation_cost == 0
                && !buff.stats.is_empty()
        })
        .unwrap();
    let definition_packets = command(
        &mut leader,
        ClientPacket::GuildBuffUpdate { action: 0, id: 0 },
    );
    assert!(definition_packets.iter().any(|packet|matches!(packet,ServerPacket::GuildBuffList{guild_buffs,..} if guild_buffs.len()==16)));
    let denied = command(
        &mut peer,
        ClientPacket::GuildBuffUpdate {
            action: 1,
            id: buff.id,
        },
    );
    assert!(!denied.iter().any(|packet|matches!(packet,ServerPacket::GuildBuffList{active_buffs,..} if active_buffs.iter().any(|entry|entry.id==buff.id&&entry.active))));
    assert_eq!(
        config
            .shared_guild_for_identity(&id("demo", 0))
            .unwrap()
            .unwrap()
            .spare_points,
        1
    );
    let before_stats = leader.world_snapshot().player_crystal_stats;
    let buff_packets = command(
        &mut leader,
        ClientPacket::GuildBuffUpdate {
            action: 1,
            id: buff.id,
        },
    );
    assert!(buff_packets.iter().any(|packet|matches!(packet,ServerPacket::GuildBuffList{active_buffs,..} if active_buffs.iter().any(|entry|entry.id==buff.id&&entry.active))));
    assert_ne!(leader.world_snapshot().player_crystal_stats, before_stats);
    assert_eq!(
        config
            .shared_guild_for_identity(&id("demo", 0))
            .unwrap()
            .unwrap()
            .spare_points,
        0
    );
    assert!(peer.execute(WorldCommand::Tick).unwrap().iter().any(|packet|matches!(packet,ServerPacket::GuildBuffList{active_buffs,..} if active_buffs.iter().any(|entry|entry.id==buff.id&&entry.active))));
    let before = leader.world_snapshot().gold;
    let packets = command(
        &mut leader,
        ClientPacket::GuildStorageGoldChange {
            change_type: 0,
            amount: 100,
        },
    );
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 100 })),
        "{packets:?}"
    );
    assert_eq!(leader.world_snapshot().gold, before - 100);
    let received = peer.execute(WorldCommand::Tick).unwrap();
    assert!(received.iter().any(|packet| matches!(
        packet,
        ServerPacket::GuildStorageGoldChange {
            change_type: 0,
            amount: 100,
            ..
        }
    )));
    assert!(!received
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseGold { .. })));
    assert!(!outsider
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GuildStorageGoldChange { .. })));
    command(
        &mut peer,
        ClientPacket::GuildStorageGoldChange {
            change_type: 1,
            amount: 1,
        },
    );
    assert_eq!(
        config
            .shared_guild_for_identity(&id("peer", 1))
            .unwrap()
            .unwrap()
            .gold,
        100
    );
    let source = leader
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|item| item.key == format!("crystal-item-{}", horn.item_index))
        .unwrap();
    let from = i32::from(source.slot)
        + if source.container == mir2_simulation::ItemContainer::Bag2 {
            40
        } else {
            0
        };
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    let failed = command(
        &mut leader,
        ClientPacket::GuildStorageItemChange {
            change_type: 0,
            from,
            to: 111,
        },
    );
    assert!(failed.iter().any(|packet| matches!(
        packet,
        ServerPacket::GuildStorageItemChange {
            change_type: 3,
            to: 111,
            ..
        }
    )));
    assert!(config
        .shared_guild_for_identity(&id("demo", 0))
        .unwrap()
        .unwrap()
        .storage
        .is_empty());
    let deposited = command(
        &mut leader,
        ClientPacket::GuildStorageItemChange {
            change_type: 0,
            from,
            to: 111,
        },
    );
    assert!(deposited.iter().any(|packet|matches!(packet,ServerPacket::GuildStorageItemChange{change_type:0,item:Some(item),..} if item.item.unique_id==source.unique_id)),"{deposited:?}");
    let received = peer.execute(WorldCommand::Tick).unwrap();
    let delta = received
        .iter()
        .position(|packet| {
            matches!(
                packet,
                ServerPacket::GuildStorageItemChange {
                    change_type: 0,
                    to: 111,
                    ..
                }
            )
        })
        .unwrap();
    assert!(received[..delta]
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NewItemInfo { .. })));
    let retrieved = command(
        &mut peer,
        ClientPacket::GuildStorageItemChange {
            change_type: 1,
            from: 111,
            to: 0,
        },
    );
    assert!(
        retrieved.iter().any(|packet| matches!(
            packet,
            ServerPacket::GuildStorageItemChange {
                change_type: 1,
                from: 111,
                to: 0,
                ..
            }
        )),
        "{retrieved:?}"
    );
    assert!(peer
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.unique_id == source.unique_id));
    command(&mut peer, ClientPacket::LogOut);
    drop(peer);
    let mut returned = factory.create_runtime(config.clone(), &ZoneId::new("bank-c"));
    login(&mut returned, "peer", 1);
    assert!(returned
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.unique_id == source.unique_id));
    assert!(config
        .shared_guild_for_identity(&id("demo", 0))
        .unwrap()
        .unwrap()
        .storage
        .is_empty());
}
fn fixture(seed_guild: bool) -> GatewayConfig {
    let config = GatewayConfig::default().with_crystal_world_runtime();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for (index, account_id, name) in [
            (0, "demo", "Scout"),
            (1, "peer", "Peer"),
            (2, "other", "Other"),
        ] {
            let mut account = AccountRecord::empty();
            account.characters.push(CharacterRecord {
                index,
                name: name.into(),
                level: 22,
                class: MirClass::Warrior,
                gender: MirGender::Male,
            });
            let mut save = CharacterSaveRecord::new(account.characters[0].clone());
            save.map_file_name = config.map.file_name.clone();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            save.gold = 2_000_000;
            account.saves.insert(index, save);
            store.accounts.insert(account_id.into(), account);
        }
        if seed_guild {
            store.shared_guilds.insert(
                ID.into(),
                SharedGuildRecord {
                    id: ID.into(),
                    name: "Knights".into(),
                    revision: 1,
                    level: 0,
                    experience: 0,
                    spare_points: 0,
                    gold: 0,
                    ranks: vec![SharedGuildRank {
                        index: 0,
                        name: "Leader".into(),
                        options: 255,
                    }],
                    members: vec![SharedGuildMember { membership_epoch: 0,
                        identity: id("demo", 0),
                        name: "Scout".into(),
                        rank_index: 0,
                    }],
                    notice: vec![],
                    storage: BTreeMap::new(),
                    buffs: BTreeMap::new(),
                    last_buff_tick_ms: 0,
                    experience_receipts: BTreeSet::new(), experience_receipt_payloads: Default::default(),
                },
            );
        }
    }
    config
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
fn login(runtime: &mut ZoneRuntimeHandle, account: &str, index: i32) {
    assert!(command(
        runtime,
        ClientPacket::Login {
            account_id: account.into(),
            password: "demo".into()
        }
    )
    .iter()
    .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    command(
        runtime,
        ClientPacket::StartGame {
            character_index: index,
        },
    );
    assert!(runtime.active_identity().is_some());
}
fn allow(runtime: &mut ZoneRuntimeHandle) {
    command(
        runtime,
        ClientPacket::Chat {
            message: "@ALLOWGUILD".into(),
            linked_items: vec![],
        },
    );
}
fn invite(runtime: &mut ZoneRuntimeHandle, name: &str) {
    command(
        runtime,
        ClientPacket::EditGuildMember {
            change_type: 0,
            rank_index: 0,
            name: name.into(),
            rank_name: String::new(),
        },
    );
}
#[test]
fn guild_shared_cross_zone_permission_refusal_accept_and_relogin() {
    let config = fixture(true);
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("guild-a"));
    let mut peer = factory.create_runtime(config.clone(), &ZoneId::new("guild-b"));
    login(&mut leader, "demo", 0);
    login(&mut peer, "peer", 1);
    let mut observer = factory.create_runtime(config.clone(), &ZoneId::new("guild-b"));
    login(&mut observer, "other", 2);
    invite(&mut leader, "Peer");
    assert!(
        !peer
            .execute(WorldCommand::Tick)
            .unwrap()
            .iter()
            .any(|p| matches!(p, ServerPacket::GuildInvite { .. })),
        "default permission is false"
    );
    allow(&mut peer);
    invite(&mut leader, "Peer");
    assert!(peer
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|p| matches!(p,ServerPacket::GuildInvite{name} if name=="Knights")));
    command(
        &mut peer,
        ClientPacket::GuildInvite {
            accept_invite: false,
        },
    );
    assert!(config
        .shared_guild_for_identity(&id("peer", 1))
        .unwrap()
        .is_none());
    invite(&mut leader, "Peer");
    peer.execute(WorldCommand::Tick).unwrap();
    let packets = command(
        &mut peer,
        ClientPacket::GuildInvite {
            accept_invite: true,
        },
    );
    assert!(packets.iter().any(|p|matches!(p,ServerPacket::GuildStatus{guild_name,my_options:0,member_count:2,..} if guild_name=="Knights")),"{packets:?}");
    let packets = leader.execute(WorldCommand::Tick).unwrap();
    assert!(packets.iter().any(|p|matches!(p,ServerPacket::GuildMemberChange{ranks,..} if ranks.iter().flat_map(|rank|&rank.members).any(|member|member.id==1&&member.name=="Peer"&&member.online))));
    let seen = observer.execute(WorldCommand::Tick).unwrap();
    assert!(seen.iter().any(|packet|matches!(packet,ServerPacket::ObjectGuildNameChanged{guild_name,..} if guild_name=="Knights")),"AOI observer must see the new guild name: {seen:?}");
    command(&mut peer, ClientPacket::LogOut);
    let last_access = config.account_store.lock().unwrap().accounts["peer"]
        .character_last_access_binary_datetimes[&1];
    assert_ne!(last_access, 0);
    let offline = leader.execute(WorldCommand::Tick).unwrap();
    assert!(offline.iter().any(|packet|matches!(packet,ServerPacket::GuildMemberChange{ranks,..} if ranks.iter().flat_map(|rank|&rank.members).any(|member|member.id==1&&!member.online&&member.last_login_binary_datetime==last_access))));
    drop(peer);
    let mut returned = factory.create_runtime(config.clone(), &ZoneId::new("guild-c"));
    login(&mut returned, "peer", 1);
    assert_eq!(
        returned.world_snapshot().stage5_systems.guild.name,
        "Knights"
    );
    assert_eq!(
        config.account_store.lock().unwrap().shared_guilds[ID]
            .members
            .len(),
        2
    );
}
#[test]
fn guild_shared_failed_join_and_replaced_presence_cannot_commit() {
    let config = fixture(true);
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut leader = factory.create_runtime(config.clone(), &ZoneId::new("guild-a"));
    let mut peer = factory.create_runtime(config.clone(), &ZoneId::new("guild-b"));
    login(&mut leader, "demo", 0);
    login(&mut peer, "peer", 1);
    allow(&mut peer);
    invite(&mut leader, "Peer");
    peer.execute(WorldCommand::Tick).unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    let packets = command(
        &mut peer,
        ClientPacket::GuildInvite {
            accept_invite: true,
        },
    );
    assert!(!packets
        .iter()
        .any(|p| matches!(p,ServerPacket::GuildStatus{guild_name,..} if guild_name=="Knights")));
    assert!(config
        .shared_guild_for_identity(&id("peer", 1))
        .unwrap()
        .is_none());
    invite(&mut leader, "Peer");
    peer.execute(WorldCommand::Tick).unwrap();
    drop(leader);
    let mut replacement = factory.create_runtime(config.clone(), &ZoneId::new("guild-c"));
    login(&mut replacement, "demo", 0);
    command(
        &mut peer,
        ClientPacket::GuildInvite {
            accept_invite: true,
        },
    );
    assert!(config
        .shared_guild_for_identity(&id("peer", 1))
        .unwrap()
        .is_none());
    invite(&mut replacement, "Peer");
    peer.execute(WorldCommand::Tick).unwrap();
    command(
        &mut peer,
        ClientPacket::GuildInvite {
            accept_invite: true,
        },
    );
    assert!(config
        .shared_guild_for_identity(&id("peer", 1))
        .unwrap()
        .is_some());
}
#[test]
fn guild_shared_original_npc_page_grants_creation_and_forged_name_return_does_not() {
    let config = fixture(false);
    // Server fixture only. No QA command enters the shared production wire path.
    let mut seed = mir2_simulation::SimulationSession::new(config.clone());
    seed.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    seed.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let horn = mir2_game_data::crystal_item_by_name("WoomaHorn").unwrap();
    seed.stage5_command(
        "qa.giveItem",
        vec![format!("crystal-item-{}", horn.item_index), "1".into()],
    );
    seed.save_active_character().unwrap();
    drop(seed);
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut owner = factory.create_runtime(config.clone(), &ZoneId::new("guild-npc"));
    login(&mut owner, "demo", 0);
    command(
        &mut owner,
        ClientPacket::GuildNameReturn {
            name: "Forged".into(),
        },
    );
    assert!(config
        .account_store
        .lock()
        .unwrap()
        .shared_guilds
        .is_empty());
    owner
        .execute(WorldCommand::TransferMap {
            key: "crystal:0122:27:33".into(),
        })
        .unwrap();
    let npc = owner
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.name.contains("Administrator"))
        .expect("original Administrator NPC must be visible");
    let packets = command(
        &mut owner,
        ClientPacket::CallNpc {
            object_id: npc.object_id,
            key: "@CREATEGUILD".into(),
        },
    );
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::GuildNameRequest)));
    command(
        &mut owner,
        ClientPacket::CallNpc {
            object_id: npc.object_id,
            key: "@Main".into(),
        },
    );
    let dialog = owner
        .world_snapshot()
        .active_npc_dialog
        .expect("source NPC page");
    assert!(
        dialog
            .links
            .iter()
            .any(|link| link.target.eq_ignore_ascii_case("@CREATEGUILD")),
        "{dialog:?}"
    );
    let packets = command(
        &mut owner,
        ClientPacket::CallNpc {
            object_id: npc.object_id,
            key: "@CREATEGUILD".into(),
        },
    );
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::GuildNameRequest)),
        "{packets:?}"
    );
    let packets = command(
        &mut owner,
        ClientPacket::GuildNameReturn {
            name: "Knights".into(),
        },
    );
    assert!(
        packets
            .iter()
            .any(|p| matches!(p,ServerPacket::GuildStatus{guild_name,..} if guild_name=="Knights")),
        "{packets:?}"
    );
    assert_eq!(owner.world_snapshot().gold, 1_000_000);
    command(
        &mut owner,
        ClientPacket::GuildNameReturn {
            name: "Again".into(),
        },
    );
    assert_eq!(config.account_store.lock().unwrap().shared_guilds.len(), 1);
    assert_eq!(owner.world_snapshot().gold, 1_000_000);
}
