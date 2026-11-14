//! Crystal Deer AI2 timid subtype. Other neutral deer never acquire an attack
//! target here. Normal deer retaliate only against a recorded direct attacker.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PassiveAiState {
    pub timid: bool,
    pub target_object_id: Option<u32>,
    pub rng_state: u64,
}

impl PassiveAiState {
    fn roll(&mut self, upper: u64) -> u64 {
        loop {
            self.rng_state = self.rng_state.wrapping_add(0x9E3779B97F4A7C15);
            let mut value = self.rng_state;
            value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
            value ^= value >> 31;
            if value >= upper.wrapping_neg() % upper {
                return value % upper;
            }
        }
    }
}

pub(in crate::runtime::zone) fn initialize_passive_monster(
    m: &mut ZoneNativeMonster,
    id: u32,
    now_ms: u64,
) {
    if m.ai != 2
        || m.special_ai
            .as_ref()
            .and_then(|s| s.passive.as_ref())
            .is_some()
    {
        return;
    }
    let mut state = PassiveAiState {
        timid: false,
        target_object_id: None,
        rng_state: (u64::from(id) << 32) ^ now_ms ^ 0xDEE2,
    };
    state.timid = state.roll(7) == 0;
    if state.timid && m.move_speed_ms >= 600 {
        m.move_speed_ms -= 300;
    }
    m.special_ai.get_or_insert_with(Default::default).passive = Some(state);
}

/// Only call for a real positive, direct hit; periodic poison is not a new attacker.
pub(super) fn passive_monster_on_damage(
    m: &mut ZoneNativeMonster,
    id: u32,
    attacker_id: Option<u32>,
    now_ms: u64,
) {
    if m.ai != 2 {
        return;
    }
    initialize_passive_monster(m, id, now_ms);
    let state = m.special_ai.as_mut().unwrap().passive.as_mut().unwrap();
    if attacker_id.is_some() {
        state.target_object_id = attacker_id;
    }
}

impl ZoneRuntime {
    /// Call after the ordinary move/control readiness gates but BEFORE the
    /// hostile_to_player early return, since neutral timid deer must move too.
    pub(super) fn try_tick_passive_monster(
        &mut self,
        id: u32,
        now_ms: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get_mut(&id)?;
        if m.ai != 2 {
            return None;
        }
        initialize_passive_monster(m, id, now_ms);
        if m.dead || m.hp <= 0 {
            return Some(Vec::new());
        }
        let position = m.position.clone();
        let friendly = m.friendly_guild.clone();
        let view = crystal_monster_by_name(&m.name).map_or(14, |t| i32::from(t.view_range));
        let mut state = m
            .special_ai
            .as_ref()
            .unwrap()
            .passive
            .as_ref()
            .unwrap()
            .clone();
        let valid = |p: &&ZonePlayer| {
            !p.dead
                && !p.hidden
                && !p.chat_profile.in_safe_zone
                && !friendly
                    .as_deref()
                    .zip(p.chat_profile.guild_name.as_deref())
                    .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
        };
        let target = self
            .players
            .values()
            .filter(valid)
            .find(|p| {
                Some(p.object_id) == state.target_object_id
                    && points_visible(&position, &p.position)
            })
            .or_else(|| {
                if !state.timid {
                    return None;
                }
                self.players
                    .values()
                    .filter(valid)
                    .filter(|p| zone_tile_distance(&position, &p.position) <= view)
                    .min_by_key(|p| (zone_tile_distance(&position, &p.position), p.object_id))
            })
            .map(|p| (p.object_id, p.position.clone()));
        state.target_object_id = target.as_ref().map(|(id, _)| *id);
        let mut out = Vec::new();
        if let Some((target_id, target)) = target {
            if !state.timid && native_monster_adjacent_to(&position, &target) {
                let player = self
                    .players
                    .values()
                    .find(|p| p.object_id == target_id)
                    .unwrap();
                let native = NativeMonsterTarget {
                    session_id: player.session_id.clone(),
                    object_id: target_id,
                    position: target.clone(),
                };
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .passive = Some(state);
                return Some(self.launch_native_monster_player_attack(
                    id,
                    native,
                    zone_direction_toward(&position, &target).unwrap_or(MirDirection::Down),
                    now_ms,
                ));
            }
            let direction = if state.timid {
                zone_direction_toward(&target, &position)
            } else {
                zone_direction_toward(&position, &target)
            };
            if let Some(away) = direction {
                let directions = [
                    MirDirection::Up,
                    MirDirection::UpRight,
                    MirDirection::Right,
                    MirDirection::DownRight,
                    MirDirection::Down,
                    MirDirection::DownLeft,
                    MirDirection::Left,
                    MirDirection::UpLeft,
                ];
                let index = directions.iter().position(|d| *d == away).unwrap();
                let clockwise = state.roll(2) == 0;
                for n in 0..8 {
                    let direction = directions[if clockwise {
                        (index + n) % 8
                    } else {
                        (index + 8 - n) % 8
                    }];
                    let dest = offset_point(&position, direction, 1);
                    if !self.can_native_monster_occupy(id, &dest) {
                        continue;
                    }
                    let m = self.native_monsters.get_mut(&id).unwrap();
                    m.position = dest.clone();
                    m.direction = direction;
                    m.next_ai_ready_at_ms = now_ms.saturating_add(m.move_speed_ms.max(300));
                    m.next_attack_ready_at_ms = m
                        .next_attack_ready_at_ms
                        .max(now_ms.saturating_add(m.move_speed_ms));
                    let packet = ServerPacket::ObjectWalk {
                        movement: ObjectMovement {
                            object_id: id,
                            position: dest.clone(),
                            direction,
                        },
                    };
                    self.apply_zone_object_packets(&[packet.clone()], now_ms);
                    out.extend(self.diff_all_zone_object_visibility());
                    let recipients = self.native_monster_visible_recipients(id, &dest);
                    if !recipients.is_empty() {
                        out.push(ZoneOutbound::ToMany {
                            session_ids: recipients,
                            packets: vec![packet],
                        });
                    }
                    break;
                }
            }
        }
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .passive = Some(state);
        Some(out)
    }
}
