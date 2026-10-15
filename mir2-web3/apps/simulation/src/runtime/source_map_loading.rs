//! Bounded preparation of raw Source terrain, not a successful `Map.Load`.
//!
//! The caller supplies bytes and original MapInfo identity. No file lookup,
//! metadata, collision cache, configured fallback, Zone, or population is read.
//! Preparing cells does not initialize respawns, routes, NPCs/scripts, safe-zone
//! objects or mining, and cannot create a loaded-map receipt or directory.
//!
//! Layouts below follow Crystal Server/MirEnvir/Map.cs:73-429. Supported input
//! has positive bounded dimensions and exactly complete records, including
//! skipped bytes. Extra bytes, truncated records and unsupported custom versions
//! remain unavailable; that is not a claim that Source `Map.Load` returned false.
//! The untagged v0 domain is exactly its 52-byte header plus 12-byte cells, after
//! excluding the tags selected earlier by Source FindType. Source cell attributes
//! and their assignment order are preserved. GetWalkableCells (Map.cs:516-525)
//! uses only Attribute.Walk, with x outermost and y innermost; closed doors and
//! dynamic/arrival-protection rules never change this base list.

use std::fmt;

use mir2_game_data::{
    BlockedMapCellTemplate, DoorMapCellTemplate, FishingCellTemplate, MapBounds, MapCellAttribute,
    StarterMapCollision,
};
use sha2::{Digest, Sha256};

// Preparation resource bounds, not original Map.Load success/failure rules.
const MAX_TERRAIN_SIDE: i32 = 4096;
const MAX_TERRAIN_CELLS: usize = 4 * 1024 * 1024;
pub(super) const MAX_TERRAIN_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceTerrainCellAttribute {
    Walk,
    LowWall,
    HighWall,
}

/// An unavailable raw preparation domain, never a Source load-failure receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceMapTerrainUnavailable {
    InputTooLarge { actual: usize, limit: usize },
    TruncatedInput { required: usize, actual: usize },
    InvalidDimensions { width: i32, height: i32 },
    DimensionLimit { width: i32, height: i32 },
    CellLimit { actual: usize, limit: usize },
    UnsupportedCustomVersion { major: u8, minor: u8 },
    UnsupportedTrailingBytes { expected: usize, actual: usize },
    ArithmeticOverflow,
    AllocationUnavailable,
}

impl fmt::Display for SourceMapTerrainUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "raw Source terrain preparation unavailable: {self:?}")
    }
}

impl std::error::Error for SourceMapTerrainUnavailable {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceTerrainFormat {
    V0,
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
    V7,
    V100,
}

impl SourceTerrainFormat {
    fn number(self) -> u8 {
        match self {
            Self::V0 => 0,
            Self::V1 => 1,
            Self::V2 => 2,
            Self::V3 => 3,
            Self::V4 => 4,
            Self::V5 => 5,
            Self::V6 => 6,
            Self::V7 => 7,
            Self::V100 => 100,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SourceCellLayout {
    format: SourceTerrainFormat,
    width: u16,
    height: u16,
    cell_count: usize,
    start: usize,
    stride: usize,
    xor: i16,
}

#[derive(Debug, Clone, Copy)]
struct DecodedSourceCell {
    attribute: SourceTerrainCellAttribute,
    // Keep the raw presence byte: 0x80 denotes a present Source door index 0.
    door: Option<u8>,
    fishing: Option<i8>,
}

/// Immutable raw preparation. Its private fields are neither serialized nor
/// proof that any map has been installed or completed Source initialization.
#[derive(Debug)]
pub(crate) struct PreparedSourceMapTerrain {
    source_ordinal: usize,
    source_index: i32,
    raw_file_name: String,
    raw_sha256: [u8; 32],
    raw_byte_len: usize,
    format: SourceTerrainFormat,
    decoded_cells: Vec<DecodedSourceCell>,
    base_walkable_cells: Vec<(i32, i32)>,
    collision: StarterMapCollision,
}

impl PreparedSourceMapTerrain {
    pub(crate) fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) fn source_index(&self) -> i32 {
        self.source_index
    }

    pub(crate) fn raw_file_name(&self) -> &str {
        &self.raw_file_name
    }

    pub(crate) fn raw_sha256(&self) -> &[u8; 32] {
        &self.raw_sha256
    }

    pub(crate) fn raw_byte_len(&self) -> usize {
        self.raw_byte_len
    }

    pub(crate) fn map_format(&self) -> u8 {
        self.format.number()
    }

    pub(crate) fn width(&self) -> u16 {
        self.collision.map_width
    }

    pub(crate) fn height(&self) -> u16 {
        self.collision.map_height
    }

    pub(crate) fn cell_attribute_at(&self, x: u16, y: u16) -> Option<SourceTerrainCellAttribute> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        let index = usize::from(x) * usize::from(self.height()) + usize::from(y);
        self.decoded_cells.get(index).map(|cell| cell.attribute)
    }

    pub(crate) fn base_walkable_cells(&self) -> &[(i32, i32)] {
        &self.base_walkable_cells
    }

    /// Decoded raw terrain/door/fishing data, with no dynamic door filtering.
    /// The Source filename retains its spelling; Source appends `.map` itself.
    pub(crate) fn collision(&self) -> &StarterMapCollision {
        &self.collision
    }
}

pub(crate) fn prepare_source_map_terrain(
    source_ordinal: usize,
    source_index: i32,
    raw_file_name: &str,
    raw_bytes: &[u8],
) -> Result<PreparedSourceMapTerrain, SourceMapTerrainUnavailable> {
    let decoded =
        decode_source_terrain(raw_file_name, raw_bytes, TerrainDecodeDomain::Preparation)?;
    Ok(PreparedSourceMapTerrain {
        source_ordinal,
        source_index,
        raw_file_name: raw_file_name.to_owned(),
        raw_sha256: Sha256::digest(raw_bytes).into(),
        raw_byte_len: raw_bytes.len(),
        format: decoded.format,
        decoded_cells: decoded.decoded_cells,
        base_walkable_cells: decoded.base_walkable_cells,
        collision: decoded.collision,
    })
}

/// The normal collision loader shares the bounded Source decoder without
/// inventing MapInfo identity, a successful-load receipt or a population proof.
/// The normal caller supplies its existing canonical collision key.
/// It follows Source's actual reads, including ignored tail bytes; this is a
/// wider domain than strict raw preparation and cannot mint Prepared identity.
pub(super) fn decode_runtime_map_collision(
    map_file_name: &str,
    raw_bytes: &[u8],
) -> Result<StarterMapCollision, SourceMapTerrainUnavailable> {
    Ok(decode_source_terrain(
        map_file_name,
        raw_bytes,
        TerrainDecodeDomain::SourceCellReads,
    )?
    .collision)
}

#[derive(Clone, Copy)]
enum TerrainDecodeDomain {
    Preparation,
    SourceCellReads,
}

struct DecodedSourceTerrain {
    format: SourceTerrainFormat,
    decoded_cells: Vec<DecodedSourceCell>,
    base_walkable_cells: Vec<(i32, i32)>,
    collision: StarterMapCollision,
}

fn decode_source_terrain(
    raw_file_name: &str,
    raw_bytes: &[u8],
    domain: TerrainDecodeDomain,
) -> Result<DecodedSourceTerrain, SourceMapTerrainUnavailable> {
    check_input_length(raw_bytes.len())?;
    let layout = source_cell_layout(raw_bytes)?;
    let record_len = layout
        .cell_count
        .checked_mul(layout.stride)
        .and_then(|len| layout.start.checked_add(len))
        .ok_or(SourceMapTerrainUnavailable::ArithmeticOverflow)?;
    let collect_base_walkable = matches!(domain, TerrainDecodeDomain::Preparation);
    if collect_base_walkable {
        require_exact_records(raw_bytes.len(), record_len)?;
    } else {
        // Source advances across skipped fields and ignores trailing bytes.
        // Require its last actual read, not an invented end-of-file equality.
        let last_read_end = match layout.format {
            SourceTerrainFormat::V0 => 12,
            SourceTerrainFormat::V1 => 14,
            SourceTerrainFormat::V2 => 12,
            SourceTerrainFormat::V3 => 19,
            SourceTerrainFormat::V4 => 7,
            SourceTerrainFormat::V5 => 14,
            SourceTerrainFormat::V6 => 1,
            SourceTerrainFormat::V7 => 13,
            SourceTerrainFormat::V100 => 26,
        };
        let minimum_read_length = (layout.cell_count - 1)
            .checked_mul(layout.stride)
            .and_then(|length| layout.start.checked_add(length))
            .and_then(|length| length.checked_add(last_read_end))
            .ok_or(SourceMapTerrainUnavailable::ArithmeticOverflow)?;
        if raw_bytes.len() < minimum_read_length {
            return Err(SourceMapTerrainUnavailable::TruncatedInput {
                required: minimum_read_length,
                actual: raw_bytes.len(),
            });
        }
    }

    let mut decoded_cells = reserved_vec(layout.cell_count)?;
    let mut walkable_count = 0;
    let mut door_count = 0;
    let mut fishing_count = 0;
    for index in 0..layout.cell_count {
        let offset = index
            .checked_mul(layout.stride)
            .and_then(|offset| layout.start.checked_add(offset))
            .ok_or(SourceMapTerrainUnavailable::ArithmeticOverflow)?;
        let cell = decode_source_cell(raw_bytes, layout, offset)?;
        walkable_count += usize::from(cell.attribute == SourceTerrainCellAttribute::Walk);
        door_count += usize::from(cell.door.is_some());
        fishing_count += usize::from(cell.fishing.is_some());
        decoded_cells.push(cell);
    }

    let mut base_walkable_cells = reserved_vec(if collect_base_walkable {
        walkable_count
    } else {
        0
    })?;
    let mut blocked_cells = reserved_vec(layout.cell_count - walkable_count)?;
    let mut doors = reserved_vec(door_count)?;
    let mut fishing_cells = reserved_vec(fishing_count)?;
    for (index, cell) in decoded_cells.iter().enumerate() {
        let x = (index / usize::from(layout.height)) as i32;
        let y = (index % usize::from(layout.height)) as i32;
        match cell.attribute {
            SourceTerrainCellAttribute::Walk => {
                if collect_base_walkable {
                    base_walkable_cells.push((x, y));
                }
            }
            SourceTerrainCellAttribute::LowWall => blocked_cells.push(BlockedMapCellTemplate {
                x,
                y,
                attribute: MapCellAttribute::LowWall,
            }),
            SourceTerrainCellAttribute::HighWall => blocked_cells.push(BlockedMapCellTemplate {
                x,
                y,
                attribute: MapCellAttribute::HighWall,
            }),
        }
        if let Some(raw_door) = cell.door {
            doors.push(DoorMapCellTemplate {
                x,
                y,
                index: raw_door & 0x7f,
                closed: true,
            });
        }
        if let Some(attribute) = cell.fishing {
            fishing_cells.push(FishingCellTemplate { x, y, attribute });
        }
    }

    Ok(DecodedSourceTerrain {
        format: layout.format,
        decoded_cells,
        base_walkable_cells,
        collision: StarterMapCollision {
            map_file_name: format!("{raw_file_name}.map"),
            map_width: layout.width,
            map_height: layout.height,
            region_bounds: MapBounds {
                min_x: 0,
                max_x: i32::from(layout.width) - 1,
                min_y: 0,
                max_y: i32::from(layout.height) - 1,
            },
            play_bounds: MapBounds {
                min_x: 0,
                max_x: i32::from(layout.width) - 1,
                min_y: 0,
                max_y: i32::from(layout.height) - 1,
            },
            blocked_cells,
            doors,
            fishing_cells,
        },
    })
}

fn reserved_vec<T>(capacity: usize) -> Result<Vec<T>, SourceMapTerrainUnavailable> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| SourceMapTerrainUnavailable::AllocationUnavailable)?;
    Ok(values)
}

fn check_input_length(len: usize) -> Result<(), SourceMapTerrainUnavailable> {
    if len > MAX_TERRAIN_BYTES {
        return Err(SourceMapTerrainUnavailable::InputTooLarge {
            actual: len,
            limit: MAX_TERRAIN_BYTES,
        });
    }
    Ok(())
}

fn require_exact_records(
    actual: usize,
    expected: usize,
) -> Result<(), SourceMapTerrainUnavailable> {
    if actual < expected {
        return Err(SourceMapTerrainUnavailable::TruncatedInput {
            required: expected,
            actual,
        });
    }
    if actual > expected {
        return Err(SourceMapTerrainUnavailable::UnsupportedTrailingBytes { expected, actual });
    }
    Ok(())
}

fn source_dimensions(
    width: i16,
    height: i16,
) -> Result<(u16, u16, usize), SourceMapTerrainUnavailable> {
    let width = i32::from(width);
    let height = i32::from(height);
    if width <= 0 || height <= 0 {
        return Err(SourceMapTerrainUnavailable::InvalidDimensions { width, height });
    }
    if width > MAX_TERRAIN_SIDE || height > MAX_TERRAIN_SIDE {
        return Err(SourceMapTerrainUnavailable::DimensionLimit { width, height });
    }
    let cells = (width as usize)
        .checked_mul(height as usize)
        .ok_or(SourceMapTerrainUnavailable::ArithmeticOverflow)?;
    if cells > MAX_TERRAIN_CELLS {
        return Err(SourceMapTerrainUnavailable::CellLimit {
            actual: cells,
            limit: MAX_TERRAIN_CELLS,
        });
    }
    Ok((width as u16, height as u16, cells))
}

fn source_cell_layout(bytes: &[u8]) -> Result<SourceCellLayout, SourceMapTerrainUnavailable> {
    // Source FindType touches through byte 19 before its default v0 branch.
    // Every supported positive-dimension map contains at least these 20 bytes.
    if bytes.len() < 20 {
        return Err(SourceMapTerrainUnavailable::TruncatedInput {
            required: 20,
            actual: bytes.len(),
        });
    }
    let format = if bytes[2] == 0x43 && bytes[3] == 0x23 {
        if bytes[0] != 1 || bytes[1] != 0 {
            return Err(SourceMapTerrainUnavailable::UnsupportedCustomVersion {
                major: bytes[0],
                minor: bytes[1],
            });
        }
        SourceTerrainFormat::V100
    } else if bytes[0] == 0 {
        SourceTerrainFormat::V5
    } else if bytes[0] == 0x0f && bytes[5] == 0x53 && bytes[14] == 0x33 {
        SourceTerrainFormat::V6
    } else if bytes[0] == 0x15 && bytes[4] == 0x32 && bytes[6] == 0x41 && bytes[19] == 0x31 {
        SourceTerrainFormat::V4
    } else if bytes[0] == 0x10 && bytes[2] == 0x61 && bytes[7] == 0x31 && bytes[14] == 0x31 {
        SourceTerrainFormat::V1
    } else if bytes[4] == 0x0f || (bytes[4] == 0x03 && bytes[18] == 0x0d && bytes[19] == 0x0a) {
        // Keep C# operator precedence from Map.cs:96. For the bounded positive
        // domain, its unsigned header W/H agree with the signed load dimensions.
        let (_, _, cells) = source_dimensions(read_i16(bytes, 0)?, read_i16(bytes, 2)?)?;
        let v2_len = cells
            .checked_mul(14)
            .and_then(|len| 52usize.checked_add(len))
            .ok_or(SourceMapTerrainUnavailable::ArithmeticOverflow)?;
        if bytes.len() > v2_len {
            SourceTerrainFormat::V3
        } else {
            SourceTerrainFormat::V2
        }
    } else if bytes[0] == 0x0d && bytes[1] == 0x4c && bytes[7] == 0x20 && bytes[11] == 0x6d {
        SourceTerrainFormat::V7
    } else {
        SourceTerrainFormat::V0
    };

    let (width, height, xor) = match format {
        SourceTerrainFormat::V0 | SourceTerrainFormat::V2 | SourceTerrainFormat::V3 => {
            (read_i16(bytes, 0)?, read_i16(bytes, 2)?, 0)
        }
        SourceTerrainFormat::V1 => {
            let xor = read_i16(bytes, 23)?;
            (read_i16(bytes, 21)? ^ xor, read_i16(bytes, 25)? ^ xor, xor)
        }
        SourceTerrainFormat::V4 => {
            let xor = read_i16(bytes, 33)?;
            (read_i16(bytes, 31)? ^ xor, read_i16(bytes, 35)? ^ xor, xor)
        }
        SourceTerrainFormat::V5 => (read_i16(bytes, 22)?, read_i16(bytes, 24)?, 0),
        SourceTerrainFormat::V6 => (read_i16(bytes, 16)?, read_i16(bytes, 18)?, 0),
        SourceTerrainFormat::V7 => (read_i16(bytes, 21)?, read_i16(bytes, 25)?, 0),
        SourceTerrainFormat::V100 => (read_i16(bytes, 4)?, read_i16(bytes, 6)?, 0),
    };
    let (width, height, cell_count) = source_dimensions(width, height)?;
    let (start, stride) = match format {
        SourceTerrainFormat::V0 => (52, 12),
        SourceTerrainFormat::V1 => (54, 15),
        SourceTerrainFormat::V2 => (52, 14),
        SourceTerrainFormat::V3 => (52, 36),
        // Source v4 reads the door at +6 then advances 6, for stride 12.
        SourceTerrainFormat::V4 => (64, 12),
        SourceTerrainFormat::V5 => {
            let half_width = usize::from(width / 2 + width % 2);
            let start = half_width
                .checked_mul(usize::from(height / 2))
                .and_then(|len| len.checked_mul(3))
                .and_then(|len| 28usize.checked_add(len))
                .ok_or(SourceMapTerrainUnavailable::ArithmeticOverflow)?;
            (start, 14)
        }
        SourceTerrainFormat::V6 => (40, 20),
        SourceTerrainFormat::V7 => (54, 15),
        SourceTerrainFormat::V100 => (8, 26),
    };
    Ok(SourceCellLayout {
        format,
        width,
        height,
        cell_count,
        start,
        stride,
        xor,
    })
}

fn decode_source_cell(
    bytes: &[u8],
    layout: SourceCellLayout,
    offset: usize,
) -> Result<DecodedSourceCell, SourceMapTerrainUnavailable> {
    let (attribute, door_offset, light_offset) = match layout.format {
        SourceTerrainFormat::V0 | SourceTerrainFormat::V2 | SourceTerrainFormat::V3 => {
            let mut attribute = high_then_low(
                read_i16(bytes, offset)? < 0,
                read_i16(bytes, offset + 2)? < 0,
            );
            // Source assigns no-floor HighWall after its LowWall assignment.
            if read_i16(bytes, offset + 4)? < 0 {
                attribute = SourceTerrainCellAttribute::HighWall;
            }
            let (door, light) = match layout.format {
                SourceTerrainFormat::V0 => (8, 11),
                SourceTerrainFormat::V2 => (6, 11),
                _ => (6, 18),
            };
            (attribute, Some(offset + door), Some(offset + light))
        }
        SourceTerrainFormat::V1 => (
            high_then_low(
                ((read_u32(bytes, offset)? ^ 0xaa38_aa38) & 0x2000_0000) != 0,
                (read_i16(bytes, offset + 6)? ^ layout.xor) < 0,
            ),
            Some(offset + 8),
            Some(offset + 13),
        ),
        SourceTerrainFormat::V4 => (
            high_then_low(
                read_i16(bytes, offset)? < 0,
                read_i16(bytes, offset + 2)? < 0,
            ),
            Some(offset + 6),
            None,
        ),
        SourceTerrainFormat::V5 | SourceTerrainFormat::V6 => {
            let flags = read_byte(bytes, offset)?;
            let attribute = if flags & 1 != 1 {
                SourceTerrainCellAttribute::HighWall
            } else if flags & 2 != 2 {
                SourceTerrainCellAttribute::LowWall
            } else {
                SourceTerrainCellAttribute::Walk
            };
            let light = (layout.format == SourceTerrainFormat::V5).then_some(offset + 13);
            (attribute, None, light)
        }
        SourceTerrainFormat::V7 => (
            high_then_low(
                read_i16(bytes, offset)? < 0,
                read_i16(bytes, offset + 6)? < 0,
            ),
            Some(offset + 8),
            Some(offset + 12),
        ),
        SourceTerrainFormat::V100 => (
            high_then_low(
                read_u32(bytes, offset + 2)? & 0x2000_0000 != 0,
                read_i16(bytes, offset + 12)? < 0,
            ),
            Some(offset + 14),
            Some(offset + 25),
        ),
    };
    let door = door_offset
        .map(|offset| read_byte(bytes, offset))
        .transpose()?
        .filter(|byte| *byte > 0);
    let fishing = light_offset
        .map(|offset| read_byte(bytes, offset))
        .transpose()?
        .filter(|byte| (100..=119).contains(byte))
        .map(|byte| (byte - 100) as i8);
    Ok(DecodedSourceCell {
        attribute,
        door,
        fishing,
    })
}

fn high_then_low(high: bool, low: bool) -> SourceTerrainCellAttribute {
    if low {
        SourceTerrainCellAttribute::LowWall
    } else if high {
        SourceTerrainCellAttribute::HighWall
    } else {
        SourceTerrainCellAttribute::Walk
    }
}

fn read_bytes<const N: usize>(
    bytes: &[u8],
    offset: usize,
) -> Result<[u8; N], SourceMapTerrainUnavailable> {
    let end = offset
        .checked_add(N)
        .ok_or(SourceMapTerrainUnavailable::ArithmeticOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(SourceMapTerrainUnavailable::TruncatedInput {
            required: end,
            actual: bytes.len(),
        })?;
    let mut result = [0; N];
    result.copy_from_slice(value);
    Ok(result)
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8, SourceMapTerrainUnavailable> {
    Ok(read_bytes::<1>(bytes, offset)?[0])
}

fn read_i16(bytes: &[u8], offset: usize) -> Result<i16, SourceMapTerrainUnavailable> {
    Ok(i16::from_le_bytes(read_bytes(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, SourceMapTerrainUnavailable> {
    Ok(u32::from_le_bytes(read_bytes(bytes, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMATS: [SourceTerrainFormat; 9] = [
        SourceTerrainFormat::V0,
        SourceTerrainFormat::V1,
        SourceTerrainFormat::V2,
        SourceTerrainFormat::V3,
        SourceTerrainFormat::V4,
        SourceTerrainFormat::V5,
        SourceTerrainFormat::V6,
        SourceTerrainFormat::V7,
        SourceTerrainFormat::V100,
    ];

    fn put_i16(bytes: &mut [u8], offset: usize, value: i16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    // Fixtures are raw layout bytes, not map-load records or online authority.
    fn raw_map(format: SourceTerrainFormat, width: u16, height: u16) -> Vec<u8> {
        let cells = usize::from(width) * usize::from(height);
        let (start, stride) = match format {
            SourceTerrainFormat::V0 => (52, 12),
            SourceTerrainFormat::V1 => (54, 15),
            SourceTerrainFormat::V2 => (52, 14),
            SourceTerrainFormat::V3 => (52, 36),
            SourceTerrainFormat::V4 => (64, 12),
            SourceTerrainFormat::V5 => (
                28 + 3 * usize::from(width / 2 + width % 2) * usize::from(height / 2),
                14,
            ),
            SourceTerrainFormat::V6 => (40, 20),
            SourceTerrainFormat::V7 => (54, 15),
            SourceTerrainFormat::V100 => (8, 26),
        };
        let mut bytes = vec![0; start + stride * cells];
        match format {
            SourceTerrainFormat::V0 | SourceTerrainFormat::V2 | SourceTerrainFormat::V3 => {
                put_i16(&mut bytes, 0, width as i16);
                put_i16(&mut bytes, 2, height as i16);
                if format != SourceTerrainFormat::V0 {
                    bytes[4] = 0x0f;
                    // Deliberately no CRLF: Source's 0x0f branch does not need it.
                }
            }
            SourceTerrainFormat::V1 => {
                bytes[0] = 0x10;
                bytes[2] = 0x61;
                bytes[7] = 0x31;
                bytes[14] = 0x31;
                let xor = 0x55_i16;
                put_i16(&mut bytes, 21, width as i16 ^ xor);
                put_i16(&mut bytes, 23, xor);
                put_i16(&mut bytes, 25, height as i16 ^ xor);
                for index in 0..cells {
                    put_u32(&mut bytes, start + index * stride, 0xaa38_aa38);
                    put_i16(&mut bytes, start + index * stride + 6, xor);
                }
            }
            SourceTerrainFormat::V4 => {
                bytes[0] = 0x15;
                bytes[4] = 0x32;
                bytes[6] = 0x41;
                bytes[19] = 0x31;
                let xor = 0x55_i16;
                put_i16(&mut bytes, 31, width as i16 ^ xor);
                put_i16(&mut bytes, 33, xor);
                put_i16(&mut bytes, 35, height as i16 ^ xor);
            }
            SourceTerrainFormat::V5 | SourceTerrainFormat::V6 => {
                if format == SourceTerrainFormat::V5 {
                    put_i16(&mut bytes, 22, width as i16);
                    put_i16(&mut bytes, 24, height as i16);
                } else {
                    bytes[0] = 0x0f;
                    bytes[5] = 0x53;
                    bytes[14] = 0x33;
                    put_i16(&mut bytes, 16, width as i16);
                    put_i16(&mut bytes, 18, height as i16);
                }
                for index in 0..cells {
                    bytes[start + index * stride] = 3;
                }
            }
            SourceTerrainFormat::V7 => {
                bytes[0] = 0x0d;
                bytes[1] = 0x4c;
                bytes[7] = 0x20;
                bytes[11] = 0x6d;
                put_i16(&mut bytes, 21, width as i16);
                put_i16(&mut bytes, 25, height as i16);
            }
            SourceTerrainFormat::V100 => {
                bytes[..4].copy_from_slice(&[1, 0, 0x43, 0x23]);
                put_i16(&mut bytes, 4, width as i16);
                put_i16(&mut bytes, 6, height as i16);
            }
        }
        bytes
    }

    fn prepare(bytes: &[u8]) -> PreparedSourceMapTerrain {
        prepare_source_map_terrain(7, 23, "Room", bytes).unwrap()
    }

    #[test]
    fn ordinary_collision_entry_uses_all_source_layouts_without_fabricated_identity() {
        for format in FORMATS {
            let bytes = raw_map(format, 3, 2);
            let prepared = prepare(&bytes);
            let actual = super::super::map::parse_runtime_map_collision(" Room.MAP ", &bytes)
                .expect("normal collision entry supports the bounded Source layout");
            let mut expected = prepared.collision().clone();
            expected.map_file_name = "room.map".to_owned();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn ordinary_collision_entry_requires_every_actual_source_read() {
        // Explicit last read boundaries from the original C# methods, including
        // formats whose last advance skips bytes that are never read.
        for (format, required) in [
            (SourceTerrainFormat::V0, 100),
            (SourceTerrainFormat::V1, 113),
            (SourceTerrainFormat::V2, 106),
            (SourceTerrainFormat::V3, 179),
            (SourceTerrainFormat::V4, 107),
            (SourceTerrainFormat::V5, 87),
            (SourceTerrainFormat::V6, 101),
            (SourceTerrainFormat::V7, 112),
            (SourceTerrainFormat::V100, 112),
        ] {
            let bytes = raw_map(format, 2, 2);
            assert!(
                super::super::map::parse_runtime_map_collision("room", &bytes[..required])
                    .is_some(),
                "{format:?}"
            );
            assert!(
                super::super::map::parse_runtime_map_collision("room", &bytes[..required - 1])
                    .is_none(),
                "{format:?}"
            );
        }
        let mut bytes = raw_map(SourceTerrainFormat::V100, 2, 2);
        bytes[0] = 2;
        assert!(super::super::map::parse_runtime_map_collision("room", &bytes).is_none());
    }

    #[test]
    fn ordinary_source_ignored_tail_does_not_mint_strict_preparation() {
        let mut bytes = raw_map(SourceTerrainFormat::V0, 2, 2);
        let before = super::super::map::parse_runtime_map_collision("room", &bytes).unwrap();
        bytes.push(0);
        assert_eq!(
            super::super::map::parse_runtime_map_collision("room", &bytes),
            Some(before)
        );
        assert!(matches!(
            prepare_source_map_terrain(0, 1, "room", &bytes),
            Err(SourceMapTerrainUnavailable::UnsupportedTrailingBytes { .. })
        ));
        let bytes = raw_map(SourceTerrainFormat::V6, 2, 2);
        assert!(super::super::map::parse_runtime_map_collision("room", &bytes[..101]).is_some());
        assert!(matches!(
            prepare_source_map_terrain(0, 1, "room", &bytes[..101]),
            Err(SourceMapTerrainUnavailable::TruncatedInput { .. })
        ));
    }

    #[test]
    fn raw_closed_door_remains_base_walkable_and_cells_are_x_major() {
        let mut bytes = raw_map(SourceTerrainFormat::V0, 2, 2);
        bytes[52 + 8] = 0x83;
        bytes[52 + 11] = 104;
        put_i16(&mut bytes, 64 + 2, i16::MIN);
        put_i16(&mut bytes, 64 + 4, i16::MIN);
        let prepared = prepare(&bytes);
        assert_eq!(prepared.base_walkable_cells(), &[(0, 0), (1, 0), (1, 1)]);
        assert_eq!(
            prepared.cell_attribute_at(0, 0),
            Some(SourceTerrainCellAttribute::Walk)
        );
        assert_eq!(
            prepared.cell_attribute_at(0, 1),
            Some(SourceTerrainCellAttribute::HighWall)
        );
        assert_eq!(prepared.cell_attribute_at(2, 0), None);
        assert_eq!(prepared.cell_attribute_at(0, 2), None);
        assert_eq!(
            prepared.collision().doors,
            vec![DoorMapCellTemplate {
                x: 0,
                y: 0,
                index: 3,
                closed: true,
            }]
        );
        assert_eq!(
            prepared.collision().fishing_cells,
            vec![FishingCellTemplate {
                x: 0,
                y: 0,
                attribute: 4,
            }]
        );
        assert_eq!(
            prepared.collision().blocked_cells[0].attribute,
            MapCellAttribute::HighWall
        );
    }

    #[test]
    fn raw_identity_and_sha256_do_not_normalize_or_acknowledge_loading() {
        let mut bytes = raw_map(SourceTerrainFormat::V0, 2, 2);
        bytes[60] = 0x83;
        bytes[63] = 104;
        put_i16(&mut bytes, 66, i16::MIN);
        put_i16(&mut bytes, 68, i16::MIN);
        let prepared = prepare_source_map_terrain(37, 201, " Room.MAP ", &bytes).unwrap();
        assert_eq!(prepared.source_ordinal(), 37);
        assert_eq!(prepared.source_index(), 201);
        assert_eq!(prepared.raw_file_name(), " Room.MAP ");
        assert_eq!(prepared.collision().map_file_name, " Room.MAP .map");
        assert_eq!(prepared.raw_byte_len(), 100);
        // Independent SHA256 golden for these 100 raw bytes, not a digest of a
        // normalized filename, collision projection or metadata record.
        let digest = prepared
            .raw_sha256()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(
            digest,
            "98f92fd11a0657b5dad17dd94f4a75ceb381d6cf82c92ceb7d9640f438c7bbc4"
        );
        let same_bytes = prepare_source_map_terrain(38, 202, "room.map", &bytes).unwrap();
        assert_eq!(same_bytes.raw_sha256(), prepared.raw_sha256());
        assert_ne!(same_bytes.source_ordinal(), prepared.source_ordinal());
        assert_ne!(same_bytes.source_index(), prepared.source_index());
        assert_ne!(same_bytes.raw_file_name(), prepared.raw_file_name());
        bytes[51] ^= 1; // Unused header bytes still belong to the raw digest.
        assert_ne!(prepare(&bytes).raw_sha256(), prepared.raw_sha256());
    }

    #[test]
    fn all_proven_layouts_decode_positive_dimensions_and_x_major_walk_cells() {
        for format in FORMATS {
            let prepared = prepare(&raw_map(format, 3, 2));
            assert_eq!(prepared.map_format(), format.number());
            assert_eq!((prepared.width(), prepared.height()), (3, 2));
            assert_eq!(
                prepared.base_walkable_cells(),
                &[(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1)]
            );
            assert!(prepared.collision().blocked_cells.is_empty());
            assert!(prepared.collision().doors.is_empty());
            assert!(prepared.collision().fishing_cells.is_empty());
        }
    }

    #[test]
    fn no_floor_is_the_last_assignment_in_v0_v2_and_v3() {
        for format in [
            SourceTerrainFormat::V0,
            SourceTerrainFormat::V2,
            SourceTerrainFormat::V3,
        ] {
            let mut bytes = raw_map(format, 2, 2);
            let stride = match format {
                SourceTerrainFormat::V0 => 12,
                SourceTerrainFormat::V2 => 14,
                _ => 36,
            };
            put_i16(&mut bytes, 52, i16::MIN);
            put_i16(&mut bytes, 52 + stride + 2, i16::MIN);
            put_i16(&mut bytes, 52 + stride * 2 + 2, i16::MIN);
            put_i16(&mut bytes, 52 + stride * 2 + 4, i16::MIN);
            let door = if format == SourceTerrainFormat::V0 {
                8
            } else {
                6
            };
            bytes[52 + stride * 3 + door] = 0x89;
            let prepared = prepare(&bytes);
            assert_eq!(
                prepared.cell_attribute_at(0, 0),
                Some(SourceTerrainCellAttribute::HighWall)
            );
            assert_eq!(
                prepared.cell_attribute_at(0, 1),
                Some(SourceTerrainCellAttribute::LowWall)
            );
            assert_eq!(
                prepared.cell_attribute_at(1, 0),
                Some(SourceTerrainCellAttribute::HighWall)
            );
            assert_eq!(prepared.base_walkable_cells(), &[(1, 1)]);
            assert_eq!(
                prepared.collision().doors,
                vec![DoorMapCellTemplate {
                    x: 1,
                    y: 1,
                    index: 9,
                    closed: true,
                }]
            );
        }
    }

    #[test]
    fn high_and_low_flags_preserve_source_priority_in_other_layouts() {
        for format in [
            SourceTerrainFormat::V1,
            SourceTerrainFormat::V4,
            SourceTerrainFormat::V7,
            SourceTerrainFormat::V100,
        ] {
            let mut bytes = raw_map(format, 4, 1);
            let (start, stride, low, door) = match format {
                SourceTerrainFormat::V1 => (54, 15, 6, 8),
                SourceTerrainFormat::V4 => (64, 12, 2, 6),
                SourceTerrainFormat::V7 => (54, 15, 6, 8),
                _ => (8, 26, 12, 14),
            };
            for (index, (high, low_flag)) in [(true, false), (false, true), (true, true)]
                .into_iter()
                .enumerate()
            {
                let offset = start + index * stride;
                let low_bits = if low_flag { i16::MIN } else { 0 };
                let high_bits = if high { 0x2000_0000 } else { 0 };
                match format {
                    SourceTerrainFormat::V1 => {
                        put_u32(&mut bytes, offset, high_bits ^ 0xaa38_aa38);
                        put_i16(&mut bytes, offset + low, low_bits ^ 0x55);
                    }
                    SourceTerrainFormat::V100 => {
                        put_u32(&mut bytes, offset + 2, high_bits);
                        put_i16(&mut bytes, offset + low, low_bits);
                    }
                    _ => {
                        put_i16(&mut bytes, offset, if high { i16::MIN } else { 0 });
                        put_i16(&mut bytes, offset + low, low_bits);
                    }
                }
            }
            bytes[start + 3 * stride + door] = 0xa9;
            let prepared = prepare(&bytes);
            assert_eq!(
                prepared.cell_attribute_at(0, 0),
                Some(SourceTerrainCellAttribute::HighWall)
            );
            assert_eq!(
                prepared.cell_attribute_at(1, 0),
                Some(SourceTerrainCellAttribute::LowWall)
            );
            assert_eq!(
                prepared.cell_attribute_at(2, 0),
                Some(SourceTerrainCellAttribute::LowWall)
            );
            assert_eq!(prepared.base_walkable_cells(), &[(3, 0)]);
            assert_eq!(
                prepared.collision().doors,
                vec![DoorMapCellTemplate {
                    x: 3,
                    y: 0,
                    index: 41,
                    closed: true,
                }]
            );
        }
    }

    #[test]
    fn mir3_flag_priority_and_odd_half_grid_offset_follow_source() {
        for format in [SourceTerrainFormat::V5, SourceTerrainFormat::V6] {
            let mut bytes = raw_map(format, 3, 2);
            let (start, stride) = if format == SourceTerrainFormat::V5 {
                (34, 14)
            } else {
                (40, 20)
            };
            bytes[start] = 0;
            bytes[start + stride] = 1;
            bytes[start + stride * 2] = 2;
            let prepared = prepare(&bytes);
            assert_eq!(
                prepared.cell_attribute_at(0, 0),
                Some(SourceTerrainCellAttribute::HighWall)
            );
            assert_eq!(
                prepared.cell_attribute_at(0, 1),
                Some(SourceTerrainCellAttribute::LowWall)
            );
            assert_eq!(
                prepared.cell_attribute_at(1, 0),
                Some(SourceTerrainCellAttribute::HighWall)
            );
            assert_eq!(prepared.base_walkable_cells(), &[(1, 1), (2, 0), (2, 1)]);
        }
    }

    #[test]
    fn v4_uses_twelve_byte_cells_and_does_not_invent_fishing() {
        let mut bytes = raw_map(SourceTerrainFormat::V4, 2, 1);
        bytes[64 + 6] = 0x80;
        bytes[64 + 11] = 110;
        put_i16(&mut bytes, 76, i16::MIN);
        let prepared = prepare(&bytes);
        assert_eq!(prepared.base_walkable_cells(), &[(0, 0)]);
        assert_eq!(
            prepared.cell_attribute_at(1, 0),
            Some(SourceTerrainCellAttribute::HighWall)
        );
        assert_eq!(
            prepared.collision().doors,
            vec![DoorMapCellTemplate {
                x: 0,
                y: 0,
                index: 0,
                closed: true
            }]
        );
        assert!(prepared.collision().fishing_cells.is_empty());
    }

    #[test]
    fn light_offsets_and_fishing_limits_match_each_source_reader() {
        for format in [
            SourceTerrainFormat::V0,
            SourceTerrainFormat::V1,
            SourceTerrainFormat::V2,
            SourceTerrainFormat::V3,
            SourceTerrainFormat::V5,
            SourceTerrainFormat::V7,
            SourceTerrainFormat::V100,
        ] {
            let mut bytes = raw_map(format, 4, 1);
            let (start, stride, light) = match format {
                SourceTerrainFormat::V0 => (52, 12, 11),
                SourceTerrainFormat::V1 => (54, 15, 13),
                SourceTerrainFormat::V2 => (52, 14, 11),
                SourceTerrainFormat::V3 => (52, 36, 18),
                SourceTerrainFormat::V5 => (28, 14, 13),
                SourceTerrainFormat::V7 => (54, 15, 12),
                _ => (8, 26, 25),
            };
            for (index, value) in [99, 100, 119, 120].into_iter().enumerate() {
                bytes[start + index * stride + light] = value;
            }
            let prepared = prepare(&bytes);
            assert_eq!(
                prepared.collision().fishing_cells,
                vec![
                    FishingCellTemplate {
                        x: 1,
                        y: 0,
                        attribute: 0
                    },
                    FishingCellTemplate {
                        x: 2,
                        y: 0,
                        attribute: 19
                    }
                ]
            );
        }
    }

    #[test]
    fn source_type_0f_branch_does_not_require_crlf_but_03_branch_does() {
        let mut v2 = raw_map(SourceTerrainFormat::V2, 2, 2);
        assert_eq!(prepare(&v2).map_format(), 2);
        v2[4] = 0x03;
        assert!(matches!(
            prepare_source_map_terrain(0, 1, "Room", &v2),
            Err(SourceMapTerrainUnavailable::UnsupportedTrailingBytes { .. })
        ));
        v2[18] = 0x0d;
        v2[19] = 0x0a;
        assert_eq!(prepare(&v2).map_format(), 2);
        assert_eq!(
            prepare(&raw_map(SourceTerrainFormat::V3, 2, 2)).map_format(),
            3
        );
    }

    #[test]
    fn incomplete_headers_records_and_optional_runtime_light_are_unavailable() {
        for format in FORMATS {
            let bytes = raw_map(format, 2, 2);
            for cut in [0, 1, 3, 19, bytes.len() - 1] {
                assert!(
                    matches!(
                        prepare_source_map_terrain(0, 1, "Room", &bytes[..cut]),
                        Err(SourceMapTerrainUnavailable::TruncatedInput { .. })
                    ),
                    "format {format:?}, cut {cut}"
                );
            }
        }
        // Source v0's last light byte is required, even if the runtime parser
        // currently treats a missing fishing/light byte as an optional field.
        let bytes = raw_map(SourceTerrainFormat::V0, 2, 2);
        assert!(matches!(
            prepare_source_map_terrain(0, 1, "Room", &bytes[..bytes.len() - 1]),
            Err(SourceMapTerrainUnavailable::TruncatedInput { .. })
        ));
    }

    #[test]
    fn skipped_record_tail_is_not_accepted_as_complete_raw_preparation() {
        let bytes = raw_map(SourceTerrainFormat::V6, 2, 2);
        // Source v6 reads the flags then skips 19 bytes. The preparation domain
        // deliberately requires the entire last record, without calling a
        // successfully-read flags byte a complete raw map or Source load.
        let truncated = &bytes[..bytes.len() - 19];
        assert!(matches!(
            prepare_source_map_terrain(0, 1, "Room", truncated),
            Err(SourceMapTerrainUnavailable::TruncatedInput { .. })
        ));
    }

    #[test]
    fn unknown_custom_version_and_trailing_layout_domain_are_unavailable() {
        let mut custom = raw_map(SourceTerrainFormat::V100, 2, 2);
        custom[0] = 2;
        assert!(matches!(
            prepare_source_map_terrain(0, 1, "Room", &custom),
            Err(SourceMapTerrainUnavailable::UnsupportedCustomVersion { major: 2, minor: 0 })
        ));
        custom[0] = 1;
        custom[1] = 1;
        assert!(matches!(
            prepare_source_map_terrain(0, 1, "Room", &custom),
            Err(SourceMapTerrainUnavailable::UnsupportedCustomVersion { major: 1, minor: 1 })
        ));
        let mut legacy = raw_map(SourceTerrainFormat::V0, 2, 2);
        legacy.push(0);
        assert!(matches!(
            prepare_source_map_terrain(0, 1, "Room", &legacy),
            Err(SourceMapTerrainUnavailable::UnsupportedTrailingBytes { .. })
        ));
    }

    #[test]
    fn nonpositive_signed_dimensions_are_unavailable_before_allocation() {
        let mut bytes = raw_map(SourceTerrainFormat::V6, 2, 2);
        for (width, height) in [(0, 2), (2, 0), (-1, 2), (2, -1), (i16::MIN, 2)] {
            put_i16(&mut bytes, 16, width);
            put_i16(&mut bytes, 18, height);
            assert!(matches!(
                prepare_source_map_terrain(0, 1, "Room", &bytes),
                Err(SourceMapTerrainUnavailable::InvalidDimensions { .. })
            ));
        }
    }

    #[test]
    fn preparation_dimensions_cell_count_and_byte_length_are_bounded() {
        let mut bytes = raw_map(SourceTerrainFormat::V6, 2, 2);
        put_i16(&mut bytes, 16, 4097);
        assert!(matches!(
            prepare_source_map_terrain(0, 1, "Room", &bytes),
            Err(SourceMapTerrainUnavailable::DimensionLimit { .. })
        ));
        put_i16(&mut bytes, 16, 4096);
        put_i16(&mut bytes, 18, 4096);
        assert!(matches!(
            prepare_source_map_terrain(0, 1, "Room", &bytes),
            Err(SourceMapTerrainUnavailable::CellLimit { .. })
        ));
        assert_eq!(source_dimensions(4096, 1024).unwrap().2, MAX_TERRAIN_CELLS);
        assert!(check_input_length(MAX_TERRAIN_BYTES).is_ok());
        assert!(matches!(
            check_input_length(MAX_TERRAIN_BYTES + 1),
            Err(SourceMapTerrainUnavailable::InputTooLarge { .. })
        ));
        assert!(matches!(
            read_bytes::<2>(&[], usize::MAX),
            Err(SourceMapTerrainUnavailable::ArithmeticOverflow)
        ));
    }

    #[test]
    fn raw_preparation_retains_each_record_without_dedup_or_a_loaded_catalogue() {
        let raw = raw_map(SourceTerrainFormat::V0, 2, 1);
        let first = prepare_source_map_terrain(3, 41, "Repeated", &raw).unwrap();
        let second = prepare_source_map_terrain(7, 89, "Repeated", &raw).unwrap();
        assert_eq!((first.source_ordinal(), first.source_index()), (3, 41));
        assert_eq!((second.source_ordinal(), second.source_index()), (7, 89));
        assert_eq!(first.raw_file_name(), second.raw_file_name());
        assert_eq!(first.raw_sha256(), second.raw_sha256());
        // An empty Source filename is retained literally by this raw stage;
        // its bytes cannot prove a later filesystem lookup or Map.Load result.
        let unnamed = prepare_source_map_terrain(463, 477, "", &raw).unwrap();
        assert_eq!(unnamed.raw_file_name(), "");
        assert_eq!(unnamed.collision().map_file_name, ".map");
    }
}
