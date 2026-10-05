//! Opt-in transport for contiguous public catalog packets in a bootstrap flush.
//! The contents remain the exact text envelopes used by legacy clients.

use std::collections::VecDeque;

use axum::extract::ws::Message;
use futures_util::SinkExt;
use mir2_protocol::catalog_transport::{
    encode_catalog_batch, CatalogTransportError, MAX_CATALOG_DECODED_BYTES, MAX_CATALOG_ENVELOPES,
};
use mir2_protocol::ServerPacket;

use super::{server_packet_to_event, SharedWebSocketSender};

fn is_catalog(packet: &ServerPacket) -> bool {
    matches!(
        packet,
        ServerPacket::NewItemInfo { .. }
            | ServerPacket::NewRecipeInfo { .. }
            | ServerPacket::GameShopInfo { .. }
            | ServerPacket::NewQuestInfo { .. }
    )
}

fn is_bootstrap(packets: &[ServerPacket]) -> bool {
    packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. }))
        || super::responses_begin_zone_bootstrap(packets)
}

// This iterator retains at most one decoded catalog batch. It never batches a
// personal receipt, identity, snapshot or other noncatalog packet. A single
// oversized catalog retains its existing Text representation.
struct SessionPacketFrames<'a> {
    remaining: &'a [ServerPacket],
    compress: bool,
    text_fallback: VecDeque<String>,
}

impl<'a> SessionPacketFrames<'a> {
    fn new(packets: &'a [ServerPacket], opted_in: bool) -> Self {
        Self {
            remaining: packets,
            compress: opted_in && is_bootstrap(packets),
            text_fallback: VecDeque::new(),
        }
    }

    fn encoded_or_text(&mut self, texts: Vec<String>) -> Result<Message, CatalogTransportError> {
        let text_bytes = texts.iter().map(String::len).sum::<usize>();
        let references = texts.iter().map(String::as_str).collect::<Vec<_>>();
        // An unexpected codec error must fail this flush, not mask a malformed
        // batch with an apparently successful fallback.
        let encoded = encode_catalog_batch(&references)?;
        if encoded.len() < text_bytes {
            Ok(Message::Binary(encoded.into()))
        } else {
            self.text_fallback = texts.into();
            Ok(Message::Text(
                self.text_fallback
                    .pop_front()
                    .expect("nonempty batch")
                    .into(),
            ))
        }
    }
}

impl Iterator for SessionPacketFrames<'_> {
    type Item = Result<Message, CatalogTransportError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(text) = self.text_fallback.pop_front() {
            return Some(Ok(Message::Text(text.into())));
        }
        let first = self.remaining.first()?;
        if !self.compress || !is_catalog(first) {
            let text = server_packet_to_event(first).to_string();
            self.remaining = &self.remaining[1..];
            return Some(Ok(Message::Text(text.into())));
        }

        let mut texts = Vec::new();
        let mut decoded_bytes = 0;
        for packet in self.remaining.iter().take(MAX_CATALOG_ENVELOPES) {
            if !is_catalog(packet) {
                break;
            }
            let text = server_packet_to_event(packet).to_string();
            let entry_bytes = 4 + text.len();
            if decoded_bytes + entry_bytes > MAX_CATALOG_DECODED_BYTES {
                if texts.is_empty() {
                    self.remaining = &self.remaining[1..];
                    return Some(Ok(Message::Text(text.into())));
                }
                break;
            }
            decoded_bytes += entry_bytes;
            texts.push(text);
        }
        self.remaining = &self.remaining[texts.len()..];
        Some(self.encoded_or_text(texts))
    }
}

// Call only at the existing serialized session-flush boundary. Live Zone
// packets, bootstrap registration activation and save/receipt ordering retain
// their existing gates. Resume sends no catalog today and stays on its Text path.
pub(super) async fn send_session_packets(
    sender: &SharedWebSocketSender,
    packets: &[ServerPacket],
    opted_in: bool,
) -> Result<(), String> {
    for frame in SessionPacketFrames::new(packets, opted_in) {
        sender
            .lock()
            .await
            .send(frame.map_err(|error| error.to_string())?)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::{self, validate_native_client_capabilities};
    use futures_util::StreamExt;
    use mir2_protocol::catalog_transport::{
        decode_catalog_batch, CATALOG_GZIP_CAPABILITY, MAX_CATALOG_WIRE_BYTES,
    };
    use mir2_protocol::{decode_server_packet, encode_frame, ServerPacketId};
    use serde_json::{json, Value};
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::task::JoinHandle;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::Message as ClientMessage;

    fn decode_payload(id: ServerPacketId, bytes: &[u8]) -> ServerPacket {
        decode_server_packet(&encode_frame(id as i16, bytes).unwrap()).unwrap()
    }

    fn catalogs() -> Vec<ServerPacket> {
        let shop = decode_payload(
            ServerPacketId::GameShopInfo,
            &mir2_game_data::crystal_game_shop_info_packet_payloads()[0],
        );
        let ServerPacket::GameShopInfo { ref item, .. } = shop else {
            unreachable!()
        };
        vec![
            ServerPacket::NewItemInfo {
                info: item.info.clone(),
            },
            decode_payload(
                ServerPacketId::NewRecipeInfo,
                &mir2_game_data::crystal_recipe_bootstrap_packets()[0].payload,
            ),
            shop,
            decode_payload(
                ServerPacketId::NewQuestInfo,
                &mir2_game_data::crystal_quest_packet_payloads()[0],
            ),
        ]
    }

    fn start() -> ServerPacket {
        ServerPacket::StartGame {
            result: 4,
            resolution: 1024,
        }
    }

    fn expected(packets: &[ServerPacket]) -> Vec<String> {
        packets
            .iter()
            .map(|packet| server_packet_to_event(packet).to_string())
            .collect()
    }

    fn flatten(frames: &[Message]) -> Vec<String> {
        frames
            .iter()
            .flat_map(|frame| match frame {
                Message::Text(text) => vec![text.to_string()],
                Message::Binary(bytes) => {
                    assert!(bytes.len() <= MAX_CATALOG_WIRE_BYTES);
                    let decoded = decode_catalog_batch(bytes).unwrap();
                    assert!(decoded.len() <= MAX_CATALOG_ENVELOPES);
                    assert!(
                        decoded.iter().map(|text| 4 + text.len()).sum::<usize>()
                            <= MAX_CATALOG_DECODED_BYTES
                    );
                    for text in &decoded {
                        let event: Value = serde_json::from_str(text).unwrap();
                        assert_eq!(event["type"], "packet");
                        assert!(matches!(
                            event["packet"].as_str(),
                            Some("NewItemInfo" | "NewRecipeInfo" | "GameShopInfo" | "NewQuestInfo")
                        ));
                    }
                    decoded
                }
                _ => panic!("unexpected outgoing frame"),
            })
            .collect()
    }

    #[test]
    fn catalog_capability_is_explicit_independent_and_case_sensitive() {
        for entries in [
            vec![],
            vec!["serverCatalogGzipV2"],
            vec!["ServerCatalogGzipV1"],
        ] {
            let capabilities = entries.into_iter().map(str::to_string).collect::<Vec<_>>();
            assert!(
                !validate_native_client_capabilities(&capabilities)
                    .unwrap()
                    .server_catalog_gzip_v1
            );
        }
        let parsed =
            validate_native_client_capabilities(&[CATALOG_GZIP_CAPABILITY.into()]).unwrap();
        assert!(parsed.server_catalog_gzip_v1);
        assert!(!parsed.native_resume_v1);
        assert!(!parsed.native_game_shop_receipt_v1);
        let parsed = validate_native_client_capabilities(&[
            CATALOG_GZIP_CAPABILITY.into(),
            web::NATIVE_RESUME_PROTOCOL.into(),
            web::NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.into(),
        ])
        .unwrap();
        assert!(
            parsed.server_catalog_gzip_v1
                && parsed.native_resume_v1
                && parsed.native_game_shop_receipt_v1
        );
        assert!(
            validate_native_client_capabilities(&[format!("{CATALOG_GZIP_CAPABILITY}\0")]).is_err()
        );
    }

    #[test]
    fn catalog_legacy_and_nonbootstrap_updates_keep_exact_text_order() {
        let mut packets = vec![start()];
        packets.extend(catalogs());
        packets.push(ServerPacket::KeepAlive { time: 99 });
        let frames = SessionPacketFrames::new(&packets, false)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(frames.iter().all(|frame| matches!(frame, Message::Text(_))));
        assert_eq!(flatten(&frames), expected(&packets));

        for first in [
            None,
            Some(ServerPacket::StartGame {
                result: 1,
                resolution: 0,
            }),
        ] {
            let packets = first.into_iter().chain(catalogs()).collect::<Vec<_>>();
            let frames = SessionPacketFrames::new(&packets, true)
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(frames.iter().all(|frame| matches!(frame, Message::Text(_))));
            assert_eq!(flatten(&frames), expected(&packets));
        }
    }

    #[test]
    fn catalog_mixed_bootstrap_preserves_all_four_types_and_personal_barriers() {
        let mut packets = vec![start()];
        packets.extend(catalogs());
        packets.push(ServerPacket::CompleteQuest {
            completed_quests: vec![17],
        });
        packets.push(ServerPacket::UserLocation {
            location: mir2_protocol::UserLocation {
                position: mir2_protocol::Point { x: 17, y: 28 },
                direction: mir2_protocol::MirDirection::Right,
            },
        });
        packets.extend(catalogs());
        packets.push(ServerPacket::KeepAlive { time: 101 });
        let frames = SessionPacketFrames::new(&packets, true)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 6);
        assert!(matches!(
            (&frames[0], &frames[1], &frames[2], &frames[3], &frames[4], &frames[5]),
            (
                Message::Text(_),
                Message::Binary(_),
                Message::Text(_),
                Message::Text(_),
                Message::Binary(_),
                Message::Text(_)
            )
        ));
        assert_eq!(flatten(&frames), expected(&packets));
    }

    #[test]
    fn catalog_count_and_utf8_decoded_limits_split_without_dropping_entries() {
        let quest = catalogs().pop().unwrap();
        let mut packets = vec![start()];
        packets.extend(std::iter::repeat_n(
            quest.clone(),
            MAX_CATALOG_ENVELOPES * 2 + 1,
        ));
        let frames = SessionPacketFrames::new(&packets, true)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let counts = frames
            .iter()
            .filter_map(|frame| match frame {
                Message::Binary(bytes) => Some(decode_catalog_batch(bytes).unwrap().len()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(counts, [64, 64, 1]);
        assert_eq!(flatten(&frames), expected(&packets));

        let ServerPacket::NewQuestInfo { mut info } = quest else {
            unreachable!()
        };
        info.description = vec!["药品与任务路径".repeat(1200)];
        let quest = ServerPacket::NewQuestInfo { info };
        let text_size = server_packet_to_event(&quest).to_string().len() + 4;
        assert!(text_size <= MAX_CATALOG_DECODED_BYTES);
        assert!(text_size * 3 > MAX_CATALOG_DECODED_BYTES);
        let mut packets = vec![start()];
        packets.extend(std::iter::repeat_n(quest, 5));
        let frames = SessionPacketFrames::new(&packets, true)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            frames
                .iter()
                .filter(|frame| matches!(frame, Message::Binary(_)))
                .count()
                >= 3
        );
        assert_eq!(flatten(&frames), expected(&packets));
    }

    #[test]
    fn catalog_oversized_single_entry_falls_back_intact_and_next_batch_still_compresses() {
        let ServerPacket::NewQuestInfo { mut info } = catalogs().pop().unwrap() else {
            unreachable!()
        };
        info.description = vec!["界".repeat(MAX_CATALOG_DECODED_BYTES)];
        let oversized = ServerPacket::NewQuestInfo { info };
        let mut packets = vec![start(), oversized];
        packets.extend(catalogs());
        let frames = SessionPacketFrames::new(&packets, true)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(matches!(
            (&frames[0], &frames[1], &frames[2]),
            (Message::Text(_), Message::Text(_), Message::Binary(_))
        ));
        assert_eq!(flatten(&frames), expected(&packets));
    }

    #[test]
    fn catalog_unprofitable_text_fallback_does_not_mask_codec_error() {
        let mut frames = SessionPacketFrames::new(&[], true);
        assert!(
            matches!(frames.encoded_or_text(vec!["x".into(), "y".into()]).unwrap(), Message::Text(text) if text == "x")
        );
        assert!(matches!(frames.next().unwrap().unwrap(), Message::Text(text) if text == "y"));
        assert!(frames.next().is_none());
        assert!(matches!(
            frames.encoded_or_text(vec![String::new()]),
            Err(CatalogTransportError::EmptyEnvelope)
        ));
        assert!(frames.next().is_none());
    }

    type TlsSocket = tokio_tungstenite::WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>;

    // An actual certificate-verified TLS tunnel fronts the same Axum websocket
    // handler used in production. No insecure certificate verifier or auth bypass.
    struct LocalWss {
        address: SocketAddr,
        client_config: Arc<rustls::ClientConfig>,
        server: JoinHandle<()>,
        tls: JoinHandle<()>,
    }

    impl Drop for LocalWss {
        fn drop(&mut self) {
            self.tls.abort();
            self.server.abort();
        }
    }

    impl LocalWss {
        async fn start(app: axum::Router) -> Self {
            let backend = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let backend_address = backend.local_addr().unwrap();
            let server = tokio::spawn(async move {
                axum::serve(
                    backend,
                    app.into_make_service_with_connect_info::<SocketAddr>(),
                )
                .await
                .unwrap();
            });
            let certificate =
                rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
            let certificate_der = certificate.cert.der().clone();
            let key =
                rustls_pki_types::PrivatePkcs8KeyDer::from(certificate.key_pair.serialize_der());
            let server_config = rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![certificate_der.clone()], key.into())
            .unwrap();
            let mut roots = rustls::RootCertStore::empty();
            roots.add(certificate_der).unwrap();
            let client_config = rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
            let tls = tokio::spawn(async move {
                let mut tunnels = tokio::task::JoinSet::new();
                loop {
                    tokio::select! {
                        accepted = listener.accept() => {
                            let (socket, _) = accepted.unwrap();
                            let acceptor = acceptor.clone();
                            tunnels.spawn(async move {
                                let mut secured = acceptor.accept(socket).await.unwrap();
                                let mut backend = TcpStream::connect(backend_address).await.unwrap();
                                let _ = tokio::io::copy_bidirectional(&mut secured, &mut backend).await;
                            });
                        }
                        _ = tunnels.join_next(), if !tunnels.is_empty() => {}
                    }
                }
            });
            Self {
                address,
                client_config: Arc::new(client_config),
                server,
                tls,
            }
        }

        async fn connect(&self) -> TlsSocket {
            let socket = TcpStream::connect(self.address).await.unwrap();
            let secured = tokio_rustls::TlsConnector::from(Arc::clone(&self.client_config))
                .connect(
                    rustls_pki_types::ServerName::try_from("localhost").unwrap(),
                    socket,
                )
                .await
                .unwrap();
            let mut request = format!("wss://localhost:{}/ws", self.address.port())
                .into_client_request()
                .unwrap();
            let origin = std::env::var("MIR2_ALLOWED_WEB_ORIGINS")
                .ok()
                .and_then(|value| {
                    value
                        .split(',')
                        .map(str::trim)
                        .find(|entry| !entry.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| format!("https://localhost:{}", self.address.port()));
            request
                .headers_mut()
                .insert("origin", origin.parse().unwrap());
            let (socket, response) = tokio_tungstenite::client_async(request, secured)
                .await
                .unwrap();
            assert_eq!(response.status(), 101);
            socket
        }
    }

    #[derive(Default)]
    struct Received {
        values: Vec<Value>,
        catalog_texts: Vec<String>,
        binary_frames: usize,
        catalog_wire_bytes: usize,
    }

    async fn read_until(
        socket: &mut TlsSocket,
        received: &mut Received,
        predicate: impl Fn(&Value) -> bool,
    ) -> Value {
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let (texts, binary) = match socket
                    .next()
                    .await
                    .expect("socket ended")
                    .expect("TLS websocket frame")
                {
                    ClientMessage::Text(text) => (vec![text.to_string()], false),
                    ClientMessage::Binary(bytes) => {
                        received.binary_frames += 1;
                        received.catalog_wire_bytes += bytes.len();
                        assert!(bytes.len() <= MAX_CATALOG_WIRE_BYTES);
                        (decode_catalog_batch(&bytes).unwrap(), true)
                    }
                    ClientMessage::Ping(_) | ClientMessage::Pong(_) => continue,
                    other => panic!("unexpected TLS websocket frame: {other:?}"),
                };
                for text in texts {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    let catalog = matches!(
                        value["packet"].as_str(),
                        Some("NewItemInfo" | "NewRecipeInfo" | "GameShopInfo" | "NewQuestInfo")
                    );
                    if binary {
                        assert!(catalog, "noncatalog escaped through binary batch");
                    }
                    if catalog {
                        if !binary {
                            received.catalog_wire_bytes += text.len();
                        }
                        received.catalog_texts.push(text);
                    }
                    let done = predicate(&value);
                    received.values.push(value.clone());
                    if done {
                        return value;
                    }
                }
            }
        })
        .await
        .expect("bounded TLS gateway response")
    }

    async fn send(socket: &mut TlsSocket, value: Value) {
        socket
            .send(ClientMessage::Text(value.to_string().into()))
            .await
            .unwrap();
    }

    fn packet(value: &Value, name: &str) -> bool {
        value["type"] == "packet" && value["packet"] == name
    }

    async fn start_ordinary_character(
        socket: &mut TlsSocket,
        account: &str,
        name: &str,
        received: &mut Received,
    ) -> i64 {
        send(socket, json!({"type":"newAccount", "accountId":account, "password":"ordinary-catalog-test-pass",
            "birthDateBinary":0, "userName":"Catalog Test", "secretQuestion":"q", "secretAnswer":"a", "emailAddress":"catalog@example.test"})).await;
        let created = read_until(socket, received, |event| packet(event, "NewAccount")).await;
        assert_eq!(created["payload"]["result"], 8);
        send(
            socket,
            json!({"type":"login", "accountId":account, "password":"ordinary-catalog-test-pass"}),
        )
        .await;
        read_until(socket, received, |event| packet(event, "LoginSuccess")).await;
        assert_eq!(received.binary_frames, 0, "login/identity must remain Text");
        send(
            socket,
            json!({"type":"newCharacter", "name":name, "class":"Warrior", "gender":"Male"}),
        )
        .await;
        let created = read_until(socket, received, |event| {
            packet(event, "NewCharacterSuccess")
        })
        .await;
        let index = created["payload"]["character"]["index"].as_i64().unwrap();
        send(socket, json!({"type":"startGame", "characterIndex":index})).await;
        let started = read_until(socket, received, |event| packet(event, "StartGame")).await;
        assert_eq!(started["payload"]["result"], 4);
        assert_eq!(
            received.binary_frames, 0,
            "StartGame receipt must remain Text"
        );
        read_until(socket, received, |event| {
            event["type"] == "worldSnapshot"
                && event["payload"]["playerObjectId"].is_number()
                && event["payload"]["entities"]
                    .as_array()
                    .is_some_and(|entities| {
                        entities
                            .iter()
                            .any(|entity| entity["objectId"] == event["payload"]["playerObjectId"])
                    })
        })
        .await;
        index
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn catalog_real_wss_opt_in_and_legacy_preserve_catalogs_login_logout_and_resume() {
        let reconnect = Arc::new(web::ReconnectSessionStore::default());
        let capacity = Arc::new(web::GatewayCapacityState::unlimited());
        let mut spectator_config = crate::spectator::SpectatorConfig::from_env();
        spectator_config.enabled = false;
        spectator_config.recording_enabled = false;
        spectator_config.public_enabled = false;
        let state = web::WebState {
            config: Arc::new(mir2_simulation::SimulationConfig::default()),
            deploy_revision: None,
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::clone(&reconnect),
            capacity: Arc::clone(&capacity),
            gameplay_event_sink: None,
            identity: Arc::new(crate::identity::IdentityService::local_for_tests()),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator: crate::spectator::SpectatorHub::new(spectator_config),
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(
                    std::env::temp_dir().join(format!("mir2-catalog-wss-{}", std::process::id())),
                ),
            )
            .unwrap(),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
        };
        let fixture = LocalWss::start(
            axum::Router::new()
                .route("/ws", axum::routing::get(web::ws_upgrade))
                .with_state(state),
        )
        .await;
        let mut legacy = fixture.connect().await;
        let mut original = Received::default();
        start_ordinary_character(&mut legacy, "catalog_legacy", "CatalogOld", &mut original).await;
        assert_eq!(original.binary_frames, 0);
        assert!(!original.catalog_texts.is_empty());
        send(&mut legacy, json!({"type":"logOut"})).await;
        read_until(&mut legacy, &mut original, |event| {
            packet(event, "LogOutSuccess")
        })
        .await;
        legacy.close(None).await.unwrap();

        let mut compressed = fixture.connect().await;
        send(&mut compressed, json!({"type":"clientCapabilities", "capabilities":[CATALOG_GZIP_CAPABILITY, web::NATIVE_RESUME_PROTOCOL]})).await;
        let mut opt_in = Received::default();
        let character_index =
            start_ordinary_character(&mut compressed, "catalog_optin", "CatalogNew", &mut opt_in)
                .await;
        assert!(opt_in.binary_frames > 0);
        assert_eq!(opt_in.catalog_texts, original.catalog_texts);
        assert!(opt_in.catalog_wire_bytes < original.catalog_wire_bytes);
        eprintln!(
            "catalog TLS roundtrip envelopes={} binary_frames={} legacy_bytes={} opt_in_bytes={}",
            opt_in.catalog_texts.len(),
            opt_in.binary_frames,
            original.catalog_wire_bytes,
            opt_in.catalog_wire_bytes,
        );
        let credential = read_until(&mut compressed, &mut opt_in, |event| {
            event["type"] == "resumeCredential"
        })
        .await;
        let first_credential = credential["credential"].as_str().unwrap().to_string();
        let parsed_credential =
            serde_json::from_value::<crate::resume::ResumeCredential>(json!(first_credential))
                .unwrap();
        compressed.close(None).await.unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            while reconnect.len() != 1
                || capacity.status().current_ws_connections != 0
                || reconnect
                    .resume_binding(&parsed_credential, web::gateway_unix_ms())
                    .is_none()
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("normal transport teardown retains native resume");

        let mut resumed = fixture.connect().await;
        send(&mut resumed, json!({"type":"clientCapabilities", "capabilities":[CATALOG_GZIP_CAPABILITY, web::NATIVE_RESUME_PROTOCOL]})).await;
        send(
            &mut resumed,
            json!({"type":"resumeSession", "credential":first_credential}),
        )
        .await;
        let mut after_resume = Received::default();
        let rotated = read_until(&mut resumed, &mut after_resume, |event| {
            event["type"] == "resumeCredential"
        })
        .await;
        assert_eq!(
            after_resume.binary_frames, 0,
            "resume playerstate is never a catalog"
        );
        let acknowledged = after_resume
            .values
            .iter()
            .position(|event| event["type"] == "sessionResumed")
            .unwrap();
        let snapshot = after_resume
            .values
            .iter()
            .enumerate()
            .skip(acknowledged + 1)
            .find(|(_, event)| event["type"] == "worldSnapshot")
            .unwrap()
            .0;
        assert!(snapshot > acknowledged);
        assert_eq!(
            after_resume.values[acknowledged]["characterIndex"],
            character_index
        );
        assert_ne!(rotated["credential"], credential["credential"]);
        send(&mut resumed, json!({"type":"logOut"})).await;
        read_until(&mut resumed, &mut after_resume, |event| {
            packet(event, "LogOutSuccess")
        })
        .await;
        assert_eq!(after_resume.binary_frames, 0);
        resumed.close(None).await.unwrap();
    }
}
