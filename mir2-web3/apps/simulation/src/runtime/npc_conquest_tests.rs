//! Public NPC packet acceptance. Fixtures seed server-owned world/store state;
//! every player action below uses CallNpc or another ordinary Crystal packet.
use super::super::components::{entity_by_object_id, player_entity, CharacterBody};
use super::super::inventory::add_or_increment_item;
use super::super::items::crystal_item_key_for_template;
use super::super::npc::ActiveNpcDialogState;
use super::super::resources::{MapRuntimeResource, NpcStateResource, PlayerRuntimeResource};
use crate::conquest::{
    default_sabuk_defenses, sabuk_policy, ConquestEventKind, SharedConquestRecord,
};
use crate::{
    AccountRecord, AccountStoreTransactionFault, CharacterRecord, ItemContainer, SharedGuildMember,
    SharedGuildRank, SharedGuildRecord, SimulationConfig, SimulationSession, Stage5FriendIdentity,
    VisibleNpcRecord, CRYSTAL_OBJECT_DATA_RANGE,
};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket};
use std::collections::{BTreeMap, BTreeSet};

const KNIGHTS: &str = "11111111111111111111111111111111";
const RAIDERS: &str = "22222222222222222222222222222222";
const REGISTRAR: u32 = 427;
const OFFICER: u32 = 1146;
const WRONG_NPC: u32 = 1427;

#[test]
fn conquest_legacy_scripts_cannot_import_or_mutate_personal_castle_authority() {
    use super::super::npc_script::*;
    use super::super::resources::Stage5SystemsResource;
    let mut fixture = fixture(true, true);
    let before = authority(&fixture);
    let wallet_before = wallet(&fixture);
    let world = fixture.session.app.world_mut();
    {
        let mut legacy = world.resource_mut::<Stage5SystemsResource>();
        legacy.stage5_systems.guild.name = "Forged Personal Owner".into();
        legacy.stage5_systems.conquest.castle_owner = "Forged Personal Owner".into();
        legacy.stage5_systems.conquest.gold = 50_000_000;
    }
    let legacy_before = serde_json::to_value(
        &world
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .conquest,
    )
    .unwrap();
    assert_eq!(crystal_npc_conquest_owner_name(world), "Knights");
    assert!(crystal_npc_condition_conquest_owner(world, &[]));
    assert!(super::super::map::conquest_movement_allowed(world, 1));
    assert!(super::super::map::conquest_movement_allowed(world, 0));
    assert!(!crystal_npc_afford_conquest_asset(
        world,
        ConquestAssetKind::Gate
    ));
    assert_eq!(
        crystal_npc_conquest_asset_label(world, ConquestAssetKind::Guard, 1),
        "Dead"
    );
    crystal_npc_set_conquest_rate(world, &["1", "99"]);
    crystal_npc_repair_conquest_asset(world, ConquestAssetKind::Gate, &["1", "1"]);
    crystal_npc_repair_all_conquest_assets(world);
    crystal_npc_set_conquest_gate_open(world, &["1", "1"], true);
    crystal_npc_start_conquest(world, &["1"]);
    crystal_npc_take_conquest_gold(world, &["1", "1000"]);
    assert_eq!(
        serde_json::to_value(
            &world
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .conquest
        )
        .unwrap(),
        legacy_before
    );
    assert_eq!(authority(&fixture), before);
    assert_eq!(wallet(&fixture), wallet_before);
    // Leaving the authoritative guild immediately retires source-script owner
    // privilege even while the forged personal snapshot still says "owner".
    fixture
        .config
        .account_store
        .lock()
        .unwrap()
        .shared_guilds
        .get_mut(KNIGHTS)
        .unwrap()
        .members
        .retain(|member| member.identity.account_id != "hero");
    assert!(!crystal_npc_condition_conquest_owner(
        fixture.session.app.world(),
        &[]
    ));
    assert!(!super::super::map::conquest_movement_allowed(
        fixture.session.app.world(),
        1
    ));
}

struct Fixture {
    config: SimulationConfig,
    session: SimulationSession,
    npc: u32,
    position: Point,
}

fn guild(id: &str, name: &str, members: &[(&str, i32, &str, u8)]) -> SharedGuildRecord {
    SharedGuildRecord {
        id: id.into(),
        name: name.into(),
        revision: 1,
        level: 0,
        experience: 0,
        spare_points: 0,
        gold: 1_000_000,
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
        members: members
            .iter()
            .map(|&(account, index, name, rank)| SharedGuildMember {
                membership_epoch: 0,
                identity: Stage5FriendIdentity {
                    account_id: account.into(),
                    character_index: index,
                },
                name: name.into(),
                rank_index: rank,
            })
            .collect(),
        notice: Vec::new(),
        storage: BTreeMap::new(),
        buffs: BTreeMap::new(),
        last_buff_tick_ms: 0,
        experience_receipts: BTreeSet::new(),
        experience_receipt_payloads: BTreeMap::new(),
    }
}

fn fixture(officer: bool, guilded: bool) -> Fixture {
    let (npc, map, position, script) = if officer {
        (
            OFFICER,
            "0150",
            Point { x: 9, y: 13 },
            "MongchonProvince/SabukWall/Conquest",
        )
    } else {
        (
            REGISTRAR,
            "0122",
            Point { x: 28, y: 33 },
            "BichonProvince/BichonWall/Administrator",
        )
    };
    let mut config = SimulationConfig::default();
    config.map.file_name = map.into();
    config.map.title = "NPC acceptance fixture".into();
    config.spawn = Point {
        x: position.x - 1,
        y: position.y,
    };
    config.visible_players.clear();
    config.visible_monsters.clear();
    config.map_transfers.clear();
    let mut policy = sabuk_policy();
    policy.utc_offset_minutes = 480;
    config.conquest_policies = vec![policy];
    config.visible_npcs = vec![
        VisibleNpcRecord {
            object_id: npc,
            name: "Canonical service".into(),
            image: 5,
            colour_argb: -1,
            position: position.clone(),
            direction: MirDirection::Down,
            quest_ids: Vec::new(),
            script_key: Some(script.into()),
        },
        VisibleNpcRecord {
            object_id: WRONG_NPC,
            name: "Different NPC".into(),
            image: 5,
            colour_argb: -1,
            position: Point {
                x: position.x,
                y: position.y + 2,
            },
            direction: MirDirection::Down,
            quest_ids: Vec::new(),
            script_key: Some("BichonProvince/BichonWall/Administrator".into()),
        },
    ];
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for (account, index, name) in [
            ("hero", 0, "Hero"),
            ("peer", 1, "Peer"),
            ("rival", 2, "Rival"),
        ] {
            let mut record = AccountRecord::new(CharacterRecord {
                index,
                name: name.into(),
                level: 30,
                class: MirClass::Warrior,
                gender: MirGender::Male,
            });
            let save = record.saves.get_mut(&index).unwrap();
            save.map_file_name = map.into();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            save.gold = 2_000_000;
            store.accounts.insert(account.into(), record);
        }
        if guilded {
            store.shared_guilds.insert(
                KNIGHTS.into(),
                guild(
                    KNIGHTS,
                    "Knights",
                    &[("hero", 0, "Hero", 0), ("peer", 1, "Peer", 1)],
                ),
            );
        }
        store.shared_guilds.insert(
            RAIDERS.into(),
            guild(RAIDERS, "Raiders", &[("rival", 2, "Rival", 0)]),
        );
        if officer && guilded {
            let mut record = SharedConquestRecord::new(1);
            record.owner_guild_id = Some(KNIGHTS.into());
            record.tax_rate_percent = 10;
            record.gold = 700;
            record.defenses = default_sabuk_defenses();
            record.defenses.get_mut("gate:1").unwrap().hp = 2_500;
            record.defenses.get_mut("archer:1").unwrap().hp = 0;
            store.shared_conquests.insert(1, record);
        }
    }
    let mut session = SimulationSession::new(config.clone());
    session.enable_shared_guild_authority();
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "hero".into(),
        password: "demo".into(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(
        start
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })),
        "{start:?}"
    );
    session.force_authoritative_player_transform(config.spawn.clone(), MirDirection::Right);
    assert_eq!(
        session
            .app
            .world()
            .resource::<MapRuntimeResource>()
            .current_map
            .file_name,
        map
    );
    assert!(entity_by_object_id(session.app.world(), npc).is_some());
    Fixture {
        config,
        session,
        npc,
        position,
    }
}

fn call(fixture: &mut Fixture, key: &str) -> Vec<ServerPacket> {
    fixture.session.handle_packet(ClientPacket::CallNpc {
        object_id: fixture.npc,
        key: key.into(),
    })
}
fn dialog(fixture: &Fixture) -> ActiveNpcDialogState {
    fixture
        .session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .clone()
        .expect("public NPC must display a page")
}
fn has_link(fixture: &Fixture, target: &str) -> bool {
    dialog(fixture)
        .links
        .iter()
        .any(|link| link.target == target)
}
fn open_registration(fixture: &mut Fixture) {
    call(fixture, "@Main");
    assert!(has_link(fixture, "@sabuk:status"));
    call(fixture, "@sabuk:status");
    assert!(has_link(fixture, "@sabuk:register"));
}
fn open_management(fixture: &mut Fixture, page: &str) {
    call(fixture, "@Main");
    assert!(has_link(fixture, "@sabuk:manage"));
    call(fixture, "@sabuk:manage");
    if page != "manage" {
        let target = format!("@sabuk:{page}");
        assert!(has_link(fixture, &target));
        call(fixture, &target);
    }
}
fn authority(
    fixture: &Fixture,
) -> (
    BTreeMap<i32, SharedConquestRecord>,
    BTreeMap<String, SharedGuildRecord>,
) {
    let store = fixture.config.account_store.lock().unwrap();
    (store.shared_conquests.clone(), store.shared_guilds.clone())
}
fn castle(fixture: &Fixture) -> SharedConquestRecord {
    fixture
        .config
        .account_store
        .lock()
        .unwrap()
        .shared_conquests[&1]
        .clone()
}
fn bank(fixture: &Fixture) -> u32 {
    fixture.config.account_store.lock().unwrap().shared_guilds[KNIGHTS].gold
}
fn wallet(fixture: &Fixture) -> u32 {
    fixture
        .session
        .app
        .world()
        .resource::<PlayerRuntimeResource>()
        .gold
}
fn grant_guild_creation(fixture: &mut Fixture) {
    let horn = mir2_game_data::crystal_item_by_name("WoomaHorn").unwrap();
    add_or_increment_item(
        fixture.session.app.world_mut(),
        ItemContainer::Bag1,
        &crystal_item_key_for_template(&horn),
        &horn.name,
        "",
        20,
        1,
        u16::from(horn.weight),
    );
    call(fixture, "@Main");
    assert!(has_link(fixture, "@CREATEGUILD"));
    assert!(call(fixture, "@CREATEGUILD")
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GuildNameRequest)));
}

#[test]
fn npc_conquest_signup_is_exposed_only_after_the_real_registrar_status_page() {
    let mut fixture = fixture(false, true);
    let before = authority(&fixture);
    let gold = wallet(&fixture);
    call(&mut fixture, "@sabuk:register");
    assert_eq!(
        authority(&fixture),
        before,
        "a fabricated first packet cannot apply"
    );
    assert!(!has_link(&fixture, "@sabuk:register"));
    open_registration(&mut fixture);
    let page = dialog(&fixture);
    assert_eq!(page.npc_object_id, REGISTRAR);
    assert_eq!(page.title, "Siege status");
    assert!(page.body.iter().any(|line| line == "Castle is unowned."));
    assert!(page.body.iter().any(|line| line == "No siege applicant."));
    assert!(page
        .body
        .iter()
        .any(|line| line.contains("daily 18:00-18:30 (UTC+8:00)")));
    call(&mut fixture, "@sabuk:register");
    let record = castle(&fixture);
    assert_eq!(record.attacker_guild_id.as_deref(), Some(KNIGHTS));
    assert_eq!(record.events.len(), 1);
    assert_eq!(record.events[0].kind, ConquestEventKind::Requested);
    assert!(!record.defenses.is_empty());
    assert_eq!(
        wallet(&fixture),
        gold,
        "the executable Crystal request has no character fee"
    );
    assert!(dialog(&fixture)
        .body
        .iter()
        .any(|line| line == "Registration saved."));
    assert!(!has_link(&fixture, "@sabuk:register"));
    let committed = authority(&fixture);
    call(&mut fixture, "@sabuk:register");
    assert_eq!(
        authority(&fixture),
        committed,
        "an old button cannot register twice"
    );
}

#[test]
fn npc_conquest_fabricated_labels_and_different_npc_cannot_mutate_shared_authority() {
    let mut fixture = fixture(false, true);
    open_registration(&mut fixture);
    let before = authority(&fixture);
    for target in [
        "@sabuk:tax:25",
        "@sabuk:open",
        "@sabuk:repair:gate:1",
        "@sabuk:withdraw",
    ] {
        call(&mut fixture, target);
        assert_eq!(
            authority(&fixture),
            before,
            "forbidden registrar target {target}"
        );
    }
    fixture.session.handle_packet(ClientPacket::CallNpc {
        object_id: WRONG_NPC,
        key: "@sabuk:register".into(),
    });
    assert_eq!(authority(&fixture), before);
    assert_eq!(dialog(&fixture).npc_object_id, WRONG_NPC);
    assert!(!has_link(&fixture, "@sabuk:status"));
    call(&mut fixture, "@sabuk:register");
    assert_eq!(
        authority(&fixture),
        before,
        "previous NPC's capability cannot survive service replacement"
    );
}

#[test]
fn npc_conquest_stale_range_map_missing_npc_death_and_logout_deny_old_signup_link() {
    for boundary in ["range", "map", "missing", "dead", "logout", "exit"] {
        let mut fixture = fixture(false, true);
        open_registration(&mut fixture);
        let before = authority(&fixture);
        match boundary {
            "range" => fixture.session.force_authoritative_player_transform(
                Point {
                    x: fixture.position.x + CRYSTAL_OBJECT_DATA_RANGE + 1,
                    y: fixture.position.y,
                },
                MirDirection::Left,
            ),
            "map" => {
                fixture
                    .session
                    .app
                    .world_mut()
                    .resource_mut::<MapRuntimeResource>()
                    .current_map
                    .file_name = "0".into()
            }
            "missing" => {
                let entity = entity_by_object_id(fixture.session.app.world(), REGISTRAR).unwrap();
                fixture.session.app.world_mut().despawn(entity);
            }
            "dead" => fixture
                .session
                .force_authoritative_player_vitals(Some(0), None),
            "logout" => {
                fixture.session.handle_packet(ClientPacket::LogOut);
            }
            "exit" => {
                call(&mut fixture, "@Exit");
            }
            _ => unreachable!(),
        }
        call(&mut fixture, "@sabuk:register");
        assert_eq!(
            authority(&fixture),
            before,
            "stale {boundary} registration mutated authority"
        );
    }
}

#[test]
fn npc_conquest_registration_rechecks_demoted_or_kicked_leader_and_closes_capability() {
    for kicked in [false, true] {
        let mut fixture = fixture(false, true);
        open_registration(&mut fixture);
        {
            let mut store = fixture.config.account_store.lock().unwrap();
            let guild = store.shared_guilds.get_mut(KNIGHTS).unwrap();
            for member in &mut guild.members {
                member.rank_index = if member.identity.account_id == "peer" {
                    0
                } else {
                    1
                };
            }
            if kicked {
                guild
                    .members
                    .retain(|member| member.identity.account_id != "hero");
            }
            guild.revision += 1;
        }
        let before = authority(&fixture);
        call(&mut fixture, "@sabuk:register");
        assert_eq!(authority(&fixture), before);
        assert!(!has_link(&fixture, "@sabuk:register"));
        assert!(dialog(&fixture)
            .body
            .iter()
            .any(|line| line.contains("registration failed")));
    }
}

#[test]
fn npc_conquest_guildless_registrar_and_public_officer_have_status_without_management() {
    let mut registrar = fixture(false, false);
    call(&mut registrar, "@Main");
    call(&mut registrar, "@sabuk:status");
    assert_eq!(dialog(&registrar).title, "Siege status");
    assert!(!has_link(&registrar, "@sabuk:register"));
    let mut officer = fixture(true, true);
    {
        officer
            .config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .get_mut(&1)
            .unwrap()
            .owner_guild_id = Some(RAIDERS.into());
    }
    let before = authority(&officer);
    call(&mut officer, "@Main");
    assert_eq!(dialog(&officer).title, "Siege status");
    assert!(dialog(&officer)
        .body
        .iter()
        .any(|line| line == "Castle owner: Raiders"));
    assert!(!has_link(&officer, "@sabuk:manage"));
    for target in [
        "@sabuk:manage",
        "@sabuk:tax:25",
        "@sabuk:repair:gate:1",
        "@sabuk:withdraw",
    ] {
        call(&mut officer, target);
        assert_eq!(authority(&officer), before);
    }
}

#[test]
fn npc_conquest_owner_management_requires_displayed_page_and_supports_tax_gate_and_bank() {
    let mut fixture = fixture(true, true);
    call(&mut fixture, "@Main");
    let before = authority(&fixture);
    call(&mut fixture, "@sabuk:tax:25");
    assert_eq!(
        authority(&fixture),
        before,
        "status page is not a tax capability"
    );
    open_management(&mut fixture, "manage");
    assert_eq!(dialog(&fixture).title, "Castle management");
    assert!(dialog(&fixture)
        .body
        .iter()
        .any(|line| line == "Guild bank: 1000000 gold"));
    call(&mut fixture, "@sabuk:tax:25");
    assert_eq!(castle(&fixture).tax_rate_percent, 25);
    let taxed = authority(&fixture);
    call(&mut fixture, "@sabuk:tax:21");
    assert_eq!(
        authority(&fixture),
        taxed,
        "client-selected unsupported rate must not apply"
    );
    call(&mut fixture, "@sabuk:open");
    assert!(castle(&fixture).defenses["gate:1"].open);
    call(&mut fixture, "@sabuk:close");
    assert!(!castle(&fixture).defenses["gate:1"].open);
    let gold = wallet(&fixture);
    call(&mut fixture, "@sabuk:withdraw");
    assert_eq!(castle(&fixture).gold, 0);
    assert_eq!(bank(&fixture), 1_000_700);
    assert_eq!(
        wallet(&fixture),
        gold,
        "tax belongs to Guild bank, not personal wallet"
    );
    let withdrawn = authority(&fixture);
    call(&mut fixture, "@sabuk:withdraw");
    assert_eq!(authority(&fixture), withdrawn);
}

#[test]
fn npc_conquest_owner_repairs_use_actual_quote_and_archers_use_separate_page() {
    let mut fixture = fixture(true, true);
    open_management(&mut fixture, "repairs");
    let repair_page = dialog(&fixture);
    assert!(repair_page
        .body
        .iter()
        .any(|line| line == "Gate 1: 2500/5000"));
    assert!(repair_page
        .body
        .iter()
        .any(|line| line == "Repair cost: 500 gold."));
    assert!(has_link(&fixture, "@sabuk:repair:gate:1"));
    assert!(!has_link(&fixture, "@sabuk:repair:archer:1"));
    let personal = wallet(&fixture);
    call(&mut fixture, "@sabuk:repair:gate:1");
    assert_eq!(castle(&fixture).defenses["gate:1"].hp, 5_000);
    assert_eq!(bank(&fixture), 999_500);
    open_management(&mut fixture, "archers");
    assert_eq!(
        dialog(&fixture)
            .links
            .iter()
            .filter(|link| link.target.starts_with("@sabuk:repair:archer:"))
            .count(),
        12
    );
    assert!(dialog(&fixture)
        .body
        .iter()
        .any(|line| line == "Archer 1: 0/9999"));
    assert!(!has_link(&fixture, "@sabuk:repair:gate:1"));
    call(&mut fixture, "@sabuk:repair:archer:1");
    assert_eq!(castle(&fixture).defenses["archer:1"].hp, 9_999);
    assert_eq!(bank(&fixture), 998_500);
    assert_eq!(wallet(&fixture), personal);
}

#[test]
fn npc_conquest_old_owner_management_links_cannot_charge_or_mutate_new_owner() {
    for (page, target) in [
        ("manage", "@sabuk:tax:25"),
        ("manage", "@sabuk:open"),
        ("repairs", "@sabuk:repair:gate:1"),
        ("archers", "@sabuk:repair:archer:1"),
        ("manage", "@sabuk:withdraw"),
    ] {
        let mut fixture = fixture(true, true);
        open_management(&mut fixture, page);
        assert!(has_link(&fixture, target));
        {
            let mut store = fixture.config.account_store.lock().unwrap();
            let record = store.shared_conquests.get_mut(&1).unwrap();
            record.owner_guild_id = Some(RAIDERS.into());
            record.revision += 1;
        }
        let before = authority(&fixture);
        let gold = wallet(&fixture);
        call(&mut fixture, target);
        assert_eq!(
            authority(&fixture),
            before,
            "old owner retained {target} permission"
        );
        assert_eq!(wallet(&fixture), gold);
        assert_eq!(dialog(&fixture).title, "Siege status");
        assert!(!has_link(&fixture, "@sabuk:manage"));
    }
}

#[test]
fn npc_conquest_repairs_fail_atomically_for_bank_shortage_and_persistence_failure() {
    for failed_persist in [false, true] {
        let mut fixture = fixture(true, true);
        if !failed_persist {
            fixture
                .config
                .account_store
                .lock()
                .unwrap()
                .shared_guilds
                .get_mut(KNIGHTS)
                .unwrap()
                .gold = 499;
        }
        open_management(&mut fixture, "repairs");
        let before = authority(&fixture);
        let gold = wallet(&fixture);
        if failed_persist {
            fixture.config.inject_account_store_transaction_fault(
                AccountStoreTransactionFault::BeforePersist,
            );
        }
        call(&mut fixture, "@sabuk:repair:gate:1");
        assert_eq!(authority(&fixture), before);
        assert_eq!(wallet(&fixture), gold);
        assert!(dialog(&fixture)
            .body
            .iter()
            .any(|line| line.contains("management failed")));
    }
}

#[test]
fn npc_conquest_normal_registrar_grants_guild_name_and_typed_return_once() {
    let mut fixture = fixture(false, false);
    let horn = mir2_game_data::crystal_item_by_name("WoomaHorn").unwrap();
    add_or_increment_item(
        fixture.session.app.world_mut(),
        ItemContainer::Bag1,
        &crystal_item_key_for_template(&horn),
        &horn.name,
        "",
        20,
        1,
        u16::from(horn.weight),
    );
    let player = player_entity(fixture.session.app.world()).unwrap();
    assert_eq!(
        fixture
            .session
            .app
            .world()
            .entity(player)
            .get::<CharacterBody>()
            .unwrap()
            .level,
        30
    );
    let before = authority(&fixture);
    assert!(!call(&mut fixture, "@CREATEGUILD")
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GuildNameRequest)));
    fixture
        .session
        .handle_packet(ClientPacket::GuildNameReturn {
            name: "Created Knights".into(),
        });
    assert_eq!(
        authority(&fixture),
        before,
        "no unsolicited name can create a Guild"
    );
    call(&mut fixture, "@Main");
    assert!(has_link(&fixture, "@CREATEGUILD"));
    assert!(call(&mut fixture, "@CREATEGUILD")
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GuildNameRequest)));
    let returned = fixture
        .session
        .handle_packet(ClientPacket::GuildNameReturn {
            name: "Created Knights".into(),
        });
    assert!(returned.iter().any(|packet| matches!(packet, ServerPacket::GuildStatus { guild_name, .. } if guild_name == "Created Knights")), "{returned:?}");
    assert_eq!(wallet(&fixture), 1_000_000);
    assert!(fixture
        .config
        .shared_guild_for_identity(&Stage5FriendIdentity {
            account_id: "hero".into(),
            character_index: 0
        })
        .unwrap()
        .is_some());
    let created = authority(&fixture);
    fixture
        .session
        .handle_packet(ClientPacket::GuildNameReturn {
            name: "Replay Knights".into(),
        });
    assert_eq!(authority(&fixture), created);
}

#[test]
fn npc_conquest_actual_guild_creation_grant_expires_at_npc_map_and_session_boundaries() {
    for boundary in [
        "range",
        "map",
        "missing",
        "replacement",
        "dead",
        "logout",
        "exit",
        "disconnect",
    ] {
        let mut fixture = fixture(false, false);
        grant_guild_creation(&mut fixture);
        match boundary {
            "range" => fixture.session.force_authoritative_player_transform(
                Point {
                    x: fixture.position.x + CRYSTAL_OBJECT_DATA_RANGE + 1,
                    y: fixture.position.y,
                },
                MirDirection::Left,
            ),
            "map" => {
                fixture
                    .session
                    .app
                    .world_mut()
                    .resource_mut::<MapRuntimeResource>()
                    .current_map
                    .file_name = "0".into()
            }
            "missing" => {
                let entity = entity_by_object_id(fixture.session.app.world(), REGISTRAR).unwrap();
                fixture.session.app.world_mut().despawn(entity);
            }
            "replacement" => {
                fixture.session.handle_packet(ClientPacket::CallNpc {
                    object_id: WRONG_NPC,
                    key: "@Main".into(),
                });
            }
            "dead" => fixture
                .session
                .force_authoritative_player_vitals(Some(0), None),
            "logout" => {
                fixture.session.handle_packet(ClientPacket::LogOut);
            }
            "exit" => {
                call(&mut fixture, "@Exit");
            }
            "disconnect" => {
                fixture.session.handle_packet(ClientPacket::Disconnect);
            }
            _ => unreachable!(),
        }
        let before = authority(&fixture);
        let gold = wallet(&fixture);
        let inventory = fixture.session.world_snapshot().inventory_items;
        let returned = fixture
            .session
            .handle_packet(ClientPacket::GuildNameReturn {
                name: "Stale Knights".into(),
            });
        assert!(!returned.iter().any(|packet| matches!(packet, ServerPacket::GuildStatus { guild_name, .. } if guild_name == "Stale Knights")));
        assert_eq!(
            authority(&fixture),
            before,
            "stale {boundary} name return created a Guild"
        );
        assert_eq!(
            wallet(&fixture),
            gold,
            "stale {boundary} name return charged personal gold"
        );
        assert_eq!(
            fixture.session.world_snapshot().inventory_items,
            inventory,
            "stale {boundary} name return consumed the Horn"
        );
        fixture
            .session
            .handle_packet(ClientPacket::GuildNameReturn {
                name: "Replay Knights".into(),
            });
        assert_eq!(
            authority(&fixture),
            before,
            "rejected grant must be consumed"
        );
    }
}
