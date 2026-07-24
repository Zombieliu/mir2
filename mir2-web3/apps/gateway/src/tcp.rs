use std::io;
use std::sync::Arc;

use mir2_protocol::{decode_client_packet, encode_server_packet, ServerPacket};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};

use crate::events::{default_gameplay_event_sink_from_env, SharedGameplayEventSink};
use crate::routing::{SharedZoneLiveOutbound, ZoneLiveOutboundRegistration};
use crate::session::catch_gateway_panic;
use crate::{GatewayConfig, GatewaySession, ZoneRegistry, ZoneTopology};

#[path = "chat_broadcast.rs"]
pub mod chat_broadcast;

use chat_broadcast::{recv_optional_chat, ChatBroadcastHub, ChatPresence, ChatProtocol};

const LIVE_ZONE_OUTBOUND_CAPACITY: usize = 256;

pub async fn run_tcp_gateway(
    addr: &str,
    config: GatewayConfig,
    chat_hub: ChatBroadcastHub,
) -> io::Result<()> {
    // Activated Crystal world: host every map full-size in the shared zone (see
    // `run_web_gateway`). Empty maps stay dormant regardless.
    if config.monster_spawn_source == mir2_simulation::MonsterSpawnSource::CrystalWorld {
        mir2_simulation::set_crystal_full_world_zone_collision(true);
    }
    let listener = TcpListener::bind(addr).await?;
    let config = Arc::new(config);
    let topology = ZoneTopology::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let zone_registry = Arc::new(
        topology.zone_registry(crate::zone_lease::default_zone_owner_lease_authority_from_env()),
    );
    let gameplay_event_sink = default_gameplay_event_sink_from_env();

    eprintln!("mir2-gateway tcp listening on {addr}");

    loop {
        let (stream, peer) = listener.accept().await?;
        let config = Arc::clone(&config);
        let zone_registry = Arc::clone(&zone_registry);
        let gameplay_event_sink = gameplay_event_sink.clone();
        let chat_hub = chat_hub.clone();

        tokio::spawn(async move {
            if let Err(error) =
                handle_client(stream, config, zone_registry, gameplay_event_sink, chat_hub).await
            {
                eprintln!("tcp client error from {peer}: {error}");
            }
        });
    }
}

async fn handle_client(
    mut stream: TcpStream,
    config: Arc<GatewayConfig>,
    zone_registry: Arc<ZoneRegistry>,
    gameplay_event_sink: Option<SharedGameplayEventSink>,
    chat_hub: ChatBroadcastHub,
) -> io::Result<()> {
    let peer = stream.peer_addr().ok();
    let mut session = match gameplay_event_sink {
        Some(sink) => GatewaySession::new_with_zone_registry_and_event_sink(
            (*config).clone(),
            &zone_registry,
            sink,
        ),
        None => GatewaySession::new_with_zone_registry((*config).clone(), &zone_registry),
    };
    session.configure_zone_owner_heartbeat(tcp_zone_owner_heartbeat_interval_ms(), 0);
    let result = handle_client_inner(&mut stream, &mut session, peer, &chat_hub).await;
    let _ = catch_gateway_panic("tcp save_active_character", || {
        session.save_active_character()
    });
    result
}

async fn handle_client_inner(
    stream: &mut TcpStream,
    session: &mut GatewaySession,
    peer: Option<std::net::SocketAddr>,
    chat_hub: &ChatBroadcastHub,
) -> io::Result<()> {
    let connect_packets = catch_gateway_panic("tcp on_connect", || session.on_connect())
        .map_err(session_panic_io_error)?;
    for packet in connect_packets {
        send_packet(stream, &packet).await?;
    }

    let (mut reader, mut writer) = stream.split();
    let (zone_outbound_tx, mut zone_outbound_rx) =
        mpsc::channel::<SharedZoneLiveOutbound>(LIVE_ZONE_OUTBOUND_CAPACITY);
    let mut active_zone_outbound_registration_id = 0;
    let mut _zone_live_outbound_registration: Option<Box<dyn ZoneLiveOutboundRegistration>> = None;
    let mut chat_presence: Option<ChatPresence> = None;
    let mut authenticated_account_id: Option<String> = None;

    loop {
        let frame = {
            let next_frame = read_frame(&mut reader);
            tokio::pin!(next_frame);
            loop {
                tokio::select! {
                    biased;

                    frame = &mut next_frame => break frame,
                    outbound = zone_outbound_rx.recv() => {
                        let Some(outbound) = outbound else {
                            return Err(io::Error::new(
                                io::ErrorKind::BrokenPipe,
                                "shared Zone live outbound channel closed",
                            ));
                        };
                        if outbound.registration_id() != active_zone_outbound_registration_id {
                            continue;
                        }
                        send_packet(&mut writer, &outbound.into_packet()).await?;
                    }
                    broadcast = recv_optional_chat(&mut chat_presence) => {
                        match broadcast {
                            Ok(packet) => send_packet(&mut writer, &packet).await?,
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => chat_presence = None,
                        }
                    }
                }
            }
        };
        let frame = match frame {
            Ok(frame) => frame,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::BrokenPipe
                ) =>
            {
                return Ok(())
            }
            Err(error) => return Err(error),
        };

        match decode_client_packet(&frame) {
            Ok(packet) => {
                if let Err(error) = session.renew_zone_owner_lease_if_due() {
                    if crate::gate15::health().is_some() {
                        eprintln!(
                            "Gate 15 Crystal owner-lease refresh is waiting for finalized placement: {error}"
                        );
                        continue;
                    }
                    return Err(session_panic_io_error(error));
                }
                let login_account_id = match &packet {
                    mir2_protocol::ClientPacket::Login { account_id, .. } => {
                        Some(account_id.clone())
                    }
                    _ => None,
                };
                let start_game_character_index = match &packet {
                    mir2_protocol::ClientPacket::StartGame { character_index } => {
                        Some(*character_index)
                    }
                    _ => None,
                };
                let starts_game = matches!(&packet, mir2_protocol::ClientPacket::StartGame { .. });
                let leaves_world = matches!(
                    &packet,
                    mir2_protocol::ClientPacket::Disconnect | mir2_protocol::ClientPacket::LogOut
                );
                if let Some(character_index) = start_game_character_index {
                    let account_id = authenticated_account_id.as_deref().ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "authenticated account is required before StartGame",
                        )
                    })?;
                    let zone_id = session.zone_id().clone();
                    if let Some(grant) =
                        crate::gate15::acquire_player_session(account_id, character_index, &zone_id)
                            .await
                            .map_err(|error| {
                                io::Error::new(
                                    io::ErrorKind::PermissionDenied,
                                    format!("Commonware session lease unavailable: {error}"),
                                )
                            })?
                    {
                        eprintln!(
                            "Gate 15 Crystal StartGame finalized {account_id}/{character_index} on {} generation {} at height {}",
                            grant.lease.zone_id,
                            grant.placement.generation,
                            grant.finalized_height
                        );
                    }
                }
                let responses = match catch_gateway_panic("tcp handle_packet", || {
                    session.handle_packet(packet)
                }) {
                    Ok(responses) => responses,
                    Err(error) if crate::gate15::health().is_some() => {
                        eprintln!(
                                "Gate 15 transient Crystal Zone command failure; keeping socket for placement recovery: {error}"
                            );
                        continue;
                    }
                    Err(error) => return Err(session_panic_io_error(error)),
                };
                if responses
                    .iter()
                    .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. }))
                {
                    authenticated_account_id = login_account_id;
                } else if responses.iter().any(|packet| {
                    matches!(
                        packet,
                        ServerPacket::Login { .. }
                            | ServerPacket::LoginBanned { .. }
                            | ServerPacket::ReturnToLogin
                    )
                }) {
                    authenticated_account_id = None;
                }
                if leaves_world || session.active_identity().is_none() {
                    chat_presence = None;
                }
                if leaves_world {
                    authenticated_account_id = None;
                }
                let next_registration = session
                    .register_zone_live_outbound(zone_outbound_tx.clone())
                    .map_err(session_panic_io_error)?;
                active_zone_outbound_registration_id = next_registration
                    .as_ref()
                    .map(|registration| registration.registration_id())
                    .unwrap_or(0);
                if let Some(registration) = next_registration.as_ref() {
                    registration.activate();
                }
                _zone_live_outbound_registration = next_registration;
                for response in responses {
                    send_packet(&mut writer, &response).await?;
                }
                if starts_game
                    && chat_presence.is_none()
                    && session.active_identity().is_some()
                    && session.zone_movement_ingress().is_some()
                {
                    chat_presence = Some(chat_hub.register(ChatProtocol::Tcp));
                }
                catch_gateway_panic("tcp save_active_character", || {
                    session.save_active_character()
                })
                .map_err(session_panic_io_error)?;
            }
            Err(error) => {
                eprintln!("tcp decode error from {:?}: {}", peer, error);
                return Ok(());
            }
        }
    }
}

fn session_panic_io_error(error: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error)
}

fn tcp_zone_owner_heartbeat_interval_ms() -> u64 {
    std::env::var("MIR2_GATEWAY_ZONE_OWNER_HEARTBEAT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(10_000)
        .clamp(100, 300_000)
}

/// Maximum time allowed to receive the remainder of a frame once its header has
/// arrived. Bounds slowloris-style attacks where a client announces a (up to
/// 64 KB) frame and then dribbles the body to pin a connection task open.
const FRAME_BODY_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

async fn read_frame(stream: &mut (impl AsyncRead + Unpin)) -> io::Result<Vec<u8>> {
    let mut header = [0_u8; 2];
    // No timeout on the header read: an idle connection legitimately waits here
    // for the next frame.
    stream.read_exact(&mut header).await?;
    let len = u16::from_le_bytes(header) as usize;

    if len < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid frame length {len}"),
        ));
    }

    let mut frame = vec![0_u8; len];
    frame[..2].copy_from_slice(&header);
    match tokio::time::timeout(FRAME_BODY_READ_TIMEOUT, stream.read_exact(&mut frame[2..])).await {
        Ok(result) => result?,
        Err(_elapsed) => {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out reading frame body",
            ))
        }
    };
    Ok(frame)
}

async fn send_packet(
    stream: &mut (impl AsyncWrite + Unpin),
    packet: &ServerPacket,
) -> io::Result<()> {
    let bytes = encode_server_packet(packet)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    stream.write_all(&bytes).await?;
    stream.flush().await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mir2_protocol::{
        decode_server_packet, encode_client_packet, ClientPacket, MirDirection, ServerPacket,
    };
    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, TcpStream};

    use super::{chat_broadcast::ChatBroadcastHub, handle_client_inner, read_frame};
    use crate::{GatewayConfig, GatewaySession, ZoneRegistry};

    async fn send_client_packets(stream: &mut TcpStream, packets: &[ClientPacket]) {
        let mut bytes = Vec::new();
        for packet in packets {
            bytes.extend(encode_client_packet(packet).expect("client packet should encode"));
        }
        stream
            .write_all(&bytes)
            .await
            .expect("client packets should write");
    }

    async fn drain_server_packets(stream: &mut TcpStream) -> Vec<ServerPacket> {
        let mut packets = Vec::new();
        while let Ok(Ok(frame)) =
            tokio::time::timeout(Duration::from_millis(50), read_frame(stream)).await
        {
            packets.push(decode_server_packet(&frame).expect("server packet should decode"));
        }
        packets
    }

    async fn read_server_packets_until(
        stream: &mut TcpStream,
        expected: impl Fn(&ServerPacket) -> bool,
    ) -> Vec<ServerPacket> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        let mut packets = Vec::new();
        loop {
            let frame = tokio::time::timeout_at(deadline, read_frame(stream))
                .await
                .expect("expected TCP response should arrive before timeout")
                .expect("TCP frame should remain readable");
            let packet = decode_server_packet(&frame).expect("server packet should decode");
            let done = expected(&packet);
            packets.push(packet);
            if done {
                return packets;
            }
        }
    }

    #[tokio::test]
    async fn delayed_zone_movement_reaches_idle_tcp_client() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let addr = listener
            .local_addr()
            .expect("listener should have an address");
        let mut client = TcpStream::connect(addr)
            .await
            .expect("test client should connect");
        let (mut server, peer) = listener
            .accept()
            .await
            .expect("server should accept client");

        let registry = ZoneRegistry::in_process();
        let chat_hub = ChatBroadcastHub::for_tests();
        let server_chat_hub = chat_hub.clone();
        let mut session =
            GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        let server_task = tokio::spawn(async move {
            handle_client_inner(&mut server, &mut session, Some(peer), &server_chat_hub).await
        });

        let _ = drain_server_packets(&mut client).await;
        send_client_packets(
            &mut client,
            &[ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }],
        )
        .await;
        let login_packets = read_server_packets_until(&mut client, |packet| {
            matches!(packet, ServerPacket::LoginSuccess { .. })
        })
        .await;
        assert!(
            login_packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
            "LoginSuccess response should arrive: {login_packets:?}"
        );
        send_client_packets(
            &mut client,
            &[ClientPacket::StartGame { character_index: 0 }],
        )
        .await;
        let start_packets = read_server_packets_until(&mut client, |packet| {
            matches!(packet, ServerPacket::StartGame { .. })
        })
        .await;
        assert!(
            start_packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::StartGame { .. })),
            "StartGame response should arrive: {start_packets:?}"
        );
        let chat_deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        while chat_hub.online_count() != 1 && tokio::time::Instant::now() < chat_deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(chat_hub.online_count(), 1);

        // The walk completes immediately; the following run is cadence-delayed.
        // Sending both frames together ensures no later client input can wake TCP.
        send_client_packets(
            &mut client,
            &[
                ClientPacket::Walk {
                    direction: MirDirection::Right,
                },
                ClientPacket::Run {
                    direction: MirDirection::Right,
                },
            ],
        )
        .await;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        let mut locations = Vec::new();
        while locations.len() < 2 {
            let frame = tokio::time::timeout_at(deadline, read_frame(&mut client))
                .await
                .expect("cadence-delayed UserLocation should reach idle TCP")
                .expect("TCP frame should remain readable");
            if let ServerPacket::UserLocation { location } =
                decode_server_packet(&frame).expect("server packet should decode")
            {
                locations.push(location.position);
            }
        }

        assert_ne!(locations[0], locations[1]);
        drop(client);
        server_task
            .await
            .expect("TCP server task should not panic")
            .expect("TCP server should close cleanly");
        assert_eq!(chat_hub.online_count(), 0);
    }
}
