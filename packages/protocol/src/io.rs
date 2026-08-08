use crate::error::{PacketCodecError, Result};

#[derive(Debug, Clone)]
pub struct PacketReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> PacketReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    pub fn finish(&self, packet_id: i16) -> Result<()> {
        let remaining = self.remaining();
        if remaining == 0 {
            Ok(())
        } else {
            Err(PacketCodecError::TrailingBytes {
                packet_id,
                remaining,
            })
        }
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        self.ensure(1)?;
        let value = self.bytes[self.offset];
        self.offset += 1;
        Ok(value)
    }

    pub fn read_i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.read_array::<2>()?))
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_array::<2>()?))
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.read_array::<4>()?))
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_array::<4>()?))
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.read_array::<4>()?))
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_array::<8>()?))
    }

    pub fn read_i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.read_array::<8>()?))
    }

    pub fn read_bool(&mut self) -> Result<bool> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(PacketCodecError::InvalidBool(value)),
        }
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        self.ensure(len)?;
        let bytes = self.bytes[self.offset..self.offset + len].to_vec();
        self.offset += len;
        Ok(bytes)
    }

    pub fn read_string(&mut self) -> Result<String> {
        let len = self.read_7bit_encoded_u32()? as usize;
        self.ensure(len)?;
        let value = std::str::from_utf8(&self.bytes[self.offset..self.offset + len])
            .map_err(|_| PacketCodecError::InvalidUtf8String)?
            .to_string();
        self.offset += len;
        Ok(value)
    }

    fn read_7bit_encoded_u32(&mut self) -> Result<u32> {
        let mut result = 0_u32;
        let mut shift = 0_u32;

        for _ in 0..5 {
            let byte = self.read_u8()?;
            result |= u32::from(byte & 0x7F) << shift;

            if byte & 0x80 == 0 {
                return Ok(result);
            }

            shift += 7;
        }

        Err(PacketCodecError::Invalid7BitEncodedInt)
    }

    fn ensure(&self, len: usize) -> Result<()> {
        let remaining = self.remaining();
        if remaining < len {
            Err(PacketCodecError::UnexpectedEof {
                needed: len,
                remaining,
            })
        } else {
            Ok(())
        }
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.ensure(N)?;
        let mut bytes = [0_u8; N];
        bytes.copy_from_slice(&self.bytes[self.offset..self.offset + N]);
        self.offset += N;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PacketWriter {
    bytes: Vec<u8>,
}

impl PacketWriter {
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.bytes
    }

    pub fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn write_i16(&mut self, value: i16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_bool(&mut self, value: bool) {
        self.write_u8(if value { 1 } else { 0 });
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn write_string(&mut self, value: &str) -> Result<()> {
        let bytes = value.as_bytes();
        self.write_7bit_encoded_u32(bytes.len() as u32);
        self.write_bytes(bytes);
        Ok(())
    }

    fn write_7bit_encoded_u32(&mut self, mut value: u32) {
        while value >= 0x80 {
            self.write_u8((value as u8) | 0x80);
            value >>= 7;
        }
        self.write_u8(value as u8);
    }
}
