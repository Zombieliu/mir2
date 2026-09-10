//! Crystal Horned family. All mutable buffs and delayed hits belong to Zone.
use super::entity_combat::{
    EntityDefence, EntityTargetPurpose, NativeEntityTarget, ZoneCombatEntityRef,
};
use super::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct HornedState {
    pub rng: u64,
    pub fear_until: u64,
    pub buff_ready: u64,
    pub shield_ready: u64,
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<ZoneCombatEntityRef>,
    pub pending: Vec<HornedHit>,
    pub buffs: Vec<HornedBuff>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HornedHit {
    pub due: u64,
    pub target: u32,
    pub session: Option<SessionId>,
    pub damage: i32,
    pub magic: bool,
    pub area: u8,
    pub buff: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<ZoneCombatEntityRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<ZoneCombatEntityRef>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HornedBuff {
    pub kind: u8,
    pub source: u32,
    pub expires: u64,
    pub min: i32,
    pub max: i32,
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum HornedStat {
    Dc,
    Mc,
    Sc,
    Ac,
    Mac,
}
/// Add only real Crystal buffs. No inferred parent-stat inheritance or nonzero
/// fallback is applied to templates such as BoulderSpirit.
pub(super) fn horned_stat_range(
    m: &ZoneNativeMonster,
    kind: HornedStat,
    min: i32,
    max: i32,
    now: u64,
) -> (i32, i32) {
    let mut result = (min, max);
    if let Some(s) = m.special_ai.as_ref().and_then(|s| s.horned.as_ref()) {
        for b in &s.buffs {
            if now >= b.expires {
                continue;
            }
            let applies = match b.kind {
                50 => matches!(kind, HornedStat::Dc | HornedStat::Mc),
                51 => matches!(kind, HornedStat::Ac | HornedStat::Mac),
                55 => matches!(kind, HornedStat::Ac),
                _ => false,
            };
            if applies {
                result.0 = result.0.saturating_add(b.min);
                result.1 = result.1.saturating_add(b.max);
            }
        }
    }
    result
}
pub(in crate::runtime::zone) fn initialize_horned_monster(m: &mut ZoneNativeMonster) {
    if matches!(m.ai, 163..=165) {
        m.special_ai
            .get_or_insert_with(Default::default)
            .horned
            .get_or_insert_with(Default::default);
    }
}
impl ZoneRuntime {
    pub(super) fn clear_horned_targets(&mut self, id: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.horned.as_mut()) {
                if s.target == Some(id) {
                    s.target = None;
                    s.target_ref = None;
                }
                s.pending.retain(|h| h.target != id);
            }
        }
    }
    fn horned_roll(&mut self, id: u32, upper: u64) -> u64 {
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
            .horned
            .as_mut()
            .unwrap();
        if s.rng == 0 {
            s.rng = (u64::from(id) << 32) ^ 0x163165;
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
    fn horned_power(&mut self, id: u32, kind: HornedStat, now: u64) -> i32 {
        let m = &self.native_monsters[&id];
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return 0;
        };
        let (min, max) = match kind {
            HornedStat::Dc => (t.min_dc, t.max_dc),
            HornedStat::Mc => (t.min_mc, t.max_mc),
            HornedStat::Sc => (t.min_sc, t.max_sc),
            HornedStat::Ac => (t.min_ac, t.max_ac),
            HornedStat::Mac => (t.min_mac, t.max_mac),
        };
        let (min, max) = horned_stat_range(m, kind, min, max, now);
        min + self.horned_roll(id, (max - min + 1).max(1) as u64) as i32
    }
    fn horned_emit(&mut self, id: u32, packets: Vec<ServerPacket>, now: u64) -> Vec<ZoneOutbound> {
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
    fn horned_attack_packet(&self, id: u32, target: u32, kind: u8, ranged: bool) -> ServerPacket {
        let m = &self.native_monsters[&id];
        if ranged {
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id: id,
                    location: m.position.clone(),
                    direction: m.direction,
                    target_id: target,
                    target: self
                        .players
                        .values()
                        .find(|p| p.object_id == target)
                        .map(|p| p.position.clone())
                        .or_else(|| {
                            self.native_monsters
                                .get(&target)
                                .map(|m| m.position.clone())
                        })
                        .unwrap_or_else(|| m.position.clone()),
                    attack_type: kind,
                    spell: 0,
                    level: 0,
                },
            }
        } else {
            ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: id,
                    location: m.position.clone(),
                    direction: m.direction,
                    spell: 0,
                    level: 0,
                    attack_type: kind,
                },
            }
        }
    }
    fn horned_targets(
        &self,
        id: u32,
        center: &Point,
        radius: i32,
        purpose: EntityTargetPurpose,
        now: u64,
    ) -> Vec<NativeEntityTarget> {
        self.native_entity_targets(id, center, radius, purpose, now)
    }
    fn horned_friends(&self, id: u32, center: &Point, radius: i32) -> Vec<u32> {
        let m = &self.native_monsters[&id];
        let mut friends: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&other, f)| {
                (other != id
                    && !f.dead
                    && f.hp > 0
                    && f.master_object_id == m.master_object_id
                    && f.hostile_to_player == m.hostile_to_player
                    && zone_tile_distance(center, &f.position) <= radius
                    && monster_visibility_is_visible(f))
                .then_some(other)
            })
            .collect();
        friends.sort_by_key(|other| {
            let p = &self.native_monsters[other].position;
            (zone_tile_distance(center, p), p.y, p.x, *other)
        });
        friends
    }
    fn horned_move(&mut self, id: u32, direction: MirDirection, now: u64) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        if now < m.next_ai_ready_at_ms {
            return Vec::new();
        }
        let clockwise = self.horned_roll(id, 2) == 0;
        for i in 0..8 {
            let dir = zone_rotated_direction(direction, if clockwise { i } else { (8 - i) % 8 });
            let dest = offset_point(&m.position, dir, 1);
            if !self.can_native_monster_occupy(id, &dest) {
                continue;
            }
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.position = dest.clone();
            live.direction = dir;
            live.next_ai_ready_at_ms = now.saturating_add(m.move_speed_ms);
            live.next_attack_ready_at_ms = live
                .next_attack_ready_at_ms
                .max(now.saturating_add(m.move_speed_ms));
            let packet = ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: id,
                    position: dest,
                    direction: dir,
                },
            };
            self.apply_zone_object_packets(&[packet.clone()], now);
            let mut out = self.diff_all_zone_object_visibility();
            out.extend(self.horned_emit(id, vec![packet], now));
            return out;
        }
        Vec::new()
    }
    fn horned_add_buff(&mut self, id: u32, buff: HornedBuff, now: u64) -> Vec<ZoneOutbound> {
        let kind = buff.kind;
        let duration = buff.expires.saturating_sub(now);
        let m = self.native_monsters.get_mut(&id).unwrap();
        let s = m
            .special_ai
            .get_or_insert_with(Default::default)
            .horned
            .get_or_insert_with(Default::default);
        s.buffs.retain(|b| b.kind != kind);
        s.buffs.push(buff);
        self.horned_emit(
            id,
            vec![ServerPacket::AddBuff {
                buff: mir2_protocol::ClientBuff {
                    buff_type: kind,
                    visible: true,
                    object_id: id,
                    expire_time: duration as i64,
                    infinite: false,
                    paused: false,
                    stats: Vec::new(),
                    values: Vec::new(),
                },
            }],
            now,
        )
    }
    pub(super) fn tick_horned_ai(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                (matches!(m.ai, 163..=165)
                    || m.special_ai.as_ref().is_some_and(|s| s.horned.is_some()))
                .then_some(id)
            })
            .collect();
        let mut out = Vec::new();
        for id in ids {
            initialize_horned_monster(self.native_monsters.get_mut(&id).unwrap());
            let m = self.native_monsters.get_mut(&id).unwrap();
            let s = m.special_ai.as_mut().unwrap().horned.as_mut().unwrap();
            let expired: Vec<_> = s
                .buffs
                .iter()
                .filter_map(|b| (now >= b.expires).then_some(b.kind))
                .collect();
            s.buffs.retain(|b| now < b.expires);
            for kind in expired {
                out.extend(self.horned_emit(
                    id,
                    vec![ServerPacket::RemoveBuff {
                        object_id: id,
                        buff_type: kind,
                    }],
                    now,
                ));
            }
            out.extend(self.resolve_horned_hits(id, now));
            let m = self.native_monsters[&id].clone();
            if !matches!(m.ai, 163..=165)
                || m.dead
                || !m.hostile_to_player
                || native_monster_control_active(&m, now)
                || !monster_visibility_can_act(&m, now)
            {
                continue;
            }
            let s = m.special_ai.as_ref().unwrap().horned.as_ref().unwrap();
            if m.ai == 164 && now > s.buff_ready {
                let view = crystal_monster_by_name(&m.name).map_or(14, |t| i32::from(t.view_range));
                let friends = self.horned_friends(id, &m.position, view);
                if !friends.is_empty() {
                    let friend = friends[self.horned_roll(id, friends.len() as u64) as usize];
                    let pos = self.native_monsters[&friend].position.clone();
                    let friend_incarnation = self.native_monsters[&friend].incarnation;
                    let live = self.native_monsters.get_mut(&id).unwrap();
                    live.direction =
                        zone_direction_toward(&m.position, &pos).unwrap_or(m.direction);
                    live.next_ai_ready_at_ms = now + 300;
                    live.next_attack_ready_at_ms = now + m.attack_speed_ms;
                    let s = live.special_ai.as_mut().unwrap().horned.as_mut().unwrap();
                    s.buff_ready = now + 20000;
                    s.pending.push(HornedHit {
                        due: now + zone_tile_distance(&m.position, &pos) as u64 * 50 + 500,
                        target: friend,
                        session: None,
                        damage: 0,
                        magic: true,
                        area: 0,
                        buff: true,
                        source_ref: Some(ZoneCombatEntityRef::Monster {
                            object_id: id,
                            incarnation: m.incarnation,
                        }),
                        target_ref: Some(ZoneCombatEntityRef::Monster {
                            object_id: friend,
                            incarnation: friend_incarnation,
                        }),
                    });
                    out.extend(self.horned_emit(
                        id,
                        vec![self.horned_attack_packet(id, friend, 1, true)],
                        now,
                    ));
                    continue;
                } else {
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .horned
                        .as_mut()
                        .unwrap()
                        .buff_ready = now + 10000;
                }
            }
            let targets = self.horned_targets(
                id,
                &m.position,
                crystal_monster_by_name(&m.name).map_or(0, |t| i32::from(t.view_range)),
                EntityTargetPurpose::Search,
                now,
            );
            let Some(target) = self.selected_native_entity_target(id, now).or_else(|| {
                targets
                    .iter()
                    .find(|p| {
                        Some(p.object_id) == s.target
                            && s.target_ref.as_ref().is_none_or(|r| r == &p.reference)
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
                    .horned
                    .as_mut()
                    .unwrap()
                    .target = None;
                continue;
            };
            self.set_native_entity_target(id, &target.reference, now);
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned
                .as_mut()
                .unwrap()
                .target = Some(target.object_id);
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned
                .as_mut()
                .unwrap()
                .target_ref = Some(target.reference.clone());
            if now <= m.next_attack_ready_at_ms || now < m.next_ai_ready_at_ms {
                continue;
            }
            let distance = zone_tile_distance(&m.position, &target.position);
            let toward =
                zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
            if matches!(m.ai, 163 | 164) && !(distance <= 6 && now < s.fear_until) {
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .horned
                    .as_mut()
                    .unwrap()
                    .fear_until = now + 5000;
                let dir = if distance >= 6 {
                    toward
                } else {
                    zone_rotated_direction(toward, 4)
                };
                out.extend(self.horned_move(id, dir, now));
                continue;
            }
            if m.ai == 165 {
                let shield = s.buffs.iter().any(|b| b.kind == 55 && now < b.expires);
                let dx = (m.position.x - target.position.x).abs();
                let dy = (m.position.y - target.position.y).abs();
                let range = distance > 0
                    && distance <= 4
                    && ((dx <= 1 && dy <= 1) || dx == dy || dx % 2 == dy % 2);
                if shield || !range {
                    out.extend(self.horned_move(
                        id,
                        if shield {
                            zone_rotated_direction(toward, 4)
                        } else {
                            toward
                        },
                        now,
                    ));
                    continue;
                }
            }
            self.native_monsters.get_mut(&id).unwrap().direction = toward;
            out.extend(self.horned_attack(id, &target, now));
        }
        out
    }
    fn horned_attack(
        &mut self,
        id: u32,
        target: &NativeEntityTarget,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        let distance = zone_tile_distance(&m.position, &target.position);
        let mut out = Vec::new();
        if m.ai == 165
            && i64::from(m.hp) * 100 / i64::from(m.max_hp.max(1)) < 50
            && now
                > m.special_ai
                    .as_ref()
                    .unwrap()
                    .horned
                    .as_ref()
                    .unwrap()
                    .shield_ready
        {
            let ready = now + 15000 + self.horned_roll(id, 5000);
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.next_ai_ready_at_ms = now + 300;
            live.next_attack_ready_at_ms = now + m.attack_speed_ms;
            live.special_ai
                .as_mut()
                .unwrap()
                .horned
                .as_mut()
                .unwrap()
                .shield_ready = ready;
            out.extend(self.horned_emit(
                id,
                vec![self.horned_attack_packet(id, target.object_id, 2, false)],
                now,
            ));
            out.extend(self.horned_add_buff(
                id,
                HornedBuff {
                    kind: 55,
                    source: id,
                    expires: now + 10000,
                    min: 500,
                    max: 500,
                },
                now,
            ));
            return out;
        }
        let (magic, area, delay, ranged, kind) = match m.ai {
            163 if distance <= 3 => (true, 3, 500, false, 0),
            163 => {
                if self.horned_roll(id, 5) == 0 {
                    out.extend(self.horned_emit(
                        id,
                        vec![self.horned_attack_packet(id, target.object_id, 1, true)],
                        now,
                    ));
                    for _ in 0..4 {
                        let dest = Point {
                            x: m.position.x + self.horned_roll(id, 9) as i32 - 4,
                            y: m.position.y + self.horned_roll(id, 9) as i32 - 4,
                        };
                        let moved = match &target.reference {
                            ZoneCombatEntityRef::Player { session_id, .. }
                                if self.can_occupy(&dest, Some(session_id)) =>
                            {
                                out.extend(self.horned_teleport_player(
                                    session_id,
                                    dest,
                                    crystal_monster_by_name(&m.name).map_or(0, |t| t.effect),
                                    now,
                                ));
                                true
                            }
                            ZoneCombatEntityRef::Monster { object_id, .. }
                                if self.can_native_monster_occupy(*object_id, &dest) =>
                            {
                                out.extend(self.teleport_special_monster(
                                    *object_id,
                                    dest,
                                    now,
                                    crystal_monster_by_name(&m.name).map_or(0, |t| t.effect),
                                ));
                                true
                            }
                            _ => false,
                        };
                        if moved {
                            break;
                        }
                    }
                    let pos = match &target.reference {
                        ZoneCombatEntityRef::Player { session_id, .. } => {
                            self.players[session_id].position.clone()
                        }
                        ZoneCombatEntityRef::Monster { object_id, .. } => {
                            self.native_monsters[object_id].position.clone()
                        }
                    };
                    self.native_monsters.get_mut(&id).unwrap().direction =
                        zone_direction_toward(&m.position, &pos).unwrap_or(m.direction);
                    return out;
                }
                (false, 0, 800, true, 0)
            }
            164 => (false, 0, distance as u64 * 50 + 500, true, 0),
            _ => {
                if distance <= 1 && self.horned_roll(id, 3) > 0 {
                    (false, 0, 300, false, 0)
                } else {
                    (false, 0, 500, false, 1)
                }
            }
        };
        if m.ai != 163 {
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.next_ai_ready_at_ms = now + 300;
            live.next_attack_ready_at_ms = now + m.attack_speed_ms;
        }
        let damage = self.horned_power(
            id,
            if magic {
                HornedStat::Mc
            } else {
                HornedStat::Dc
            },
            now,
        );
        if damage <= 0 {
            if m.ai == 164 {
                out.extend(self.horned_emit(
                    id,
                    vec![self.horned_attack_packet(id, target.object_id, 0, true)],
                    now,
                ));
            }
            return out;
        }
        let live = self.native_monsters.get_mut(&id).unwrap();
        live.next_ai_ready_at_ms = now + if m.ai == 163 { 500 } else { 300 };
        live.next_attack_ready_at_ms = now + m.attack_speed_ms;
        let mut victims = vec![target.clone()];
        if m.ai == 165 && kind == 1 {
            victims.clear();
            let left = offset_point(&m.position, zone_rotated_direction(m.direction, 6), 1);
            let right = offset_point(&m.position, zone_rotated_direction(m.direction, 2), 1);
            for start in [&m.position, &left, &right] {
                for step in 1..=4 {
                    let tile = offset_point(start, m.direction, step);
                    if self.collision.is_blocked(&tile) {
                        continue;
                    }
                    if let Some(victim) = self
                        .horned_targets(id, &tile, 0, EntityTargetPurpose::Impact, now)
                        .first()
                    {
                        victims.push(victim.clone());
                    }
                }
            }
        }
        for victim in victims {
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned
                .as_mut()
                .unwrap()
                .pending
                .push(HornedHit {
                    due: now
                        + delay
                        + if m.ai == 165 && kind == 1 {
                            zone_tile_distance(&m.position, &victim.position) as u64 * 50
                        } else {
                            0
                        },
                    target: victim.object_id,
                    session: match &victim.reference {
                        ZoneCombatEntityRef::Player { session_id, .. } => Some(session_id.clone()),
                        _ => None,
                    },
                    source_ref: Some(ZoneCombatEntityRef::Monster {
                        object_id: id,
                        incarnation: m.incarnation,
                    }),
                    target_ref: Some(victim.reference),
                    damage,
                    magic,
                    area,
                    buff: false,
                });
        }
        out.extend(self.horned_emit(
            id,
            vec![self.horned_attack_packet(id, target.object_id, kind, ranged)],
            now,
        ));
        out
    }
    fn resolve_horned_hits(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let pending = std::mem::take(
            &mut self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .horned
                .as_mut()
                .unwrap()
                .pending,
        );
        let mut out = Vec::new();
        for hit in pending {
            if now < hit.due {
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .horned
                    .as_mut()
                    .unwrap()
                    .pending
                    .push(hit);
                continue;
            }
            if let Some(ZoneCombatEntityRef::Monster {
                object_id,
                incarnation,
            }) = &hit.source_ref
            {
                if self.native_entity_monster_ref(*object_id)
                    != Some(ZoneCombatEntityRef::Monster {
                        object_id: *object_id,
                        incarnation: *incarnation,
                    })
                {
                    continue;
                }
            }
            if hit.buff {
                if !self.objects.contains_key(&id) || !self.objects.contains_key(&hit.target) {
                    continue;
                }
                if let Some(ZoneCombatEntityRef::Monster {
                    object_id,
                    incarnation,
                }) = &hit.target_ref
                {
                    if self.native_entity_monster_ref(*object_id)
                        != Some(ZoneCombatEntityRef::Monster {
                            object_id: *object_id,
                            incarnation: *incarnation,
                        })
                    {
                        continue;
                    }
                }

                let Some(friend) = self
                    .native_monsters
                    .get(&hit.target)
                    .filter(|m| !m.dead)
                    .cloned()
                else {
                    continue;
                };
                let m = self.native_monsters[&id].clone();
                let Some(t) = crystal_monster_by_name(&m.name) else {
                    continue;
                };
                let (min, max) = horned_stat_range(&m, HornedStat::Mc, t.min_mc, t.max_mc, now);
                let kind = if t.effect == 0 {
                    50
                } else if t.effect == 1 {
                    51
                } else {
                    continue;
                };
                for other in self.horned_friends(id, &friend.position, 4) {
                    out.extend(self.horned_add_buff(
                        other,
                        HornedBuff {
                            kind,
                            source: id,
                            expires: now + 10000,
                            min,
                            max,
                        },
                        now,
                    ));
                }
                continue;
            }
            if let (Some(source), Some(target)) = (&hit.source_ref, &hit.target_ref) {
                // Mage completion validates its originally captured target before rescan.
                if !self.native_entity_can_attack(source, target, EntityTargetPurpose::Impact, now)
                {
                    continue;
                }
                let targets = if hit.area > 0 {
                    self.horned_targets(
                        id,
                        &self.native_monsters[&id].position,
                        i32::from(hit.area),
                        EntityTargetPurpose::VisibleImpact,
                        now,
                    )
                    .into_iter()
                    .map(|t| t.reference)
                    .collect::<Vec<_>>()
                } else {
                    vec![target.clone()]
                };
                let ai = self.native_monsters[&id].ai;
                let defence = if ai == 163 {
                    if hit.magic {
                        EntityDefence::MACAgility
                    } else {
                        EntityDefence::AC
                    }
                } else {
                    EntityDefence::ACAgility
                };
                for target in targets {
                    let (_, events) = self.resolve_native_entity_hit(
                        source, &target, hit.damage, defence, false, now,
                    );
                    out.extend(events);
                }
                continue;
            }
            let Some(session) = hit.session else {
                continue;
            };
            let Some(player) = self
                .players
                .get(&session)
                .filter(|p| p.object_id == hit.target && !p.dead && !p.chat_profile.in_safe_zone)
            else {
                continue;
            };
            let targets = if hit.area > 0 {
                let pos = self.native_monsters[&id].position.clone();
                self.horned_targets(
                    id,
                    &pos,
                    i32::from(hit.area),
                    EntityTargetPurpose::VisibleImpact,
                    now,
                )
            } else {
                vec![NativeEntityTarget {
                    reference: self.native_entity_player_ref(&session).unwrap(),
                    object_id: hit.target,
                    position: player.position.clone(),
                }]
            };
            for target in targets {
                let ZoneCombatEntityRef::Player { session_id, .. } = target.reference else {
                    continue;
                };
                out.extend(self.resolve_pending_native_player_hit(
                    PendingNativePlayerHit {
                        ready_at_ms: now,
                        attacker_object_id: id,
                        attacker_ai: self.native_monsters[&id].ai,
                        target_session_id: session_id,
                        target_object_id: target.object_id,
                        damage: hit.damage,
                        magic: hit.magic,
                    },
                    now,
                ));
            }
        }
        out
    }
}

impl ZoneRuntime {
    fn horned_teleport_player(
        &mut self,
        session: &SessionId,
        dest: Point,
        effect: u8,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let old = self.players[session].clone();
        let id = old.object_id;
        let observers: Vec<_> = self
            .players
            .iter()
            .filter_map(|(sid, p)| {
                (sid != session && p.visible_object_ids.contains(&id)).then_some(sid.clone())
            })
            .collect();
        let mut out = vec![ZoneOutbound::ToMany {
            session_ids: observers.clone(),
            packets: vec![
                ServerPacket::ObjectTeleportOut {
                    object_id: id,
                    effect_type: effect,
                },
                ServerPacket::ObjectRemove { object_id: id },
            ],
        }];
        self.occupancy.remove(&tile_key(&old.position));
        self.occupancy.insert(tile_key(&dest), session.clone());
        let player = self.players.get_mut(session).unwrap();
        player.position = dest.clone();
        player.movement_actions.clear();
        player.run_step_until_ms = 0;
        if let Some(map) = mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name) {
            player.chat_profile.in_safe_zone = map
                .safe_zones
                .iter()
                .any(|z| zone_tile_distance(&dest, &z.location) <= i32::from(z.size));
        }
        let location = user_location_packet(player);
        self.player_grid.moved(session, &dest);
        self.ecs.move_player(session, &dest);
        out.push(ZoneOutbound::ToSession {
            session_id: session.clone(),
            packets: vec![location],
        });
        out.extend(self.diff_visibility_for(session));
        out.extend(self.diff_zone_object_visibility_for(session));
        let overlap = observers
            .into_iter()
            .filter(|sid| self.players[sid].visible_object_ids.contains(&id))
            .collect();
        out.push(ZoneOutbound::ToMany {
            session_ids: overlap,
            packets: object_player_packets(&self.players[session]),
        });
        let recipients = self
            .players
            .iter()
            .filter_map(|(sid, p)| {
                (sid == session || p.visible_object_ids.contains(&id)).then_some(sid.clone())
            })
            .collect();
        out.push(ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![ServerPacket::ObjectTeleportIn {
                object_id: id,
                effect_type: effect,
            }],
        });
        out.push(ZoneOutbound::SaveTransform {
            session_id: session.clone(),
            position: dest,
            direction: old.direction,
        });
        let _ = now;
        out
    }
}
