use std::io;
use std::sync::Arc;

use mir2_protocol::{decode_client_packet, encode_server_packet, ServerPacket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::events::{default_gameplay_event_sink_from_env, SharedGameplayEventSink};
use crate::session::catch_gateway_panic;
use crate::{GatewayConfig, GatewaySession, ZoneRegistry};

pub async fn run_tcp_gateway(addr: &str, config: GatewayConfig) -> io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let config = Arc::new(config);
    let zone_registry = Arc::new(ZoneRegistry::in_process_with_owner_lease_authority(
        crate::zone_lease::default_zone_owner_lease_authority_from_env(),
    ));
    let gameplay_event_sink = default_gameplay_event_sink_from_env();

    eprintln!("mir2-gateway tcp listening on {addr}");

    loop {
        let (stream, peer) = listener.accept().await?;
        let config = Arc::clone(&config);
        let zone_registry = Arc::clone(&zone_registry);
        let gameplay_event_sink = gameplay_event_sink.clone();

        tokio::spawn(async move {
            if let Err(error) =
                handle_client(stream, config, zone_registry, gameplay_event_sink).await
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
    let result = handle_client_inner(&mut stream, &mut session, peer).await;
    let _ = catch_gateway_panic("tcp save_active_character", || {
        session.save_active_character()
    });
    result
}

async fn handle_client_inner(
    stream: &mut TcpStream,
    session: &mut GatewaySession,
    peer: Option<std::net::SocketAddr>,
) -> io::Result<()> {
    let connect_packets = catch_gateway_panic("tcp on_connect", || session.on_connect())
        .map_err(session_panic_io_error)?;
    for packet in connect_packets {
        send_packet(stream, &packet).await?;
    }

    loop {
        let frame = match read_frame(stream).await {
            Ok(frame) => frame,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        };

        match decode_client_packet(&frame) {
            Ok(packet) => {
                let responses =
                    catch_gateway_panic("tcp handle_packet", || session.handle_packet(packet))
                        .map_err(session_panic_io_error)?;
                for response in responses {
                    send_packet(stream, &response).await?;
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

/// Maximum time allowed to receive the remainder of a frame once its header has
/// arrived. Bounds slowloris-style attacks where a client announces a (up to
/// 64 KB) frame and then dribbles the body to pin a connection task open.
const FRAME_BODY_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

async fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
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

async fn send_packet(stream: &mut TcpStream, packet: &ServerPacket) -> io::Result<()> {
    let bytes = encode_server_packet(packet)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    stream.write_all(&bytes).await?;
    stream.flush().await
}
