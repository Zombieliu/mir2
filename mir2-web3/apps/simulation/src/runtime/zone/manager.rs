use std::collections::BTreeMap;

use mir2_protocol::{MirDirection, Point, Spell};

use super::runtime::ZoneRuntime;
use super::types::{SessionId, ZoneCommand, ZoneJoin, ZoneKey, ZoneOutbound};

#[derive(Debug, Clone, Default)]
pub struct ZoneManager {
    zones: BTreeMap<ZoneKey, ZoneRuntime>,
    session_zones: BTreeMap<SessionId, ZoneKey>,
}

impl ZoneManager {
    pub fn new() -> Self {
        Self::default()
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

    pub fn tick_all(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        self.zones
            .values_mut()
            .flat_map(|zone| zone.tick(now_ms))
            .collect()
    }

    pub fn zone(&self, key: &ZoneKey) -> Option<&ZoneRuntime> {
        self.zones.get(key)
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
