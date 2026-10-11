//! Trusted health presentation cannot sign death, erase movement or settle loot.
use super::*;
use crate::routing::death_drop_anchors;

const ID: u32 = 8_010_001;

fn health(percent: u8) -> ServerPacket {
    ServerPacket::ObjectHealth {
        info: ObjectHealthInfo {
            object_id: ID,
            percent,
            expire: 5,
        },
    }
}

fn walk() -> ServerPacket {
    ServerPacket::ObjectWalk {
        movement: ObjectMovement {
            object_id: ID,
            position: Point { x: 332, y: 270 },
            direction: MirDirection::Right,
        },
    }
}

fn died() -> ServerPacket {
    ServerPacket::ObjectDied {
        info: ObjectDiedInfo {
            object_id: ID,
            location: Point { x: 333, y: 270 },
            direction: MirDirection::Down,
            kind: 0,
        },
    }
}

fn state(hp: Option<i32>, max_hp: Option<i32>) -> SharedInProcessZoneState {
    let mut state = SharedInProcessZoneState::new();
    let mut entity = shared_monster_entity(ID);
    entity.hp = hp;
    entity.max_hp = max_hp;
    state.sync_map_layer(
        "0".into(),
        vec![entity],
        BTreeSet::new(),
        vec![],
        BTreeSet::new(),
    );
    state
}

#[test]
fn health_zero_known_live_hp_does_not_create_corpse_or_disable_action() {
    let mut state = state(Some(1), Some(200));
    state.apply_shared_entity_packets("0", &[health(0), walk()]);
    let map = state.map_layer(Some("0")).unwrap();
    let entity = &map.entities[&ID];
    assert!(!entity.dead);
    assert_eq!(entity.hp, Some(1));
    assert_eq!((entity.x, entity.y), (332, 270));
    assert!(!map.dead_entity_ids.contains_key(&ID));
    assert!(state.shared_entity_allows_action("0", ID));
}

#[test]
fn health_zero_unknown_hp_and_before_spawn_have_no_death_authority() {
    let mut state = state(None, None);
    state.apply_shared_entity_packets("0", &[health(0)]);
    let map = state.map_layer(Some("0")).unwrap();
    assert!(!map.entities[&ID].dead);
    assert_eq!(map.entities[&ID].hp, None);
    assert!(!map.dead_entity_ids.contains_key(&ID));
    state.apply_shared_entity_packets(
        "0",
        &[ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: ID + 1,
                percent: 0,
                expire: 5,
            },
        }],
    );
    assert!(!state
        .map_layer(Some("0"))
        .unwrap()
        .dead_entity_ids
        .contains_key(&(ID + 1)));
}

#[test]
fn health_zero_cannot_authorize_death_drop_anchor_or_settlement() {
    let mut state = state(Some(1), Some(200));
    let map = state.map_layer(Some("0")).unwrap();
    assert!(death_drop_anchors(&map, &[health(0)]).is_empty());
    assert!(!packets_may_commit_shared_death_drops(&[health(0)]));
    assert_eq!(death_drop_anchors(&map, &[died()]).len(), 1);
    assert!(packets_may_commit_shared_death_drops(&[died()]));
    let attempted = state.commit_death_drops(
        "0",
        &[health(0)],
        &[shared_gold_drop(ID + 1, 330, 269, None, None)],
    );
    assert!(attempted.is_empty());
    let map = state.map_layer(Some("0")).unwrap();
    assert!(map.committed_death_drop_anchors.is_empty());
    assert!(map.ground_drops.is_empty());
}

#[test]
fn health_zero_owner_backlog_keeps_walk_and_real_death_rejects_old_actions() {
    let mut dead_ids = BTreeSet::new();
    let mut live = vec![health(0), walk()];
    filter_stale_owner_dead_entity_packets(&mut dead_ids, &mut live);
    assert_eq!(live.len(), 2);
    assert!(dead_ids.is_empty());
    let mut actual = vec![died(), health(100), walk()];
    filter_stale_owner_dead_entity_packets(&mut dead_ids, &mut actual);
    assert_eq!(actual, vec![died()]);
    assert_eq!(dead_ids, BTreeSet::from([ID]));
}

#[test]
fn health_zero_real_death_before_and_after_spawn_preserves_corpse_and_hp_zero() {
    let mut after = state(Some(1), Some(200));
    after.apply_shared_entity_packets("0", &[health(0), died(), health(100), walk()]);
    let map = after.map_layer(Some("0")).unwrap();
    assert!(map.entities[&ID].dead);
    assert_eq!(map.entities[&ID].hp, Some(0));
    assert_eq!((map.entities[&ID].x, map.entities[&ID].y), (333, 270));
    let mut before = SharedInProcessZoneState::new();
    before.apply_shared_entity_packets(
        "0",
        &[
            died(),
            health(0),
            ServerPacket::ObjectMonster {
                info: shared_monster_info(ID, 0),
            },
            health(100),
            walk(),
        ],
    );
    let map = before.map_layer(Some("0")).unwrap();
    assert!(map.entities[&ID].dead);
    assert_eq!(map.entities[&ID].hp, Some(0));
    assert_eq!((map.entities[&ID].x, map.entities[&ID].y), (333, 270));
}

#[test]
fn health_zero_registered_owner_full_channel_keeps_global_attack_until_one_wire_rebase() {
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
        Duration::from_secs(3600),
        BTreeMap::new(),
    ));
    let registry = ZoneRegistry::new(ZoneId::primary(), factory.clone());
    let mut owner = GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
    start_new_character(&mut owner, "health-zero-live-owner", "HealthZeroOwner");
    let key = ZonePresenceKey::from_identity(&owner.active_identity().unwrap());
    let shared = factory
        .resources_for_zone(owner.zone_id())
        .zone_state
        .clone();
    let object_id = shared.lock().unwrap().players[&key].zone_object_id;
    assert_ne!(object_id, 1000);
    shared.lock().unwrap().take_pending_zone_packets(&key);
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let registration = owner
        .register_zone_live_outbound(super::super::SharedZoneLiveOutboundSender::single(sender))
        .unwrap()
        .unwrap();
    registration.activate();
    while receiver.try_recv().is_ok() {}
    let original = vec![
        ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id,
                percent: 0,
                expire: 5,
            },
        },
        ServerPacket::ObjectAttack {
            info: mir2_protocol::ObjectAttackInfo {
                object_id,
                location: Point { x: 332, y: 270 },
                direction: MirDirection::Right,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        },
    ];
    shared
        .lock()
        .unwrap()
        .queue_zone_packets(key.clone(), original.clone());
    let first = receiver.try_recv().unwrap().into_packet();
    assert!(
        matches!(first, ServerPacket::ObjectHealth { info } if info.object_id == 1000 && info.percent == 0)
    );
    assert_eq!(
        shared.lock().unwrap().pending_zone_packets[&key],
        original[1..]
    );
    shared
        .lock()
        .unwrap()
        .retry_pending_realtime_zone_outbounds();
    let second = receiver.try_recv().unwrap().into_packet();
    assert!(
        matches!(second, ServerPacket::ObjectAttack { ref info } if info.object_id == 1000),
        "actual wire packet: {second:?}"
    );
    shared
        .lock()
        .unwrap()
        .retry_pending_realtime_zone_outbounds();
    assert!(receiver.try_recv().is_err());
    assert!(!shared
        .lock()
        .unwrap()
        .pending_zone_packets
        .contains_key(&key));
}

mod ordinary_transport {
    use super::*;
    use crate::tcp::chat_broadcast::ChatBroadcastHub;
    use futures_util::{SinkExt, StreamExt};
    use mir2_protocol::{decode_server_packet, encode_client_packet};
    use mir2_simulation::{AccountRecord, CharacterRecord, MonsterSpawnSource};
    use serde_json::{json, Value};
    use std::{
        ffi::OsString,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{client::IntoClientRequest, Message},
        MaybeTlsStream, WebSocketStream,
    };

    const MAP: &str = "health-zero-source-transport";
    const PASSWORD: &str = "isolated-health-zero-password";
    type WebClient = WebSocketStream<MaybeTlsStream<TcpStream>>;

    struct Environment {
        inherited: Vec<(OsString, OsString)>,
        directory: PathBuf,
    }
    impl Environment {
        fn new() -> Self {
            let inherited: Vec<_> = std::env::vars_os()
                .filter(|(name, _)| name.to_string_lossy().starts_with("MIR2_"))
                .collect();
            for (name, _) in &inherited {
                std::env::remove_var(name);
            }
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir()
                .join(format!("mir2-health-zero-{}-{nonce}", std::process::id()));
            std::env::set_var("MIR2_RUNTIME_ENV", "test");
            std::env::set_var(
                "MIR2_IDENTITY_SESSION_SECRET",
                "isolated-health-zero-session-secret-32",
            );
            std::env::set_var(
                "MIR2_IDENTITY_RECOVERY_PEPPER",
                "isolated-health-zero-recovery-pepper-32",
            );
            std::env::set_var("MIR2_AI_LIVE_DATA_DIR", &directory);
            Self {
                inherited,
                directory,
            }
        }
    }
    impl Drop for Environment {
        fn drop(&mut self) {
            for (name, _) in std::env::vars_os()
                .filter(|(name, _)| name.to_string_lossy().starts_with("MIR2_"))
                .collect::<Vec<_>>()
            {
                std::env::remove_var(name);
            }
            for (name, value) in &self.inherited {
                std::env::set_var(name, value);
            }
            if self.directory.is_absolute()
                && self.directory.parent() == Some(std::env::temp_dir().as_path())
            {
                let _ = std::fs::remove_dir_all(&self.directory);
            }
        }
    }
    struct Servers(Vec<tokio::task::JoinHandle<std::io::Result<()>>>);
    impl Drop for Servers {
        fn drop(&mut self) {
            for server in &self.0 {
                server.abort();
            }
        }
    }

    fn prepared(environment: &Environment) -> GatewayConfig {
        let mut config = GatewayConfig::default()
            .with_crystal_world_runtime()
            .with_account_store_path(environment.directory.join("accounts.json"))
            .with_save_recovery_dir(environment.directory.join("recovery"))
            .with_save_recovery_mac_key(std::array::from_fn::<u8, 32, _>(|index| index as u8 + 93))
            .unwrap();
        config.monster_spawn_source = MonsterSpawnSource::StarterScenario;
        config.map.file_name = MAP.into();
        config.map.title = "Isolated health source fixture".into();
        config.spawn = Point { x: 10, y: 10 };
        config.map_collision.map_file_name = MAP.into();
        config.map_collision.map_width = 40;
        config.map_collision.map_height = 40;
        config.map_collision.play_bounds = mir2_game_data::MapBounds {
            min_x: 0,
            max_x: 39,
            min_y: 0,
            max_y: 39,
        };
        config.map_collision.region_bounds = config.map_collision.play_bounds.clone();
        config.map_collision.blocked_cells.clear();
        config.map_collision.doors.clear();
        config.visible_players.clear();
        config.visible_monsters.clear();
        config.visible_npcs.clear();
        config.safe_zones.clear();
        config.map_transfers.clear();
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        for (account, index, name, y) in [
            ("health-zero-tcp", 1, "ZeroTcp", 10),
            ("health-zero-web", 2, "ZeroWeb", 11),
        ] {
            let mut record = AccountRecord::new(CharacterRecord {
                index,
                name: name.into(),
                level: 30,
                class: MirClass::Warrior,
                gender: MirGender::Male,
            });
            record.password = PASSWORD.into();
            let save = record.saves.get_mut(&index).unwrap();
            save.map_file_name = MAP.into();
            save.map_title = config.map.title.clone();
            save.position = Point { x: 10, y };
            // Level30 Warrior's real Source max HP is419. Only prepared current HP is1.
            save.hp = 1;
            save.max_hp = 419;
            save.mp = 116;
            save.max_mp = 116;
            save.max_experience = config.experience_required_for_level(30);
            store.accounts.insert(account.into(), record);
        }
        drop(store);
        config
    }

    async fn tcp_send(client: &mut TcpStream, packet: ClientPacket) {
        client
            .write_all(&encode_client_packet(&packet).unwrap())
            .await
            .unwrap();
    }
    async fn tcp_until(
        client: &mut TcpStream,
        expected: impl Fn(&ServerPacket) -> bool,
    ) -> Vec<ServerPacket> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut packets = Vec::new();
        loop {
            let frame = tokio::time::timeout_at(deadline, async {
                let mut header = [0; 2];
                client.read_exact(&mut header).await?;
                let length = u16::from_le_bytes(header) as usize;
                assert!(length >= 4);
                let mut frame = vec![0; length];
                frame[..2].copy_from_slice(&header);
                client.read_exact(&mut frame[2..]).await?;
                Ok::<_, std::io::Error>(frame)
            })
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "TCP response absent; received {:?}",
                    packets
                        .iter()
                        .map(mir2_protocol::server_packet_name)
                        .collect::<Vec<_>>()
                )
            })
            .unwrap();
            let packet = decode_server_packet(&frame).unwrap();
            let done = expected(&packet);
            packets.push(packet);
            if done {
                return packets;
            }
        }
    }
    async fn web_send(client: &mut WebClient, event: Value) {
        client
            .send(Message::Text(event.to_string().into()))
            .await
            .unwrap();
    }
    fn named(event: &Value, name: &str) -> bool {
        event["type"] == "packet" && event["packet"] == name
    }
    async fn web_until(client: &mut WebClient, expected: impl Fn(&Value) -> bool) -> Vec<Value> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut events = Vec::new();
        loop {
            let message = tokio::time::timeout_at(deadline, client.next())
                .await
                .unwrap_or_else(|_| {
                    panic!(
                        "WS response absent; received {:?}",
                        events
                            .iter()
                            .map(|e: &Value| e["packet"].clone())
                            .collect::<Vec<_>>()
                    )
                })
                .unwrap()
                .unwrap();
            let event: Value = match message {
                Message::Text(text) => serde_json::from_str(&text).unwrap(),
                Message::Ping(bytes) => {
                    client.send(Message::Pong(bytes)).await.unwrap();
                    continue;
                }
                _ => continue,
            };
            assert!(event["type"] != "error", "normal packet rejected: {event}");
            let done = expected(&event);
            events.push(event);
            if done {
                return events;
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn health_zero_normal_tcp_ws_group_health_keeps_both_players_moving() {
        let environment = Environment::new();
        let config = prepared(&environment);
        let factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
        let registry = Arc::new(ZoneRegistry::new(ZoneId::primary(), factory.clone()));
        let hub = ChatBroadcastHub::from_env().unwrap();
        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let web_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_address = tcp_listener.local_addr().unwrap();
        let web_address = web_listener.local_addr().unwrap();
        let servers = Servers(vec![
            tokio::spawn(crate::tcp::serve_tcp_gateway_with_zone_registry(
                tcp_listener,
                config.clone(),
                hub.clone(),
                registry.clone(),
            )),
            tokio::spawn(crate::web::serve_web_gateway_with_zone_registry(
                web_listener,
                config.clone(),
                hub,
                registry,
            )),
        ]);
        let mut tcp = TcpStream::connect(tcp_address).await.unwrap();
        tcp_send(
            &mut tcp,
            ClientPacket::Login {
                account_id: "health-zero-tcp".into(),
                password: PASSWORD.into(),
            },
        )
        .await;
        tcp_until(&mut tcp, |p| matches!(p, ServerPacket::LoginSuccess { .. })).await;
        tcp_send(&mut tcp, ClientPacket::StartGame { character_index: 1 }).await;
        let start = tcp_until(&mut tcp, |p| {
            matches!(p, ServerPacket::UserInformation { .. })
        })
        .await;
        assert!(start
            .iter()
            .any(|p| matches!(p, ServerPacket::UserInformation { info } if info.hp == 1)));
        let source_vitals = {
            let resources = factory.resources_for_zone(&ZoneId::primary());
            let state = resources.zone_state.lock().unwrap();
            let (key, _) = state
                .players
                .iter()
                .find(|(_, p)| p.entity.name == "ZeroTcp")
                .unwrap();
            state
                .zone_manager
                .player_vitals(&state.zone_sessions[key])
                .unwrap()
        };
        assert_eq!(source_vitals.0, 1);
        assert!(source_vitals.1 >= 200);
        let mut request = format!("ws://{web_address}/ws")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("origin", format!("http://{web_address}").parse().unwrap());
        let (mut web, _) = connect_async(request).await.unwrap();
        web_send(
            &mut web,
            json!({"type":"login","accountId":"health-zero-web","password":PASSWORD}),
        )
        .await;
        web_until(&mut web, |e| named(e, "LoginSuccess")).await;
        web_send(&mut web, json!({"type":"startGame","characterIndex":2})).await;
        let initial = web_until(&mut web, |e| {
            named(e, "ObjectPlayer") && e["payload"]["name"] == "ZeroTcp"
        })
        .await;
        let owner_id = initial
            .iter()
            .find(|e| named(e, "ObjectPlayer") && e["payload"]["name"] == "ZeroTcp")
            .unwrap()["payload"]["objectId"]
            .as_u64()
            .unwrap() as u32;
        let initial = tcp_until(
            &mut tcp,
            |p| matches!(p, ServerPacket::ObjectPlayer { info } if info.name == "ZeroWeb"),
        )
        .await;
        let peer_id = initial
            .iter()
            .find_map(|p| match p {
                ServerPacket::ObjectPlayer { info } if info.name == "ZeroWeb" => {
                    Some(info.object_id)
                }
                _ => None,
            })
            .unwrap();
        tcp_send(&mut tcp, ClientPacket::SwitchGroup { allow_group: true }).await;
        tcp_until(&mut tcp, |p| {
            matches!(p, ServerPacket::SwitchGroup { allow_group: true })
        })
        .await;
        web_send(&mut web, json!({"type":"switchGroup","allowGroup":true})).await;
        web_until(&mut web, |e| named(e, "SwitchGroup")).await;
        tcp_send(
            &mut tcp,
            ClientPacket::AddMember {
                name: "ZeroWeb".into(),
            },
        )
        .await;
        web_until(&mut web, |e| named(e, "GroupInvite")).await;
        web_send(&mut web, json!({"type":"groupInvite","acceptInvite":true})).await;
        tcp_until(&mut tcp, |p| matches!(p, ServerPacket::ObjectHealth { info } if info.object_id == peer_id && info.percent == 0)).await;
        web_until(&mut web, |e| {
            named(e, "ObjectHealth")
                && e["payload"]["objectId"] == owner_id
                && e["payload"]["percent"] == 0
        })
        .await;
        tcp_send(
            &mut tcp,
            ClientPacket::Walk {
                direction: MirDirection::Right,
            },
        )
        .await;
        tcp_until(&mut tcp, |p| matches!(p, ServerPacket::UserLocation { location } if location.position == (Point { x: 11, y: 10 }))).await;
        web_until(&mut web, |e| {
            named(e, "ObjectWalk") && e["payload"]["objectId"] == owner_id
        })
        .await;
        web_send(&mut web, json!({"type":"walk","direction":"Right"})).await;
        web_until(&mut web, |e| {
            named(e, "UserLocation") && e["payload"]["x"] == 11 && e["payload"]["y"] == 11
        })
        .await;
        tcp_until(&mut tcp, |p| matches!(p, ServerPacket::ObjectWalk { movement } if movement.object_id == peer_id && movement.position == (Point { x: 11, y: 11 }))).await;
        {
            let resources = factory.resources_for_zone(&ZoneId::primary());
            let state = resources.zone_state.lock().unwrap();
            let map = state.map_layer(Some(MAP)).unwrap();
            assert!(!map.dead_entity_ids.contains_key(&owner_id));
            assert!(!map.dead_entity_ids.contains_key(&peer_id));
        }
        let store = config.account_store.lock().unwrap();
        assert_eq!(store.accounts["health-zero-tcp"].saves[&1].pk_points, 0);
        assert_eq!(store.accounts["health-zero-web"].saves[&2].pk_points, 0);
        drop(store);
        drop(tcp);
        web.close(None).await.unwrap();
        drop(web);
        for server in &servers.0 {
            server.abort();
        }
    }
}
