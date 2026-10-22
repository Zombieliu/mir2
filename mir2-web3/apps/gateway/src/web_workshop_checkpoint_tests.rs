// Real Axum/TCP WebSocket + Shared Zone + File Source. Stone setup uses a
// controlled PickAxe/map wall and genuine Zone mining capability, never a raw
// carrier or registry grant. Atelier retains its controlled initial copper.
mod workshop_checkpoint_websocket_tests {
    use super::*;
    use mir2_simulation::{
        AccountStoreTransactionFault, CharacterSaveRecord, SimulationSession, ZoneBounds,
        ZoneCollision, ZoneCommand, ZoneKey, ZoneRuntime,
    };
    use std::{ffi::OsString, fs, path::PathBuf};

    struct Environment(Vec<(&'static str, Option<OsString>)>);
    impl Environment {
        fn enter(idle: bool) -> Self {
            let map = std::env::var_os("MIR2_CRYSTAL_MAP_PACK")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("src/routing/tests/fixtures/mining-map-pack")
                });
            assert!(map.join("0.map.gz").is_file());
            let values = [
                ("MIR2_STONE_WORKSHOPS", OsString::from("1")),
                ("MIR2_PLAYER_ATELIER", OsString::from("1")),
                ("MIR2_CRYSTAL_MAP_PACK", map.into_os_string()),
                (
                    "MIR2_GATEWAY_SAVE_QUEUE_LIMIT",
                    OsString::from(if idle { "65535" } else { "1" }),
                ),
                ("MIR2_GATEWAY_SAVE_DEBOUNCE_MS", OsString::from("600000")),
                (
                    "MIR2_GATEWAY_SAVE_CHECKPOINT_SECONDS",
                    OsString::from(if idle { "1" } else { "600" }),
                ),
                ("MIR2_GATEWAY_RUNTIME_TICK_MS", OsString::from("100")),
                (
                    "MIR2_GATEWAY_RUNTIME_TICK_BOOTSTRAP_GRACE_MS",
                    OsString::from("0"),
                ),
            ];
            let mut previous = Vec::new();
            for (key, value) in values {
                previous.push((key, std::env::var_os(key)));
                std::env::set_var(key, value);
            }
            mir2_simulation::set_crystal_full_world_zone_collision(true);
            Self(previous)
        }
    }
    impl Drop for Environment {
        fn drop(&mut self) {
            mir2_simulation::set_crystal_full_world_zone_collision(false);
            for (key, value) in self.0.drain(..).rev() {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    // File Source can perform several complete fsync-backed commits for one
    // ordinary action. Keep this budget local, without retrying any command or
    // weakening the required economic/menu acknowledgement.
    async fn read_workshop_until(
        socket: &mut TestWebSocket,
        label: &str,
        predicate: impl Fn(&Value) -> bool,
    ) -> (Value, Vec<Value>) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        let mut observed = Vec::<Value>::new();
        loop {
            let message = tokio::time::timeout_at(deadline, socket.next())
                .await
                .unwrap_or_else(|_| {
                    let kinds: Vec<_> = observed
                        .iter()
                        .rev()
                        .take(24)
                        .map(|event| {
                            event["packet"]
                                .as_str()
                                .or_else(|| event["type"].as_str())
                                .unwrap_or("unknown")
                        })
                        .collect();
                    panic!("timed out waiting for {label}; last frame kinds={kinds:?}")
                })
                .unwrap_or_else(|| panic!("WebSocket closed waiting for {label}"))
                .unwrap_or_else(|error| panic!("WebSocket failed waiting for {label}: {error}"));
            match message {
                ClientWebSocketMessage::Text(text) => {
                    let event: Value = serde_json::from_str(text.as_ref()).unwrap();
                    observed.push(event.clone());
                    if predicate(&event) {
                        return (event, observed);
                    }
                    assert_ne!(
                        event["type"], "error",
                        "unexpected error waiting for {label}: {}",
                        event["message"]
                    );
                }
                ClientWebSocketMessage::Close(_) => panic!("WebSocket closed waiting for {label}"),
                _ => {}
            }
        }
    }

    fn prepare_source_fixture(config: &SimulationConfig, atelier: bool) -> i32 {
        // Complete normal account/character creation synchronously before any
        // WebSocket exists. No connected Source may race this initial fixture.
        let mut source = SimulationSession::new(config.clone());
        assert!(source
            .try_handle_packet(ClientPacket::NewAccount {
                account_id: "WorkshopSocket".into(),
                password: "WorkshopPass123".into(),
                birth_date_binary: 0,
                user_name: "Workshop fixture".into(),
                secret_question: "q".into(),
                secret_answer: "a".into(),
                email_address: String::new(),
            })
            .unwrap()
            .iter()
            .any(|p| matches!(p, ServerPacket::NewAccount { result: 8 })));
        assert!(source
            .try_handle_packet(ClientPacket::Login {
                account_id: "WorkshopSocket".into(),
                password: "WorkshopPass123".into(),
            })
            .unwrap()
            .iter()
            .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
        let index = source
            .try_handle_packet(ClientPacket::NewCharacter {
                name: "WorkshopWire".into(),
                gender: MirGender::Male,
                class: MirClass::Warrior,
            })
            .unwrap()
            .into_iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .expect("ordinary Source character creation");
        assert!(
            source.active_identity().is_none(),
            "fixture has never entered StartGame"
        );
        drop(source);
        {
            // Only initial isolated Source fixture preparation. All displayed
            // NPC navigation and economic mutations below use ordinary JSON.
            let ore =
                mir2_game_data::crystal_item_by_index(if atelier { 824 } else { 836 }).unwrap();
            let mut store = config.account_store.lock().unwrap();
            let account = store.accounts.get_mut("WorkshopSocket").unwrap();
            account
                .characters
                .iter_mut()
                .find(|c| c.index == index)
                .unwrap()
                .level = 12;
            let save = account.saves.get_mut(&index).unwrap();
            save.character.level = 12;
            save.gold = 10000;
            save.map_file_name = "0".into();
            save.map_title = "BichonProvince".into();
            save.position = if atelier {
                Point { x: 321, y: 266 }
            } else {
                Point { x: 302, y: 222 }
            };
            save.inventory_items_json = (0..if atelier { 3 } else { 1 }).map(|slot| json!({
                "key":format!("crystal-item-{}", if atelier { 824 } else { 836 }),"name":if atelier { "CopperOre" } else { ore.name.as_str() },"icon":ore.image,"slot":slot,
                "unique_id":9000001u64+slot,"container":"bag1","quantity":1,"description":if atelier { "Source ore fixture" } else { "Controlled mining PickAxe fixture" },
                "durability_current":if atelier { 3000 } else { ore.durability },"durability_max":ore.durability,"weight":ore.weight,"equip_slot":if atelier { None::<&str> } else { Some("weapon") },
                "attack":0,"defence":0,"heal_hp":0,"heal_mp":0,
                "user_item_metadata":{"item_index":if atelier { 824 } else { 836 }}
            }).to_string()).collect();
            save.belt_items_json.clear();
        }
        config.save_account_store().unwrap();
        if !atelier {
            prepare_mined_raw(config, index);
        }
        index
    }

    fn prepare_mined_raw(config: &SimulationConfig, index: i32) {
        let mut source = SimulationSession::new(config.clone());
        source
            .try_handle_packet(ClientPacket::Login {
                account_id: "WorkshopSocket".into(),
                password: "WorkshopPass123".into(),
            })
            .unwrap();
        source
            .try_handle_packet(ClientPacket::StartGame {
                character_index: index,
            })
            .unwrap();
        let equipped = source
            .try_handle_packet(ClientPacket::EquipItem {
                grid: mir2_protocol::MirGridType::Inventory,
                unique_id: 9_000_001,
                to: 0,
            })
            .unwrap();
        assert!(equipped
            .iter()
            .any(|p| matches!(p, ServerPacket::EquipItem { success: true, .. })));
        let mut mining = source.active_character_checkpoint().unwrap();
        mining.map_file_name = "D401".into();
        mining.map_title = "DeadMineEntrance".into();
        mining.position = Point { x: 24, y: 181 };
        mining.direction = mir2_protocol::MirDirection::Up;
        source.restore_active_character_checkpoint(&mining).unwrap();
        source.save_active_character().unwrap();
        source.begin_stone_workshops().unwrap();
        let join = source
            .active_zone_join_snapshot("workshop-source-mining")
            .unwrap();
        let sid = join.session_id.clone();
        let mut zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("D401"),
            ZoneCollision::unbounded()
                .with_bounds(ZoneBounds::new(0, 511, 0, 511))
                .with_blocked_cells([Point { x: 24, y: 180 }]),
        );
        zone.handle(ZoneCommand::Join(join));
        // Join deliberately does not authorize combat/harvest. Match the
        // Gateway's authoritative_zone_combat_commands using the live personal
        // snapshot; missing admission fields must fail rather than default.
        let snapshot = source.world_snapshot();
        let player = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == mir2_simulation::WorldEntityKind::SelfPlayer)
            .unwrap();
        let attack_blocked = snapshot.active_buffs.iter().any(|buff| {
            matches!(
                buff.key.as_str(),
                "crystal-paralysis" | "crystal-dazed" | "crystal-frozen" | "crystal-blindness"
            )
        });
        zone.handle(ZoneCommand::sync_player_combat_state(
            sid.clone(),
            player.class.unwrap(),
            player.has_class_weapon.unwrap(),
            player.riding_mount.unwrap(),
            player.can_mount_attack.unwrap(),
            player.dead || snapshot.player_hp.unwrap() <= 0,
            attack_blocked,
            player.fishing.unwrap(),
        ));
        zone.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: sid.clone(),
            stats: source.zone_player_combat_stats(),
        });
        let mut now = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64)
            .saturating_add(10_000);
        assert!(
            zone.player_harvest_admitted(&sid, now),
            "trusted source must admit mining"
        );
        let tool = source.shared_mining_tool().unwrap();
        let mut refilled = false;
        for _ in 0..5 {
            let swing = zone
                .prepare_mining_swing(&sid, mir2_protocol::MirDirection::Up, &tool, now)
                .unwrap();
            assert_eq!(swing.tool_damage(), 0);
            assert!(source.apply_shared_mining_swing(&swing).unwrap().is_empty());
            assert!(zone.commit_mining_swing(&swing).is_some());
            let state: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
            let spot = &state["mining"]["spots"]["24,180"];
            if spot["stones_left"].as_u64().unwrap() > 0 {
                refilled = true;
                break;
            }
            now = spot["regen_at_ms"].as_u64().unwrap() + 1;
        }
        assert!(refilled, "bounded source mining refill");
        let mut mined = false;
        for _ in 0..8192 {
            now = now.saturating_add(tool.attack_delay_ms.max(550));
            let swing = zone
                .prepare_mining_swing(&sid, mir2_protocol::MirDirection::Up, &tool, now)
                .unwrap();
            if swing.raw_stone_template_id() != Some(50001) {
                continue;
            }
            let packets = source.apply_shared_mining_swing(&swing).unwrap();
            assert!(zone.commit_mining_swing(&swing).is_some());
            assert_eq!(
                packets
                    .iter()
                    .filter(|p| matches!(p,
                ServerPacket::GainedItem { item } if item.item_index == 50001))
                    .count(),
                1
            );
            mined = true;
            break;
        }
        assert!(
            mined,
            "bounded natural source rates must produce rough raw stone"
        );
        let mut workshop = source.active_character_checkpoint().unwrap();
        workshop.map_file_name = "0".into();
        workshop.map_title = "BichonProvince".into();
        workshop.position = Point { x: 302, y: 222 };
        source
            .restore_active_character_checkpoint(&workshop)
            .unwrap();
        source.save_active_character().unwrap();
        source.try_handle_packet(ClientPacket::LogOut).unwrap();
    }

    fn expected_page(dialog: &Value, action: &str) -> bool {
        let Some(links) = dialog["links"].as_array() else {
            return false;
        };
        let has = |prefix: &str| {
            links.iter().any(|link| {
                link["target"]
                    .as_str()
                    .is_some_and(|s| s.starts_with(prefix))
            })
        };
        match action {
            "@stone:v1:home" | "@stone:v1:cut:" => has("@stone:v1:list:stone:"),
            "@stone:v1:list:stone:" => has("@stone:v1:stone:"),
            "@stone:v1:stone:" => has("@stone:v1:appraise:"),
            "@stone:v1:appraise:" => links.iter().any(|link| {
                link["target"].as_str().is_some_and(|s| {
                    s.starts_with("@stone:v1:cut:") && s.split(':').nth(4) == Some("1")
                })
            }),
            "@atelier:v1:home" => has("@atelier:v1:recipe:"),
            "@atelier:v1:recipe:traveler" => has("@atelier:v1:craft:traveler:"),
            "@atelier:v1:craft:traveler:" => has("@atelier:v1:wear:traveler:"),
            "@atelier:v1:wear:traveler:" => has("@atelier:v1:clear:"),
            "@atelier:v1:clear:" => has("@atelier:v1:wear:traveler:") && !has("@atelier:v1:clear:"),
            _ => false,
        }
    }

    struct Fixture {
        socket: TestWebSocket,
        server: tokio::task::JoinHandle<()>,
        config: Arc<SimulationConfig>,
        path: PathBuf,
        index: i32,
        snapshot: Value,
        idle: bool,
    }
    impl Fixture {
        async fn new(atelier: bool, idle: bool) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("mir2-workshop-ws-{}-{stamp}", std::process::id()));
            let path = root.join("accounts.json");
            let config = Arc::new(
                SimulationConfig::default()
                    .with_crystal_world_runtime()
                    .with_platinum_176_profile()
                    .with_account_store_path(path.clone())
                    .with_save_recovery_dir(root.join("recovery"))
                    .with_save_recovery_mac_key(std::array::from_fn::<_, 32, _>(|index| {
                        (index as u8).wrapping_mul(13)
                    }))
                    .unwrap(),
            );
            let index = prepare_source_fixture(config.as_ref(), atelier);
            let state = super::super::WebState {
                config: config.clone(),
                deploy_revision: None,
                zone_registry: Arc::new(crate::ZoneRegistry::new(
                    crate::ZoneId::primary(),
                    Arc::new(crate::routing::SharedInProcessZoneRuntimeFactory::new()),
                )),
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
                json!({"type":"login","accountId":"WorkshopSocket","password":"WorkshopPass123"}),
            )
            .await;
            read_workshop_until(&mut socket, "workshop login", |e| {
                test_packet(e, "LoginSuccess")
            })
            .await;
            // Consume the authenticated CharacterSelect projection before
            // enqueueing StartGame; it cannot be mistaken for its InGame world.
            read_workshop_until(&mut socket, "workshop CharacterSelect world", |e| {
                e["type"] == "worldSnapshot"
                    && e["payload"]["playerObjectId"].is_null()
                    && e["payload"]["playerHp"].is_null()
            })
            .await;
            send_test_websocket_json(
                &mut socket,
                json!({"type":"startGame","characterIndex":index}),
            )
            .await;
            let (started, _) = read_workshop_until(&mut socket, "workshop StartGame", |e| {
                test_packet(e, "StartGame")
            })
            .await;
            assert_eq!(started["payload"]["result"], 4);
            let (snapshot, _) = read_workshop_until(&mut socket, "workshop StartGame world", |e| {
                e["type"] == "worldSnapshot"
                    && e["payload"]["mapFileName"] == "0"
                    && e["payload"]["playerObjectId"]
                        .as_u64()
                        .is_some_and(|id| id != 0)
                    && e["payload"]["playerHp"].as_i64().is_some_and(|hp| hp > 0)
            })
            .await;
            Self {
                socket,
                server,
                config,
                path,
                index,
                snapshot,
                idle,
            }
        }
        fn saved(&self) -> CharacterSaveRecord {
            let store: AccountStore =
                serde_json::from_slice(&fs::read(&self.path).unwrap()).unwrap();
            store.accounts["WorkshopSocket"].saves[&self.index].clone()
        }
        fn target(&self, prefix: &str) -> String {
            self.snapshot["payload"]["activeNpcDialog"]["links"]
                .as_array()
                .unwrap()
                .iter()
                .find_map(|l| l["target"].as_str().filter(|s| s.starts_with(prefix)))
                .unwrap()
                .to_owned()
        }
        async fn interact(&mut self, npc: u32) {
            send_test_websocket_json(&mut self.socket, json!({"type":"interact","objectId":npc}))
                .await;
            let (snapshot, _) = read_workshop_until(&mut self.socket, "workshop real NPC", |e| {
                e["type"] == "worldSnapshot"
                    && e["payload"]["activeNpcDialog"]["npcObjectId"] == npc
            })
            .await;
            self.snapshot = snapshot;
        }
        async fn choose(&mut self, prefix: &str) -> Vec<Value> {
            let target = self.target(prefix);
            send_test_websocket_json(
                &mut self.socket,
                json!({"type":"selectNpcDialog","target":target}),
            )
            .await;
            let (snapshot, observed) =
                read_workshop_until(&mut self.socket, "workshop final page", |e| {
                    e["type"] == "worldSnapshot"
                        && expected_page(&e["payload"]["activeNpcDialog"], prefix)
                })
                .await;
            self.snapshot = snapshot;
            // A dirty idle queue may already be committing the next revision.
            // Bind to File at its checkpoint ACK below, not an interim frame.
            self.assert_final_page(&observed, !self.idle);
            observed
        }
        fn assert_final_page(&self, observed: &[Value], checkpoint_settled: bool) {
            let dialog = &self.snapshot["payload"]["activeNpcDialog"];
            let saved = self.saved();
            assert_eq!(
                self.snapshot["payload"]["gold"].as_u64(),
                Some(u64::from(saved.gold))
            );
            let mut page = vec![dialog["title"].as_str().unwrap().to_owned()];
            page.extend(
                dialog["body"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned()),
            );
            for l in dialog["links"].as_array().unwrap() {
                let target = l["target"].as_str().unwrap();
                assert!(!target.starts_with("@stone:v1:seal:"));
                page.push(format!("<{}/{}>", l["text"].as_str().unwrap(), target));
                if [
                    "@stone:v1:appraise:",
                    "@stone:v1:cut:",
                    "@atelier:v1:craft:",
                    "@atelier:v1:wear:",
                    "@atelier:v1:clear:",
                ]
                .iter()
                .any(|p| target.starts_with(p))
                    && checkpoint_settled
                {
                    assert_eq!(
                        target.rsplit(':').nth(1).unwrap().parse::<u64>(),
                        Ok(saved.revision)
                    );
                }
            }
            page.push(dialog["footer"].as_str().unwrap().to_owned());
            let pages: Vec<_> = observed
                .iter()
                .filter(|e| test_packet(e, "NPCResponse"))
                .map(|e| e["payload"]["page"].clone())
                .collect();
            if self.idle {
                assert_eq!(pages.last(), Some(&json!(page)));
            } else {
                assert_eq!(
                    pages,
                    vec![json!(page)],
                    "only the final post-checkpoint quote may escape"
                );
            }
        }
        async fn idle_checkpoint(&mut self) {
            let before = self.saved();
            // The File commit can precede delivery of its refresh frame. The
            // actual previously displayed quote is the stable wire baseline.
            let quoted_before = self.snapshot["payload"]["activeNpcDialog"]["links"]
                .as_array()
                .unwrap()
                .iter()
                .find_map(|link| {
                    let target = link["target"].as_str()?;
                    ["@stone:v1:appraise:", "@stone:v1:cut:"]
                        .iter()
                        .any(|p| target.starts_with(p))
                        .then(|| target.rsplit(':').nth(1).unwrap().parse::<u64>().unwrap())
                })
                .expect("actual displayed economic quote before idle checkpoint");
            assert!(before.revision == quoted_before || before.revision == quoted_before + 1);
            let prefix = if self.snapshot["payload"]["activeNpcDialog"]["title"] == "Stone workshop"
            {
                "@stone:v1:"
            } else {
                "@atelier:v1:"
            };
            let (snapshot, observed) =
                read_workshop_until(&mut self.socket, "idle real checkpoint page", |e| {
                    e["type"] == "worldSnapshot"
                        && e["payload"]["activeNpcDialog"]["links"]
                            .as_array()
                            .is_some_and(|links| {
                                links.iter().any(|l| {
                                    l["target"].as_str().is_some_and(|target| {
                                        target.starts_with(prefix)
                                            && target
                                                .rsplit(':')
                                                .nth(1)
                                                .and_then(|s| s.parse::<u64>().ok())
                                                .is_some_and(|r| r == quoted_before + 1)
                                    })
                                })
                            })
                })
                .await;
            self.snapshot = snapshot;
            let after = self.saved();
            assert_eq!(after.revision, quoted_before + 1);
            assert_eq!(after.gold, before.gold);
            assert_eq!(after.inventory_items_json, before.inventory_items_json);
            assert_eq!(after.npc_purchase_journal, before.npc_purchase_journal);
            assert!(!observed
                .iter()
                .any(|e| test_packet(e, "LoseGold") || test_packet(e, "GainedItem")));
            self.assert_final_page(&observed, true);
        }
        async fn close(mut self) {
            let _ = self.socket.close(None).await;
            self.server.abort();
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workshop_checkpoint_real_websocket_file_forced_save_keeps_first_stone_quotes_current()
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _env = Environment::enter(false);
        let mut f = Fixture::new(false, false).await;
        let raw = f
            .saved()
            .inventory_items_json
            .iter()
            .map(|row| serde_json::from_str::<Value>(row).unwrap())
            .find(|item| item["stone_serial"].as_u64().is_some())
            .unwrap();
        let uid = raw["unique_id"].as_u64().unwrap();
        f.interact(19).await;
        for p in [
            "@stone:v1:home",
            "@stone:v1:list:stone:",
            "@stone:v1:stone:",
        ] {
            f.choose(p).await;
        }
        for (p, fee) in [("@stone:v1:appraise:", 100), ("@stone:v1:cut:", 1500)] {
            let before = f.saved();
            let observed = f.choose(p).await;
            assert_eq!(
                f.saved().revision,
                before.revision + 3,
                "retain economic, outer Source and Web full saves"
            );
            assert_eq!(f.saved().gold, before.gold - fee);
            let carrier: Value = f
                .saved()
                .inventory_items_json
                .iter()
                .map(|row| serde_json::from_str::<Value>(row).unwrap())
                .find(|item| item["unique_id"].as_u64() == Some(uid))
                .unwrap();
            assert_eq!(carrier["slot"], raw["slot"]);
            assert_eq!(carrier["quantity"], 1);
            if p == "@stone:v1:cut:" {
                assert!(carrier["stone_serial"].is_null());
                assert_ne!(carrier["key"], "crystal-item-50001");
            } else {
                assert!(carrier["stone_serial"].as_u64().is_some());
            }
            assert_eq!(
                observed
                    .iter()
                    .filter(|e| test_packet(e, "LoseGold"))
                    .count(),
                usize::from(fee > 0)
            );
        }
        assert_eq!(f.saved().gold, 8400);
        f.close().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workshop_checkpoint_real_websocket_file_forced_save_keeps_atelier_wear_clear_current()
    {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _env = Environment::enter(false);
        let mut f = Fixture::new(true, false).await;
        f.interact(33).await;
        for p in ["@atelier:v1:home", "@atelier:v1:recipe:traveler"] {
            f.choose(p).await;
        }
        let stats = f.snapshot["payload"]["playerCrystalStats"].clone();
        for p in [
            "@atelier:v1:craft:traveler:",
            "@atelier:v1:wear:traveler:",
            "@atelier:v1:clear:",
        ] {
            let before = f.saved();
            f.choose(p).await;
            assert_eq!(f.saved().revision, before.revision + 3);
            assert_eq!(f.saved().gold, 9500);
            assert_eq!(f.snapshot["payload"]["playerCrystalStats"], stats);
        }
        f.close().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workshop_checkpoint_real_websocket_idle_file_save_republishes_before_first_cut() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _env = Environment::enter(true);
        let mut f = Fixture::new(false, true).await;
        f.interact(19).await;
        for p in [
            "@stone:v1:home",
            "@stone:v1:list:stone:",
            "@stone:v1:stone:",
        ] {
            f.choose(p).await;
        }
        f.idle_checkpoint().await;
        f.choose("@stone:v1:appraise:").await;
        f.idle_checkpoint().await;
        f.choose("@stone:v1:cut:").await;
        assert_eq!(f.saved().gold, 8400);
        f.close().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn workshop_checkpoint_real_websocket_file_save_failure_never_sends_success_quotes() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _env = Environment::enter(false);
        for fault in [
            AccountStoreTransactionFault::BeforeFileRename,
            AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
        ] {
            let mut f = Fixture::new(false, false).await;
            f.interact(19).await;
            f.choose("@stone:v1:home").await;
            f.choose("@stone:v1:list:stone:").await;
            let before = f.saved();
            let bytes = fs::read(&f.path).unwrap();
            let target = f.target("@stone:v1:stone:");
            f.config.inject_account_store_transaction_fault(fault);
            send_test_websocket_json(
                &mut f.socket,
                json!({"type":"selectNpcDialog","target":target}),
            )
            .await;
            let (_, observed) =
                read_workshop_until(&mut f.socket, "workshop checkpoint failure", |e| {
                    e["type"] == "error"
                })
                .await;
            assert!(!observed
                .iter()
                .any(|e| test_packet(e, "LoseGold") || test_packet(e, "GainedItem")));
            assert!(!observed
                .iter()
                .filter(|e| test_packet(e, "NPCResponse"))
                .any(|e| e["payload"]["page"]
                    .as_array()
                    .is_some_and(|p| p
                        .iter()
                        .any(|s| s.as_str().is_some_and(
                            |s| s.contains("@stone:v1:") || s.contains("@atelier:v1:")
                        )))));
            assert_eq!(f.saved().gold, before.gold);
            assert_eq!(f.saved().inventory_items_json, before.inventory_items_json);
            let after = f.saved();
            if after.revision == before.revision {
                assert_eq!(fs::read(&f.path).unwrap(), bytes);
            } else {
                assert_eq!(after.revision, before.revision + 1);
                let committed_bytes = fs::read(&f.path).unwrap();
                let error = f.config.save_account_store().unwrap_err();
                assert!(error.contains("writes are frozen"), "{error}");
                assert_eq!(fs::read(&f.path).unwrap(), committed_bytes);
            }
            f.close().await;
        }
    }
}
