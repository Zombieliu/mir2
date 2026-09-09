//! Crystal GreatFoxSpirit AI50 and its GuardianRock AI48 activation dependency.
//! Player targets are shared Zone actors, never personal-session projections.
use super::entity_combat::{
    EntityDefence, EntityTargetPurpose, NativeEntityTarget, ZoneCombatEntityRef,
};
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GreatFoxState {
    pub stage: u8,
    pub recall_ready_ms: u64,
    pub rng_state: u64,
    pub target_object_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<ZoneCombatEntityRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed_move_ready_ms: Option<u64>,
    pub lifecycle_alive: Option<bool>,
    pub guardian_active: bool,
    pub pending: Vec<GreatFoxHit>,
}
impl Default for GreatFoxState {
    fn default() -> Self {
        Self {
            stage: 0,
            recall_ready_ms: 0,
            rng_state: 0,
            target_object_id: None,
            target_ref: None,
            pushed_move_ready_ms: None,
            lifecycle_alive: None,
            guardian_active: true,
            pending: Vec::new(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GreatFoxHit {
    pub due_ms: u64,
    pub target_session: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<ZoneCombatEntityRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<ZoneCombatEntityRef>,
    pub target_id: u32,
    pub damage: i32,
    pub guardian_pull: bool,
}

pub(super) fn great_fox_accepts_hp_change(m: &ZoneNativeMonster) -> bool {
    m.ai != 48
}
pub(in crate::runtime::zone) fn initialize_great_fox_monster(m: &mut ZoneNativeMonster) {
    if matches!(m.ai, 48 | 50) {
        m.special_ai
            .get_or_insert_with(Default::default)
            .great_fox
            .get_or_insert_with(Default::default);
    }
    if m.ai == 48 {
        m.direction = MirDirection::Up;
    }
}
pub(super) fn great_fox_can_move(m: &ZoneNativeMonster) -> bool {
    !matches!(m.ai, 48 | 50)
}
pub(super) fn great_fox_movement_ready(m: &ZoneNativeMonster, now: u64) -> bool {
    m.special_ai
        .as_ref()
        .and_then(|s| s.great_fox.as_ref())
        .and_then(|s| s.pushed_move_ready_ms)
        .is_none_or(|ready| now > ready)
}
pub(super) fn sync_great_fox_packet(m: &ZoneNativeMonster, packet: &mut ServerPacket) {
    if let ServerPacket::ObjectMonster { info } = packet {
        if m.ai == 50 {
            info.extra_byte = m
                .special_ai
                .as_ref()
                .and_then(|s| s.great_fox.as_ref())
                .map_or(0, |s| s.stage);
        }
        if m.ai == 48 {
            info.direction = MirDirection::Up;
        }
    }
}

impl ZoneRuntime {
    /// Retire all references to one player life before Leave, replacement Join,
    /// or revive. Guardian events follow its current target, so retiring that
    /// target also retires its already queued pulls before reacquisition.
    pub(super) fn clear_great_fox_targets(&mut self, object_id: u32) {
        for monster in self.native_monsters.values_mut() {
            let Some(state) = monster
                .special_ai
                .as_mut()
                .and_then(|s| s.great_fox.as_mut())
            else {
                continue;
            };
            let current = state.target_object_id == Some(object_id);
            if current {
                state.target_object_id = None;
                state.target_ref = None;
            }
            state
                .pending
                .retain(|hit| hit.target_id != object_id && !(current && hit.guardian_pull));
        }
    }

    fn great_fox_roll(&mut self, id: u32, now: u64, upper: u64) -> u64 {
        let s = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .get_or_insert_with(Default::default)
            .great_fox
            .get_or_insert_with(Default::default);
        if s.rng_state == 0 {
            s.rng_state = (u64::from(id) << 32) ^ now ^ 0x50F04;
        }
        loop {
            s.rng_state = s.rng_state.wrapping_add(0x9E3779B97F4A7C15);
            let mut n = s.rng_state;
            n = (n ^ (n >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            n = (n ^ (n >> 27)).wrapping_mul(0x94D049BB133111EB);
            n ^= n >> 31;
            if n >= upper.wrapping_neg() % upper {
                return n % upper;
            }
        }
    }
    fn great_fox_targets(
        &self,
        id: u32,
        m: &ZoneNativeMonster,
        radius: i32,
        now: u64,
    ) -> Vec<NativeEntityTarget> {
        self.native_entity_targets(id, &m.position, radius, EntityTargetPurpose::Search, now)
    }
    fn great_fox_magic_resist(&self, target: &ZoneCombatEntityRef) -> u64 {
        match target {
            ZoneCombatEntityRef::Player { session_id, .. } => self
                .players
                .get(session_id)
                .map_or(0, |p| p.combat_stats.magic_resist),
            ZoneCombatEntityRef::Monster { object_id, .. } => self
                .native_monsters
                .get(object_id)
                .map_or(0, |m| zone_native_monster_buff_stat_total(m, 30)),
        }
        .clamp(0, 10) as u64
    }

    /// Run every Zone tick before generic monster AI. Generic AI must skip 48/50.
    pub(super) fn tick_great_fox_ai(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| matches!(m.ai, 48 | 50).then_some(id))
            .collect();
        let mut out = Vec::new();
        // Apply boss lifecycle transitions before any guardian can start a cast.
        for &id in &ids {
            let m = self.native_monsters[&id].clone();
            let alive = !m.dead && m.hp > 0;
            let state = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .get_or_insert_with(Default::default)
                .great_fox
                .get_or_insert_with(Default::default);
            let old_alive = state.lifecycle_alive;
            state.lifecycle_alive = Some(alive);
            if m.ai == 50 && old_alive != Some(alive) {
                let rocks: Vec<_> = self
                    .native_monsters
                    .iter()
                    .filter_map(|(&rock, r)| {
                        (r.ai == 48 && zone_tile_distance(&m.position, &r.position) <= 20)
                            .then_some(rock)
                    })
                    .collect();
                for rock in rocks {
                    self.native_monsters
                        .get_mut(&rock)
                        .unwrap()
                        .special_ai
                        .get_or_insert_with(Default::default)
                        .great_fox
                        .get_or_insert_with(Default::default)
                        .guardian_active = alive;
                }
            }
        }
        for id in ids {
            let m = self.native_monsters[&id].clone();
            let alive = !m.dead && m.hp > 0;
            if !alive {
                // Crystal keeps already cast actions until corpse despawn.
                out.extend(self.resolve_great_fox_effects(id, now));
                continue;
            }
            if m.ai == 50 && m.max_hp >= 4 {
                let next = (4 - m.hp / (m.max_hp / 4)) as u8; // Preserve Crystal's integer division/cast.
                let s = self
                    .native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .great_fox
                    .as_mut()
                    .unwrap();
                if next > s.stage {
                    s.stage = next;
                    let m = &self.native_monsters[&id];
                    if let Some(object) = self.objects.get_mut(&id) {
                        sync_great_fox_packet(m, &mut object.packet);
                        let packet = object.packet.clone();
                        let recipients = self.native_monster_visible_recipients(id, &m.position);
                        if !recipients.is_empty() {
                            out.push(ZoneOutbound::ToMany {
                                session_ids: recipients,
                                packets: vec![packet],
                            });
                        }
                    }
                }
            }
            out.extend(self.resolve_great_fox_effects(id, now));
            let m = self.native_monsters[&id].clone();
            if native_monster_control_active(&m, now) || now < m.hallucination_until_ms {
                continue;
            }
            let s = m.special_ai.as_ref().unwrap().great_fox.as_ref().unwrap();
            if m.ai == 48 && !s.guardian_active {
                continue;
            }
            let view = crystal_monster_by_name(&m.name).map_or(14, |t| i32::from(t.view_range));
            let targets = self.great_fox_targets(id, &m, view, now);
            let Some(target) = self.selected_native_entity_target(id, now).or_else(|| {
                targets
                    .iter()
                    .find(|t| {
                        s.target_ref
                            .as_ref()
                            .map_or(Some(t.object_id) == s.target_object_id, |r| {
                                r == &t.reference
                            })
                    })
                    .or_else(|| targets.first())
                    .cloned()
            }) else {
                self.clear_native_entity_target(id);
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .great_fox
                    .as_mut()
                    .unwrap()
                    .target_object_id = None;
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .great_fox
                    .as_mut()
                    .unwrap()
                    .target_ref = None;
                continue;
            };
            self.set_native_entity_target(id, &target.reference, now);
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .great_fox
                .as_mut()
                .unwrap()
                .target_object_id = Some(target.object_id);
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .great_fox
                .as_mut()
                .unwrap()
                .target_ref = Some(target.reference.clone());
            let distance = zone_tile_distance(&m.position, &target.position);
            if m.ai == 50
                && distance > 3
                && self.great_fox_roll(id, now, 10) == 0
                && now >= s.recall_ready_ms
            {
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .great_fox
                    .as_mut()
                    .unwrap()
                    .recall_ready_ms = now.saturating_add(10000);
                let recall = self.great_fox_targets(id, &m, 30, now);
                if !recall.is_empty() && self.great_fox_roll(id, now, 4) > 0 {
                    let mut tried = false;
                    for victim in recall {
                        if zone_tile_distance(&m.position, &victim.position) <= 3 {
                            continue;
                        }
                        let resist = self.great_fox_magic_resist(&victim.reference);
                        if self.great_fox_roll(id, now, 10) < resist {
                            continue;
                        }
                        let dirs = [
                            MirDirection::Up,
                            MirDirection::UpRight,
                            MirDirection::Right,
                            MirDirection::DownRight,
                            MirDirection::Down,
                            MirDirection::DownLeft,
                            MirDirection::Left,
                        ];
                        let dest = offset_point(
                            &m.position,
                            dirs[self.great_fox_roll(id, now, 7) as usize],
                            1,
                        );
                        match &victim.reference {
                            ZoneCombatEntityRef::Player { session_id, .. } => {
                                if self.can_occupy(&dest, Some(session_id)) {
                                    out.extend(
                                        self.great_fox_relocate_player(session_id, dest, true, now),
                                    );
                                }
                            }
                            ZoneCombatEntityRef::Monster { object_id, .. } => {
                                if self.can_native_monster_occupy(*object_id, &dest) {
                                    out.extend(
                                        self.teleport_special_monster(*object_id, dest, now, 0),
                                    );
                                }
                            }
                        }
                        // Crystal fallback is the occupied boss tile. Shared Zone
                        // refuses overlap rather than corrupting occupancy.
                        tried = true;
                        break;
                    }
                    if tried {
                        continue;
                    }
                }
            }
            if now <= m.next_attack_ready_at_ms
                || now < m.next_ai_ready_at_ms
                || distance > if m.ai == 50 { 7 } else { view }
            {
                continue;
            }
            if m.ai == 48 {
                // Crystal does not reserve an action lock until completion.
                let live = self.native_monsters.get_mut(&id).unwrap();
                live.special_ai
                    .as_mut()
                    .unwrap()
                    .great_fox
                    .as_mut()
                    .unwrap()
                    .pending
                    .push(GreatFoxHit {
                        due_ms: now.saturating_add(500),
                        target_session: match &target.reference {
                            ZoneCombatEntityRef::Player { session_id, .. } => {
                                Some(session_id.clone())
                            }
                            _ => None,
                        },
                        source_ref: Some(ZoneCombatEntityRef::Monster {
                            object_id: id,
                            incarnation: m.incarnation,
                        }),
                        target_ref: Some(target.reference.clone()),
                        target_id: target.object_id,
                        damage: 0,
                        guardian_pull: true,
                    });
                continue;
            }
            let damage = crystal_monster_by_name(&m.name).map_or(7, |t| {
                zone_roll_stat_range(t.min_dc, t.max_dc, now, id, 0x50DC)
            });
            if damage <= 0 {
                continue;
            }
            let ranged = distance == 0 || distance > 2;
            let victims = self.great_fox_targets(id, &m, if ranged { 7 } else { 2 }, now);
            if victims.is_empty() {
                continue;
            }
            let mut packets = vec![if ranged {
                ServerPacket::ObjectRangeAttack {
                    info: ObjectRangeAttackInfo {
                        object_id: id,
                        location: m.position.clone(),
                        direction: m.direction,
                        target_id: 0,
                        target: Point { x: 0, y: 0 },
                        attack_type: 0,
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
                        attack_type: 0,
                    },
                }
            }];
            if let Some(last) = victims.last() {
                self.set_native_entity_target(id, &last.reference, now);
            }
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.next_ai_ready_at_ms = now.saturating_add(300);
            live.next_attack_ready_at_ms = now.saturating_add(live.attack_speed_ms);
            let s = live
                .special_ai
                .as_mut()
                .unwrap()
                .great_fox
                .as_mut()
                .unwrap();
            for victim in victims {
                s.target_object_id = Some(victim.object_id);
                s.target_ref = Some(victim.reference.clone());
                s.pending.push(GreatFoxHit {
                    due_ms: now.saturating_add(300),
                    target_session: match &victim.reference {
                        ZoneCombatEntityRef::Player { session_id, .. } => Some(session_id.clone()),
                        _ => None,
                    },
                    source_ref: Some(ZoneCombatEntityRef::Monster {
                        object_id: id,
                        incarnation: m.incarnation,
                    }),
                    target_ref: Some(victim.reference),
                    target_id: victim.object_id,
                    damage,
                    guardian_pull: false,
                });
                if ranged {
                    packets.push(ServerPacket::ObjectEffect {
                        info: ObjectEffectInfo {
                            object_id: victim.object_id,
                            effect: 8,
                            effect_type: 0,
                            delay_time: 0,
                            time: 0,
                        },
                    });
                }
            }
            self.apply_zone_object_packets(&packets, now);
            let recipients = self.native_monster_visible_recipients(id, &m.position);
            if !recipients.is_empty() {
                out.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets,
                });
            }
        }
        out
    }

    fn resolve_great_fox_effects(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let effects = std::mem::take(
            &mut self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .great_fox
                .as_mut()
                .unwrap()
                .pending,
        );
        let mut future = Vec::new();
        let mut out = Vec::new();
        for hit in effects {
            if now < hit.due_ms {
                future.push(hit);
                continue;
            }
            // New actions carry both lives. Legacy player actions retain their
            // original compatibility path, never invent a native target life.
            let Some(source) = hit
                .source_ref
                .clone()
                .or_else(|| self.native_entity_monster_ref(id))
            else {
                continue;
            };
            let target = if hit.guardian_pull {
                let state = self.native_monsters[&id]
                    .special_ai
                    .as_ref()
                    .unwrap()
                    .great_fox
                    .as_ref()
                    .unwrap();
                self.selected_native_entity_target(id, now)
                    .map(|t| t.reference)
                    .or_else(|| state.target_ref.clone())
                    .or_else(|| {
                        self.players
                            .values()
                            .find(|p| Some(p.object_id) == state.target_object_id)
                            .and_then(|p| self.native_entity_player_ref(&p.session_id))
                    })
            } else {
                hit.target_ref.clone().or_else(|| {
                    hit.target_session
                        .as_ref()
                        .and_then(|sid| self.native_entity_player_ref(sid))
                        .filter(|r| r.object_id() == hit.target_id)
                })
            };
            let Some(target) = target else {
                continue;
            };
            if !self.native_entity_can_attack(&source, &target, EntityTargetPurpose::Impact, now) {
                continue;
            }
            if hit.guardian_pull {
                let m = self.native_monsters[&id].clone();
                let old = match &target {
                    ZoneCombatEntityRef::Player { session_id, .. } => {
                        self.players[session_id].position.clone()
                    }
                    ZoneCombatEntityRef::Monster { object_id, .. } => {
                        self.native_monsters[object_id].position.clone()
                    }
                };
                let resist = self.great_fox_magic_resist(&target);
                let packet = ServerPacket::ObjectRangeAttack {
                    info: ObjectRangeAttackInfo {
                        object_id: id,
                        location: m.position.clone(),
                        direction: MirDirection::Up,
                        target_id: target.object_id(),
                        target: old.clone(),
                        attack_type: 0,
                        spell: 0,
                        level: 0,
                    },
                };
                let recipients = self.native_monster_visible_recipients(id, &m.position);
                if !recipients.is_empty() {
                    out.push(ZoneOutbound::ToMany {
                        session_ids: recipients,
                        packets: vec![packet],
                    });
                }
                if self.great_fox_roll(id, now, 10) >= resist {
                    if let Some(dir) = zone_direction_toward(&old, &m.position)
                        .filter(|_| zone_tile_distance(&old, &m.position) > 1)
                    {
                        let distance = (zone_tile_distance(&old, &m.position) - 1).clamp(0, 4);
                        match &target {
                            ZoneCombatEntityRef::Player { session_id, .. } => out
                                .extend(self.great_fox_pull_player(session_id, dir, distance, now)),
                            ZoneCombatEntityRef::Monster { object_id, .. } => out.extend(
                                self.great_fox_pull_monster(*object_id, dir, distance, now),
                            ),
                        }
                    }
                }
                let m = self.native_monsters.get_mut(&id).unwrap();
                m.next_ai_ready_at_ms = now.saturating_add(300);
                m.next_attack_ready_at_ms = now.saturating_add(m.attack_speed_ms);
                continue;
            }
            let (damage, impact) = self.resolve_native_entity_hit(
                &source,
                &target,
                hit.damage,
                EntityDefence::MAC,
                false,
                now,
            );
            out.extend(impact);
            if damage > 0 {
                for slow in [true, false] {
                    // PoisonTarget draws SC before its resistance/chance gates,
                    // even though these controls do not use the poison value.
                    if let Some(t) = crystal_monster_by_name(&self.native_monsters[&id].name) {
                        let _value = t.min_sc
                            + self.great_fox_roll(id, now, (t.max_sc - t.min_sc + 1).max(1) as u64)
                                as i32;
                    }
                    let resist = self.native_entity_poison_resist(&target).clamp(0, 10) as u64;
                    if self.great_fox_roll(id, now, 10) >= resist
                        && self.great_fox_roll(id, now, 5) == 0
                    {
                        if slow {
                            out.extend(
                                self.apply_native_entity_slow(&source, &target, 15, 1000, now),
                            );
                        } else {
                            out.extend(
                                self.apply_native_entity_paralysis(&source, &target, 5, 1000, now),
                            );
                        }
                    }
                }
            }
        }
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .great_fox
            .as_mut()
            .unwrap()
            .pending = future;
        out
    }

    pub(super) fn great_fox_pull_monster(
        &mut self,
        id: u32,
        direction: MirDirection,
        distance: i32,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !crystal_monster_by_name(&self.native_monsters[&id].name).is_some_and(|t| t.can_push) {
            return Vec::new();
        }
        let reverse = zone_rotated_direction(direction, 4);
        let mut out = Vec::new();
        let mut steps = 0u64;
        for _ in 0..distance {
            let old = self.native_monsters[&id].position.clone();
            let destination = offset_point(&old, direction, 1);
            if !self.can_native_monster_occupy(id, &destination) {
                break;
            }
            let before: Vec<_> = self
                .players
                .iter()
                .filter_map(|(sid, p)| p.visible_object_ids.contains(&id).then_some(sid.clone()))
                .collect();
            let m = self.native_monsters.get_mut(&id).unwrap();
            m.position = destination.clone();
            m.direction = reverse;
            let packet = ServerPacket::ObjectPushed {
                object_id: id,
                location: destination,
                direction: reverse,
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            out.extend(self.diff_all_zone_object_visibility());
            let recipients: Vec<_> = before
                .into_iter()
                .filter(|sid| self.players[sid].visible_object_ids.contains(&id))
                .collect();
            if !recipients.is_empty() {
                out.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets: vec![packet],
                });
            }
            steps += 1;
        }
        let m = self.native_monsters.get_mut(&id).unwrap();
        m.next_ai_ready_at_ms = m.next_ai_ready_at_ms.max(now.saturating_add(300 * steps));
        let state = m
            .special_ai
            .get_or_insert_with(Default::default)
            .great_fox
            .get_or_insert_with(Default::default);
        state.pushed_move_ready_ms = Some(
            state
                .pushed_move_ready_ms
                .unwrap_or(0)
                .max(now.saturating_add(500 * steps)),
        );
        // Crystal additionally processes spells on the final tile immediately.
        // Shared ground-spell owners currently resolve that tile on their tick.
        out
    }

    pub(super) fn great_fox_pull_player(
        &mut self,
        session: &SessionId,
        direction: MirDirection,
        distance: i32,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let mut out = Vec::new();
        let reverse = zone_rotated_direction(direction, 4);
        let id = self.players[session].object_id;
        let mut moved = false;
        for _ in 0..distance {
            let old = self.players[session].position.clone();
            let destination = offset_point(&old, direction, 1);
            if !self.can_player_movement_occupy(&destination, Some(session)) {
                break;
            }
            let before: BTreeSet<_> = self
                .players
                .iter()
                .filter_map(|(sid, p)| {
                    (sid != session && p.visible_object_ids.contains(&id)).then_some(sid.clone())
                })
                .collect();
            self.occupancy.remove(&tile_key(&old));
            self.occupancy
                .insert(tile_key(&destination), session.clone());
            let player = self.players.get_mut(session).unwrap();
            player.position = destination.clone();
            player.direction = reverse;
            player.movement_actions.clear();
            player.run_step_until_ms = 0;
            if let Some(map) = mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name) {
                player.chat_profile.in_safe_zone = map.safe_zones.iter().any(|zone| {
                    zone_tile_distance(&zone.location, &destination) <= i32::from(zone.size)
                });
            }
            self.player_grid.moved(session, &destination);
            self.ecs.move_player(session, &destination);
            out.extend(self.diff_visibility_for(session));
            out.extend(self.diff_zone_object_visibility_for(session));
            let recipients: Vec<_> = before
                .into_iter()
                .filter(|sid| self.players[sid].visible_object_ids.contains(&id))
                .collect();
            if !recipients.is_empty() {
                out.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets: vec![ServerPacket::ObjectPushed {
                        object_id: id,
                        location: destination.clone(),
                        direction: reverse,
                    }],
                });
            }
            out.push(ZoneOutbound::ToSession {
                session_id: session.clone(),
                packets: vec![ServerPacket::Pushed {
                    location: destination,
                    direction: reverse,
                }],
            });
            moved = true;
        }
        // HumanObject.Pushed applies ActionTime even when a blocked tile causes
        // zero displacement. Never shorten a stronger existing action lock.
        let player = self.players.get_mut(session).unwrap();
        player.movement_ready_at_ms = player.movement_ready_at_ms.max(now.saturating_add(500));
        player.next_attack_ready_at_ms =
            player.next_attack_ready_at_ms.max(now.saturating_add(500));
        player.next_spell_ready_at_ms = player.next_spell_ready_at_ms.max(now.saturating_add(500));
        if moved {
            out.push(ZoneOutbound::ToSession {
                session_id: session.clone(),
                packets: vec![user_location_packet(player)],
            });
            out.push(ZoneOutbound::SaveTransform {
                session_id: session.clone(),
                position: player.position.clone(),
                direction: reverse,
            });
        }
        out
    }

    fn great_fox_relocate_player(
        &mut self,
        session: &SessionId,
        destination: Point,
        teleport: bool,
        _now: u64,
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
        let mut out = Vec::new();
        if !observers.is_empty() {
            let packets = if teleport {
                vec![
                    ServerPacket::ObjectTeleportOut {
                        object_id: id,
                        effect_type: 0,
                    },
                    ServerPacket::ObjectRemove { object_id: id },
                ]
            } else {
                vec![
                    ServerPacket::ObjectPushed {
                        object_id: id,
                        location: destination.clone(),
                        direction: old.direction,
                    },
                    ServerPacket::ObjectRemove { object_id: id },
                ]
            };
            out.push(ZoneOutbound::ToMany {
                session_ids: observers.clone(),
                packets,
            });
        }
        self.occupancy.remove(&tile_key(&old.position));
        let player = self.players.get_mut(session).unwrap();
        player.position = destination.clone();
        player.movement_actions.clear();
        player.movement_ready_at_ms = 0;
        player.run_step_until_ms = 0;
        self.occupancy
            .insert(tile_key(&destination), session.clone());
        self.player_grid.moved(session, &destination);
        self.ecs.move_player(session, &destination);
        out.push(ZoneOutbound::ToSession {
            session_id: session.clone(),
            packets: vec![user_location_packet(player)],
        });
        out.extend(self.diff_visibility_for(session));
        out.extend(self.diff_zone_object_visibility_for(session));
        // The symmetric diff preserves both old visibility sets. It handles
        // old-only removals and new-only spawns; overlap viewers saw teleport
        // out/remove above and therefore need a fresh authoritative spawn.
        let overlap: Vec<_> = observers
            .into_iter()
            .filter(|sid| {
                self.players
                    .get(sid)
                    .is_some_and(|p| p.visible_object_ids.contains(&id))
            })
            .collect();
        if !overlap.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids: overlap,
                packets: object_player_packets(&self.players[session]),
            });
        }
        if teleport {
            let recipients: Vec<_> = self
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
                    effect_type: 0,
                }],
            });
        }
        out.push(ZoneOutbound::SaveTransform {
            session_id: session.clone(),
            position: destination,
            direction: old.direction,
        });
        out
    }
}

#[cfg(test)]
mod forced_target_tests {
    use super::*;
    use mir2_protocol::MirGender;
    fn fixture(ai: u8, id: u32) -> ZoneRuntime {
        let mut z = ZoneRuntime::new(ZoneKey::for_map("forced-leaf"));
        for (name, oid, x) in [
            ("nearest", 101, 99),
            ("forced", 102, if ai == 50 { 106 } else { 101 }),
        ] {
            z.handle(ZoneCommand::Join(ZoneJoin {
                session_id: SessionId::new(name),
                account_id: name.into(),
                character_index: 1,
                object_id: oid,
                name: name.into(),
                class: MirClass::Warrior,
                gender: MirGender::Male,
                level: 50,
                hp: 10000,
                max_hp: 10000,
                mp: 0,
                map_file_name: "forced-leaf".into(),
                position: Point { x, y: 100 },
                direction: MirDirection::Up,
                chat_profile: Default::default(),
                combat_stats: Default::default(),
            }));
        }
        z.spawn_authoritative_monster(
            &ZoneMonsterSpawn {
                object_id: id,
                name: "ArcherGuard".into(),
                name_colour_argb: -1,
                image: 132,
                ai,
                disposition: Some(crate::config::WorldEntityDisposition::Hostile),
                level: 37,
                max_hp: 10000,
                hp: 10000,
                experience: 0,
                move_speed_ms: 500,
                attack_speed_ms: 1000,
                friendly_guild: None,
                defense: Default::default(),
                position: Point { x: 100, y: 100 },
                direction: MirDirection::Up,
                respawn: None,
                drops: Vec::new(),
            },
            0,
        );
        // Seed an existing local cache without advancing movement or attack clocks.
        let nearest = z
            .native_entity_player_ref(&SessionId::new("nearest"))
            .unwrap();
        let special = z
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap();
        match ai {
            165 => {
                let s = special.horned.as_mut().unwrap();
                s.target = Some(101);
                s.target_ref = Some(nearest);
            }
            169 => {
                let s = special.horned_encounter.as_mut().unwrap();
                s.target = Some((SessionId::new("nearest"), 101));
                s.target_ref = Some(nearest);
            }
            52 => {
                let s = special.evil_mir.as_mut().unwrap();
                s.target = Some((SessionId::new("nearest"), 101));
                s.target_ref = Some(nearest);
            }
            50 => {
                let s = special.great_fox.as_mut().unwrap();
                s.target_object_id = Some(101);
                s.target_ref = Some(nearest);
            }
            _ => unreachable!(),
        }
        let target = z
            .native_entity_player_ref(&SessionId::new("forced"))
            .unwrap();
        assert!(z.set_native_entity_target(id, &target, 0));
        z
    }
    fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
        out.iter()
            .flat_map(|o| match o {
                ZoneOutbound::ToSession { packets, .. }
                | ZoneOutbound::ToMany { packets, .. }
                | ZoneOutbound::ToAll { packets } => packets.as_slice(),
                _ => &[],
            })
            .collect()
    }
    #[test]
    fn forced_target_changes_horned_and_encounter_attack_direction() {
        for ai in [165, 169] {
            let mut z = fixture(ai, 98000 + u32::from(ai));
            let out = z.tick(1);
            assert!(
                packets(&out).iter().any(|p| match p {
                    ServerPacket::ObjectAttack { info } =>
                        info.object_id == 98000 + u32::from(ai)
                            && info.direction == MirDirection::Right,
                    ServerPacket::ObjectRangeAttack { info } =>
                        info.object_id == 98000 + u32::from(ai)
                            && info.direction == MirDirection::Right,
                    _ => false,
                }),
                "AI {ai} must aim actual attack at forced target opposite cached nearest"
            );
        }
    }
    #[test]
    fn forced_target_changes_evil_mir_spell_anchor() {
        for id in 98500..98600 {
            let mut z = fixture(52, id);
            let out = z.tick(1);
            if packets(&out)
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==id))
            {
                continue;
            }
            assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==id && info.target_id==102)));
            return;
        }
        panic!("normal range branch not exercised");
    }
    #[test]
    fn forced_distant_target_changes_great_fox_attack_mode_without_reordering_area() {
        let mut z = fixture(50, 98750);
        let out = z.tick(1);
        assert!(
            packets(&out)
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==98750)),
            "far forced target chooses ranged mode even though cached nearest is adjacent"
        );
    }
}
