//! Single-writer, shared Crystal Map.Mine state. A prepared swing is a trusted
//! server capability, never a client command; personal inventory commits first.
use super::*;
use mir2_game_data::{crystal_mine_set, crystal_mining_item, crystal_mining_map};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneMiningTool {
    pub unique_id: u64,
    pub durability: u16,
    pub accuracy: i32,
    pub strong: i32,
    pub mine_rate_percent: i32,
    pub attack_delay_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneMinedOre {
    pub item_name: String,
    /// Crystal ore quantity/purity, independent of the item's MaxDura.
    pub current_dura: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ZoneMineSpot {
    stones_left: u8,
    regen_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PendingMiningEffect {
    ready_at_ms: u64,
    player_object_id: u32,
    rubble_object_id: u32,
    location: Point,
    direction: MirDirection,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ZoneMiningState {
    // String coordinate keys keep the canonical JSON unambiguous and sparse.
    spots: BTreeMap<String, ZoneMineSpot>,
    sequence: u64,
    effects: Vec<PendingMiningEffect>,
}

#[derive(Debug, Clone)]
pub struct ZoneMiningSwing {
    pub(crate) account_id: String,
    pub(crate) character_index: i32,
    pub(crate) object_id: u32,
    pub(crate) tool: ZoneMiningTool,
    pub(crate) ore: Option<ZoneMinedOre>,
    pub(crate) durability_damage: u16,
    session_id: SessionId,
    position: Point,
    direction: MirDirection,
    wall: Point,
    now_ms: u64,
    sequence: u64,
    before: Option<ZoneMineSpot>,
    after: ZoneMineSpot,
    hit: bool,
}

impl ZoneMiningSwing {
    pub(crate) fn now_ms(&self) -> u64 {
        self.now_ms
    }
    pub(crate) fn player_transform(&self) -> (Point, MirDirection) {
        (self.position.clone(), self.direction)
    }
    pub fn tool_damage(&self) -> u16 {
        self.durability_damage
    }
    pub fn ore(&self) -> Option<&ZoneMinedOre> {
        self.ore.as_ref()
    }
}

fn spot_key(point: &Point) -> String {
    format!("{},{}", point.x, point.y)
}

fn mining_roll(
    now_ms: u64,
    object_id: u32,
    sequence: u64,
    wall: &Point,
    salt: u64,
    modulo: u64,
) -> u64 {
    let seed = now_ms
        ^ sequence.wrapping_mul(0x9e3779b97f4a7c15)
        ^ u64::from(object_id).rotate_left(23)
        ^ (wall.x as u64).wrapping_mul(73856093)
        ^ (wall.y as u64).wrapping_mul(19349663);
    zone_hazard_hash(seed, salt) % modulo.max(1)
}

impl ZoneRuntime {
    /// Normal Attack/None reaches this only after live-target melee selection.
    /// Failed admission changes neither clocks, stone counts nor inventory.
    pub fn prepare_mining_swing(
        &self,
        session_id: &SessionId,
        direction: MirDirection,
        tool: &ZoneMiningTool,
        now_ms: u64,
    ) -> Option<ZoneMiningSwing> {
        let player = self.players.get(session_id)?;
        if self.mining.sequence == u64::MAX
            || tool.unique_id == 0
            || tool.durability == 0
            || !zone_player_harvest_admitted(player, now_ms)
            || now_ms < player.next_attack_ready_at_ms
        {
            return None;
        }
        let wall = offset_point(&player.position, direction, 1);
        // Out-of-bounds space and unavailable collision are not mineable.
        if !self.collision.is_mineable_wall(&wall) {
            return None;
        }
        let map = crystal_mining_map(&self.key.map_file_name)?;
        let set = crystal_mine_set(map.mine_index_at(wall.x, wall.y))?;
        let before = self.mining.spots.get(&spot_key(&wall)).cloned();
        let mut after = before.clone().unwrap_or_default();
        let sequence = self.mining.sequence;
        let roll =
            |salt, modulo| mining_roll(now_ms, player.object_id, sequence, &wall, salt, modulo);
        let mut hit = false;
        let mut durability_damage = 0;
        let mut ore = None;
        if after.stones_left > 0 {
            after.stones_left -= 1;
            hit = (roll(0x10, 100) as i32)
                < i32::from(set.hit_rate).saturating_add(tool.accuracy.saturating_mul(10));
            if hit {
                let damage = 5 + roll(0x30, 15) as i32;
                durability_damage = if tool.strong > 0 {
                    (damage - tool.strong).max(1)
                } else {
                    damage
                } as u16;
                if (roll(0x20, 100) as i32)
                    < i32::from(set.drop_rate).saturating_add(tool.mine_rate_percent)
                {
                    let slot = roll(0x50, u64::from(set.total_slots)) as u16;
                    if let Some(drop) = set
                        .drops
                        .iter()
                        .find(|d| slot >= d.min_slot && slot <= d.max_slot)
                    {
                        if let Some(template) = crystal_mining_item(&drop.item_name) {
                            let dura = if template.item_type
                                == crate::runtime::crystal_compat::CRYSTAL_ITEM_TYPE_ORE
                            {
                                let base = u64::from(drop.min_dura)
                                    + roll(
                                        0x60,
                                        u64::from(drop.max_dura.saturating_sub(drop.min_dura)),
                                    );
                                let bonus = if drop.bonus_chance > 0
                                    && roll(0x70, 100) <= u64::from(drop.bonus_chance)
                                {
                                    roll(0x80, u64::from(drop.max_bonus_dura))
                                } else {
                                    0
                                };
                                (base.saturating_add(bonus) * 1000).min(u64::from(u16::MAX)) as u16
                            } else {
                                template.durability
                            };
                            ore = Some(ZoneMinedOre {
                                item_name: template.name,
                                current_dura: dura,
                            });
                        }
                    }
                }
            }
        } else if now_ms > after.regen_at_ms {
            // The refill swing never also pays ore or damages the tool.
            after.regen_at_ms =
                now_ms.saturating_add(u64::from(set.spot_regen_rate_minutes) * 60_000);
            after.stones_left = roll(0x40, u64::from(set.max_stones)) as u8;
        }
        Some(ZoneMiningSwing {
            account_id: player.account_id.clone(),
            character_index: player.character_index,
            object_id: player.object_id,
            tool: tool.clone(),
            ore,
            durability_damage,
            session_id: session_id.clone(),
            position: player.position.clone(),
            direction,
            wall,
            now_ms,
            sequence,
            before,
            after,
            hit,
        })
    }

    /// Called under the Gateway's existing single-writer mutation gate after
    /// the exact tool/personal award transaction succeeded. Stale tickets fail.
    pub fn commit_mining_swing(&mut self, swing: &ZoneMiningSwing) -> Option<Vec<ZoneOutbound>> {
        let fresh = self.prepare_mining_swing(
            &swing.session_id,
            swing.direction,
            &swing.tool,
            swing.now_ms,
        )?;
        if fresh.account_id != swing.account_id
            || fresh.character_index != swing.character_index
            || fresh.object_id != swing.object_id
            || fresh.position != swing.position
            || fresh.wall != swing.wall
            || fresh.sequence != swing.sequence
            || fresh.before != swing.before
            || fresh.after != swing.after
        {
            return None;
        }
        let next_sequence = self.mining.sequence.checked_add(1)?;
        self.cancel_movement_for_combat(&swing.session_id);
        let player = self.players.get_mut(&swing.session_id)?;
        player.direction = swing.direction;
        player.movement_ready_at_ms = swing.now_ms.saturating_add(550);
        player.next_attack_ready_at_ms = swing
            .now_ms
            .saturating_add(swing.tool.attack_delay_ms.max(550));
        self.mining.sequence = next_sequence;
        self.mining
            .spots
            .insert(spot_key(&swing.wall), swing.after.clone());
        let mut out = self.owner_location_correction(&swing.session_id);
        out.push(ZoneOutbound::ToMany {
            session_ids: self.player_status_recipients(swing.object_id, &swing.position),
            packets: vec![ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: swing.object_id,
                    location: swing.position.clone(),
                    direction: swing.direction,
                    spell: Spell::None as u8,
                    level: 0,
                    attack_type: 0,
                },
            }],
        });
        if swing.hit {
            let rubble = self.objects.iter().find_map(|(&id, o)| match &o.packet {
                ServerPacket::ObjectSpell { info }
                    if info.spell == Spell::Rubble && o.position == swing.position =>
                {
                    Some(id)
                }
                _ => None,
            });
            let id = rubble.unwrap_or_else(|| self.unique_object_id(0));
            if rubble.is_none() {
                let packet = ServerPacket::ObjectSpell {
                    info: ObjectSpellInfo {
                        object_id: id,
                        location: swing.position.clone(),
                        spell: Spell::Rubble,
                        direction: MirDirection::Up,
                        param: false,
                    },
                };
                self.apply_zone_object_packets(&[packet], swing.now_ms);
                out.extend(self.diff_all_zone_object_visibility());
            }
            if let Some(object) = self.objects.get_mut(&id) {
                object.expires_at_ms = Some(swing.now_ms.saturating_add(300_000));
            }
            self.mining.effects.push(PendingMiningEffect {
                ready_at_ms: swing.now_ms.saturating_add(400),
                player_object_id: swing.object_id,
                rubble_object_id: id,
                location: swing.position.clone(),
                direction: swing.direction,
            });
        }
        Some(out)
    }

    pub(super) fn tick_mining_effects(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let mut out = Vec::new();
        let mut pending = Vec::new();
        for effect in std::mem::take(&mut self.mining.effects) {
            if effect.ready_at_ms > now_ms {
                pending.push(effect);
                continue;
            }
            let direction = self
                .players
                .values()
                .find(|p| p.object_id == effect.player_object_id)
                .map_or(effect.direction, |p| p.direction);
            let mut packets = vec![ServerPacket::MapEffect {
                location: effect.location.clone(),
                effect: 12,
                value: direction as u8,
            }];
            if let Some(object) = self.objects.get_mut(&effect.rubble_object_id) {
                if let ServerPacket::ObjectSpell { info } = &mut object.packet {
                    info.direction = MirDirection::try_from((info.direction as u8 + 1).min(6))
                        .unwrap_or(MirDirection::Up);
                    packets.push(object.packet.clone());
                }
            }
            let recipients: Vec<_> = self
                .players
                .iter()
                .filter(|(_, p)| points_visible(&p.position, &effect.location))
                .map(|(id, _)| id.clone())
                .collect();
            if !recipients.is_empty() {
                out.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets,
                });
            }
        }
        self.mining.effects = pending;
        out
    }
}
