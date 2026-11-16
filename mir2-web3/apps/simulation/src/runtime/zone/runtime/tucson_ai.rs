//! Crystal TucsonGeneral131: rage rocks retain absolute spawn/start/expiry clocks.
use super::entity_combat::{EntityDefence, EntityTargetPurpose, ZoneCombatEntityRef};
use super::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct TucsonState {
    rage_ms: u64,
    action_ms: u64,
    sequence: u64,
    hits: Vec<TucsonHit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entity_hits: Vec<TucsonEntityHit>,
    rocks: Vec<TucsonRock>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TucsonEntityHit {
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
    due_ms: u64,
    damage: i32,
    stomp: bool,
    defence: EntityDefence,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TucsonHit {
    hit: PendingNativePlayerHit,
    stomp: bool,
    accuracy: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TucsonRock {
    #[serde(default)]
    source: Option<ZoneCombatEntityRef>,
    id: u32,
    point: Point,
    spawn_ms: u64,
    start_ms: u64,
    expire_ms: u64,
    next_ms: u64,
    value: i32,
    spawned: bool,
}
pub(in crate::runtime::zone) fn initialize_tucson(m: &mut ZoneNativeMonster) {
    if m.ai == 131 {
        m.special_ai
            .get_or_insert_with(Default::default)
            .tucson
            .get_or_insert_with(Default::default);
    }
}
fn tucson_roll(s: &mut TucsonState, id: u32, now: u64, n: u64) -> u64 {
    s.sequence = s.sequence.wrapping_add(1);
    let mut x = now ^ u64::from(id) ^ s.sequence.wrapping_mul(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    (x ^ (x >> 31)) % n.max(1)
}
impl ZoneRuntime {
    pub(super) fn clear_tucson_target_life(&mut self, target: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.tucson.as_mut()) {
                s.hits.retain(|h| h.hit.target_object_id != target);
                s.entity_hits.retain(|h| h.target.object_id() != target);
            }
        }
    }
    pub(super) fn native_monster_has_pending_tucson(&self, id: u32) -> bool {
        self.native_monsters
            .get(&id)
            .and_then(|m| m.special_ai.as_ref())
            .and_then(|s| s.tucson.as_ref())
            .is_some_and(|s| !s.rocks.is_empty() || !s.hits.is_empty() || !s.entity_hits.is_empty())
    }
    fn tucson_targets(
        &self,
        m: &ZoneNativeMonster,
        point: &Point,
        range: i32,
    ) -> Vec<NativeMonsterTarget> {
        self.players
            .values()
            .filter(|p| {
                !p.dead
                    && !p.chat_profile.in_safe_zone
                    && zone_tile_distance(&p.position, point) <= range
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
            .collect()
    }
    pub(super) fn try_tick_tucson(&mut self, id: u32, now: u64) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if m.ai != 131 {
            return None;
        }
        if m.dead || !m.hostile_to_player {
            return Some(Vec::new());
        }
        initialize_tucson(self.native_monsters.get_mut(&id)?);
        let mut s = self.native_monsters[&id]
            .special_ai
            .as_ref()?
            .tucson
            .clone()?;
        let t = crystal_monster_by_name(&m.name)?;
        let target = self.selected_native_entity_target(id, now).or_else(|| {
            self.native_entity_targets(
                id,
                &m.position,
                i32::from(t.view_range),
                EntityTargetPurpose::Search,
                now,
            )
            .into_iter()
            .next()
        })?;
        self.set_native_entity_target(id, &target.reference, now);
        let source = self.native_entity_monster_ref(id)?;
        if now <= s.action_ms {
            return Some(Vec::new());
        }
        if now <= m.next_attack_ready_at_ms
            || zone_tile_distance(&m.position, &target.position) > i32::from(t.view_range)
        {
            return None;
        }
        let direction = zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
        s.action_ms = now.saturating_add(300);
        let mut attack_ready = now.saturating_add(m.attack_speed_ms);
        let ranged =
            m.position == target.position || zone_tile_distance(&m.position, &target.position) > 2;
        let range_packet;
        let attack_type;
        let target_id;
        if now > s.rage_ms {
            s.rage_ms = now.saturating_add(20000);
            attack_ready = now.saturating_add(8000);
            range_packet = true;
            attack_type = 0;
            target_id = 0;
            let targets =
                self.native_entity_targets(id, &m.position, 10, EntityTargetPurpose::Impact, now);
            let view = i32::from(t.view_range);
            for _ in 0..15 {
                let mut point = Point {
                    x: m.position.x + tucson_roll(&mut s, id, now, (view * 2 + 1) as u64) as i32
                        - view,
                    y: m.position.y + tucson_roll(&mut s, id, now, (view * 2 + 1) as u64) as i32
                        - view,
                };
                if tucson_roll(&mut s, id, now, 3) == 0 && !targets.is_empty() {
                    point = targets[tucson_roll(&mut s, id, now, targets.len() as u64) as usize]
                        .position
                        .clone();
                }
                if point.x == m.position.x
                    || point.y == m.position.y
                    || self.collision.is_blocked(&point)
                {
                    continue;
                }
                let start = tucson_roll(&mut s, id, now, 5000);
                let max = t
                    .max_dc
                    .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_DC));
                let value = if max > t.min_dc {
                    t.min_dc.saturating_add(tucson_roll(&mut s,id,now,(max-t.min_dc)as u64)as i32)
                } else {
                    t.min_dc
                };
                s.rocks.push(TucsonRock {
                    source: Some(source.clone()),
                    id: self.unique_object_id(0),
                    point,
                    spawn_ms: now.saturating_add(start),
                    start_ms: now.saturating_add(1000 + start),
                    expire_ms: now.saturating_add(2000 + start),
                    next_ms: 0,
                    value,
                    spawned: false,
                });
            }
        } else {
            let melee = !ranged && tucson_roll(&mut s, id, now, 4) > 0;
            let stomp = melee && tucson_roll(&mut s, id, now, 3) == 0;
            let projectile = !melee && tucson_roll(&mut s, id, now, 4) > 0;
            range_packet = !melee;
            attack_type = if melee {
                if stomp {
                    1
                } else {
                    0
                }
            } else if projectile {
                1
            } else {
                2
            };
            target_id = target.object_id;
            let (min, max) = if melee {
                if stomp {
                    (
                        t.min_mc,
                        t.max_mc.saturating_add(zone_native_monster_buff_stat_total(
                            &m,
                            CRYSTAL_STAT_MAX_MC,
                        )),
                    )
                } else {
                    (
                        t.min_dc,
                        t.max_dc.saturating_add(zone_native_monster_buff_stat_total(
                            &m,
                            CRYSTAL_STAT_MAX_DC,
                        )),
                    )
                }
            } else {
                (
                    t.min_sc,
                    t.max_sc
                        .saturating_add(zone_native_monster_buff_stat_total(
                            &m,
                            CRYSTAL_STAT_MAX_SC,
                        ))
                        .saturating_mul(if projectile { 1 } else { 2 }),
                )
            };
            let damage =
                zone_roll_stat_range(min, max, now, id, tucson_roll(&mut s, id, now, u64::MAX));
            if damage > 0 {
                s.entity_hits.push(TucsonEntityHit {
                    source,
                    target: target.reference.clone(),
                    due_ms: now.saturating_add(if projectile {
                        500 + zone_tile_distance(&m.position, &target.position) as u64 * 50
                    } else {
                        500
                    }),
                    damage,
                    stomp,
                    defence: if projectile {
                        EntityDefence::MACAgility
                    } else {
                        EntityDefence::ACAgility
                    },
                });
            }
        }
        let packet = if range_packet {
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id: id,
                    location: m.position.clone(),
                    direction,
                    target_id,
                    target: target.position.clone(),
                    attack_type,
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
                    attack_type,
                    spell: 0,
                    level: 0,
                },
            }
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        let live = self.native_monsters.get_mut(&id)?;
        live.direction = direction;
        live.next_ai_ready_at_ms = live.next_ai_ready_at_ms.max(s.action_ms);
        live.next_attack_ready_at_ms = attack_ready;
        live.special_ai.as_mut()?.tucson = Some(s);
        Some(vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(
                id,
                target.object_id,
                &target.position,
            ),
            packets: vec![packet],
        }])
    }
    pub(super) fn tick_tucson_delays(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self.native_monsters.keys().copied().collect();
        let mut out = Vec::new();
        for id in ids {
            let m = self.native_monsters[&id].clone();
            let Some(mut s) = m.special_ai.as_ref().and_then(|s| s.tucson.clone()) else {
                continue;
            };
            let mut hits = Vec::new();
            let mut entity_hits = Vec::new();
            for h in std::mem::take(&mut s.entity_hits) {
                if now >= h.due_ms {
                    entity_hits.push(h);
                } else {
                    s.entity_hits.push(h);
                }
            }
            for h in std::mem::take(&mut s.hits) {
                if now >= h.hit.ready_at_ms {
                    hits.push(h)
                } else {
                    s.hits.push(h)
                }
            }
            for h in entity_hits {
                if !self.native_entity_can_attack(
                    &h.source,
                    &h.target,
                    EntityTargetPurpose::Impact,
                    now,
                ) {
                    continue;
                }
                let targets = if h.stomp {
                    self.native_entity_targets(id, &m.position, 3, EntityTargetPurpose::Impact, now)
                        .into_iter()
                        .map(|t| t.reference)
                        .collect::<Vec<_>>()
                } else {
                    vec![h.target]
                };
                for target in targets {
                    let resist = self.native_entity_poison_resist(&target).max(0) as u64;
                    let (damage, events) = self.resolve_native_entity_hit(
                        &h.source, &target, h.damage, h.defence, false, now,
                    );
                    out.extend(events);
                    if h.stomp
                        && damage > 0
                        && tucson_roll(&mut s, id, now, 10) >= resist
                        && tucson_roll(&mut s, id, now, 3) == 0
                    {
                        out.extend(
                            self.apply_native_entity_paralysis(&h.source, &target, 5, 1000, now),
                        );
                    }
                }
            }
            // Existing persisted player hits remain readable; new casts never write this queue.
            for h in hits {
                let Some(p) = self.players.get(&h.hit.target_session_id).filter(|p| {
                    p.object_id == h.hit.target_object_id && !p.dead && !p.chat_profile.in_safe_zone
                }) else {
                    continue;
                };
                let targets = if h.stomp {
                    self.tucson_targets(&m, &m.position, 3)
                } else {
                    self.tucson_targets(&m, &p.position, 0)
                        .into_iter()
                        .filter(|t| t.object_id == h.hit.target_object_id)
                        .collect()
                };
                for target in targets {
                    let p = &self.players[&target.session_id];
                    if crate::runtime::combat::crystal_accuracy_roll(
                        now,
                        id,
                        p.object_id,
                        p.combat_stats.agility.max(0) as u64 + 1,
                    ) > h.accuracy.max(0) as u64
                    {
                        continue;
                    }
                    let hp = p.hp;
                    let resist = p.combat_stats.poison_resist.max(0) as u64;
                    let mut hit = h.hit.clone();
                    hit.target_session_id = target.session_id.clone();
                    hit.target_object_id = target.object_id;
                    out.extend(self.resolve_pending_native_player_hit(hit, now));
                    if h.stomp
                        && self
                            .players
                            .get(&target.session_id)
                            .is_some_and(|p| !p.dead && p.hp < hp)
                        && tucson_roll(&mut s, id, now, 10) >= resist
                        && tucson_roll(&mut s, id, now, 3) == 0
                        && tucson_roll(&mut s, id, now, 10) >= resist
                    {
                        out.extend(self.apply_native_player_status_poison(
                            &target.session_id,
                            CRYSTAL_POISON_PARALYSIS,
                            5000,
                            now,
                        ));
                    }
                }
            }
            for mut r in std::mem::take(&mut s.rocks) {
                // Bind legacy rows once on restore; all newly created spells
                // capture caster identity at cast time, not at their first tick.
                if r.source.is_none() {
                    r.source = self.native_entity_monster_ref(id);
                }
                if r.source != self.native_entity_monster_ref(id) || !self.objects.contains_key(&id)
                {
                    if r.spawned {
                        self.objects.remove(&r.id);
                        self.object_grid.remove(&r.id);
                        out.extend(self.diff_all_zone_object_visibility());
                    }
                    continue;
                }
                if now < r.spawn_ms {
                    s.rocks.push(r);
                    continue;
                }
                if now > r.expire_ms {
                    if r.spawned {
                        self.objects.remove(&r.id);
                        self.object_grid.remove(&r.id);
                        out.extend(self.diff_all_zone_object_visibility());
                    }
                    continue;
                }
                if !r.spawned {
                    r.spawned = true;
                    let packet = ServerPacket::ObjectSpell {
                        info: ObjectSpellInfo {
                            object_id: r.id,
                            location: r.point.clone(),
                            spell: Spell::TucsonGeneralRock,
                            direction: MirDirection::Up,
                            param: false,
                        },
                    };
                    self.object_grid.insert(r.id, &r.point);
                    self.objects.insert(
                        r.id,
                        ZoneObject {
                            object_id: r.id,
                            position: r.point.clone(),
                            packet,
                            health: None,
                            mana: None,
                            expires_at_ms: Some(r.expire_ms.saturating_add(1)),
                            buffs: BTreeMap::new(),
                        },
                    );
                    out.extend(self.diff_all_zone_object_visibility());
                }
                if now >= r.next_ms {
                    r.next_ms = now.saturating_add(1000);
                    if now >= r.start_ms && self.objects.contains_key(&id) && r.value > 0 {
                        for target in self.native_entity_targets(
                            id,
                            &r.point,
                            0,
                            EntityTargetPurpose::Impact,
                            now,
                        ) {
                            if let Some(source) = r.source.as_ref() {
                                // SpellObject.TucsonGeneralRock uses Struck(AC),
                                // distinct from a caster Attacked hit (StoneTrap is immune).
                                let (_, events) = self.resolve_native_entity_hit(
                                    source,
                                    &target.reference,
                                    r.value,
                                    EntityDefence::AC,
                                    true,
                                    now,
                                );
                                out.extend(events);
                            }
                        }
                    }
                }
                s.rocks.push(r);
            }
            s.entity_hits.retain(|h| {
                self.native_entity_can_attack(
                    &h.source,
                    &h.target,
                    EntityTargetPurpose::Impact,
                    now,
                )
            });
            s.hits.retain(|h| {
                self.players
                    .get(&h.hit.target_session_id)
                    .is_some_and(|p| p.object_id == h.hit.target_object_id && !p.dead)
            });
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .tucson = Some(s);
        }
        out
    }
}
