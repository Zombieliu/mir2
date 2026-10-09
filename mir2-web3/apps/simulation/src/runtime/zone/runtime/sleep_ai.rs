//! Crystal FrostTiger idle sitting and DragonStatue non-death sleep lifecycle.
//! Their unrelated attack variants are deliberately left to combat dispatch.
use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SleepAiState {
    #[serde(default)]
    pub(crate) stationary: Option<super::statue_centipede_ai::StatueCentipedeState>,
    pub sitting: bool,
    pub sleeping: bool,
    pub sit_down_at_ms: u64,
    pub wake_at_ms: u64,
    pub target_object_id: Option<u32>,
    pub rng_state: u64,
}

impl SleepAiState {
    fn next_idle_deadline(&mut self, now_ms: u64) -> u64 {
        // Per-instance, saved stream; same range as Random.Next(120000),
        // with rejection sampling instead of modulo-biased range selection.
        let threshold = 120_000u64.wrapping_neg() % 120_000;
        loop {
            self.rng_state = self.rng_state.wrapping_add(0x9E3779B97F4A7C15);
            let mut value = self.rng_state;
            value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
            value ^= value >> 31;
            if value >= threshold {
                return self
                    .sit_down_at_ms
                    .max(now_ms.saturating_add(value % 120_000));
            }
        }
    }
}

pub(in crate::runtime::zone) fn initialize_sleep_monster(
    m: &mut ZoneNativeMonster,
    id: u32,
    now_ms: u64,
) {
    if !matches!(m.ai, 34 | 54) {
        return;
    }
    if m.special_ai
        .as_ref()
        .and_then(|s| s.sleep.as_ref())
        .is_some()
    {
        return;
    }
    let mut state = SleepAiState {
        rng_state: now_ms ^ (u64::from(id) << 32) ^ 0x5A17C0DE,
        ..Default::default()
    };
    if m.ai == 34 {
        state.sit_down_at_ms = state.next_idle_deadline(now_ms);
    }
    if m.ai == 54 && matches!(m.direction, MirDirection::Left | MirDirection::UpLeft) {
        m.direction = MirDirection::DownLeft;
    }
    m.special_ai.get_or_insert_with(Default::default).sleep = Some(state);
}

pub(super) fn sleep_monster_accepts_hp_change(m: &ZoneNativeMonster) -> bool {
    m.ai != 54
        || !m
            .special_ai
            .as_ref()
            .and_then(|s| s.sleep.as_ref())
            .is_some_and(|s| s.sleeping)
}

/// Invoke only after positive applied damage, before the normal death decision.
pub(super) fn sleep_monster_on_damage(
    m: &mut ZoneNativeMonster,
    id: u32,
    attacker_id: Option<u32>,
    now_ms: u64,
) {
    if m.ai != 34 {
        return;
    }
    initialize_sleep_monster(m, id, now_ms);
    if let Some(attacker) = attacker_id {
        m.special_ai
            .as_mut()
            .unwrap()
            .sleep
            .as_mut()
            .unwrap()
            .target_object_id = Some(attacker);
    }
}

/// Returns true when zero HP is a sleep transition, NOT a kill. The caller
/// must leave killed=false and never issue XP, drops or a respawn schedule.
pub(super) fn intercept_sleep_monster_death(m: &mut ZoneNativeMonster, now_ms: u64) -> bool {
    if m.ai != 54 || m.dead || m.hp > 0 {
        return false;
    }
    let state = m
        .special_ai
        .get_or_insert_with(Default::default)
        .sleep
        .get_or_insert_with(Default::default);
    if !state.sleeping {
        state.sleeping = true;
        state.wake_at_ms = now_ms.saturating_add(15 * 60 * 1_000);
    }
    m.hp = 0;
    true
}

pub(super) fn sleep_monster_can_act(m: &ZoneNativeMonster) -> bool {
    m.special_ai
        .as_ref()
        .and_then(|s| s.sleep.as_ref())
        .is_none_or(|s| !(m.ai == 34 && s.sitting || m.ai == 54 && s.sleeping))
}

pub(super) fn sleep_monster_can_move(m: &ZoneNativeMonster) -> bool {
    m.ai != 54 && sleep_monster_can_act(m)
}

pub(super) fn sync_sleep_monster_packet(m: &ZoneNativeMonster, packet: &mut ServerPacket) {
    if let ServerPacket::ObjectMonster { info } = packet {
        if m.ai == 54 {
            info.direction = m.direction;
        }
        if m.ai == 34 {
            let sitting = m
                .special_ai
                .as_ref()
                .and_then(|s| s.sleep.as_ref())
                .is_some_and(|s| s.sitting);
            info.extra = sitting;
            info.hidden = sitting;
        }
    }
}

impl ZoneRuntime {
    pub(super) fn sleep_monster_target(
        &self,
        m: &ZoneNativeMonster,
    ) -> Option<NativeMonsterTarget> {
        let id = m.special_ai.as_ref()?.sleep.as_ref()?.target_object_id?;
        self.players
            .values()
            .find(|p| {
                p.object_id == id
                    && !p.dead
                    && !p.hidden
                    && !p.chat_profile.in_safe_zone
                    && points_visible(&m.position, &p.position)
                    && !m
                        .friendly_guild
                        .as_deref()
                        .zip(p.chat_profile.guild_name.as_deref())
                        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            })
            .map(|p| NativeMonsterTarget {
                session_id: p.session_id.clone(),
                object_id: p.object_id,
                position: p.position.clone(),
            })
    }

    pub(super) fn tick_sleep_monster_states(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (!m.dead && matches!(m.ai, 34 | 54)).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            initialize_sleep_monster(self.native_monsters.get_mut(&id).unwrap(), id, now_ms);
            let before = self.native_monsters[&id].clone();
            if before.ai == 54 {
                let state = before.special_ai.as_ref().unwrap().sleep.as_ref().unwrap();
                if !state.sleeping || now_ms <= state.wake_at_ms {
                    continue;
                }
                let m = self.native_monsters.get_mut(&id).unwrap();
                m.special_ai
                    .as_mut()
                    .unwrap()
                    .sleep
                    .as_mut()
                    .unwrap()
                    .sleeping = false;
                m.hp = m.max_hp;
                // Original wake branch returns before base.ProcessAI.
                m.next_ai_ready_at_ms = m.next_ai_ready_at_ms.max(now_ms.saturating_add(1));
                let packet = ServerPacket::ObjectHealth {
                    info: ObjectHealthInfo {
                        object_id: id,
                        percent: 100,
                        expire: 0,
                    },
                };
                // Repair old retained zero-health death inference without a
                // fake ObjectRevived lifecycle event.
                self.dead_object_ids.remove(&id);
                self.harvested_object_ids.remove(&id);
                if let Some(object) = self.objects.get_mut(&id) {
                    object.health = None;
                    if let ServerPacket::ObjectMonster { info } = &mut object.packet {
                        info.dead = false;
                    }
                }
                self.apply_zone_object_packets(&[packet.clone()], now_ms);
                let recipients = self.native_monster_visible_recipients(id, &before.position);
                if !recipients.is_empty() {
                    out.push(ZoneOutbound::ToMany {
                        session_ids: recipients,
                        packets: vec![packet],
                    });
                }
                continue;
            }
            let has_target = self.sleep_monster_target(&before).is_some();
            let state = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .sleep
                .as_mut()
                .unwrap();
            let was_sitting = state.sitting;
            if !state.sitting && now_ms > state.sit_down_at_ms {
                state.sitting = true;
            }
            if has_target {
                state.sitting = false;
                state.sit_down_at_ms = state.next_idle_deadline(now_ms);
            } else {
                state.target_object_id = None;
            }
            if state.sitting == was_sitting {
                continue;
            }
            let sitting = state.sitting;
            let m = &self.native_monsters[&id];
            if let Some(object) = self.objects.get_mut(&id) {
                sync_sleep_monster_packet(m, &mut object.packet);
            }
            let packet = ServerPacket::ObjectSitDown {
                movement: ObjectMovement {
                    object_id: id,
                    position: before.position.clone(),
                    direction: before.direction,
                },
                sitting,
            };
            self.apply_zone_object_packets(&[packet.clone()], now_ms);
            let recipients = self.native_monster_visible_recipients(id, &before.position);
            if !recipients.is_empty() {
                out.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets: vec![packet],
                });
            }
        }
        out
    }
}
