use crate::error::{PacketCodecError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketFrame {
    pub packet_id: i16,
    pub payload: Vec<u8>,
}

pub fn decode_frame(bytes: &[u8]) -> Result<PacketFrame> {
    if bytes.len() < 4 {
        return Err(PacketCodecError::FrameTooShort {
            declared: bytes.len(),
        });
    }

    let declared = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
    let actual = bytes.len();

    if declared < 4 {
        return Err(PacketCodecError::FrameTooShort { declared });
    }

    if declared != actual {
        return Err(PacketCodecError::InvalidFrameLength { declared, actual });
    }

    let packet_id = i16::from_le_bytes([bytes[2], bytes[3]]);
    let payload = bytes[4..].to_vec();

    Ok(PacketFrame { packet_id, payload })
}

pub fn encode_frame(packet_id: i16, payload: &[u8]) -> Result<Vec<u8>> {
    let frame_size = payload.len() + 4;

    if frame_size > u16::MAX as usize {
        return Err(PacketCodecError::PacketTooLarge { size: frame_size });
    }

    let mut bytes = Vec::with_capacity(frame_size);
    bytes.extend_from_slice(&(frame_size as u16).to_le_bytes());
    bytes.extend_from_slice(&packet_id.to_le_bytes());
    bytes.extend_from_slice(payload);
    Ok(bytes)
}
