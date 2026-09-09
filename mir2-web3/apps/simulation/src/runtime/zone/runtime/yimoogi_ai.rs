//! Crystal Yimoogi.cs (AI36). Targets and sister identity belong to the Zone.
use super::entity_combat::{
    EntityDefence, EntityTargetPurpose, NativeEntityTarget, ZoneCombatEntityRef,
};
use super::stage_summons::StageSummonState;
use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct YimoogiState {
    spawn_ms: u64,
    child: bool,
    child_spawned: bool,
    sister: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sister_incarnation: Option<u64>,
    target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_ref: Option<ZoneCombatEntityRef>,
    final_teleport: bool,
    sequence: u64,
    action_ms: u64,
    hits: Vec<YimoogiHit>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct YimoogiHit {
    hit: Option<PendingNativePlayerHit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_ref: Option<ZoneCombatEntityRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_ref: Option<ZoneCombatEntityRef>,
    #[serde(default, skip_serializing_if = "yimoogi_zero_time")]
    due: u64,
    #[serde(default, skip_serializing_if = "yimoogi_zero_damage")]
    damage: i32,
    accuracy: Option<i32>,
}
fn yimoogi_zero_time(value: &u64) -> bool {
    *value == 0
}
fn yimoogi_zero_damage(value: &i32) -> bool {
    *value == 0
}
pub(in crate::runtime::zone) fn initialize_yimoogi(m: &mut ZoneNativeMonster, now: u64) {
    if m.ai == 36 {
        m.special_ai
            .get_or_insert_with(Default::default)
            .yimoogi
            .get_or_insert_with(|| YimoogiState {
                spawn_ms: now.saturating_add(4000),
                ..Default::default()
            });
    }
}
fn yimoogi_roll(s: &mut YimoogiState, id: u32, now: u64, n: u64) -> u64 {
    s.sequence = s.sequence.wrapping_add(1);
    let mut x = now ^ u64::from(id) ^ s.sequence.wrapping_mul(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    (x ^ (x >> 31)) % n.max(1)
}
impl ZoneRuntime {
    pub(super) fn yimoogi_set_attacker_target(
        &mut self,
        id: u32,
        target: &ZoneCombatEntityRef,
        now: u64,
    ) {
        let Some(source) = self.native_entity_monster_ref(id) else {
            return;
        };
        if !self.native_entity_can_attack(&source, target, EntityTargetPurpose::Impact, now) {
            return;
        }
        let sister = {
            let Some(m) = self
                .native_monsters
                .get_mut(&id)
                .filter(|m| m.ai == 36 && !m.dead)
            else {
                return;
            };
            initialize_yimoogi(m, now);
            let s = m.special_ai.as_mut().unwrap().yimoogi.as_mut().unwrap();
            s.target = Some(target.object_id());
            s.target_ref = Some(target.clone());
            s.sister.zip(s.sister_incarnation)
        };
        if let Some((id, life)) = sister {
            if let Some(m) = self
                .native_monsters
                .get_mut(&id)
                .filter(|m| m.ai == 36 && !m.dead && m.incarnation == life)
            {
                if let Some(s) = m.special_ai.as_mut().and_then(|s| s.yimoogi.as_mut()) {
                    s.target = Some(target.object_id());
                    s.target_ref = Some(target.clone());
                }
            }
        }
    }
    pub(super) fn yimoogi_allows_drop(&self, id: u32) -> bool {
        let sister = self
            .native_monsters
            .get(&id)
            .and_then(|m| m.special_ai.as_ref())
            .and_then(|s| s.yimoogi.as_ref())
            .and_then(|s| s.sister.zip(s.sister_incarnation));
        !sister.is_some_and(|(id, life)| {
            self.native_monsters
                .get(&id)
                .is_some_and(|m| m.ai == 36 && !m.dead && m.incarnation == life)
        })
    }
    pub(super) fn clear_yimoogi_target_life(&mut self, target: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.yimoogi.as_mut()) {
                if s.target == Some(target) {
                    s.target = None;
                    s.target_ref = None;
                }
                s.hits.retain(|h| {
                    h.target_ref
                        .as_ref()
                        .map(|r| r.object_id())
                        .or_else(|| h.hit.as_ref().map(|h| h.target_object_id))
                        != Some(target)
                });
            }
        }
    }
    fn yimoogi_entity_target(
        &self,
        id: u32,
        m: &ZoneNativeMonster,
        now: u64,
    ) -> Option<NativeEntityTarget> {
        let s = m.special_ai.as_ref()?.yimoogi.as_ref()?;
        let reference = s.target_ref.clone().or_else(|| {
            self.players
                .values()
                .find(|p| Some(p.object_id) == s.target)
                .and_then(|p| self.native_entity_player_ref(&p.session_id))
        })?;
        let source = self.native_entity_monster_ref(id)?;
        if !self.native_entity_can_attack(&source, &reference, EntityTargetPurpose::Impact, now) {
            return None;
        }
        let position = match &reference {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                self.players.get(session_id)?.position.clone()
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                self.native_monsters.get(object_id)?.position.clone()
            }
        };
        Some(NativeEntityTarget {
            object_id: reference.object_id(),
            reference,
            position,
        })
    }
    pub(super) fn yimoogi_target(&self, m: &ZoneNativeMonster) -> Option<NativeMonsterTarget> {
        let id = m.special_ai.as_ref()?.yimoogi.as_ref()?.target?;
        let p = self.players.values().find(|p| {
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
        })?;
        Some(NativeMonsterTarget {
            session_id: p.session_id.clone(),
            object_id: p.object_id,
            position: p.position.clone(),
        })
    }
    /// Global prethink; spawning and final teleport precede base action/control gates.
    pub(super) fn tick_yimoogi_states(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (m.ai == 36 && !m.dead).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            initialize_yimoogi(self.native_monsters.get_mut(&id).unwrap(), now);
            let m = self.native_monsters[&id].clone();
            let mut s = m.special_ai.as_ref().unwrap().yimoogi.clone().unwrap();
            if !s.child && !s.final_teleport && m.hp <= m.max_hp / 10 {
                if let Some(bounds) = self.collision.bounds() {
                    let width = i64::from(bounds.max_x) - i64::from(bounds.min_x) + 1;
                    let height = i64::from(bounds.max_y) - i64::from(bounds.min_y) + 1;
                    let mut destination = None;
                    if width > 0 && height > 0 {
                        for _ in 0..40 {
                            let p = Point {
                                x: (i64::from(bounds.min_x)
                                    + yimoogi_roll(&mut s, id, now, width as u64) as i64)
                                    as i32,
                                y: (i64::from(bounds.min_y)
                                    + yimoogi_roll(&mut s, id, now, height as u64) as i64)
                                    as i32,
                            };
                            if p != m.position && self.can_native_monster_occupy(id, &p) {
                                destination = Some(p);
                                break;
                            }
                        }
                    }
                    if let Some(point) = destination {
                        out.extend(self.teleport_special_monster(id, point, now, 1));
                        s.final_teleport = true;
                        for _ in 0..2 {
                            let (_, events) = self.spawn_special_relative(
                                id,
                                "WhiteSerpent",
                                &m.position,
                                &m,
                                s.target_ref.clone(),
                                false,
                                now,
                            );
                            out.extend(events);
                        }
                        s.target = None;
                        s.target_ref = None;
                        if let Some(sister) = s
                            .sister
                            .zip(s.sister_incarnation)
                            .and_then(|(id, life)| {
                                self.native_monsters
                                    .get_mut(&id)
                                    .filter(|m| m.ai == 36 && m.incarnation == life)
                            })
                            .and_then(|m| m.special_ai.as_mut())
                            .and_then(|s| s.yimoogi.as_mut())
                        {
                            sister.target = None;
                            sister.target_ref = None;
                        }
                    }
                }
            }
            if !s.child && !s.child_spawned && now > s.spawn_ms {
                let live = self.native_monsters[&id].clone();
                let packet = ServerPacket::ObjectAttack {
                    info: ObjectAttackInfo {
                        object_id: id,
                        location: live.position.clone(),
                        direction: live.direction,
                        spell: 0,
                        level: 0,
                        attack_type: 2,
                    },
                };
                out.push(ZoneOutbound::ToMany {
                    session_ids: self.native_monster_visible_recipients(id, &live.position),
                    packets: vec![packet],
                });
                let front = offset_point(&live.position, live.direction, 1);
                let (child, events) = self.spawn_special_relative(
                    id,
                    &m.name,
                    &front,
                    &live,
                    s.target_ref.clone(),
                    true,
                    now,
                );
                out.extend(events);
                if let Some(child) = child {
                    s.child_spawned = true;
                    s.sister = Some(child);
                    s.sister_incarnation = self.native_monsters.get(&child).map(|m| m.incarnation);
                }
                s.action_ms = now.saturating_add(300);
                let live = self.native_monsters.get_mut(&id).unwrap();
                live.next_ai_ready_at_ms = live.next_ai_ready_at_ms.max(s.action_ms);
                live.next_attack_ready_at_ms = now.saturating_add(live.attack_speed_ms);
            }
            if s.child && s.target.is_none() {
                let sister_position = s
                    .sister
                    .zip(s.sister_incarnation)
                    .and_then(|(sister, life)| {
                        self.native_monsters.get(&sister).filter(|other| {
                            other.ai == 36 && !other.dead && other.incarnation == life
                        })
                    })
                    .map(|other| other.position.clone());
                if let Some(position) = sister_position
                    .filter(|position| zone_tile_distance(&m.position, position) <= 2)
                {
                    out.extend(self.yimoogi_move_toward(id, &position, now));
                }
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .yimoogi = Some(s);
        }
        out
    }
    pub(super) fn spawn_special_relative(
        &mut self,
        parent: u32,
        name: &str,
        center: &Point,
        m: &ZoneNativeMonster,
        target: Option<ZoneCombatEntityRef>,
        child: bool,
        now: u64,
    ) -> (Option<u32>, Vec<ZoneOutbound>) {
        let Some(t) = crystal_monster_by_name(name) else {
            return (None, Vec::new());
        };
        // Zone collision is authoritative: no stacking parent, sisters, or snakes.
        let position = (0..=3)
            .flat_map(|r| (-r..=r).flat_map(move |x| (-r..=r).map(move |y| (x, y))))
            .map(|(x, y)| Point {
                x: center.x + x,
                y: center.y + y,
            })
            .find(|p| self.can_native_monster_occupy(0, p));
        let Some(position) = position else {
            return (None, Vec::new());
        };
        let id = self.unique_object_id(0);
        let spawn = ZoneMonsterSpawn {
            object_id: id,
            name: t.name.clone(),
            name_colour_argb: -1,
            image: t.image,
            ai: if child { 36 } else { t.ai },
            disposition: m.disposition,
            level: t.level,
            max_hp: t.hp.max(1),
            hp: t.hp.max(1),
            experience: t.experience,
            move_speed_ms: u64::from(t.move_speed),
            attack_speed_ms: u64::from(t.attack_speed),
            friendly_guild: m.friendly_guild.clone(),
            defense: super::super::types::ZoneMonsterDefense::from_crystal_template(&t),
            position,
            direction: m.direction,
            respawn: None,
            drops: if mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name)
                .is_some_and(|map| map.no_drop_monster)
            {
                Vec::new()
            } else {
                crate::runtime::drops::zone_ground_drop_snapshots_for_monster_at_tick(
                    id,
                    &t.name,
                    now / 300,
                )
            },
        };
        let (ok, out) = self.spawn_authoritative_monster(&spawn, now);
        if !ok {
            return (None, out);
        }
        let parent_life = self.native_monsters.get(&parent).map(|m| m.incarnation);
        let inherited = target.clone();
        let m = self.native_monsters.get_mut(&id).unwrap();
        m.next_ai_ready_at_ms = now.saturating_add(2000);
        m.next_attack_ready_at_ms = now.saturating_add(2000);
        if child {
            initialize_yimoogi(m, now);
            let s = m.special_ai.as_mut().unwrap().yimoogi.as_mut().unwrap();
            s.child = true;
            s.sister = Some(parent);
            s.sister_incarnation = parent_life;
            s.target = target.as_ref().map(|r| r.object_id());
            s.target_ref = target.clone();
            s.action_ms = now.saturating_add(2000);
        } else {
            m.special_ai
                .get_or_insert_with(Default::default)
                .stage_summons = Some(StageSummonState {
                stage: 0,
                slave_object_ids: Vec::new(),
                parent_object_id: Some(parent),
                inherited_target_object_id: target.as_ref().map(|r| r.object_id()),
                target_bound: true,
                parent_incarnation: parent_life,
                slave_lives: BTreeMap::new(),
            });
        }
        if let Some(reference) = inherited {
            self.set_native_entity_target(id, &reference, now);
        }
        (Some(id), out)
    }
    fn yimoogi_move_toward(&mut self, id: u32, target: &Point, now: u64) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        if now <= m.next_ai_ready_at_ms
            || native_monster_control_active(&m, now)
            || !super::great_fox_ai::great_fox_movement_ready(&m, now)
        {
            return Vec::new();
        }
        let Some(dir) = zone_direction_toward(&m.position, target) else {
            return Vec::new();
        };
        let dest = offset_point(&m.position, dir, 1);
        if !self.can_native_monster_occupy(id, &dest) {
            return Vec::new();
        }
        let old: BTreeSet<_> = self
            .native_monster_visible_recipients(id, &m.position)
            .into_iter()
            .collect();
        let live = self.native_monsters.get_mut(&id).unwrap();
        live.position = dest.clone();
        live.direction = dir;
        live.next_ai_ready_at_ms = now.saturating_add(live.move_speed_ms.max(300));
        let packet = ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id: id,
                position: dest.clone(),
                direction: dir,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        let mut out = self.diff_all_zone_object_visibility();
        let recipients = self
            .native_monster_visible_recipients(id, &dest)
            .into_iter()
            .filter(|sid| old.contains(sid))
            .collect();
        out.push(ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![packet],
        });
        out
    }
    pub(super) fn try_tick_yimoogi(&mut self, id: u32, now: u64) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if m.ai != 36 {
            return None;
        }
        if m.dead || !m.hostile_to_player {
            return Some(Vec::new());
        }
        initialize_yimoogi(self.native_monsters.get_mut(&id)?, now);
        let mut s = self.native_monsters[&id]
            .special_ai
            .as_ref()?
            .yimoogi
            .clone()?;
        let Some(target) = self.yimoogi_entity_target(id, &m, now) else {
            if let Some((sister, life)) = s.sister.zip(s.sister_incarnation) {
                if let Some(other) = self
                    .native_monsters
                    .get_mut(&sister)
                    .filter(|m| m.ai == 36 && m.incarnation == life)
                    .and_then(|m| m.special_ai.as_mut())
                    .and_then(|s| s.yimoogi.as_mut())
                {
                    other.target = None;
                    other.target_ref = None;
                }
            }
            s.target = None;
            s.target_ref = None;
            self.native_monsters
                .get_mut(&id)?
                .special_ai
                .as_mut()?
                .yimoogi = Some(s);
            return Some(Vec::new());
        };
        let dx = (target.position.x - m.position.x).abs();
        let dy = (target.position.y - m.position.y).abs();
        let melee =
            dx.max(dy) > 0 && dx.max(dy) <= 2 && (dx.max(dy) <= 1 || dx == dy || dx % 2 == dy % 2);
        if now <= s.action_ms || now <= m.next_attack_ready_at_ms {
            return if melee {
                Some(Vec::new())
            } else {
                Some(self.yimoogi_move_toward(id, &target.position, now))
            };
        }
        if dx.max(dy) > 7 {
            return Some(self.yimoogi_move_toward(id, &target.position, now));
        }
        let t = crystal_monster_by_name(&m.name)?;
        let damage = zone_roll_stat_range(
            t.min_dc,
            t.max_dc
                .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_DC)),
            now,
            id,
            yimoogi_roll(&mut s, id, now, u64::MAX),
        );
        let ranged = !melee || yimoogi_roll(&mut s, id, now, 5) == 0;
        let poison = ranged && dx.max(dy) <= 4 && yimoogi_roll(&mut s, id, now, 6) == 0;
        let direction = zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
        s.action_ms = now.saturating_add(300);
        let live = self.native_monsters.get_mut(&id)?;
        live.direction = direction;
        live.next_ai_ready_at_ms = live.next_ai_ready_at_ms.max(s.action_ms);
        live.next_attack_ready_at_ms = now
            .saturating_add(live.attack_speed_ms)
            .saturating_add(if ranged { 500 } else { 0 });
        let mut out = Vec::new();
        if !ranged || damage > 0 {
            let packet = if ranged && !poison {
                ServerPacket::ObjectRangeAttack {
                    info: ObjectRangeAttackInfo {
                        object_id: id,
                        location: m.position.clone(),
                        direction,
                        target_id: target.object_id,
                        target: target.position.clone(),
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
                        direction,
                        attack_type: if poison { 1 } else { 0 },
                        spell: 0,
                        level: 0,
                    },
                }
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_combat_recipients(
                    id,
                    target.object_id,
                    &target.position,
                ),
                packets: vec![packet],
            });
        }
        if damage > 0 {
            if poison {
                // PoisonTarget's SC draw precedes resistance and chance=1.
                let _value = t.min_sc
                    + yimoogi_roll(&mut s, id, now, (t.max_sc - t.min_sc + 1).max(1) as u64) as i32;
                let resist = self
                    .native_entity_poison_resist(&target.reference)
                    .clamp(0, 10) as u64;
                if yimoogi_roll(&mut s, id, now, 10) >= resist {
                    let _chance = yimoogi_roll(&mut s, id, now, 1);
                    let source = ZoneCombatEntityRef::Monster {
                        object_id: id,
                        incarnation: m.incarnation,
                    };
                    out.extend(self.apply_native_entity_red_poison(
                        &source,
                        &target.reference,
                        6,
                        2000,
                        now,
                    ));
                }
            } else {
                s.hits.push(YimoogiHit {
                    hit: None,
                    source_ref: Some(ZoneCombatEntityRef::Monster {
                        object_id: id,
                        incarnation: m.incarnation,
                    }),
                    target_ref: Some(target.reference),
                    due: now.saturating_add(if ranged { 500 } else { 300 }),
                    damage,
                    accuracy: if ranged { None } else { Some(t.accuracy) },
                });
            }
        }
        self.native_monsters
            .get_mut(&id)?
            .special_ai
            .as_mut()?
            .yimoogi = Some(s);
        Some(out)
    }
    pub(super) fn tick_yimoogi_hits(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let mut hits = Vec::new();
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.yimoogi.as_mut()) {
                for h in std::mem::take(&mut s.hits) {
                    if now >= h.hit.as_ref().map_or(h.due, |h| h.ready_at_ms) {
                        hits.push(h)
                    } else {
                        s.hits.push(h)
                    }
                }
            }
        }
        let mut out = Vec::new();
        for h in hits {
            if let (Some(source), Some(target)) = (h.source_ref.as_ref(), h.target_ref.as_ref()) {
                let (_, events) = self.resolve_native_entity_hit(
                    source,
                    target,
                    h.damage,
                    if h.accuracy.is_some() {
                        EntityDefence::MACAgility
                    } else {
                        EntityDefence::MAC
                    },
                    false,
                    now,
                );
                out.extend(events);
                continue;
            }
            // Old player-only checkpoints retain the old packet-shaped payload.
            let Some(hit) = h.hit else {
                continue;
            };
            let Some(p) = self.players.get(&hit.target_session_id).filter(|p| {
                p.object_id == hit.target_object_id && !p.dead && !p.chat_profile.in_safe_zone
            }) else {
                continue;
            };
            if h.accuracy.is_some_and(|a| {
                crate::runtime::combat::crystal_accuracy_roll(
                    now,
                    hit.attacker_object_id,
                    p.object_id,
                    p.combat_stats.agility.max(0) as u64 + 1,
                ) > a.max(0) as u64
            }) {
                continue;
            }
            out.extend(self.resolve_pending_native_player_hit(hit, now));
        }
        out
    }
}
