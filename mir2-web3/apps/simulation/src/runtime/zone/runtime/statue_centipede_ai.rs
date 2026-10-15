//! Crystal DragonStatue54 and EvilCentipede14 stationary area completions.
use super::entity_combat::{EntityDefence, EntityTargetPurpose, ZoneCombatEntityRef};
use super::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct StatueCentipedeState {
    target: Option<ZoneCombatEntityRef>,
    pending: Vec<StationaryCompletion>,
    sequence: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StationaryCompletion {
    source: ZoneCombatEntityRef,
    due: u64,
}
fn state(m: &ZoneNativeMonster) -> Option<&StatueCentipedeState> {
    let special = m.special_ai.as_ref()?;
    match m.ai {
        14 => special.visibility.as_ref()?.stationary.as_ref(),
        54 => special.sleep.as_ref()?.stationary.as_ref(),
        _ => None,
    }
}
fn store(m: &mut ZoneNativeMonster, s: StatueCentipedeState) {
    let special = m.special_ai.get_or_insert_with(Default::default);
    match m.ai {
        14 => {
            special
                .visibility
                .get_or_insert_with(Default::default)
                .stationary = Some(s)
        }
        54 => {
            special
                .sleep
                .get_or_insert_with(Default::default)
                .stationary = Some(s)
        }
        _ => {}
    }
}
fn roll(s: &mut StatueCentipedeState, id: u32, now: u64, n: u64) -> u64 {
    s.sequence = s.sequence.wrapping_add(1);
    let mut x = now ^ u64::from(id) ^ s.sequence.wrapping_mul(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    (x ^ (x >> 31)) % n.max(1)
}
impl ZoneRuntime {
    pub(super) fn clear_statue_centipede_target_life(&mut self, id: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(mut s) = state(m).cloned() {
                if s.target.as_ref().is_some_and(|t| t.object_id() == id) {
                    s.target = None;
                }
                store(m, s);
            }
        }
    }
    /// Before generic decoy/player attack dispatch, after normal control gates.
    pub(super) fn try_tick_statue_centipede(
        &mut self,
        id: u32,
        now: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if !matches!(m.ai, 14 | 54) {
            return None;
        }
        if m.dead
            || !m.hostile_to_player
            || !monster_visibility_can_act(&m, now)
            || !sleep_monster_can_act(&m)
            || native_monster_control_active(&m, now)
            || now < m.next_ai_ready_at_ms
        {
            return Some(Vec::new());
        }
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return Some(Vec::new());
        };
        let radius = if m.ai == 14 {
            7
        } else {
            i32::from(t.view_range)
        };
        let targets =
            self.native_entity_targets(id, &m.position, radius, EntityTargetPurpose::Search, now);
        let mut s = state(&m).cloned().unwrap_or_default();
        let inherited = self.selected_native_entity_target(id, now);
        let target = inherited.as_ref().or_else(|| {
            targets
                .iter()
                .find(|t| s.target.as_ref() == Some(&t.reference))
                .or_else(|| targets.first())
        });
        s.target = target.map(|t| t.reference.clone());
        if let Some(target) = target {
            self.set_native_entity_target(id, &target.reference, now);
        }
        if target.is_none() || targets.is_empty() || now <= m.next_attack_ready_at_ms {
            store(self.native_monsters.get_mut(&id)?, s);
            return Some(Vec::new());
        }
        let source = self.native_entity_monster_ref(id)?;
        s.pending.push(StationaryCompletion {
            source,
            due: now.saturating_add(500),
        });
        let live = self.native_monsters.get_mut(&id)?;
        live.next_ai_ready_at_ms = live.next_ai_ready_at_ms.max(now.saturating_add(300));
        live.next_attack_ready_at_ms = now.saturating_add(m.attack_speed_ms);
        store(live, s);
        if m.ai == 54 {
            return Some(Vec::new());
        } // Statue emits its animation only on completion.
        let packet = ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: id,
                location: m.position.clone(),
                direction: m.direction,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        Some(vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &m.position),
            packets: vec![packet],
        }])
    }
    pub(super) fn tick_statue_centipede_delays(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids = self.native_monsters.keys().copied().collect::<Vec<_>>();
        let mut out = Vec::new();
        for id in ids {
            let m = self.native_monsters[&id].clone();
            let Some(mut s) = state(&m).cloned() else {
                continue;
            };
            let mut due = Vec::new();
            for h in std::mem::take(&mut s.pending) {
                if now >= h.due {
                    due.push(h);
                } else {
                    s.pending.push(h);
                }
            }
            store(self.native_monsters.get_mut(&id).unwrap(), s.clone());
            for h in due {
                if self.native_entity_monster_ref(id).as_ref() != Some(&h.source)
                    || !self.objects.contains_key(&id)
                {
                    continue;
                }
                let targets = if m.ai == 14 {
                    self.native_entity_targets(id, &m.position, 7, EntityTargetPurpose::Impact, now)
                } else {
                    // Crystal reads its current Target, not data[0] captured at launch.
                    let current = self
                        .selected_native_entity_target(id, now)
                        .map(|t| t.reference)
                        .or_else(|| {
                            state(&self.native_monsters[&id]).and_then(|s| s.target.clone())
                        });
                    let Some(target) = current.filter(|t| {
                        self.native_entity_can_attack(
                            &h.source,
                            t,
                            EntityTargetPurpose::Impact,
                            now,
                        )
                    }) else {
                        continue;
                    };
                    let position = match &target {
                        ZoneCombatEntityRef::Player { session_id, .. } => {
                            self.players[session_id].position.clone()
                        }
                        ZoneCombatEntityRef::Monster { object_id, .. } => {
                            self.native_monsters[object_id].position.clone()
                        }
                    };
                    let packet = ServerPacket::ObjectRangeAttack {
                        info: ObjectRangeAttackInfo {
                            object_id: id,
                            location: m.position.clone(),
                            direction: m.direction,
                            target_id: target.object_id(),
                            target: position.clone(),
                            attack_type: 0,
                            spell: 0,
                            level: 0,
                        },
                    };
                    self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
                    out.push(ZoneOutbound::ToMany {
                        session_ids: self.native_monster_combat_recipients(
                            id,
                            target.object_id(),
                            &position,
                        ),
                        packets: vec![packet],
                    });
                    self.native_entity_targets(
                        id,
                        &position,
                        2,
                        EntityTargetPurpose::VisibleImpact,
                        now,
                    )
                };
                let Some(t) = crystal_monster_by_name(&m.name) else {
                    continue;
                };
                for target in targets {
                    if m.ai == 14 {
                        s.target = Some(target.reference.clone());
                        self.set_native_entity_target(id, &target.reference, now);
                        store(self.native_monsters.get_mut(&id).unwrap(), s.clone());
                    }
                    let damage = zone_roll_stat_range(
                        t.min_dc,
                        t.max_dc.saturating_add(zone_native_monster_buff_stat_total(
                            &m,
                            CRYSTAL_STAT_MAX_DC,
                        )),
                        now,
                        id,
                        roll(&mut s, id, now, u64::MAX),
                    );
                    let (actual, events) = self.resolve_native_entity_hit(
                        &h.source,
                        &target.reference,
                        damage,
                        EntityDefence::MAC,
                        false,
                        now,
                    );
                    out.extend(events);
                    if m.ai == 14 && actual > 0 {
                        for (chance, duration, green) in [(5, 15, true), (15, 5, false)] {
                            let value = zone_roll_stat_range(
                                t.min_sc,
                                t.max_sc.saturating_add(zone_native_monster_buff_stat_total(
                                    &m,
                                    CRYSTAL_STAT_MAX_SC,
                                )),
                                now,
                                id,
                                roll(&mut s, id, now, u64::MAX),
                            );
                            let resist =
                                self.native_entity_poison_resist(&target.reference).max(0) as u64;
                            if roll(&mut s, id, now, 10) < resist
                                || roll(&mut s, id, now, chance) != 0
                            {
                                continue;
                            }
                            if green {
                                if matches!(&target.reference, ZoneCombatEntityRef::Player { .. })
                                    && roll(&mut s, id, now, 10) < resist
                                {
                                    continue;
                                }
                                out.extend(self.apply_native_entity_green_poison(
                                    &h.source,
                                    &target.reference,
                                    value,
                                    duration,
                                    2000,
                                    now,
                                ));
                            } else {
                                out.extend(self.apply_native_entity_paralysis(
                                    &h.source,
                                    &target.reference,
                                    duration,
                                    2000,
                                    now,
                                ));
                            }
                        }
                    }
                    // Central damage may clear this target life. Preserve that
                    // authoritative clear instead of writing the old clone back.
                    let live_target =
                        state(&self.native_monsters[&id]).and_then(|s| s.target.clone());
                    s.target = live_target;
                }
            }
            if let Some(live) = self.native_monsters.get_mut(&id) {
                store(live, s);
            }
        }
        out
    }
}
