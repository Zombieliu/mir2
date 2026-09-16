//! Crystal AxeSkeleton.ProcessTarget: a retreat once per five-second attack window.
use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ReactiveAiState {
    pub fear_until_ms: u64,
    #[serde(default, skip_serializing_if = "reactive_zero")]
    pub teleport_ready_at_ms: u64,
    #[serde(default, skip_serializing_if = "reactive_zero")]
    pub rng_state: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_slows: Vec<PendingFoxSlow>,
}

fn reactive_zero(value: &u64) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingFoxSlow {
    target_session_id: SessionId,
    target_object_id: u32,
    due_at_ms: u64,
}

pub(super) fn initialize_holy_deva(m: &mut ZoneNativeMonster, now_ms: u64) {
    if m.ai == 38 {
        m.direction = MirDirection::DownLeft;
        m.next_ai_ready_at_ms = m.next_ai_ready_at_ms.max(now_ms.saturating_add(2000));
    }
}

pub(super) fn sync_holy_deva_packet(m: &ZoneNativeMonster, packet: &mut ServerPacket) {
    if m.ai == 38 {
        if let ServerPacket::ObjectMonster { info } = packet {
            info.image = 117;
            info.extra = true;
            info.direction = m.direction;
        }
    }
}

impl ZoneRuntime {
    /// Called when an object ID ceases to identify its current player life.
    pub(super) fn clear_reactive_targets(&mut self, object_id: u32) {
        for monster in self.native_monsters.values_mut() {
            if let Some(state) = monster
                .special_ai
                .as_mut()
                .and_then(|s| s.reactive.as_mut())
            {
                state
                    .pending_slows
                    .retain(|hit| hit.target_object_id != object_id);
            }
        }
        self.pending_native_player_hits.retain(|hit| {
            hit.target_object_id != object_id || !matches!(hit.attacker_ai, 8 | 38 | 45 | 46)
        });
    }

    fn fox_roll(&mut self, id: u32, now: u64, upper: u64) -> u64 {
        let state = self
            .native_monsters
            .get_mut(&id)
            .expect("fox exists")
            .special_ai
            .get_or_insert_with(Default::default)
            .reactive
            .get_or_insert_with(Default::default);
        if state.rng_state == 0 {
            state.rng_state = (u64::from(id) << 32) ^ now ^ 0xF04C0DE;
        }
        loop {
            state.rng_state = state.rng_state.wrapping_add(0x9E3779B97F4A7C15);
            let mut value = state.rng_state;
            value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
            value ^= value >> 31;
            if value >= upper.wrapping_neg() % upper {
                return value % upper;
            }
        }
    }

    /// Resolve the WhiteFox Type1 spell independently of HP damage. Kept in
    /// the caster's checkpointed state so restoration never re-rolls a cast.
    pub(super) fn tick_reactive_effects(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                m.special_ai
                    .as_ref()
                    .and_then(|s| s.reactive.as_ref())
                    .is_some_and(|s| !s.pending_slows.is_empty())
                    .then_some(id)
            })
            .collect();
        let mut out = Vec::new();
        for id in ids {
            let pending = std::mem::take(
                &mut self
                    .native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .reactive
                    .as_mut()
                    .unwrap()
                    .pending_slows,
            );
            let mut future = Vec::new();
            for effect in pending {
                if now_ms < effect.due_at_ms {
                    future.push(effect);
                    continue;
                }
                let Some(target) = self.players.get(&effect.target_session_id) else {
                    continue;
                };
                if target.object_id != effect.target_object_id
                    || target.dead
                    || target.chat_profile.in_safe_zone
                {
                    continue;
                }
                let level = i32::from(target.level);
                let resist = target.combat_stats.poison_resist.clamp(0, 10) as u64;
                if self.fox_roll(id, now_ms, 20) as i32 >= 54 - level
                    || self.fox_roll(id, now_ms, 10) < resist
                {
                    continue;
                }
                let target = self.players.get_mut(&effect.target_session_id).unwrap();
                zone_add_player_status_poison(target, CRYSTAL_POISON_SLOW, 5_000, now_ms);
                let position = target.position.clone();
                let packet = ServerPacket::ObjectPoisoned {
                    object_id: target.object_id,
                    poison: target.poison,
                };
                let recipients =
                    self.native_monster_combat_recipients(id, effect.target_object_id, &position);
                if !recipients.is_empty() {
                    out.push(ZoneOutbound::ToMany {
                        session_ids: recipients,
                        packets: vec![packet],
                    });
                }
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .reactive
                .as_mut()
                .unwrap()
                .pending_slows = future;
        }
        out
    }

    fn reactive_fox_teleport(
        &mut self,
        id: u32,
        destination: Point,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        self.teleport_special_monster(id, destination, now_ms, 2)
    }

    /// Called after control/visibility checks and selecting a valid player target.
    /// None preserves the ordinary path for every other AI family.
    pub(super) fn try_tick_reactive_monster(
        &mut self,
        id: u32,
        monster: &ZoneNativeMonster,
        target: &NativeMonsterTarget,
        now_ms: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        if !matches!(monster.ai, 8 | 38 | 45 | 46) {
            return None;
        }
        // Original ProcessTarget begins with !CanAttack; even retreat waits
        // for attack readiness, with a strict > boundary.
        if now_ms <= monster.next_attack_ready_at_ms {
            return Some(Vec::new());
        }
        let fear = monster
            .special_ai
            .as_ref()
            .and_then(|s| s.reactive.as_ref())
            .map_or(0, |s| s.fear_until_ms);
        let distance = zone_tile_distance(&monster.position, &target.position);
        if distance <= 6 && now_ms < fear {
            if monster.ai == 45
                && distance <= 1
                && now_ms
                    > monster
                        .special_ai
                        .as_ref()
                        .and_then(|s| s.reactive.as_ref())
                        .map_or(0, |s| s.teleport_ready_at_ms)
            {
                self.native_monsters
                    .get_mut(&id)?
                    .special_ai
                    .get_or_insert_with(Default::default)
                    .reactive
                    .get_or_insert_with(Default::default)
                    .teleport_ready_at_ms = now_ms.saturating_add(10_000);
                for _ in 0..40 {
                    let dx = self.fox_roll(id, now_ms, 29) as i32 - 14;
                    let dy = self.fox_roll(id, now_ms, 29) as i32 - 14;
                    let Some(x) = monster.position.x.checked_add(dx) else {
                        continue;
                    };
                    let Some(y) = monster.position.y.checked_add(dy) else {
                        continue;
                    };
                    let destination = Point { x, y };
                    if self.can_native_monster_occupy(id, &destination) {
                        return Some(self.reactive_fox_teleport(id, destination, now_ms));
                    }
                }
                return Some(Vec::new());
            }
            let direction = zone_direction_toward(&monster.position, &target.position)
                .unwrap_or(monster.direction);
            if monster.ai == 46 && self.fox_roll(id, now_ms, 8) == 0 {
                let m = self.native_monsters.get_mut(&id)?;
                m.direction = direction;
                m.next_ai_ready_at_ms = now_ms.saturating_add(300);
                m.next_attack_ready_at_ms = now_ms.saturating_add(m.attack_speed_ms);
                m.special_ai
                    .get_or_insert_with(Default::default)
                    .reactive
                    .get_or_insert_with(Default::default)
                    .pending_slows
                    .push(PendingFoxSlow {
                        target_session_id: target.session_id.clone(),
                        target_object_id: target.object_id,
                        due_at_ms: now_ms.saturating_add(300),
                    });
                let packet = ServerPacket::ObjectRangeAttack {
                    info: ObjectRangeAttackInfo {
                        object_id: id,
                        location: monster.position.clone(),
                        direction,
                        target_id: target.object_id,
                        target: target.position.clone(),
                        attack_type: 1,
                        spell: 0,
                        level: 0,
                    },
                };
                self.apply_zone_object_packets(&[packet.clone()], now_ms);
                let recipients =
                    self.native_monster_combat_recipients(id, target.object_id, &target.position);
                return Some(if recipients.is_empty() {
                    Vec::new()
                } else {
                    vec![ZoneOutbound::ToMany {
                        session_ids: recipients,
                        packets: vec![packet],
                    }]
                });
            }
            let hits_before = self.pending_native_player_hits.len();
            let mut out = self.launch_native_monster_player_ranged_attack(
                id,
                target.clone(),
                direction,
                now_ms,
            );
            // Reuse authoritative damage/defence exactly once. The ordinary
            // 600ms hit delay is replaced by Crystal ProjectileAttack's travel.
            for hit in &mut self.pending_native_player_hits[hits_before..] {
                hit.ready_at_ms = now_ms.saturating_add(if matches!(monster.ai, 38 | 45) {
                    500
                } else {
                    distance as u64 * 50 + 500
                });
                if matches!(monster.ai, 38 | 45 | 46) {
                    hit.magic = true;
                }
            }
            if monster.ai == 45 {
                let attack_type = self.fox_roll(id, now_ms, 2) as u8;
                for outbound in &mut out {
                    if let ZoneOutbound::ToMany { packets, .. } = outbound {
                        for packet in packets {
                            if let ServerPacket::ObjectRangeAttack { info } = packet {
                                info.attack_type = attack_type;
                            }
                        }
                    }
                }
            }
            if let Some(m) = self.native_monsters.get_mut(&id) {
                m.next_ai_ready_at_ms = now_ms.saturating_add(300);
            }
            return Some(out);
        }
        self.native_monsters
            .get_mut(&id)?
            .special_ai
            .get_or_insert_with(Default::default)
            .reactive
            .get_or_insert_with(Default::default)
            .fear_until_ms = now_ms.saturating_add(5_000);
        // Wild HolyDeva never approaches an out-of-range target here. Unlike
        // AxeSkeleton it only retreats when inside its six-tile attack radius.
        if monster.ai == 38 && distance >= 6 {
            return Some(Vec::new());
        }
        let toward =
            zone_direction_toward(&monster.position, &target.position).unwrap_or(monster.direction);
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
        let first = if distance >= 6 {
            toward
        } else {
            zone_direction_toward(&target.position, &monster.position).unwrap_or(monster.direction)
        };
        let index = directions.iter().position(|d| *d == first).unwrap();
        // Crystal flips a random clockwise/counterclockwise fallback. This
        // deterministic choice preserves replay while trying each direction once.
        let clockwise = (now_ms ^ u64::from(id)) & 1 == 0;
        // Both retreat and MonsterObject.MoveTo rotate through the remaining
        // seven directions when the first tile is blocked.
        for n in 0..8 {
            let idx = if clockwise {
                (index + n) % 8
            } else {
                (index + 8 - n) % 8
            };
            let direction = directions[idx];
            let position = offset_point(&monster.position, direction, 1);
            if !self.can_native_monster_occupy(id, &position) {
                continue;
            }
            let m = self.native_monsters.get_mut(&id).expect("selected monster");
            m.position = position.clone();
            m.direction = direction;
            m.next_ai_ready_at_ms = now_ms.saturating_add(m.move_speed_ms.max(300));
            m.next_attack_ready_at_ms = m
                .next_attack_ready_at_ms
                .max(now_ms.saturating_add(m.move_speed_ms));
            let packet = ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: id,
                    position: position.clone(),
                    direction,
                },
            };
            self.apply_zone_object_packets(&[packet.clone()], now_ms);
            let mut out = self.diff_all_zone_object_visibility();
            let recipients = self.native_monster_visible_recipients(id, &position);
            if !recipients.is_empty() {
                out.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets: vec![packet],
                });
            }
            return Some(out);
        }
        Some(Vec::new())
    }
}
