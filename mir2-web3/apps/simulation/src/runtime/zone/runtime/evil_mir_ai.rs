//! Crystal EvilMir AI52, unlinked encounter. DragonSystem integration is separate.
use super::entity_combat::{
    EntityDefence, EntityTargetPurpose, NativeEntityTarget, ZoneCombatEntityRef,
};
use super::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct EvilMirState {
    pub rng: u64,
    pub target: Option<(SessionId, u32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<ZoneCombatEntityRef>,
    pub mass: bool,
    pub pending: Vec<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_sources: Vec<EvilMirPendingSource>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvilMirPendingSource {
    pub due: u64,
    pub source: ZoneCombatEntityRef,
}
pub(in crate::runtime::zone) fn initialize_evil_mir(m: &mut ZoneNativeMonster) {
    if m.ai == 52 {
        m.special_ai
            .get_or_insert_with(Default::default)
            .evil_mir
            .get_or_insert_with(Default::default);
        m.direction = MirDirection::Up;
    }
}
impl ZoneRuntime {
    pub(super) fn clear_evil_mir_targets(&mut self, id: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.evil_mir.as_mut()) {
                if s.target.as_ref().is_some_and(|(_, t)| *t == id)
                    || s.target_ref.as_ref().is_some_and(|r| r.object_id() == id)
                {
                    s.target = None;
                    s.target_ref = None;
                    s.pending.clear();
                    s.pending_sources.clear();
                }
            }
        }
    }
    fn evil_mir_roll(&mut self, id: u32, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let s = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .evil_mir
            .as_mut()
            .unwrap();
        if s.rng == 0 {
            s.rng = (u64::from(id) << 32) ^ 52;
        }
        s.rng = s.rng.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = s.rng;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        (x ^ (x >> 31)) % n
    }
    fn evil_mir_power(&mut self, id: u32, sc: bool) -> i32 {
        let Some(t) = crystal_monster_by_name(&self.native_monsters[&id].name) else {
            return 0;
        };
        let (a, b) = if sc {
            (t.min_sc, t.max_sc)
        } else {
            (t.min_dc, t.max_dc)
        };
        let a = i32::from(a);
        let b = i32::from(b).max(a);
        a + self.evil_mir_roll(id, (b - a + 1) as u64) as i32
    }
    fn evil_mir_targets(
        &self,
        id: u32,
        center: &Point,
        radius: i32,
        search: bool,
        now: u64,
    ) -> Vec<NativeEntityTarget> {
        self.native_entity_targets(
            id,
            center,
            radius,
            if search {
                EntityTargetPurpose::Search
            } else {
                EntityTargetPurpose::Impact
            },
            now,
        )
    }
    fn set_evil_mir_target(&mut self, id: u32, target: Option<&NativeEntityTarget>, now: u64) {
        let s = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .evil_mir
            .as_mut()
            .unwrap();
        s.target = target.and_then(|t| match &t.reference {
            ZoneCombatEntityRef::Player {
                session_id,
                object_id,
                ..
            } => Some((session_id.clone(), *object_id)),
            _ => None,
        });
        s.target_ref = target.map(|t| t.reference.clone());
        if let Some(target) = target {
            self.set_native_entity_target(id, &target.reference, now);
        } else {
            self.clear_native_entity_target(id);
        }
    }
    pub(super) fn tick_evil_mir_ai(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (m.ai == 52).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            // Initialization must happen at spawn: preserve directional animation state here.
            if self.native_monsters[&id]
                .special_ai
                .as_ref()
                .and_then(|s| s.evil_mir.as_ref())
                .is_none()
            {
                initialize_evil_mir(self.native_monsters.get_mut(&id).unwrap());
            }
            let pending = std::mem::take(
                &mut self
                    .native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .evil_mir
                    .as_mut()
                    .unwrap()
                    .pending,
            );
            let pending: Vec<_> = pending
                .into_iter()
                .map(|due| {
                    let state = self
                        .native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .evil_mir
                        .as_mut()
                        .unwrap();
                    let source = state
                        .pending_sources
                        .iter()
                        .position(|p| p.due == due)
                        .map(|index| state.pending_sources.remove(index).source);
                    (due, source)
                })
                .collect();
            for (due, pending_source) in pending {
                if now < due {
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .evil_mir
                        .as_mut()
                        .unwrap()
                        .pending
                        .push(due);
                    if let Some(source) = pending_source {
                        self.native_monsters
                            .get_mut(&id)
                            .unwrap()
                            .special_ai
                            .as_mut()
                            .unwrap()
                            .evil_mir
                            .as_mut()
                            .unwrap()
                            .pending_sources
                            .push(EvilMirPendingSource { due, source });
                    }
                    continue;
                }
                // Legacy due-only events retain their old format; newly queued events bind caster life.
                let Some(source) = pending_source.or_else(|| self.native_entity_monster_ref(id))
                else {
                    continue;
                };
                let m = self.native_monsters[&id].clone();
                let s = m.special_ai.as_ref().unwrap().evil_mir.as_ref().unwrap();
                // CompleteAttack deliberately reads the global Target and MassAttack now,
                // not the target/mode that launched this particular delayed callback.
                let current = self
                    .selected_native_entity_target(id, now)
                    .map(|t| t.reference)
                    .or_else(|| s.target_ref.clone())
                    .or_else(|| {
                        s.target.as_ref().and_then(|(sid, tid)| {
                            self.native_entity_player_ref(sid)
                                .filter(|r| r.object_id() == *tid)
                        })
                    });
                let Some(current) = current else {
                    continue;
                };
                if !self.native_entity_can_attack(
                    &source,
                    &current,
                    EntityTargetPurpose::Impact,
                    now,
                ) {
                    continue;
                }
                let target_position = match &current {
                    ZoneCombatEntityRef::Player { session_id, .. } => {
                        self.players[session_id].position.clone()
                    }
                    ZoneCombatEntityRef::Monster { object_id, .. } => {
                        self.native_monsters[object_id].position.clone()
                    }
                };
                let center = if s.mass {
                    m.position.clone()
                } else {
                    target_position
                };
                let targets =
                    self.evil_mir_targets(id, &center, if s.mass { 17 } else { 2 }, false, now);
                for target in targets {
                    // Source assigns Target on every iteration, even if the hit is absorbed.
                    self.set_evil_mir_target(id, Some(&target), now);
                    let mut damage = self.evil_mir_power(id, false);
                    if !s.mass {
                        damage = damage * 3 / 4;
                    }
                    if damage == 0 {
                        continue;
                    }
                    // One MAC resistance/armour gate in the entity carrier, never a fake player.
                    let (applied, events) = self.resolve_native_entity_hit(
                        &source,
                        &target.reference,
                        damage,
                        EntityDefence::MAC,
                        false,
                        now,
                    );
                    out.extend(events);
                    if applied > 0 {
                        for green in [true, false] {
                            let value = self.evil_mir_power(id, true);
                            let resist = self.native_entity_poison_resist(&target.reference) as u64;
                            let human =
                                matches!(&target.reference, ZoneCombatEntityRef::Player { .. });
                            // Green's carrier is fully admitted. The control-poison
                            // carrier owns HumanObject's second resistance roll;
                            // neither carrier adds that roll for monster victims.
                            if self.evil_mir_roll(id, 10) >= resist
                                && self.evil_mir_roll(id, 5) == 0
                                && (!green || !human || self.evil_mir_roll(id, 10) >= resist)
                            {
                                if green {
                                    out.extend(self.apply_native_entity_green_poison(
                                        &source,
                                        &target.reference,
                                        value,
                                        15,
                                        2000,
                                        now,
                                    ));
                                } else {
                                    out.extend(self.apply_native_entity_paralysis(
                                        &source,
                                        &target.reference,
                                        5,
                                        1000,
                                        now,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            let m = self.native_monsters[&id].clone();
            if m.dead
                || !m.hostile_to_player
                || native_monster_control_active(&m, now)
                || !monster_visibility_can_act(&m, now)
            {
                continue;
            }
            let view = crystal_monster_by_name(&m.name).map_or(8, |t| i32::from(t.view_range));
            let targets = self.evil_mir_targets(id, &m.position, view, true, now);
            let s = m.special_ai.as_ref().unwrap().evil_mir.as_ref().unwrap();
            let target = self.selected_native_entity_target(id, now).or_else(|| {
                targets
                    .iter()
                    .find(|t| {
                        s.target_ref.as_ref().map_or_else(
                            || s.target.as_ref().is_some_and(|(_, id)| *id == t.object_id),
                            |r| r == &t.reference,
                        )
                    })
                    .or(targets.first())
                    .cloned()
            });
            self.set_evil_mir_target(id, target.as_ref(), now);
            let Some(target) = target else { continue };
            if now <= m.next_attack_ready_at_ms || now < m.next_ai_ready_at_ms {
                continue;
            }
            let mass = self.evil_mir_roll(id, 8) == 0;
            let direction = if mass {
                m.direction
            } else {
                match zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction) {
                    MirDirection::DownRight | MirDirection::Right => MirDirection::Up,
                    MirDirection::Left | MirDirection::UpLeft => MirDirection::Right,
                    _ => MirDirection::UpRight,
                }
            };
            let due = now
                + if mass {
                    500
                } else {
                    zone_tile_distance(&m.position, &target.position) as u64 * 50 + 620
                };
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.direction = direction;
            live.next_ai_ready_at_ms = now + 300;
            live.next_attack_ready_at_ms = now + live.attack_speed_ms;
            let s = live.special_ai.as_mut().unwrap().evil_mir.as_mut().unwrap();
            s.mass = mass;
            s.pending.push(due);
            s.pending_sources.push(EvilMirPendingSource {
                due,
                source: ZoneCombatEntityRef::Monster {
                    object_id: id,
                    incarnation: m.incarnation,
                },
            });
            let packet = if mass {
                ServerPacket::ObjectAttack {
                    info: ObjectAttackInfo {
                        object_id: id,
                        location: m.position.clone(),
                        direction,
                        attack_type: 0,
                        spell: 0,
                        level: 0,
                    },
                }
            } else {
                ServerPacket::ObjectRangeAttack {
                    info: ObjectRangeAttackInfo {
                        object_id: id,
                        location: m.position.clone(),
                        direction,
                        target_id: target.object_id,
                        target: target.position,
                        attack_type: 0,
                        spell: 0,
                        level: 0,
                    },
                }
            };
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_visible_recipients(id, &m.position),
                packets: vec![packet],
            });
        }
        out
    }
}
