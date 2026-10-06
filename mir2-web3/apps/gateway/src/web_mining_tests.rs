// Included in web::tests. This witness uses an actual Axum/TCP WebSocket,
// ordinary native JSON commands and a unique owned account file.
mod mining_websocket_tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use std::{fs, io::Read, path::PathBuf};

    struct MapEnvironment(Option<std::ffi::OsString>);
    impl MapEnvironment {
        fn enter() -> Self {
            let previous = std::env::var_os("MIR2_CRYSTAL_MAP_PACK");
            let path = previous.clone().map(PathBuf::from).unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("src/routing/tests/fixtures/mining-map-pack")
            });
            assert!(
                path.join("0.map.gz").is_file() && path.join("d401.map.gz").is_file(),
                "source Bichon and D401 are required: {path:?}"
            );
            std::env::set_var("MIR2_CRYSTAL_MAP_PACK", path);
            mir2_simulation::set_crystal_full_world_zone_collision(true);
            Self(previous)
        }
    }
    impl Drop for MapEnvironment {
        fn drop(&mut self) {
            mir2_simulation::set_crystal_full_world_zone_collision(false);
            match &self.0 {
                Some(value) => std::env::set_var("MIR2_CRYSTAL_MAP_PACK", value),
                None => std::env::remove_var("MIR2_CRYSTAL_MAP_PACK"),
            }
        }
    }

    fn source_wall() -> (Point, Point) {
        let path =
            PathBuf::from(std::env::var_os("MIR2_CRYSTAL_MAP_PACK").unwrap()).join("d401.map.gz");
        let compressed = fs::read(path).unwrap();
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(compressed.as_slice())
            .read_to_end(&mut bytes)
            .unwrap();
        let v100 = bytes[0..4] == [1, 0, 0x43, 0x23];
        let read16 = |offset| i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
        let (width, height, start, xor) = if v100 {
            (i32::from(read16(4)), i32::from(read16(6)), 8, 0)
        } else {
            assert_eq!(
                (bytes[0], bytes[2], bytes[7], bytes[14]),
                (0x10, 0x61, 0x31, 0x31),
                "source D401 must use known original v1 or canonical v100 cells"
            );
            let xor = read16(23);
            (
                i32::from(read16(21) ^ xor),
                i32::from(read16(25) ^ xor),
                54,
                xor,
            )
        };
        assert!(width > 0 && height > 0);
        let mut walkable = vec![false; (width * height) as usize];
        let mut offset = start;
        for x in 0..width {
            for y in 0..height {
                let (high, low, door) = if v100 {
                    let high =
                        i32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap())
                            & 0x2000_0000
                            != 0;
                    let low = read16(offset + 12) & i16::MIN != 0;
                    let door = bytes[offset + 14] > 0;
                    offset += 26;
                    (high, low, door)
                } else {
                    let high = (i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
                        ^ 0xAA38_AA38_u32 as i32)
                        & 0x2000_0000
                        != 0;
                    let low = (read16(offset + 6) ^ xor) & i16::MIN != 0;
                    let door = bytes[offset + 8] > 0;
                    offset += 15;
                    (high, low, door)
                };
                walkable[(x * height + y) as usize] = !high && !low && !door;
            }
        }
        assert_eq!(offset, bytes.len());
        let mut cells = Vec::new();
        for x in 1..width - 1 {
            for y in 1..height - 1 {
                if walkable[(x * height + y) as usize] && !walkable[(x * height + y - 1) as usize] {
                    cells.push((Point { x, y }, Point { x, y: y - 1 }));
                }
            }
        }
        cells.sort_by_key(|(p, _)| (p.x - 24).abs() + (p.y - 181).abs());
        cells.into_iter().next().unwrap()
    }

    fn shared_mining(factory: &crate::routing::SharedInProcessZoneRuntimeFactory) -> Value {
        let checkpoint: Value =
            serde_json::from_slice(&factory.world_checkpoint_bytes().unwrap()).unwrap();
        for shared in checkpoint["zones"].as_object().unwrap().values() {
            let bytes = STANDARD
                .decode(shared["zoneManagerBytes"].as_str().unwrap())
                .unwrap();
            let manager: Value = serde_json::from_slice(&bytes).unwrap();
            for zone in manager["zones"].as_array().unwrap() {
                if zone[0]["map_file_name"] == "D401" {
                    let bytes = STANDARD.decode(zone[1].as_str().unwrap()).unwrap();
                    let runtime: Value = serde_json::from_slice(&bytes).unwrap();
                    return runtime["mining"].clone();
                }
            }
        }
        panic!("the actual Gateway factory must contain its D401 mining runtime");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mining_real_websocket_attack_direction_refill_cooldown_logout_and_reentry() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _environment = MapEnvironment::enter();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("mir2-mining-ws-{}-{nanos}", std::process::id()));
        let path = root.join("accounts.json");
        let config = Arc::new(
            SimulationConfig::default()
                .with_crystal_world_runtime()
                .with_platinum_176_profile()
                .with_account_store_path(path.clone())
                .with_save_recovery_dir(root.join("recovery"))
                .with_save_recovery_mac_key(std::array::from_fn::<_, 32, _>(|index| {
                    (index as u8).wrapping_mul(11)
                }))
                .unwrap(),
        );
        let factory = Arc::new(crate::routing::SharedInProcessZoneRuntimeFactory::new());
        let registry = Arc::new(crate::ZoneRegistry::new(
            crate::ZoneId::primary(),
            factory.clone(),
        ));
        let state = super::super::WebState {
            config: config.clone(),
            deploy_revision: None,
            zone_registry: registry,
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::new(super::super::ReconnectSessionStore::default()),
            capacity: Arc::new(super::super::GatewayCapacityState::unlimited()),
            gameplay_event_sink: None,
            identity: Arc::new(crate::identity::IdentityService::local_for_tests()),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator: snapshot_test_spectator(false),
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(root.join("ai")),
            )
            .unwrap(),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
        };
        let app = axum::Router::new()
            .route("/ws", axum::routing::get(super::super::ws_upgrade))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        let mut request = format!("ws://{address}/ws").into_client_request().unwrap();
        let origin = std::env::var("MIR2_ALLOWED_WEB_ORIGINS")
            .ok()
            .and_then(|s| {
                s.split(',')
                    .map(str::trim)
                    .find(|s| !s.is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| format!("http://{address}"));
        request
            .headers_mut()
            .insert("origin", origin.parse().unwrap());
        let (mut socket, response) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(response.status(), 101);
        send_test_websocket_json(
            &mut socket,
            json!({"type":"newAccount","accountId":"MiningSocket",
            "password":"MiningSocketPass123","birthDateBinary":0,"userName":"Mining QA",
            "secretQuestion":"q","secretAnswer":"a","emailAddress":""}),
        )
        .await;
        let (account, _) =
            read_test_websocket_until(&mut socket, "mining ordinary new account", |e| {
                test_packet(e, "NewAccount")
            })
            .await;
        assert_eq!(account["payload"]["result"], 8);
        send_test_websocket_json(
            &mut socket,
            json!({"type":"login","accountId":"MiningSocket","password":"MiningSocketPass123"}),
        )
        .await;
        read_test_websocket_until(&mut socket, "mining ordinary login", |e| {
            test_packet(e, "LoginSuccess")
        })
        .await;
        send_test_websocket_json(
            &mut socket,
            json!({"type":"newCharacter","name":"MineWire","gender":"Male","class":"Warrior"}),
        )
        .await;
        let (created, _) =
            read_test_websocket_until(&mut socket, "mining ordinary new character", |e| {
                test_packet(e, "NewCharacterSuccess")
            })
            .await;
        let index = created["payload"]["character"]["index"].as_i64().unwrap() as i32;
        let (position, wall) = source_wall();
        let info = mir2_game_data::crystal_item_by_name("PickAxe").unwrap();
        {
            // Source-faithful progression/tool preparation before StartGame;
            // no admin command is accepted from this socket.
            let mut store = config.account_store.lock().unwrap();
            let save = store
                .accounts
                .get_mut("MiningSocket")
                .unwrap()
                .saves
                .get_mut(&index)
                .unwrap();
            save.character.level = 12;
            save.map_file_name = "D401".into();
            save.map_title = "DeadMineEntrance".into();
            save.position = position.clone();
            save.direction = MirDirection::Up;
            save.equipment_items_json=vec![json!({
                "key":format!("crystal-item-{}",info.item_index),"slot":"weapon","quantity":1,
                "name":info.name,"icon":info.image,"shape":info.shape,"description":"source mining wire fixture",
                "durability_current":10_000,"durability_max":10_000,"attack":0,"defence":0,
                "user_item_unique_id":700_001,"user_item_metadata":{"item_index":info.item_index}
            }).to_string()];
        }
        config.save_account_store().unwrap();
        send_test_websocket_json(
            &mut socket,
            json!({"type":"startGame","characterIndex":index}),
        )
        .await;
        let (started, _) = read_test_websocket_until(&mut socket, "mining StartGame", |e| {
            test_packet(e, "StartGame")
        })
        .await;
        assert_eq!(started["payload"]["result"], 4);
        send_test_websocket_json(
            &mut socket,
            json!({"type":"attackDirection","direction":"Up","spell":0}),
        )
        .await;
        let (attack, observed) =
            read_test_websocket_until(&mut socket, "actual mining ObjectAttack", |e| {
                test_packet(e, "ObjectAttack")
                    && e["payload"]["location"]["x"] == position.x
                    && e["payload"]["location"]["y"] == position.y
            })
            .await;
        assert_eq!(attack["payload"]["spell"], 0);
        assert_eq!(attack["payload"]["direction"], "Up");
        assert!(!observed
            .iter()
            .any(|e| test_packet(e, "GainedItem") || test_packet(e, "DuraChanged")));
        let mining = shared_mining(&factory);
        assert_eq!(mining["sequence"], 1);
        assert!(
            mining["spots"][format!("{},{}", wall.x, wall.y)]["stones_left"]
                .as_u64()
                .unwrap()
                < 80
        );
        send_test_websocket_json(
            &mut socket,
            json!({"type":"attackDirection","direction":"Up","spell":0}),
        )
        .await;
        send_test_websocket_json(&mut socket, json!({"type":"keepAlive","time":700002})).await;
        let (_, spam) =
            read_test_websocket_until(&mut socket, "wire mining cooldown barrier", |e| {
                test_packet(e, "KeepAlive") && e["payload"]["time"] == 700002
            })
            .await;
        assert!(!spam.iter().any(|e| test_packet(e, "ObjectAttack")
            || test_packet(e, "GainedItem")
            || test_packet(e, "DuraChanged")));
        assert_eq!(shared_mining(&factory), mining);
        send_test_websocket_json(&mut socket, json!({"type":"logOut"})).await;
        read_test_websocket_until(&mut socket, "wire mining saved logout", |e| {
            test_packet(e, "LogOutSuccess")
        })
        .await;
        assert_eq!(shared_mining(&factory), mining);
        let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(
            saved.to_string().contains("700001"),
            "actual account file retains the exact tool UID"
        );
        send_test_websocket_json(
            &mut socket,
            json!({"type":"startGame","characterIndex":index}),
        )
        .await;
        let (reentry, _) = read_test_websocket_until(&mut socket, "mining ordinary reentry", |e| {
            test_packet(e, "StartGame")
        })
        .await;
        assert_eq!(reentry["payload"]["result"], 4);
        assert_eq!(shared_mining(&factory), mining);
        send_test_websocket_json(&mut socket, json!({"type":"logOut"})).await;
        read_test_websocket_until(&mut socket, "mining final normal logout", |e| {
            test_packet(e, "LogOutSuccess")
        })
        .await;
        socket.close(None).await.unwrap();
        server.abort();
    }
}
