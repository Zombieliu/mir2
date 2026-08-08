use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use mir2_protocol::{decode_server_packet, encode_client_packet, ClientPacket, ServerPacket};

const DEFAULT_ADDR: &str = "127.0.0.1:7000";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = env::var("MIR2_GATEWAY_TCP_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let mut stream = TcpStream::connect(&addr)?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;

    println!("connected to {addr}");
    let _ = read_and_print(&mut stream)?;

    send(
        &mut stream,
        ClientPacket::ClientVersion {
            version_hash: vec![],
        },
    )?;
    let _ = read_and_print(&mut stream)?;

    send(
        &mut stream,
        ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        },
    )?;
    let _ = read_and_print(&mut stream)?;

    send(&mut stream, ClientPacket::StartGame { character_index: 0 })?;
    for _ in 0..8 {
        if !read_and_print(&mut stream)? {
            break;
        }
    }

    send(
        &mut stream,
        ClientPacket::Walk {
            direction: mir2_protocol::MirDirection::Right,
        },
    )?;
    let _ = read_and_print(&mut stream)?;

    send(
        &mut stream,
        ClientPacket::Chat {
            message: "hello from smoke".to_string(),
            linked_items: Vec::new(),
        },
    )?;
    let _ = read_and_print(&mut stream)?;
    let _ = read_and_print(&mut stream)?;

    Ok(())
}

fn send(stream: &mut TcpStream, packet: ClientPacket) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = encode_client_packet(&packet)?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    println!("sent: {:?}", packet);
    Ok(())
}

fn read_and_print(stream: &mut TcpStream) -> Result<bool, Box<dyn std::error::Error>> {
    let frame = match read_frame(stream) {
        Ok(frame) => frame,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(Box::new(error)),
    };
    let packet = decode_server_packet(&frame)?;
    match packet {
        ServerPacket::UserInformation { info } => {
            println!(
                "recv: UserInformation name={} class={:?} level={} pos=({}, {}) hp={} mp={} gold={}",
                info.name,
                info.class,
                info.level,
                info.location.x,
                info.location.y,
                info.hp,
                info.mp,
                info.gold
            );
        }
        ServerPacket::ObjectPlayer { info } => {
            println!(
                "recv: ObjectPlayer name={} class={:?} level={} pos=({}, {})",
                info.name, info.class, info.level, info.location.x, info.location.y
            );
        }
        ServerPacket::NewMonsterInfo { info } => {
            println!(
                "recv: NewMonsterInfo name={} image={} pos=({}, {})",
                info.name, info.image, info.location.x, info.location.y
            );
        }
        ServerPacket::NewNpcInfo { info } => {
            println!(
                "recv: NewNpcInfo name={} image={} pos=({}, {}) quests={}",
                info.name,
                info.image,
                info.location.x,
                info.location.y,
                info.quest_ids.len()
            );
        }
        other => println!("recv: {:?}", other),
    }
    Ok(true)
}

fn read_frame(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut header = [0_u8; 2];
    stream.read_exact(&mut header)?;
    let len = u16::from_le_bytes(header) as usize;
    let mut frame = vec![0_u8; len];
    frame[..2].copy_from_slice(&header);
    stream.read_exact(&mut frame[2..])?;
    Ok(frame)
}
