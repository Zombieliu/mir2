//! Dynamic door state — open/close mechanics mirroring Crystal's
//! `Map.OpenDoor`, `Map.Process` (auto-close after 5000 ms) and
//! `Map.CheckDoorOpen` (closed doors block movement).
//!
//! Door cells are parsed from the `.map` file and grouped by door index in
//! [`DoorRegistry`]. While a door is closed its cells live in
//! `MapRuntimeResource.closed_door_cells`, which the movement collision code
//! already treats as blocked. Opening a door removes its cells from that set;
//! closing re-inserts them.

use bevy_ecs::prelude::World;
use mir2_protocol::ServerPacket;

use super::resources::{runtime_tick, MapRuntimeResource};

/// Crystal closes doors 5000 ms after they were opened (`Map.Process`). At one
/// runtime tick per 1000 ms that is five ticks.
pub(super) const DOOR_OPEN_DURATION_TICKS: u64 = 5;

/// Open the door with the given index (matching Crystal's `Map.OpenDoor`).
///
/// Re-opening an already-open door simply refreshes its auto-close timer, just
/// like Crystal which re-sets `LastTick` and re-broadcasts. Returns the
/// `OpenDoor` packet(s) to emit, or an empty vec if no such door exists.
pub(super) fn open_door(world: &mut World, door_index: u8) -> Vec<ServerPacket> {
    let tick = runtime_tick(world);
    let close_at = tick.saturating_add(DOOR_OPEN_DURATION_TICKS);

    let opened = {
        let mut map = world.resource_mut::<MapRuntimeResource>();
        match map.doors.find_mut(door_index) {
            Some(door) => {
                door.close_at_tick = Some(close_at);
                Some((door.index, door.cells.clone()))
            }
            None => None,
        }
    };

    let Some((index, cells)) = opened else {
        return Vec::new();
    };

    {
        let mut map = world.resource_mut::<MapRuntimeResource>();
        for cell in &cells {
            map.closed_door_cells.remove(cell);
        }
    }

    vec![ServerPacket::OpenDoor {
        door_index: index,
        close: false,
    }]
}

/// Auto-close any open door whose timer has elapsed, re-blocking its cells and
/// emitting the broadcast close packet (Crystal's `Map.Process` door loop).
pub(super) fn tick_doors(world: &mut World, packets: &mut Vec<ServerPacket>) {
    let tick = runtime_tick(world);

    let closing: Vec<(u8, Vec<(i32, i32)>)> = {
        let map = world.resource::<MapRuntimeResource>();
        if map.doors.is_empty() {
            return;
        }
        map.doors
            .doors
            .iter()
            .filter_map(|door| match door.close_at_tick {
                Some(close_at) if tick >= close_at => Some((door.index, door.cells.clone())),
                _ => None,
            })
            .collect()
    };

    if closing.is_empty() {
        return;
    }

    {
        let mut map = world.resource_mut::<MapRuntimeResource>();
        for (index, _) in &closing {
            if let Some(door) = map.doors.find_mut(*index) {
                door.close_at_tick = None;
            }
        }
        for (_, cells) in &closing {
            for cell in cells {
                map.closed_door_cells.insert(*cell);
            }
        }
    }

    for (index, _) in closing {
        packets.push(ServerPacket::OpenDoor {
            door_index: index,
            close: true,
        });
    }
}
