use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketCodecError {
    UnexpectedEof { needed: usize, remaining: usize },
    InvalidFrameLength { declared: usize, actual: usize },
    FrameTooShort { declared: usize },
    PacketTooLarge { size: usize },
    NegativeLength { field: &'static str, value: i32 },
    Invalid7BitEncodedInt,
    InvalidUtf8String,
    InvalidRequestId,
    InvalidBool(u8),
    InvalidEnumValue { type_name: &'static str, value: u8 },
    UnknownClientPacketId(i16),
    UnknownServerPacketId(i16),
    UnsupportedLinkedItemCount(i32),
    UnsupportedUserInformationSection { section: &'static str, count: i32 },
    TrailingBytes { packet_id: i16, remaining: usize },
}

pub type Result<T> = std::result::Result<T, PacketCodecError>;

impl Display for PacketCodecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof { needed, remaining } => {
                write!(
                    f,
                    "unexpected eof: needed {needed} more bytes, only {remaining} remaining"
                )
            }
            Self::InvalidFrameLength { declared, actual } => {
                write!(
                    f,
                    "invalid frame length: declared {declared} bytes, actual buffer {actual} bytes"
                )
            }
            Self::FrameTooShort { declared } => {
                write!(
                    f,
                    "invalid frame length {declared}: packet frames must be at least 4 bytes"
                )
            }
            Self::PacketTooLarge { size } => {
                write!(f, "packet too large to fit u16 frame length: {size} bytes")
            }
            Self::NegativeLength { field, value } => {
                write!(f, "negative length for {field}: {value}")
            }
            Self::Invalid7BitEncodedInt => write!(f, "invalid .NET 7-bit encoded integer"),
            Self::InvalidUtf8String => write!(f, "invalid utf-8 string data"),
            Self::InvalidRequestId => write!(f, "invalid request id"),
            Self::InvalidBool(value) => write!(f, "invalid bool value: {value}"),
            Self::InvalidEnumValue { type_name, value } => {
                write!(f, "invalid {type_name} enum value: {value}")
            }
            Self::UnknownClientPacketId(packet_id) => {
                write!(f, "unknown client packet id: {packet_id}")
            }
            Self::UnknownServerPacketId(packet_id) => {
                write!(f, "unknown server packet id: {packet_id}")
            }
            Self::UnsupportedLinkedItemCount(count) => {
                write!(
                    f,
                    "linked chat items are not implemented yet: count={count}"
                )
            }
            Self::UnsupportedUserInformationSection { section, count } => {
                write!(
                    f,
                    "user information section {section} is not implemented yet: count={count}"
                )
            }
            Self::TrailingBytes {
                packet_id,
                remaining,
            } => {
                write!(f, "packet {packet_id} has {remaining} trailing bytes")
            }
        }
    }
}

impl std::error::Error for PacketCodecError {}
