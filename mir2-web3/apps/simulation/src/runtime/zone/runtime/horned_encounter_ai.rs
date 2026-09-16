//! HornedSorceror 169 and HornedCommander 171 encounter state and actual map spells.
use super::entity_combat::{
    EntityDefence, EntityTargetPurpose, NativeEntityTarget, ZoneCombatEntityRef,
};
use super::*;
use crate::WorldEntityDisposition;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct HornedEncounterState {
    pub rng: u64,
    pub target: Option<(SessionId, u32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<ZoneCombatEntityRef>,
    pub immune: bool,
    pub stomp_ready: u64,
    pub tornado_ready: u64,
    pub advanced: bool,
    pub called_boulders: bool,
    pub called_spikes: bool,
    pub called_shield: bool,
    pub spike_ready: u64,
    pub shield_until: Option<u64>,
    pub next_spike: u8,
    pub slaves: Vec<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slave_lives: BTreeMap<u32, u64>,
    pub parent: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_incarnation: Option<u64>,
    pub death_handled: bool,
    pub reward_owner: Option<SessionId>,
    pub reward_until: u64,
    pub hits: Vec<HornedEncounterHit>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HornedEncounterHit {
    pub due: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<ZoneCombatEntityRef>,
    pub target: Option<(SessionId, u32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<ZoneCombatEntityRef>,
    pub point: Option<Point>,
    pub radius: u8,
    pub front: u8,
    pub damage: i32,
    pub accuracy: Option<i32>,
    pub release: bool,
    pub teleport: bool,
    pub cancelled: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct HornedEncounterWorld {
    pub spells: Vec<HornedMapSpell>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HornedMapSpell {
    pub object_id: u32,
    pub source: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<ZoneCombatEntityRef>,
    pub source_removed: bool,
    pub spell: u8,
    pub location: Point,
    pub cast_location: Point,
    pub direction: MirDirection,
    pub show: bool,
    pub value: i32,
    pub start: u64,
    pub expires: u64,
    pub next_tick: u64,
    pub tick_speed: u64,
    pub spawned: bool,
}
pub(in crate::runtime::zone) fn initialize_horned_encounter(m: &mut ZoneNativeMonster) {
    if matches!(m.ai, 169 | 171) {
        m.special_ai
            .get_or_insert_with(Default::default)
            .horned_encounter
            .get_or_insert_with(Default::default);
    }
}
pub(super) fn horned_encounter_is_attack_target(m: &ZoneNativeMonster) -> bool {
    !m.special_ai
        .as_ref()
        .and_then(|s| s.horned_encounter.as_ref())
        .is_some_and(|s| s.immune)
}
pub(super) fn horned_encounter_on_damage(
    m: &mut ZoneNativeMonster,
    owner: Option<&SessionId>,
    now: u64,
) {
    let Some(s) = m
        .special_ai
        .as_mut()
        .and_then(|s| s.horned_encounter.as_mut())
    else {
        return;
    };
    if let Some(owner) = owner {
        if s.reward_owner.is_none() || now > s.reward_until {
            s.reward_owner = Some(owner.clone());
        }
        if s.reward_owner.as_ref() == Some(owner) {
            s.reward_until = now + 5000;
        }
    }
}
impl ZoneRuntime {
    pub(super) fn clear_horned_encounter_targets(&mut self, id: u32) {
        let session = self
            .players
            .values()
            .find(|p| p.object_id == id)
            .map(|p| p.session_id.clone());
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m
                .special_ai
                .as_mut()
                .and_then(|s| s.horned_encounter.as_mut())
            {
                if session.is_some() && s.reward_owner == session {
                    s.reward_owner = None;
                }
                if s.target.as_ref().is_some_and(|(_, target)| *target == id)
                    || s.target_ref.as_ref().is_some_and(|r| r.object_id() == id)
                {
                    s.target = None;
                    s.target_ref = None;
                }
                for hit in &mut s.hits {
                    if hit.target.as_ref().is_some_and(|(_, target)| *target == id)
                        || hit.target_ref.as_ref().is_some_and(|r| r.object_id() == id)
                    {
                        hit.cancelled = true;
                        hit.target = None;
                        hit.target_ref = None;
                    }
                }
            }
        }
    }
    pub(super) fn invalidate_horned_encounter_source(&mut self, id: u32) {
        if let Some(w) = self.horned_encounter_world.as_mut() {
            for s in &mut w.spells {
                if s.source == id {
                    s.source_removed = true;
                }
            }
        }
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m
                .special_ai
                .as_mut()
                .and_then(|s| s.horned_encounter.as_mut())
            {
                s.slaves.retain(|child| *child != id);
                s.slave_lives.remove(&id);
                if s.parent == Some(id) {
                    s.parent = None;
                    s.parent_incarnation = None;
                }
            }
        }
    }
    fn encounter_roll(&mut self, id: u32, upper: u64) -> u64 {
        if upper == 0 {
            return 0;
        }
        let s = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .horned_encounter
            .as_mut()
            .unwrap();
        if s.rng == 0 {
            s.rng = (u64::from(id) << 32) ^ 0x169171;
        }
        loop {
            s.rng = s.rng.wrapping_add(0x9E3779B97F4A7C15);
            let mut n = s.rng;
            n = (n ^ (n >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            n = (n ^ (n >> 27)).wrapping_mul(0x94D049BB133111EB);
            n ^= n >> 31;
            if n >= upper.wrapping_neg() % upper {
                return n % upper;
            }
        }
    }
    fn encounter_power(&mut self, id: u32, kind: HornedStat, now: u64) -> i32 {
        let m = &self.native_monsters[&id];
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return 0;
        };
        let (min, max) = match kind {
            HornedStat::Dc => (t.min_dc, t.max_dc),
            HornedStat::Mc => (t.min_mc, t.max_mc),
            HornedStat::Sc => (t.min_sc, t.max_sc),
            _ => return 0,
        };
        let (min, max) = horned_stat_range(m, kind, min, max, now);
        min + self.encounter_roll(id, (max - min + 1).max(1) as u64) as i32
    }
    fn encounter_targets(
        &self,
        id: u32,
        center: &Point,
        radius: i32,
        sight: bool,
        now: u64,
    ) -> Vec<NativeEntityTarget> {
        self.native_entity_targets(
            id,
            center,
            radius,
            if sight {
                EntityTargetPurpose::Search
            } else {
                EntityTargetPurpose::Impact
            },
            now,
        )
    }
    fn encounter_emit(
        &mut self,
        id: u32,
        packets: Vec<ServerPacket>,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let pos = self.native_monsters[&id].position.clone();
        self.apply_zone_object_packets(&packets, now);
        let session_ids = self.native_monster_visible_recipients(id, &pos);
        if session_ids.is_empty() {
            Vec::new()
        } else {
            vec![ZoneOutbound::ToMany {
                session_ids,
                packets,
            }]
        }
    }
    fn encounter_attack_packet(&self, id: u32, kind: u8, level: u8, ranged: bool) -> ServerPacket {
        let m = &self.native_monsters[&id];
        if ranged {
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id: id,
                    location: m.position.clone(),
                    direction: m.direction,
                    target_id: 0,
                    target: Point { x: 0, y: 0 },
                    attack_type: kind,
                    spell: 0,
                    level,
                },
            }
        } else {
            ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: id,
                    location: m.position.clone(),
                    direction: m.direction,
                    spell: 0,
                    level,
                    attack_type: kind,
                },
            }
        }
    }
    fn encounter_move(&mut self, id: u32, toward: &Point, now: u64) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        if now < m.next_ai_ready_at_ms {
            return Vec::new();
        }
        let Some(first) = zone_direction_toward(&m.position, toward) else {
            return Vec::new();
        };
        let clockwise = self.encounter_roll(id, 2) == 0;
        for i in 0..8 {
            let dir = zone_rotated_direction(first, if clockwise { i } else { (8 - i) % 8 });
            let dest = offset_point(&m.position, dir, 1);
            if !self.can_native_monster_occupy(id, &dest) {
                continue;
            }
            let m = self.native_monsters.get_mut(&id).unwrap();
            m.position = dest.clone();
            m.direction = dir;
            m.next_ai_ready_at_ms = now + m.move_speed_ms;
            m.next_attack_ready_at_ms = m.next_attack_ready_at_ms.max(now + m.move_speed_ms);
            let packet = ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: id,
                    position: dest,
                    direction: dir,
                },
            };
            self.apply_zone_object_packets(&[packet.clone()], now);
            let mut out = self.diff_all_zone_object_visibility();
            out.extend(self.encounter_emit(id, vec![packet], now));
            return out;
        }
        Vec::new()
    }
    fn encounter_queue(
        &mut self,
        id: u32,
        target: Option<&NativeEntityTarget>,
        due: u64,
        damage: i32,
        accuracy: Option<i32>,
        radius: u8,
        front: u8,
        release: bool,
        point: Option<Point>,
    ) {
        let source_ref = self.native_entity_monster_ref(id);
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .horned_encounter
            .as_mut()
            .unwrap()
            .hits
            .push(HornedEncounterHit {
                due,
                target: target.and_then(|p| match &p.reference {
                    ZoneCombatEntityRef::Player {
                        session_id,
                        object_id,
                        ..
                    } => Some((session_id.clone(), *object_id)),
                    _ => None,
                }),
                target_ref: target.map(|p| p.reference.clone()),
                source_ref,
                point,
                radius,
                front,
                damage,
                accuracy,
                release,
                teleport: false,
                cancelled: false,
            });
    }
    fn encounter_line(&mut self, id: u32, length: i32, damage: i32, now: u64) {
        let m = self.native_monsters[&id].clone();
        let accuracy = crystal_monster_by_name(&m.name).map_or(0, |t| t.accuracy);
        for i in 1..=length {
            let tile = offset_point(&m.position, m.direction, i);
            if self.collision.is_blocked(&tile) {
                continue;
            }
            if let Some(target) = self.encounter_targets(id, &tile, 0, false, now).first() {
                self.encounter_queue(
                    id,
                    Some(target),
                    now + 300 + zone_tile_distance(&m.position, &target.position) as u64 * 50,
                    damage,
                    Some(accuracy),
                    0,
                    0,
                    true,
                    None,
                );
            }
        }
    }
    fn encounter_thrust(
        &mut self,
        id: u32,
        target: &NativeEntityTarget,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        let Some(dir) = zone_direction_toward(&m.position, &target.position) else {
            return Vec::new();
        };
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return Vec::new();
        };
        let (_, damage) = horned_stat_range(&m, HornedStat::Dc, t.min_dc, t.max_dc, now);
        let mut destination = m.position.clone();
        let mut crossed = Vec::new();
        for _ in 0..3 {
            let next = offset_point(&destination, dir, 1);
            if self.collision.is_blocked(&next) {
                break;
            }
            destination = next.clone();
            crossed.push(next);
        }
        // A dash attacks intermediate occupied cells, but its authoritative
        // final tile must remain free. Unlike the source precheck bug, every
        // crossed tile must also be statically walkable.
        while crossed
            .last()
            .is_some_and(|p| !self.can_native_monster_occupy(id, p))
        {
            crossed.pop();
        }
        let Some(last) = crossed.last() else {
            return Vec::new();
        };
        destination = last.clone();
        let distance = crossed.len() as i32;
        // Commit once at a free final tile; crossed victims are resolved at +500ms.
        self.native_monsters.get_mut(&id).unwrap().position = destination.clone();
        for point in crossed {
            if damage > 0 {
                self.encounter_queue(id, None, now + 500, damage, None, 0, 0, false, Some(point));
            }
        }
        let packet = ServerPacket::ObjectDashAttack {
            object_id: id,
            direction: m.direction,
            location: destination,
            distance,
        };
        self.apply_zone_object_packets(&[packet.clone()], now);
        let mut out = self.diff_all_zone_object_visibility();
        out.extend(self.encounter_emit(id, vec![packet], now));
        out
    }
    fn sorceror_attack(
        &mut self,
        id: u32,
        target: &NativeEntityTarget,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        let s = m
            .special_ai
            .as_ref()
            .unwrap()
            .horned_encounter
            .as_ref()
            .unwrap();
        let injured = i64::from(m.hp) * 100 / i64::from(m.max_hp.max(1)) < 90;
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .next_attack_ready_at_ms = now + m.attack_speed_ms;
        if now > s.stomp_ready && injured && self.encounter_roll(id, 4) == 0 {
            let loops = 5 + self.encounter_roll(id, 5) as u8;
            let duration = u64::from(loops) * 500;
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.next_ai_ready_at_ms = now + duration + 500;
            live.next_attack_ready_at_ms = now + duration + 500 + m.attack_speed_ms;
            let s = live
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap();
            s.immune = true;
            s.stomp_ready = now + 20000;
            let out = self.encounter_emit(
                id,
                vec![self.encounter_attack_packet(id, 2, loops, false)],
                now,
            );
            let damage = self
                .encounter_power(id, HornedStat::Sc, now)
                .saturating_mul(i32::from(loops));
            // Preserve source zero-SC behavior: it returns before scheduling immunity release.
            if damage > 0 {
                self.encounter_queue(
                    id,
                    Some(target),
                    now + duration + 500,
                    damage,
                    None,
                    2,
                    0,
                    true,
                    None,
                );
            }
            return out;
        }
        if now > s.tornado_ready && injured && self.encounter_roll(id, 4) == 0 {
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .tornado_ready = now + 15000;
            self.encounter_field(
                id,
                m.position.clone(),
                2,
                Spell::HornedSorcererDustTornado,
                now + 1000,
                now + 16000,
                1000,
                now,
            );
            return self.encounter_emit(
                id,
                vec![self.encounter_attack_packet(id, 3, 0, false)],
                now,
            );
        }
        let range = zone_tile_distance(&m.position, &target.position);
        if range > 0 && range <= 1 {
            let kind = if self.encounter_roll(id, 5) > 2 {
                Some((0, HornedStat::Dc, 2))
            } else if self.encounter_roll(id, 5) > 2 {
                Some((1, HornedStat::Mc, 3))
            } else {
                None
            };
            if let Some((kind, stat, length)) = kind {
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .next_ai_ready_at_ms = now + 300;
                let out = self.encounter_emit(
                    id,
                    vec![self.encounter_attack_packet(id, kind, 0, false)],
                    now,
                );
                let damage = self.encounter_power(id, stat, now);
                if damage > 0 {
                    self.encounter_line(id, length, damage, now);
                }
                return out;
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .next_ai_ready_at_ms = now + 500;
            return self.encounter_thrust(id, target, now);
        }
        if self.encounter_roll(id, 3) == 0 {
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .next_ai_ready_at_ms = now + 500;
            self.encounter_thrust(id, target, now)
        } else {
            self.encounter_move(id, &target.position, now)
        }
    }
    fn encounter_field(
        &mut self,
        id: u32,
        center: Point,
        radius: i32,
        spell: Spell,
        start: u64,
        expires: u64,
        tick_speed: u64,
        now: u64,
    ) {
        let m = self.native_monsters[&id].clone();
        if m.dead {
            return;
        }
        for y in center.y - radius..=center.y + radius {
            for x in center.x - radius..=center.x + radius {
                let location = Point { x, y };
                if x < 0 || y < 0 {
                    continue;
                }
                if self.collision.is_blocked(&location) {
                    continue;
                }
                if spell == Spell::HornedCommanderRockFall && location == m.position {
                    continue;
                }
                let value = self.encounter_power(id, HornedStat::Mc, now);
                let object_id = self.unique_object_id(0);
                self.horned_encounter_world
                    .get_or_insert_with(Default::default)
                    .spells
                    .push(HornedMapSpell {
                        object_id,
                        source: id,
                        source_ref: Some(ZoneCombatEntityRef::Monster {
                            object_id: id,
                            incarnation: m.incarnation,
                        }),
                        source_removed: false,
                        spell: spell as u8,
                        location: location.clone(),
                        cast_location: center.clone(),
                        direction: if spell == Spell::HornedSorcererDustTornado {
                            m.direction
                        } else {
                            MirDirection::Up
                        },
                        show: location == center,
                        value,
                        start,
                        expires,
                        next_tick: 0,
                        tick_speed,
                        spawned: false,
                    });
            }
        }
    }
    pub(super) fn tick_horned_encounter_ai(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| matches!(m.ai, 169 | 171).then_some(id))
            .collect();
        if ids.is_empty() && self.horned_encounter_world.is_none() {
            return Vec::new();
        }
        for &id in &ids {
            initialize_horned_encounter(self.native_monsters.get_mut(&id).unwrap());
        }
        let mut out = Vec::new();
        // KillRockSpikes on a dead commander precedes any map-spell damage.
        for &id in &ids {
            if self.native_monsters[&id].ai == 171 && self.native_monsters[&id].dead {
                out.extend(self.commander_prethink(id, now));
            }
        }
        out.extend(self.tick_horned_encounter_spells(now));
        for id in ids {
            out.extend(self.resolve_horned_encounter_hits(id, now));
            let m = self.native_monsters[&id].clone();
            if m.ai == 171 {
                out.extend(self.commander_prethink(id, now));
            }
            let m = self.native_monsters[&id].clone();
            if m.dead
                || !m.hostile_to_player
                || native_monster_control_active(&m, now)
                || !monster_visibility_can_act(&m, now)
            {
                continue;
            }
            let s = m
                .special_ai
                .as_ref()
                .unwrap()
                .horned_encounter
                .as_ref()
                .unwrap();
            if m.ai == 171 && s.immune {
                continue;
            }
            let targets = self.encounter_targets(
                id,
                &m.position,
                crystal_monster_by_name(&m.name).map_or(0, |t| i32::from(t.view_range)),
                true,
                now,
            );
            let Some(target) = self.selected_native_entity_target(id, now).or_else(|| {
                targets
                    .iter()
                    .find(|p| {
                        s.target_ref.as_ref().map_or_else(
                            || s.target.as_ref().is_some_and(|(_, id)| *id == p.object_id),
                            |r| r == &p.reference,
                        )
                    })
                    .or(targets.first())
                    .cloned()
            }) else {
                self.clear_native_entity_target(id);
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .horned_encounter
                    .as_mut()
                    .unwrap()
                    .target = None;
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .horned_encounter
                    .as_mut()
                    .unwrap()
                    .target_ref = None;
                continue;
            };
            self.set_native_entity_target(id, &target.reference, now);
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .target = match &target.reference {
                ZoneCombatEntityRef::Player {
                    session_id,
                    object_id,
                    ..
                } => Some((session_id.clone(), *object_id)),
                _ => None,
            };
            live.special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .target_ref = Some(target.reference.clone());
            let range = zone_tile_distance(&m.position, &target.position);
            let ready = now > m.next_attack_ready_at_ms && now >= m.next_ai_ready_at_ms;
            if range <= if m.ai == 169 { 5 } else { 1 } && ready {
                self.native_monsters.get_mut(&id).unwrap().direction =
                    zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
                out.extend(if m.ai == 169 {
                    self.sorceror_attack(id, &target, now)
                } else {
                    self.commander_attack(id, &target, now)
                });
            } else if m.ai == 169 || ready {
                let front = if m.ai == 169 {
                    let dir = match &target.reference {
                        ZoneCombatEntityRef::Player { session_id, .. } => {
                            self.players[session_id].direction
                        }
                        ZoneCombatEntityRef::Monster { object_id, .. } => {
                            self.native_monsters[object_id].direction
                        }
                    };
                    offset_point(&target.position, dir, 1)
                } else {
                    target.position
                };
                out.extend(self.encounter_move(id, &front, now));
            }
        }
        out
    }
}

impl ZoneRuntime {
    fn resolve_horned_encounter_hits(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let hits = std::mem::take(
            &mut self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .hits,
        );
        let mut out = Vec::new();
        for hit in hits {
            if now < hit.due {
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .horned_encounter
                    .as_mut()
                    .unwrap()
                    .hits
                    .push(hit);
                continue;
            }
            if let Some(source) = &hit.source_ref {
                if self.native_entity_monster_ref(id).as_ref() != Some(source)
                    || !self.objects.contains_key(&id)
                {
                    continue;
                }
            }
            if hit.release {
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .horned_encounter
                    .as_mut()
                    .unwrap()
                    .immune = false;
            }
            if hit.cancelled {
                continue;
            }
            if hit.teleport {
                self.clear_native_entity_target(id);
                let center = self.native_monsters[&id].position.clone();
                for _ in 0..10 {
                    let dest = Point {
                        x: center.x + self.encounter_roll(id, 21) as i32 - 10,
                        y: center.y + self.encounter_roll(id, 21) as i32 - 10,
                    };
                    if self.can_native_monster_occupy(id, &dest) {
                        out.extend(self.teleport_special_monster(id, dest, now, 10));
                        break;
                    }
                }
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .horned_encounter
                    .as_mut()
                    .unwrap()
                    .target = None;
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .horned_encounter
                    .as_mut()
                    .unwrap()
                    .target_ref = None;
                continue;
            }
            let m = self.native_monsters[&id].clone();
            let source = hit
                .source_ref
                .clone()
                .or_else(|| self.native_entity_monster_ref(id));
            let initial = hit
                .target_ref
                .as_ref()
                .and_then(|target| {
                    let source = source.as_ref()?;
                    if !self.native_entity_can_attack(
                        source,
                        target,
                        EntityTargetPurpose::Impact,
                        now,
                    ) {
                        return None;
                    }
                    let position = match target {
                        ZoneCombatEntityRef::Player { session_id, .. } => {
                            self.players.get(session_id)?.position.clone()
                        }
                        ZoneCombatEntityRef::Monster { object_id, .. } => {
                            self.native_monsters.get(object_id)?.position.clone()
                        }
                    };
                    Some(NativeEntityTarget {
                        reference: target.clone(),
                        object_id: target.object_id(),
                        position,
                    })
                })
                .or_else(|| {
                    if hit.target_ref.is_none() {
                        hit.target.as_ref().and_then(|(session, target)| {
                            self.players
                                .get(session)
                                .filter(|p| {
                                    p.object_id == *target
                                        && !p.dead
                                        && !p.chat_profile.in_safe_zone
                                })
                                .map(|p| NativeEntityTarget {
                                    reference: self.native_entity_player_ref(session).unwrap(),
                                    object_id: *target,
                                    position: p.position.clone(),
                                })
                        })
                    } else {
                        None
                    }
                });
            if (hit.target.is_some() || hit.target_ref.is_some()) && initial.is_none() {
                continue;
            }
            let targets = if let Some(point) = &hit.point {
                self.encounter_targets(id, point, 0, false, now)
                    .into_iter()
                    .take(1)
                    .collect()
            } else if hit.radius > 0 {
                let center = offset_point(&m.position, m.direction, i32::from(hit.front));
                self.encounter_targets(id, &center, i32::from(hit.radius), false, now)
            } else {
                initial.into_iter().collect::<Vec<_>>()
            };
            for target in targets {
                let Some(source) = &source else {
                    continue;
                };
                let (_, events) = self.resolve_native_entity_hit(
                    source,
                    &target.reference,
                    hit.damage,
                    if hit.accuracy.is_some() {
                        EntityDefence::ACAgility
                    } else {
                        EntityDefence::AC
                    },
                    false,
                    now,
                );
                out.extend(events);
            }
        }
        out
    }
    fn tick_horned_encounter_spells(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let Some(mut state) = self.horned_encounter_world.take() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        state.spells.retain_mut(|field| {
            if now < field.start {
                return true;
            }
            if now > field.expires {
                if field.spawned && field.show {
                    let session_ids = self
                        .players
                        .iter()
                        .filter_map(|(s, p)| {
                            p.visible_object_ids
                                .contains(&field.object_id)
                                .then_some(s.clone())
                        })
                        .collect();
                    self.remove_retained_zone_object(field.object_id);
                    out.push(ZoneOutbound::ToMany {
                        session_ids,
                        packets: vec![ServerPacket::ObjectRemove {
                            object_id: field.object_id,
                        }],
                    });
                }
                return false;
            }
            if !field.spawned {
                if field.show {
                    let spell = if field.spell == Spell::HornedSorcererDustTornado as u8 {
                        Spell::HornedSorcererDustTornado
                    } else if field.spell == Spell::HornedCommanderRockFall as u8 {
                        Spell::HornedCommanderRockFall
                    } else {
                        Spell::HornedCommanderRockSpike
                    };
                    let packet = ServerPacket::ObjectSpell {
                        info: ObjectSpellInfo {
                            object_id: field.object_id,
                            location: field.cast_location.clone(),
                            spell,
                            direction: field.direction,
                            param: false,
                        },
                    };
                    self.object_grid
                        .insert(field.object_id, &field.cast_location);
                    self.objects.insert(
                        field.object_id,
                        ZoneObject {
                            object_id: field.object_id,
                            position: field.cast_location.clone(),
                            packet,
                            health: None,
                            mana: None,
                            expires_at_ms: None,
                            buffs: BTreeMap::new(),
                        },
                    );
                    out.extend(self.diff_all_zone_object_visibility());
                }
                field.spawned = true;
            }
            if now >= field.next_tick {
                field.next_tick = now.saturating_add(field.tick_speed);
                if !field.source_removed
                    && self.objects.contains_key(&field.source)
                    && field.source_ref.as_ref().is_none_or(|r| {
                        self.native_entity_monster_ref(field.source).as_ref() == Some(r)
                    })
                    && field.value > 0
                {
                    for target in
                        self.encounter_targets(field.source, &field.location, 0, false, now)
                    {
                        let Some(source) = field
                            .source_ref
                            .clone()
                            .or_else(|| self.native_entity_monster_ref(field.source))
                        else {
                            continue;
                        };
                        let (_, events) = self.resolve_native_entity_hit(
                            &source,
                            &target.reference,
                            field.value,
                            EntityDefence::AC,
                            true,
                            now,
                        );
                        out.extend(events);
                    }
                }
            }
            true
        });
        self.horned_encounter_world = Some(state);
        out
    }
    fn commander_cancel_spikes(&mut self, id: u32, now: u64) {
        let source_ref = self.native_entity_monster_ref(id);
        if let Some(w) = self.horned_encounter_world.as_mut() {
            for field in &mut w.spells {
                if field.source == id
                    && field
                        .source_ref
                        .as_ref()
                        .is_none_or(|r| source_ref.as_ref() == Some(r))
                    && field.spell == Spell::HornedCommanderRockSpike as u8
                {
                    field.expires = now;
                }
            }
        }
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .horned_encounter
            .as_mut()
            .unwrap()
            .spike_ready = u64::MAX;
    }
    fn commander_kill_slaves(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let ids = self.native_monsters[&id]
            .special_ai
            .as_ref()
            .unwrap()
            .horned_encounter
            .as_ref()
            .unwrap()
            .slaves
            .clone();
        let mut out = Vec::new();
        for slave in ids {
            let expected = self.native_monsters[&id]
                .special_ai
                .as_ref()
                .unwrap()
                .horned_encounter
                .as_ref()
                .unwrap()
                .slave_lives
                .get(&slave)
                .copied();
            if expected.is_some_and(|life| {
                self.native_monsters
                    .get(&slave)
                    .is_none_or(|m| m.incarnation != life)
            }) {
                continue;
            }

            let Some(m) = self
                .native_monsters
                .get(&slave)
                .filter(|m| !m.dead)
                .cloned()
            else {
                continue;
            };
            if m.special_ai
                .as_ref()
                .and_then(|s| s.horned_encounter.as_ref())
                .and_then(|s| s.parent_incarnation)
                .is_some_and(|life| life != self.native_monsters[&id].incarnation)
            {
                continue;
            }
            let owner = m
                .special_ai
                .as_ref()
                .and_then(|s| s.horned_encounter.as_ref())
                .filter(|s| now <= s.reward_until)
                .and_then(|s| s.reward_owner.clone())
                .filter(|sid| self.players.contains_key(sid));
            let Some((_, _, killed, name, experience, position, direction, _, drops, boss_audit)) =
                self.apply_native_monster_scripted_death_damage(slave, owner.as_ref(), now)
            else {
                continue;
            };
            if !killed {
                continue;
            }
            out.extend(self.encounter_emit(
                slave,
                vec![ServerPacket::ObjectDied {
                    info: ObjectDiedInfo {
                        object_id: slave,
                        location: position.clone(),
                        direction,
                        kind: 0,
                    },
                }],
                now,
            ));
            if let Some(owner) = owner {
                let drops = self.spawn_native_monster_drops(
                    &name,
                    &position,
                    self.players[&owner].object_id,
                    drops,
                    now,
                );
                out.extend(self.diff_all_zone_object_visibility());
                out.extend(self.group_monster_kill_awards(
                    &owner,
                    ZoneMonsterKillAward {
                        source_receipt_key: None,
                        experience_selection: None,
                        monster_object_id: slave,
                        killed_at_ms: now,
                        monster_name: name,
                        experience,
                        drops,
                        boss_audit,
                    },
                ));
            }
        }
        out
    }
    fn commander_spawn_slave(
        &mut self,
        id: u32,
        name: &str,
        position: Point,
        direction: MirDirection,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.can_occupy(&position, None) {
            return Vec::new();
        }
        let Some(t) = crystal_monster_by_name(name) else {
            return Vec::new();
        };
        let child = self.unique_object_id(0);
        let spawn = ZoneMonsterSpawn {
            crystal_drop_seed: Some(now / 300),
            object_id: child,
            name: t.name.clone(),
            name_colour_argb: -1,
            image: t.image,
            ai: t.ai,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: t.level,
            max_hp: t.hp.max(1),
            hp: t.hp.max(1),
            experience: t.experience,
            move_speed_ms: u64::from(t.move_speed),
            attack_speed_ms: u64::from(t.attack_speed),
            friendly_guild: self.native_monsters[&id].friendly_guild.clone(),
            defense: super::super::types::ZoneMonsterDefense::from_crystal_template(&t),
            position,
            direction,
            respawn: None,
            drops: if mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name)
                .is_some_and(|m| m.no_drop_monster)
            {
                Vec::new()
            } else {
                crate::runtime::drops::zone_ground_drop_snapshots_for_monster_at_tick(
                    child,
                    name,
                    now / 300,
                )
            },
        };
        let (ok, out) = self.spawn_authoritative_monster(&spawn, now);
        if ok {
            let target = self.native_monsters[&id]
                .special_ai
                .as_ref()
                .unwrap()
                .horned_encounter
                .as_ref()
                .unwrap()
                .target
                .clone();
            let parent_incarnation = self.native_monsters[&id].incarnation;
            let target_ref = self.native_monsters[&id]
                .special_ai
                .as_ref()
                .unwrap()
                .horned_encounter
                .as_ref()
                .unwrap()
                .target_ref
                .clone();
            let child_incarnation = self.native_monsters[&child].incarnation;
            let m = self.native_monsters.get_mut(&child).unwrap();
            m.next_ai_ready_at_ms = now;
            let state = m
                .special_ai
                .get_or_insert_with(Default::default)
                .horned_encounter
                .get_or_insert_with(Default::default);
            state.parent = Some(id);
            state.parent_incarnation = Some(parent_incarnation);
            state.target = target;
            state.target_ref = target_ref;
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .slave_lives
                .insert(child, child_incarnation);
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .slaves
                .push(child);
        }
        out
    }
    fn commander_prethink(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        let s = m
            .special_ai
            .as_ref()
            .unwrap()
            .horned_encounter
            .as_ref()
            .unwrap()
            .clone();
        let mut out = Vec::new();
        if m.dead {
            if !s.death_handled {
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .horned_encounter
                    .as_mut()
                    .unwrap()
                    .death_handled = true;
                self.commander_cancel_spikes(id, now);
                out.extend(self.commander_kill_slaves(id, now));
            }
            return out;
        }
        if s.shield_until.is_some_and(|time| now >= time) {
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.next_ai_ready_at_ms = now + 300;
            live.next_attack_ready_at_ms = now + m.attack_speed_ms;
            let s = live
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap();
            s.shield_until = None;
            s.immune = false;
            out.extend(self.encounter_emit(
                id,
                vec![ServerPacket::RemoveBuff {
                    object_id: id,
                    buff_type: 56,
                }],
                now,
            ));
        }
        let hp = i64::from(m.hp) * 100 / i64::from(m.max_hp.max(1));
        if hp < 80 && !s.called_boulders {
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .called_boulders = true;
            let center = Point { x: 26, y: 32 };
            if zone_tile_distance(&m.position, &center) <= 20
                && self.can_native_monster_occupy(id, &center)
            {
                out.extend(self.teleport_special_monster(id, center, now, 10));
            }
            let pos = self.native_monsters[&id].position.clone();
            for i in 0..8 {
                let dir = zone_rotated_direction(MirDirection::Up, i);
                let point = offset_point(&pos, dir, if i % 2 == 0 { 9 } else { 7 });
                let facing = zone_direction_toward(&point, &pos).unwrap_or(dir);
                out.extend(self.commander_spawn_slave(id, "BoulderSpirit", point, facing, now));
            }
        }
        if self.native_monsters[&id]
            .special_ai
            .as_ref()
            .unwrap()
            .horned_encounter
            .as_ref()
            .unwrap()
            .immune
        {
            return out;
        }
        if hp < 10 && !s.called_shield {
            self.commander_cancel_spikes(id, now);
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.next_ai_ready_at_ms = now + 300;
            live.next_attack_ready_at_ms = now + m.attack_speed_ms;
            let state = live
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap();
            state.called_shield = true;
            state.immune = true;
            state.shield_until = Some(now + 20000);
            out.extend(self.encounter_emit(
                id,
                vec![
                    self.encounter_attack_packet(id, 4, 20, false),
                    ServerPacket::AddBuff {
                        buff: mir2_protocol::ClientBuff {
                            buff_type: 56,
                            visible: true,
                            object_id: id,
                            expire_time: 20000,
                            infinite: false,
                            paused: false,
                            stats: Vec::new(),
                            values: Vec::new(),
                        },
                    },
                ],
                now,
            ));
            let live = self.native_monsters[&id].clone();
            let front = offset_point(&live.position, live.direction, 1);
            // Crystal falls back from Front to CurrentLocation and permits
            // overlap. The shared Zone keeps occupancy exclusive, so only
            // this shield summon searches nearby free cells instead. Boulder
            // ring summons still use their exact source offsets.
            let spawn_position = if self.can_occupy(&front, None) {
                Some(front)
            } else {
                (1i32..=3)
                    .flat_map(|radius| {
                        (-radius..=radius).flat_map(move |dy| {
                            (-radius..=radius)
                                .filter(move |dx| dx.abs().max(dy.abs()) == radius)
                                .map(move |dx| (dx, dy))
                        })
                    })
                    .filter_map(|(dx, dy)| {
                        Some(Point {
                            x: live.position.x.checked_add(dx)?,
                            y: live.position.y.checked_add(dy)?,
                        })
                    })
                    .find(|point| self.can_occupy(point, None))
            };
            if let Some(position) = spawn_position {
                out.extend(self.commander_spawn_slave(
                    id,
                    "HornedSorceror",
                    position,
                    live.direction,
                    now,
                ));
            }
            return out;
        }
        if (10..50).contains(&hp) && !s.called_spikes {
            let state = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap();
            state.called_spikes = true;
            state.next_spike = 0;
            out.extend(self.encounter_emit(
                id,
                vec![self.encounter_attack_packet(id, 2, 0, true)],
                now,
            ));
        }
        let state = self.native_monsters[&id]
            .special_ai
            .as_ref()
            .unwrap()
            .horned_encounter
            .as_ref()
            .unwrap()
            .clone();
        if state.called_spikes && now > state.spike_ready {
            let mut found = false;
            let mut index = state.next_spike;
            while index < 49 {
                let center = Point {
                    x: 26 + (i32::from(index / 7) - 3) * 5,
                    y: 32 + (i32::from(index % 7) - 3) * 5,
                };
                index += 1;
                self.encounter_field(
                    id,
                    center.clone(),
                    2,
                    Spell::HornedCommanderRockSpike,
                    now + 500,
                    now + 600500,
                    1000,
                    now,
                );
                if !self.collision.is_blocked(&center) {
                    found = true;
                    break;
                }
            }
            let state = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap();
            state.next_spike = index;
            state.spike_ready = if found { now + 5000 } else { u64::MAX };
        }
        if hp < 100 && !s.advanced {
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .advanced = true;
        } else if hp == 100 && s.advanced {
            let state = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap();
            state.advanced = false;
            state.called_boulders = false;
            state.called_spikes = false;
            state.called_shield = false;
            state.spike_ready = 0;
            state.next_spike = 0;
            self.commander_cancel_spikes(id, now);
            out.extend(self.commander_kill_slaves(id, now));
        }
        out
    }
    fn commander_attack(
        &mut self,
        id: u32,
        target: &NativeEntityTarget,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        let advanced = m
            .special_ai
            .as_ref()
            .unwrap()
            .horned_encounter
            .as_ref()
            .unwrap()
            .advanced;
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .next_attack_ready_at_ms = now + m.attack_speed_ms;
        if advanced && self.encounter_roll(id, 20) == 0 {
            let loops = 5 + self.encounter_roll(id, 5) as u8;
            let duration = u64::from(loops) * 500;
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.next_ai_ready_at_ms = now + duration + 500;
            live.next_attack_ready_at_ms = now + duration + 500 + m.attack_speed_ms;
            live.special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .immune = true;
            let out = self.encounter_emit(
                id,
                vec![self.encounter_attack_packet(id, 3, loops, false)],
                now,
            );
            let front = offset_point(&m.position, m.direction, 2);
            for x in [-15, -5, 5] {
                for y in [-15, -5, 5] {
                    let center = Point {
                        x: front.x + x,
                        y: front.y + y,
                    };
                    let offset = self.encounter_roll(id, 200);
                    self.encounter_field(
                        id,
                        center,
                        10,
                        Spell::HornedCommanderRockFall,
                        now + 500 + offset,
                        now + duration + 500 + offset,
                        2000,
                        now,
                    );
                }
            }
            let damage = self
                .encounter_power(id, HornedStat::Dc, now)
                .saturating_mul(i32::from(loops));
            self.encounter_queue(
                id,
                Some(target),
                now + duration + 500,
                damage,
                None,
                5,
                2,
                true,
                None,
            );
            return out;
        }
        if advanced && self.encounter_roll(id, 15) == 0 {
            let loops = 5 + self.encounter_roll(id, 5) as u8;
            let duration = u64::from(loops) * 700;
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.next_ai_ready_at_ms = now + duration + 1500;
            live.next_attack_ready_at_ms = now + duration + 1500 + m.attack_speed_ms;
            live.special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .immune = true;
            let damage = self
                .encounter_power(id, HornedStat::Dc, now)
                .saturating_mul(i32::from(loops));
            for delay in [500, 1000] {
                self.encounter_queue(
                    id,
                    Some(target),
                    now + duration + delay,
                    damage,
                    None,
                    3,
                    0,
                    true,
                    None,
                );
            }
            return self.encounter_emit(
                id,
                vec![self.encounter_attack_packet(id, 2, loops, false)],
                now,
            );
        }
        if advanced && self.encounter_roll(id, 10) == 0 {
            let out =
                self.encounter_emit(id, vec![self.encounter_attack_packet(id, 1, 0, true)], now);
            let damage = self.encounter_power(id, HornedStat::Dc, now);
            if damage > 0 {
                self.encounter_queue(id, Some(target), now + 300, damage, None, 3, 2, true, None);
            }
            return out;
        }
        if advanced && self.encounter_roll(id, 10) == 0 {
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned_encounter
                .as_mut()
                .unwrap()
                .hits
                .push(HornedEncounterHit {
                    due: now + 300,
                    source_ref: Some(ZoneCombatEntityRef::Monster {
                        object_id: id,
                        incarnation: m.incarnation,
                    }),
                    target: None,
                    target_ref: None,
                    point: None,
                    radius: 0,
                    front: 0,
                    damage: 0,
                    accuracy: None,
                    release: true,
                    teleport: true,
                    cancelled: false,
                });
            return self.encounter_emit(
                id,
                vec![self.encounter_attack_packet(id, 0, 0, true)],
                now,
            );
        }
        let kind = self.encounter_roll(id, 2) as u8;
        let out = self.encounter_emit(
            id,
            vec![self.encounter_attack_packet(id, kind, 0, false)],
            now,
        );
        let damage = self.encounter_power(id, HornedStat::Dc, now);
        if damage > 0 {
            let accuracy = crystal_monster_by_name(&m.name).map_or(0, |t| t.accuracy);
            self.encounter_queue(
                id,
                Some(target),
                now + 500,
                damage,
                Some(accuracy),
                0,
                0,
                true,
                None,
            );
        }
        out
    }
}
