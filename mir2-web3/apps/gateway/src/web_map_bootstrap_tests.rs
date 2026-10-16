// Included in web::tests. Exercise the actual low-latency flush over a real
// loopback WebSocket; a final marker makes missing/extra snapshots observable.
mod map_bootstrap_websocket_tests {
    use super::super::{
        flush_session_updates, transport, GatewayRouteRefreshConfig, GatewaySaveQueueConfig,
        SharedWebSocketSender, WebSessionRouteRefresh, WebSessionSaveQueue,
    };
    use super::*;
    use axum::extract::ws::WebSocketUpgrade;
    use tokio::sync::oneshot;

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
        ) -> Vec<Value> {
            let cache = crate::InMemoryGatewaySessionCache::default();
            let mut saves = WebSessionSaveQueue::new(GatewaySaveQueueConfig::new(
                Duration::from_secs(10),
                Duration::from_secs(30),
                3,
            ));
            let mut routes =
                WebSessionRouteRefresh::new(GatewayRouteRefreshConfig::new(Duration::from_secs(5)));
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
                true,
                false,
                false,
            )
            .await
            .expect("actual low-latency flush");
            self.sender
                .send(axum::extract::ws::Message::Text(
                    json!({"type":"fixtureFlushComplete"}).to_string().into(),
                ))
                .await
                .unwrap();
            let (_, mut frames) = read_test_websocket_until(
                &mut self.client,
                "complete low-latency flush",
                |event| event["type"] == "fixtureFlushComplete",
            )
            .await;
            frames.pop();
            frames
        }
    }

    async fn ordinary_weapon_store_entry() -> (crate::GatewaySession, Vec<ServerPacket>) {
        let mut config = SimulationConfig::default().with_crystal_world_runtime();
        // Only fixture placement. The actual transfer is the ordinary Walk
        // through the imported 0(335,299) -> 0103(5,12) doorway. Set this after
        // the public builder, which selects the map's safe-zone start point.
        config.spawn = Point { x: 335, y: 298 };
        assert_eq!(config.map.map_index, 1);
        assert_eq!(config.map.file_name, "0");
        let mut session = crate::GatewaySession::new(config);
        assert!(session
            .try_handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into(),
            })
            .unwrap()
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        session
            .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
            .unwrap();
        assert_eq!(session.world_snapshot().map_file_name.as_deref(), Some("0"));
        let mut packets = session
            .try_handle_packet(ClientPacket::Walk {
                direction: MirDirection::Down,
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !super::super::responses_begin_zone_bootstrap(&packets) && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
            packets.extend(session.tick());
        }
        assert_eq!(
            session.world_snapshot().map_file_name.as_deref(),
            Some("0103")
        );
        // Isolate bootstrap from incidental quest/progression packets so the
        // pre-fix flush cannot pass by taking an unrelated snapshot branch.
        packets.retain(|packet| {
            matches!(
                packet,
                ServerPacket::MapInformation { .. } | ServerPacket::UserLocation { .. }
            )
        });
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::MapInformation { info } if info.map_index == 10 && info.file_name == "0103"
        )));
        (session, packets)
    }

    async fn assert_destination_flush(map_changed: bool) {
        let (mut session, mut packets) = ordinary_weapon_store_entry().await;
        if map_changed {
            // The normal doorway emits MapInformation. Exercise the supported
            // MapChanged wire variant with that same real destination metadata.
            for packet in &mut packets {
                if let ServerPacket::MapInformation { info } = packet {
                    *packet = ServerPacket::MapChanged {
                        map_index: info.map_index,
                        file_name: info.file_name.clone(),
                        title: info.title.clone(),
                        mini_map: info.mini_map,
                        big_map: info.big_map,
                        lights: info.lights,
                        location: Point { x: 5, y: 12 },
                        direction: MirDirection::Down,
                        map_dark_light: info.map_dark_light,
                        music: info.music,
                        weather: info.weather_particles,
                    };
                }
            }
        }
        let mut socket = FlushSocket::open().await;
        let frames = socket.flush(&mut session, packets).await;
        let boundary = if map_changed {
            "MapChanged"
        } else {
            "MapInformation"
        };
        let boundary_index = frames
            .iter()
            .position(|event| test_packet(event, boundary))
            .unwrap();
        let snapshots: Vec<_> = frames
            .iter()
            .enumerate()
            .filter(|(_, event)| event["type"] == "worldSnapshot")
            .collect();
        assert_eq!(snapshots.len(), 1, "one complete destination bootstrap");
        assert!(
            snapshots[0].0 > boundary_index,
            "identity must precede its snapshot"
        );
        let payload = &snapshots[0].1["payload"];
        assert_eq!(payload["mapFileName"], "0103");
        assert!(payload["inventoryItems"].is_array());
        assert!(payload["knownSkills"].is_array());
        let entities = payload["entities"].as_array().unwrap();
        let player = entities
            .iter()
            .find(|entity| entity["kind"] == "selfPlayer")
            .unwrap();
        assert_eq!(
            (player["x"].as_i64(), player["y"].as_i64()),
            (Some(5), Some(12))
        );
        assert!(
            entities.iter().any(|entity| entity["kind"] == "npc"),
            "destination NPCs must be present"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn low_latency_map_bootstrap_map_information_flushes_complete_destination() {
        assert_destination_flush(false).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn low_latency_map_bootstrap_map_changed_flushes_complete_destination() {
        assert_destination_flush(true).await;
    }

    #[test]
    fn low_latency_map_bootstrap_keeps_existing_quest_snapshot_triggers() {
        for packet in [
            ServerPacket::CompleteQuest {
                completed_quests: vec![],
            },
            ServerPacket::ChangeQuest {
                quest_id: 2_100_004,
                completed: false,
                taken: true,
                quest_state: 1,
                task_list: vec!["Completed daily commissions: 0/2".into()],
                new: false,
                track_quest: false,
            },
        ] {
            assert!(super::super::low_latency_responses_require_world_snapshot(
                &[packet]
            ));
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn low_latency_map_bootstrap_ordinary_walk_location_and_tick_stay_packet_only() {
        assert!(super::super::is_low_latency_action(&SessionAction::Packet(
            ClientPacket::Walk {
                direction: MirDirection::Down,
            }
        )));
        assert!(super::super::is_low_latency_action(&SessionAction::Tick));
        let mut session = demo_game_session();
        let mut socket = FlushSocket::open().await;
        for packets in [
            vec![],
            vec![ServerPacket::UserLocation {
                location: UserLocation {
                    position: Point { x: 330, y: 270 },
                    direction: MirDirection::Down,
                },
            }],
            vec![ServerPacket::ObjectHealth {
                info: mir2_protocol::ObjectHealthInfo {
                    object_id: 42,
                    percent: 75,
                    expire: 0,
                },
            }],
        ] {
            let packet_count = packets.len();
            let frames = socket.flush(&mut session, packets).await;
            assert_eq!(
                frames.len(),
                packet_count,
                "no full snapshot for ordinary deltas"
            );
            assert!(frames.iter().all(|event| event["type"] == "packet"));
        }
    }
}
