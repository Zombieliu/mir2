use std::collections::BTreeMap;

use bevy_tasks::{ComputeTaskPool, TaskPool};
use mir2_protocol::{MirDirection, Point, Spell};

/// Below this many zones, parallel ticking's task overhead outweighs the gain,
/// so `tick_all` runs inline. Measured break-even; see the multi-zone scaling
/// section of `examples/zone_load.rs` and docs/L2-ECS-ZONE-DESIGN.md.
const PARALLEL_TICK_MIN_ZONES: usize = 4;

use super::runtime::ZoneRuntime;
use super::types::{SessionId, ZoneCommand, ZoneJoin, ZoneKey, ZoneOutbound};

// Not `Clone`/`Default`-derived: it holds `ZoneRuntime`s which own a
// non-cloneable `bevy_ecs::World`, and nothing cloned/defaulted the manager.
#[derive(Debug)]
pub struct ZoneManager {
    zones: BTreeMap<ZoneKey, ZoneRuntime>,
    session_zones: BTreeMap<SessionId, ZoneKey>,
}

impl ZoneManager {
    pub fn new() -> Self {
        Self {
            zones: BTreeMap::new(),
            session_zones: BTreeMap::new(),
        }
    }

    pub fn join(&mut self, join: ZoneJoin) -> Vec<ZoneOutbound> {
        let key = ZoneKey::for_map(join.map_file_name.clone());
        let mut outbounds = Vec::new();
        if let Some(previous_key) = self.session_zones.get(&join.session_id).cloned() {
            if previous_key != key {
                outbounds.extend(self.handle_for_key(
                    previous_key,
                    ZoneCommand::Leave {
                        session_id: join.session_id.clone(),
                    },
                ));
            }
        }
        outbounds.extend(self.handle_for_key(key, ZoneCommand::Join(join)));
        outbounds
    }

    pub fn handle(&mut self, command: ZoneCommand) -> Vec<ZoneOutbound> {
        match &command {
            ZoneCommand::Join(join) => self.join(join.clone()),
            ZoneCommand::Leave { session_id }
            | ZoneCommand::Walk { session_id, .. }
            | ZoneCommand::Run { session_id, .. }
            | ZoneCommand::Turn { session_id, .. }
            | ZoneCommand::UpdateChatProfile { session_id, .. }
            | ZoneCommand::UpdatePlayerCombatStats { session_id, .. }
            | ZoneCommand::SyncPlayerTransform { session_id, .. }
            | ZoneCommand::Chat { session_id, .. }
            | ZoneCommand::BroadcastPackets { session_id, .. }
            | ZoneCommand::SyncSharedObjects { session_id, .. }
            | ZoneCommand::BroadcastSharedObjectPackets { session_id, .. }
            | ZoneCommand::SyncGroundDrops { session_id, .. }
            | ZoneCommand::SpawnMonster { session_id, .. }
            | ZoneCommand::PlayerAttackObject { session_id, .. }
            | ZoneCommand::PlayerRangeAttackObject { session_id, .. }
            | ZoneCommand::PlayerCastMagic { session_id, .. }
            | ZoneCommand::ClaimGroundDrop { session_id, .. }
            | ZoneCommand::ClaimNearestGroundDrop { session_id, .. }
            | ZoneCommand::CommitGroundDropClaim { session_id, .. }
            | ZoneCommand::CancelGroundDropClaim { session_id, .. }
            | ZoneCommand::OpenDoor { session_id, .. }
            | ZoneCommand::ConfigureHazards { session_id, .. }
            | ZoneCommand::TickPlayerMovement { session_id, .. } => {
                let Some(key) = self.session_zones.get(session_id).cloned() else {
                    return Vec::new();
                };
                self.handle_for_key(key, command)
            }
            ZoneCommand::Tick { now_ms } => self.tick_all(*now_ms),
        }
    }

    pub fn handle_for_key(&mut self, key: ZoneKey, command: ZoneCommand) -> Vec<ZoneOutbound> {
        match &command {
            ZoneCommand::Join(join) => {
                self.session_zones
                    .insert(join.session_id.clone(), key.clone());
            }
            ZoneCommand::Leave { session_id } => {
                self.session_zones.remove(session_id);
            }
            _ => {}
        }

        let zone = self
            .zones
            .entry(key.clone())
            .or_insert_with(|| ZoneRuntime::new(key));
        zone.handle(command)
    }

    /// Tick every zone. Zones are fully independent (each owns its own ECS
    /// `World` and state), so above a break-even count this ticks them in
    /// parallel on a persistent `ComputeTaskPool` — the Stage-A multi-core win
    /// from docs/L2-ECS-ZONE-DESIGN.md. Chunks are spawned in zone-key order and
    /// the pool returns results in spawn order, so the output is byte-identical
    /// to a sequential tick regardless of which core runs each zone. Below the
    /// threshold (the common case, including today's single "primary" zone) it
    /// ticks inline to avoid task overhead.
    pub fn tick_all(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let zone_count = self.zones.len();
        if zone_count < PARALLEL_TICK_MIN_ZONES {
            return self.tick_all_sequential(now_ms);
        }

        let pool = ComputeTaskPool::get_or_init(TaskPool::default);
        let workers = pool.thread_num().max(1);
        let chunk_size = zone_count.div_ceil(workers).max(1);
        let mut zones: Vec<&mut ZoneRuntime> = self.zones.values_mut().collect();

        // Each task ticks a contiguous chunk of zones serially and returns their
        // concatenated outbounds. Chunks are spawned in order; `scope` preserves
        // spawn order, so flattening yields the same order as a sequential tick.
        let per_chunk: Vec<Vec<ZoneOutbound>> = pool.scope(|scope| {
            for chunk in zones.chunks_mut(chunk_size) {
                scope.spawn(async move {
                    chunk
                        .iter_mut()
                        .flat_map(|zone| zone.tick(now_ms))
                        .collect::<Vec<ZoneOutbound>>()
                });
            }
        });
        per_chunk.into_iter().flatten().collect()
    }

    pub fn zone(&self, key: &ZoneKey) -> Option<&ZoneRuntime> {
        self.zones.get(key)
    }

    /// Tick all zones on the current thread. Identical output to `tick_all`;
    /// used to force single-threaded execution (constrained environments) and as
    /// the benchmark baseline that proves the parallel path's speedup.
    pub fn tick_all_sequential(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        self.zones
            .values_mut()
            .flat_map(|zone| zone.tick(now_ms))
            .collect()
    }

    pub fn player_transform(&self, session_id: &SessionId) -> Option<(Point, MirDirection)> {
        let key = self.session_zones.get(session_id)?;
        let zone = self.zones.get(key)?;
        Some((
            zone.player_position(session_id)?,
            zone.player_direction(session_id)?,
        ))
    }

    pub fn can_player_cast_magic(
        &self,
        session_id: &SessionId,
        object_id: u32,
        spell: Spell,
        direction: MirDirection,
        target: &Point,
        cast: bool,
        damage: i32,
        mp_cost: i32,
        cooldown_ms: u64,
        now_ms: u64,
    ) -> bool {
        let Some(key) = self.session_zones.get(session_id) else {
            return false;
        };
        let Some(zone) = self.zones.get(key) else {
            return false;
        };
        zone.can_player_cast_magic(
            session_id,
            object_id,
            spell,
            direction,
            target,
            cast,
            damage,
            mp_cost,
            cooldown_ms,
            now_ms,
        )
    }

    pub fn player_cast_magic_requires_item_consumption(
        &self,
        session_id: &SessionId,
        spell: Spell,
    ) -> bool {
        let Some(key) = self.session_zones.get(session_id) else {
            return true;
        };
        let Some(zone) = self.zones.get(key) else {
            return true;
        };
        zone.player_cast_magic_requires_item_consumption(session_id, spell)
    }
}

#[cfg(test)]
mod parallel_tick_tests {
    //! Stage-A multi-core proof: parallel `tick_all` must produce byte-identical
    //! output to a sequential tick. Zones are independent (each owns its World),
    //! so the parallel path differs only in *which core* runs each zone, never in
    //! the result. See docs/L2-ECS-ZONE-DESIGN.md.
    use super::*;
    use crate::runtime::zone::types::{ZoneChatProfile, ZonePlayerCombatStats};
    use mir2_protocol::{MirClass, MirGender};

    fn join_on_map(map: &str, session: &str, object_id: u32, x: i32, y: i32) -> ZoneJoin {
        ZoneJoin {
            session_id: SessionId::new(session),
            account_id: format!("{session}-acct"),
            character_index: object_id as i32,
            object_id,
            name: format!("P{object_id}"),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 7,
            hp: 60,
            max_hp: 60,
            mp: 100,
            map_file_name: map.to_string(),
            position: Point { x, y },
            direction: MirDirection::Down,
            chat_profile: ZoneChatProfile::default(),
            combat_stats: ZonePlayerCombatStats::default(),
        }
    }

    // Populate a manager with `maps` zones, each holding two adjacent players who
    // walk, so every zone has real per-tick visibility/movement work.
    fn populate(maps: usize) -> ZoneManager {
        let mut mgr = ZoneManager::new();
        for m in 0..maps {
            let map = format!("map{m}");
            mgr.join(join_on_map(&map, &format!("{map}-a"), 1, 330, 270));
            mgr.join(join_on_map(&map, &format!("{map}-b"), 2, 332, 270));
            mgr.handle_for_key(
                ZoneKey::for_map(map.clone()),
                ZoneCommand::Walk {
                    session_id: SessionId::new(format!("{map}-a")),
                    direction: MirDirection::Right,
                    seq: 1,
                    now_ms: 0,
                },
            );
        }
        mgr
    }

    #[test]
    fn parallel_tick_all_matches_sequential_across_many_zones() {
        // Two independently-built, identical managers; tick one in parallel and
        // the other sequentially, then compare the full outbound streams.
        let mut parallel = populate(8);
        let mut sequential = populate(8);
        let par_out = parallel.tick_all(10);
        let seq_out = sequential.tick_all_sequential(10);
        assert_eq!(
            par_out, seq_out,
            "parallel tick_all must equal the sequential reference"
        );
        assert!(!par_out.is_empty(), "expected real per-zone outbound work");
    }

    #[test]
    fn single_zone_takes_sequential_path_and_still_works() {
        let mut mgr = populate(1);
        let out = mgr.tick_all(10);
        // One zone: cheap sequential path, still produces the same kind of output.
        assert_eq!(out, populate(1).tick_all_sequential(10));
    }
}
