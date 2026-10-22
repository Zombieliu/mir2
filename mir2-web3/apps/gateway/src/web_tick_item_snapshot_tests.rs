// Included in web::tests. Real GatewaySession fishing Tick and ordinary ride
// mutations followed by the production flush over loopback WebSocket.
mod tick_item_snapshot_websocket_tests {
    use super::super::{
        flush_session_updates, transport, GatewayRouteRefreshConfig, GatewaySaveQueueConfig,
        SharedWebSocketSender, WebSessionRouteRefresh, WebSessionSaveQueue,
    };
    use super::*;
    use axum::extract::ws::WebSocketUpgrade;
    use std::{ffi::OsString, path::PathBuf};
    use tokio::sync::oneshot;

    const ROD_UID: u64 = 90_001;
    const BAIT_UID: u64 = 90_004;
    const FLOAT_UID: u64 = 90_003;
    const MOUNT_UID: u64 = 92_001;
    const SADDLE_UID: u64 = 92_002;
    const WEAPON_UID: u64 = 92_003;

    struct Environment(Option<OsString>);
    impl Environment {
        fn enter() -> Self {
            // Require the real imported 700x700 Bichon pack; a synthetic map
            // with permissive fishing defaults cannot prove this shore route.
            let map = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../web/lib/generated/crystal-map-pack");
            assert!(
                map.join("0.map.gz").is_file(),
                "original Bichon map required"
            );
            let previous = std::env::var_os("MIR2_CRYSTAL_MAP_PACK");
            std::env::set_var("MIR2_CRYSTAL_MAP_PACK", map);
            mir2_simulation::set_crystal_full_world_zone_collision(true);
            Self(previous)
        }
    }
    impl Drop for Environment {
        fn drop(&mut self) {
            mir2_simulation::set_crystal_full_world_zone_collision(false);
            match self.0.take() {
                Some(value) => std::env::set_var("MIR2_CRYSTAL_MAP_PACK", value),
                None => std::env::remove_var("MIR2_CRYSTAL_MAP_PACK"),
            }
        }
    }

    struct FlushSocket {
        sender: SharedWebSocketSender,
        client: TestWebSocket,
        server: tokio::task::JoinHandle<()>,
    }
    impl Drop for FlushSocket {
        fn drop(&mut self) {
            self.server.abort();
        }
    }
    impl FlushSocket {
        async fn open() -> Self {
            let (ready_tx, ready_rx) = oneshot::channel();
            let ready = Arc::new(Mutex::new(Some(ready_tx)));
            let router = axum::Router::new().route(
                "/ws",
                axum::routing::get(move |upgrade: WebSocketUpgrade| {
                    let ready = ready.lock().unwrap().take().unwrap();
                    async move {
                        upgrade.on_upgrade(move |socket| async move {
                            let (sink, mut stream) = socket.split();
                            let sender = Arc::new(transport::SocketSender::new(
                                sink,
                                transport::TransportSignal::new(),
                            ));
                            let _ = ready.send(sender);
                            while let Some(Ok(_)) = stream.next().await {}
                        })
                    }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });
            let (client, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
                .await
                .unwrap();
            let sender = tokio::time::timeout(Duration::from_secs(5), ready_rx)
                .await
                .unwrap()
                .unwrap();
            Self {
                sender,
                client,
                server,
            }
        }

        async fn flush(
            &mut self,
            session: &mut crate::GatewaySession,
            packets: Vec<ServerPacket>,
            low_latency: bool,
        ) -> Vec<Value> {
            let cache = crate::InMemoryGatewaySessionCache::default();
            let mut saves = WebSessionSaveQueue::new(GatewaySaveQueueConfig::new(
                Duration::from_secs(600),
                Duration::from_secs(600),
                64,
            ));
            let mut routes = WebSessionRouteRefresh::new(GatewayRouteRefreshConfig::new(
                Duration::from_secs(600),
            ));
            // The ordinary Tick call uses action=false, low_latency=true,
            // queue_save=false, force_refresh=false without workshop changes.
            flush_session_updates(
                &self.sender,
                session,
                &cache,
                &mut saves,
                &mut routes,
                packets,
                false,
                None,
                false,
                low_latency,
                false,
                false,
            )
            .await
            .expect("production Tick flush");
            self.sender
                .send(axum::extract::ws::Message::Text(
                    json!({"type":"tickItemFlushComplete"}).to_string().into(),
                ))
                .await
                .unwrap();
            let (_, mut frames) =
                read_test_websocket_until(&mut self.client, "complete Tick item flush", |event| {
                    event["type"] == "tickItemFlushComplete"
                })
                .await;
            frames.pop();
            frames
        }
    }

    fn source_item(index: i32, uid: u64, slot: u8, count: u32) -> String {
        let template = mir2_game_data::crystal_item_by_index(index).unwrap();
        assert!(count > 0 && count <= u32::from(template.stack_size.max(1)));
        let durability = (template.durability > 0).then_some(template.durability);
        json!({
            "key":format!("crystal-item-{index}"),"name":template.name,"icon":template.image,
            "slot":slot,"unique_id":uid,"container":"bag1","quantity":count,
            "description":"isolated fishing transport test fixture",
            "durability_current":durability,"durability_max":durability,
            "weight":template.weight,"equip_slot":if index == 793 {json!("weapon")} else {Value::Null},
            "attack":0,"defence":0,"heal_hp":0,"heal_mp":0,
            "identified":true,"user_item_metadata":{"item_index":index}
        }).to_string()
    }

    fn fixture(bait_count: u32, spare_stacks: u8) -> crate::GatewaySession {
        // Explicit, offline-only in-memory fixtures. This does not seed a live
        // authority, add a grant ingress, or claim ordinary item acquisition.
        let config = SimulationConfig::default().with_crystal_world_runtime();
        assert!(config.account_store_path.is_none() && config.account_store_database_url.is_none());
        let mut items = vec![
            source_item(793, ROD_UID, 0, 1),
            source_item(795, 90_002, 1, 1),
            source_item(796, FLOAT_UID, 2, 1),
            source_item(798, BAIT_UID, 3, bait_count),
            source_item(800, 90_005, 4, 1),
            source_item(802, 90_006, 5, 1),
        ];
        for spare in 0..spare_stacks {
            items.push(source_item(798, 91_000 + u64::from(spare), 6 + spare, 100));
        }
        {
            let mut store = config.account_store.lock().unwrap();
            let account = store.accounts.get_mut("demo").unwrap();
            account
                .characters
                .iter_mut()
                .find(|c| c.index == 0)
                .unwrap()
                .level = 100;
            let save = account.saves.get_mut(&0).unwrap();
            save.character.level = 100;
            save.character.class = MirClass::Warrior;
            save.character.gender = MirGender::Male;
            save.map_file_name = "0".into();
            save.map_title = "BichonProvince".into();
            save.position = Point { x: 340, y: 242 };
            save.direction = MirDirection::DownLeft;
            save.hp = 3464;
            save.max_hp = 3464;
            save.mp = 361;
            save.max_mp = 361;
            save.inventory_items_json = items;
            save.belt_items_json.clear();
            save.equipment_items_json.clear();
            save.equipment_items_explicit_empty = true;
            save.quest_states_json.clear();
            save.skill_states_json.clear();
            save.buff_states_json.clear();
        }
        let mut session = crate::GatewaySession::new(config);
        assert!(session
            .try_handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into(),
            })
            .unwrap()
            .iter()
            .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
        assert!(session
            .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
            .unwrap()
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
        assert!(session
            .try_handle_packet(ClientPacket::EquipItem {
                unique_id: ROD_UID,
                grid: MirGridType::Inventory,
                to: 0,
            })
            .unwrap()
            .iter()
            .any(|p| matches!(p, ServerPacket::EquipItem { success: true, .. })));
        for (to, uid) in [
            (0, 90_002),
            (1, FLOAT_UID),
            (2, BAIT_UID),
            (3, 90_005),
            (4, 90_006),
        ] {
            assert!(session
                .try_handle_packet(ClientPacket::EquipSlotItem {
                    grid: MirGridType::Inventory,
                    unique_id: uid,
                    to,
                    grid_to: MirGridType::Fishing,
                    to_unique_id: ROD_UID,
                })
                .unwrap()
                .iter()
                .any(|p| matches!(p, ServerPacket::EquipSlotItem { success: true, .. })));
        }
        assert_eq!(
            rod(&client_snapshot(&session))["tooltipSource"]["userItem"]["slots"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        session
    }

    fn client_snapshot(session: &crate::GatewaySession) -> Value {
        serde_json::to_value(session.world_snapshot().client_view()).unwrap()
    }
    fn mount_fixture() -> crate::GatewaySession {
        // Only this offline in-memory test creates items. Equipping the real
        // templates, attaching the saddle and riding use ordinary packets.
        let config = SimulationConfig::default().with_crystal_world_runtime();
        assert!(config.account_store_path.is_none() && config.account_store_database_url.is_none());
        let mut mount: Value = serde_json::from_str(&source_item(764, MOUNT_UID, 0, 1)).unwrap();
        mount["equip_slot"] = json!("mount");
        mount["socket_slots"] = json!(mir2_game_data::crystal_item_by_index(764).unwrap().slots);
        let mut weapon: Value = serde_json::from_str(&source_item(221, WEAPON_UID, 2, 1)).unwrap();
        weapon["equip_slot"] = json!("weapon");
        {
            let mut store = config.account_store.lock().unwrap();
            let account = store.accounts.get_mut("demo").unwrap();
            assert_eq!(account.gm_level, 0, "ordinary player fixture");
            account
                .characters
                .iter_mut()
                .find(|c| c.index == 0)
                .unwrap()
                .level = 100;
            let save = account.saves.get_mut(&0).unwrap();
            save.character.level = 100;
            save.character.class = MirClass::Warrior;
            save.character.gender = MirGender::Male;
            save.map_file_name = "0".into();
            save.map_title = "BichonProvince".into();
            save.position = Point { x: 351, y: 279 };
            save.direction = MirDirection::Down;
            save.hp = 3464;
            save.max_hp = 3464;
            save.mp = 361;
            save.max_mp = 361;
            save.inventory_items_json = vec![
                mount.to_string(),
                source_item(782, SADDLE_UID, 1, 1),
                weapon.to_string(),
            ];
            save.belt_items_json.clear();
            save.equipment_items_json.clear();
            save.equipment_items_explicit_empty = true;
            save.quest_states_json.clear();
            save.skill_states_json.clear();
            save.buff_states_json.clear();
        }
        let mut session = crate::GatewaySession::new(config);
        assert!(session
            .try_handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into(),
            })
            .unwrap()
            .iter()
            .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
        assert!(session
            .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
            .unwrap()
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
        for (unique_id, to) in [(WEAPON_UID, 0), (MOUNT_UID, 13)] {
            assert!(session
                .execute_production_player_command(
                    true,
                    mir2_simulation::WorldCommand::ClientPacket(ClientPacket::EquipItem {
                        unique_id,
                        grid: MirGridType::Inventory,
                        to,
                    })
                )
                .unwrap()
                .packets
                .iter()
                .any(|p| matches!(p, ServerPacket::EquipItem { success: true, .. })));
        }
        assert!(session
            .execute_production_player_command(
                true,
                mir2_simulation::WorldCommand::ClientPacket(ClientPacket::EquipSlotItem {
                    grid: MirGridType::Inventory,
                    unique_id: SADDLE_UID,
                    to: 2,
                    grid_to: MirGridType::Mount,
                    to_unique_id: MOUNT_UID,
                })
            )
            .unwrap()
            .packets
            .iter()
            .any(|p| matches!(p, ServerPacket::EquipSlotItem { success: true, .. })));
        let snapshot = client_snapshot(&session);
        let worn_mount = snapshot["equipmentItems"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["uniqueId"] == MOUNT_UID)
            .expect("ordinary worn BengalTiger");
        assert_eq!(
            worn_mount["tooltipSource"]["userItem"]["slots"][2]["unique_id"],
            SADDLE_UID
        );
        session
    }
    fn self_player(snapshot: &Value) -> &Value {
        snapshot["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entity| {
                entity["kind"] == "selfPlayer" && entity["objectId"] == snapshot["playerObjectId"]
            })
            .expect("complete self player appearance")
    }
    fn rod(snapshot: &Value) -> &Value {
        snapshot["equipmentItems"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["uniqueId"] == ROD_UID)
            .expect("exact worn rod")
    }
    fn bait(snapshot: &Value) -> &Value {
        &rod(snapshot)["tooltipSource"]["userItem"]["slots"][2]
    }
    fn start(session: &mut crate::GatewaySession, auto: bool) {
        let cast = session
            .try_handle_packet(ClientPacket::FishingCast { cast_out: true })
            .unwrap();
        assert!(
            cast.iter().any(|p| matches!(
                p,
                ServerPacket::FishingUpdate {
                    fishing: true,
                    progress_percent: 0,
                    ..
                }
            )),
            "real map0 shore must accept cast"
        );
        if auto {
            assert!(session
                .try_handle_packet(ClientPacket::FishingChangeAutocast { auto_cast: true })
                .unwrap()
                .iter()
                .any(|p| matches!(p, ServerPacket::FishingUpdate { fishing: true, .. })));
        }
    }
    fn tick(session: &mut crate::GatewaySession) -> Vec<ServerPacket> {
        session
            .execute_with_outcome(mir2_simulation::WorldCommand::Tick)
            .expect("real shared/personal Tick execution")
            .packets
    }
    fn assert_latest(frames: &[Value], expected: &Value, item_packet_count: usize) {
        let snapshots: Vec<_> = frames
            .iter()
            .enumerate()
            .filter(|(_, frame)| frame["type"] == "worldSnapshot")
            .collect();
        assert_eq!(snapshots.len(), 1, "exactly one complete current snapshot");
        assert!(
            snapshots[0].0 >= item_packet_count,
            "ordinary item packets precede snapshot"
        );
        let actual = &snapshots[0].1["payload"];
        assert_eq!(
            actual["inventoryItems"], expected["inventoryItems"],
            "fresh actual bag source"
        );
        assert_eq!(
            actual["equipmentItems"], expected["equipmentItems"],
            "fresh worn rod and every nested child"
        );
        assert_eq!(actual["playerObjectId"], expected["playerObjectId"]);
        assert_eq!(actual["mapFileName"], "0");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tick_item_snapshot_actual_autocast_depletion_delete_publishes_empty_nested_bait() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _environment = Environment::enter();
        let mut session = fixture(2, 0);
        start(&mut session, true);
        assert_eq!(bait(&client_snapshot(&session))["count"], 1);
        let mut packets = Vec::new();
        for _ in 0..5 {
            packets = tick(&mut session);
        }
        let deleted: Vec<_> = packets
            .into_iter()
            .filter(|p| {
                matches!(
                    p,
                    ServerPacket::DeleteItem {
                        unique_id: BAIT_UID,
                        count: 1
                    }
                )
            })
            .collect();
        assert_eq!(
            deleted.len(),
            1,
            "actual automatic next cast consumed final bait"
        );
        let expected = client_snapshot(&session);
        assert!(bait(&expected).is_null());
        let mut socket = FlushSocket::open().await;
        for low_latency in [true, false] {
            let frames = socket
                .flush(&mut session, deleted.clone(), low_latency)
                .await;
            assert_latest(&frames, &expected, 1);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tick_item_snapshot_actual_float_wear_publishes_latest_nested_durability() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _environment = Environment::enter();
        let mut session = fixture(2, 0);
        start(&mut session, false);
        let before = client_snapshot(&session);
        let float_before = rod(&before)["tooltipSource"]["userItem"]["slots"][1]["current_dura"]
            .as_u64()
            .unwrap();
        let mut packets = Vec::new();
        for _ in 0..4 {
            packets = tick(&mut session);
        }
        let wear: Vec<_> = packets
            .into_iter()
            .filter(|p| {
                matches!(
                    p,
                    ServerPacket::DuraChanged {
                        unique_id: FLOAT_UID,
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(wear.len(), 1, "actual full-progress tick wore float");
        let expected = client_snapshot(&session);
        assert_eq!(
            rod(&expected)["tooltipSource"]["userItem"]["slots"][1]["current_dura"],
            float_before - 1
        );
        let mut socket = FlushSocket::open().await;
        for low_latency in [true, false] {
            let frames = socket.flush(&mut session, wear.clone(), low_latency).await;
            assert_latest(&frames, &expected, 1);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tick_item_snapshot_actual_autocast_batch_sends_one_complete_snapshot_after_packets() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _environment = Environment::enter();
        let mut session = fixture(2, 0);
        start(&mut session, true);
        let mut packets = Vec::new();
        for _ in 0..5 {
            packets = tick(&mut session);
        }
        assert!(packets
            .iter()
            .any(|p| matches!(p, ServerPacket::DeleteItem { .. })));
        assert!(packets
            .iter()
            .any(|p| matches!(p, ServerPacket::DuraChanged { .. })));
        assert!(packets
            .iter()
            .any(|p| matches!(p, ServerPacket::FishingUpdate { .. })));
        // Strip unrelated vitals/quest/world events: they must not mask a
        // missing item trigger by selecting a pre-existing snapshot branch.
        packets.retain(|p| {
            matches!(
                p,
                ServerPacket::GainedItem { .. }
                    | ServerPacket::DeleteItem { .. }
                    | ServerPacket::DuraChanged { .. }
                    | ServerPacket::FishingUpdate { .. }
            )
        });
        let packet_count = packets.len();
        let expected = client_snapshot(&session);
        let mut socket = FlushSocket::open().await;
        let frames = socket.flush(&mut session, packets, true).await;
        assert_latest(&frames, &expected, packet_count);
        assert_eq!(
            frames.len(),
            packet_count + 1,
            "coalesce item batch into one snapshot"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tick_item_snapshot_real_fishing_loot_gained_item_publishes_actual_fish_and_rod() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _environment = Environment::enter();
        let mut session = fixture(100, 7);
        start(&mut session, true);
        let mut fish_packet = None;
        // Use unchanged public source probabilities, real Tick and ordinary
        // attachment/cast packets. No forced roll, Tick setter or QA command.
        for _ in 0..4_000 {
            let packets = tick(&mut session);
            fish_packet = packets.iter().find(|p| matches!(p,
                ServerPacket::GainedItem { item } if mir2_game_data::crystal_item_by_index(item.item_index)
                    .is_some_and(|template| template.item_type == 33)
            )).cloned();
            if fish_packet.is_some() {
                break;
            }
            if packets
                .iter()
                .any(|p| matches!(p, ServerPacket::FishingUpdate { fishing: false, .. }))
            {
                let current = client_snapshot(&session);
                assert!(
                    bait(&current).is_null(),
                    "ordinary cast stopped before exhausting fixture bait"
                );
                let spare = session
                    .world_snapshot()
                    .inventory_items
                    .into_iter()
                    .find(|item| item.key == "crystal-item-798")
                    .expect("bounded original-table sample has prepared spare bait");
                assert!(session
                    .try_handle_packet(ClientPacket::EquipSlotItem {
                        grid: MirGridType::Inventory,
                        unique_id: spare.unique_id,
                        to: 2,
                        grid_to: MirGridType::Fishing,
                        to_unique_id: ROD_UID,
                    })
                    .unwrap()
                    .iter()
                    .any(|p| matches!(p, ServerPacket::EquipSlotItem { success: true, .. })));
                start(&mut session, true);
            }
        }
        let packet = fish_packet.expect(
            "unchanged 00Fishing table must produce actual fish in bounded deterministic fixture",
        );
        let ServerPacket::GainedItem { item } = &packet else {
            unreachable!()
        };
        assert!(item.unique_id > 0 && item.count > 0);
        let expected = client_snapshot(&session);
        let fish = expected["inventoryItems"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["uniqueId"] == item.unique_id)
            .expect("actual fish exists in source bag");
        assert_eq!(fish["tooltipSource"]["info"]["item_type"], 33);
        assert_eq!(
            fish["tooltipSource"]["userItem"]["unique_id"],
            item.unique_id
        );
        let mut socket = FlushSocket::open().await;
        // Isolating the actual GainedItem proves it does not rely on a same-
        // batch DuraChanged/DeleteItem or incidental health/quest packet.
        for low_latency in [true, false] {
            let frames = socket
                .flush(&mut session, vec![packet.clone()], low_latency)
                .await;
            assert_latest(&frames, &expected, 1);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tick_item_snapshot_actual_progress_only_stays_packet_only_in_both_flush_modes() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _environment = Environment::enter();
        let mut session = fixture(2, 0);
        start(&mut session, false);
        let before = client_snapshot(&session);
        let packets: Vec<_> = tick(&mut session)
            .into_iter()
            .filter(|p| {
                matches!(
                    p,
                    ServerPacket::FishingUpdate {
                        progress_percent: 25,
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(packets.len(), 1);
        let after = client_snapshot(&session);
        assert_eq!(before["inventoryItems"], after["inventoryItems"]);
        assert_eq!(before["equipmentItems"], after["equipmentItems"]);
        let mut socket = FlushSocket::open().await;
        for low_latency in [true, false] {
            let frames = socket
                .flush(&mut session, packets.clone(), low_latency)
                .await;
            assert_eq!(
                frames.len(),
                1,
                "progress-only Tick must not force snapshot"
            );
            assert!(test_packet(&frames[0], "FishingUpdate"));
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tick_item_snapshot_ordinary_mount_ride_chat_flushes_complete_sprite_on_and_off() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _environment = Environment::enter();
        for low_latency in [true, false] {
            let mut session = mount_fixture();
            let on_foot = client_snapshot(&session);
            let foot_player = self_player(&on_foot);
            assert_eq!(foot_player["ridingMount"], false);
            assert!(foot_player["sprite"]["mountLibrary"].is_null());
            assert_eq!(foot_player["sprite"]["weaponLibrary"], "CWeapon/00");
            let owner_id = on_foot["playerObjectId"].as_u64().unwrap();
            let mut socket = FlushSocket::open().await;
            for riding in [true, false] {
                let packets = session
                    .execute_production_player_command(
                        true,
                        mir2_simulation::WorldCommand::ClientPacket(ClientPacket::Chat {
                            message: "@ride".into(),
                            linked_items: Vec::new(),
                        }),
                    )
                    .expect("authenticated ordinary @ride mutation")
                    .packets;
                let updates: Vec<_> = packets
                    .iter()
                    .filter(|packet| {
                        matches!(packet,
                            ServerPacket::MountUpdate { object_id, mount_type: 0, riding_mount }
                                if u64::from(*object_id) == owner_id && *riding_mount == riding
                        )
                    })
                    .cloned()
                    .collect();
                assert_eq!(updates.len(), 1, "real @ride must toggle exact owner once");
                let expected = client_snapshot(&session);
                let player = self_player(&expected);
                assert_eq!(player["ridingMount"], riding);
                assert_eq!(expected["equipmentItems"], on_foot["equipmentItems"]);
                assert_eq!(expected["inventoryItems"], on_foot["inventoryItems"]);
                if riding {
                    assert_eq!(player["sprite"]["mountLibrary"], "Mount/00");
                    assert_eq!(player["sprite"]["mountFrameOffset"], 0);
                    assert!(player["sprite"]["weaponLibrary"].is_null());
                    assert!(player["sprite"]["weaponFrameOffset"].is_null());
                } else {
                    assert_eq!(
                        player["sprite"], foot_player["sprite"],
                        "dismount restores complete foot and weapon libraries"
                    );
                }
                // First isolate the real owner MountUpdate, preventing an
                // incidental world/vitals packet from masking this trigger.
                // Then preserve the entire real response batch and require
                // one snapshot after all original packets, in both modes.
                for batch in [updates, packets] {
                    let packet_count = batch.len();
                    let frames = socket.flush(&mut session, batch, low_latency).await;
                    assert_latest(&frames, &expected, packet_count);
                    assert_eq!(
                        frames.len(),
                        packet_count + 1,
                        "ride response batch coalesces into one snapshot"
                    );
                    let update = frames
                        .iter()
                        .find(|frame| {
                            test_packet(frame, "MountUpdate")
                                && frame["payload"]["objectId"] == owner_id
                        })
                        .unwrap();
                    assert_eq!(update["payload"]["ridingMount"], riding);
                    assert_eq!(update["payload"]["mountType"], 0);
                    let snapshot = frames
                        .iter()
                        .find(|frame| frame["type"] == "worldSnapshot")
                        .unwrap();
                    assert_eq!(self_player(&snapshot["payload"]), player,
                        "flush includes actual complete current self sprite; riding={riding}, low_latency={low_latency}");
                }
            }
        }
    }
}
