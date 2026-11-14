//! Internal spatial identities for the whole-Zone compatibility topology.
//!
//! The directory is a deterministic projection of live `ZoneRuntime`s: every
//! logical Zone owns partition zero and every spatial Cell resolves to it. It is
//! not persisted topology, a scheduler, or an ownership/lease authority. The
//! Gateway's existing owner lease remains the only source of placement fencing.

use mir2_protocol::Point;
use serde::{Deserialize, Serialize};

use super::ZoneKey;

/// Positive square Cell side in map tiles, independent of the AOI index size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "i32", into = "i32")]
pub struct CellSize(i32);

impl CellSize {
    /// Frozen geometry of the initial whole-Zone topology. Changing this value
    /// requires a new topology revision, even while all Cells share one writer.
    pub const WHOLE_ZONE: Self = Self(64);

    pub fn new(tiles: i32) -> Result<Self, String> {
        if tiles <= 0 {
            return Err("Cell size must be a positive number of map tiles".into());
        }
        Ok(Self(tiles))
    }

    pub const fn tiles(self) -> i32 {
        self.0
    }
}

impl TryFrom<i32> for CellSize {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<CellSize> for i32 {
    fn from(value: CellSize) -> Self {
        value.tiles()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CellCoord {
    pub x: i32,
    pub y: i32,
}

impl CellCoord {
    pub fn at(position: &Point, size: CellSize) -> Self {
        Self {
            x: position.x.div_euclid(size.tiles()),
            y: position.y.div_euclid(size.tiles()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PartitionId(pub u64);

impl PartitionId {
    pub const WHOLE_ZONE: Self = Self(0);
}

/// Logical identity; never substitute a physical host or Cell into `ZoneKey`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PartitionKey {
    pub zone: ZoneKey,
    pub partition_id: PartitionId,
}

impl PartitionKey {
    pub fn whole_zone(zone: ZoneKey) -> Self {
        Self {
            zone,
            partition_id: PartitionId::WHOLE_ZONE,
        }
    }
}

/// Cell-to-partition mapping revision, separate from the owner lease's fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct TopologyRevision(u64);

impl TopologyRevision {
    pub const WHOLE_ZONE: Self = Self(1);

    pub fn new(value: u64) -> Result<Self, String> {
        if value == 0 {
            return Err("topology revision must be positive".into());
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for TopologyRevision {
    type Error = String;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<TopologyRevision> for u64 {
    fn from(value: TopologyRevision) -> Self {
        value.value()
    }
}

/// Reference to the map's unique delayed-effect owner. It grants no write
/// permission. Current pending effects remain in the one original ZoneRuntime;
/// changing an actor's Cell does not clone or transfer their canonical queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MapEffectOwnerWire", into = "MapEffectOwnerWire")]
pub struct MapEffectOwnerRef {
    zone: ZoneKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MapEffectOwnerWire {
    version: u32,
    zone: ZoneKey,
}

impl MapEffectOwnerRef {
    pub const VERSION: u32 = 1;

    pub fn for_zone(zone: ZoneKey) -> Self {
        Self { zone }
    }

    pub fn zone(&self) -> &ZoneKey {
        &self.zone
    }

    pub const fn version(&self) -> u32 {
        Self::VERSION
    }
}

impl TryFrom<MapEffectOwnerWire> for MapEffectOwnerRef {
    type Error = String;

    fn try_from(value: MapEffectOwnerWire) -> Result<Self, Self::Error> {
        if value.version != Self::VERSION {
            return Err(format!(
                "unsupported map effect owner version {}",
                value.version
            ));
        }
        Ok(Self::for_zone(value.zone))
    }
}

impl From<MapEffectOwnerRef> for MapEffectOwnerWire {
    fn from(value: MapEffectOwnerRef) -> Self {
        Self {
            version: MapEffectOwnerRef::VERSION,
            zone: value.zone,
        }
    }
}

/// Derived, server-only directory row. No owner epoch is invented here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZonePartitionDescriptor {
    pub partition: PartitionKey,
    pub topology_revision: TopologyRevision,
    pub cell_size: CellSize,
    pub effect_owner: MapEffectOwnerRef,
}

impl ZonePartitionDescriptor {
    pub(crate) fn whole_zone(zone: ZoneKey) -> Self {
        Self {
            partition: PartitionKey::whole_zone(zone.clone()),
            topology_revision: TopologyRevision::WHOLE_ZONE,
            cell_size: CellSize::WHOLE_ZONE,
            effect_owner: MapEffectOwnerRef::for_zone(zone),
        }
    }

    pub fn locate(&self, position: &Point) -> PartitionLocation {
        PartitionLocation {
            partition: self.partition.clone(),
            cell: CellCoord::at(position, self.cell_size),
            topology_revision: self.topology_revision,
            effect_owner: self.effect_owner.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PartitionLocation {
    pub partition: PartitionKey,
    pub cell: CellCoord,
    pub topology_revision: TopologyRevision,
    pub effect_owner: MapEffectOwnerRef,
}
