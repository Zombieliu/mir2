//! Crystal MudZombie.cs (AI108) and BoulderSpirit.cs (AI170), shared-player leaf.
//! MudZombie: line DC hits at 550/600ms; ranged MAC hit at 500ms with an extra
//! 500ms attack cooldown. No push: MonsterObject.LineAttack's push defaults false.
//! BoulderSpirit: Die is immediate, CompleteDeath is delayed 300ms, not vice versa.
//! Shared players and eligible native monster targets;
//! Hero targets remain outside the native entity carrier. Green uses its own carrier.
//! Imported names currently have zero DC/MC; do not invent nonzero fallback stats.
use super::entity_combat::{EntityDefence, EntityTargetPurpose, ZoneCombatEntityRef};
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MudBoulderState {
    rng: u64,
    cool_eye: bool,
    target: Option<(SessionId, u32)>,
    hits: Vec<MudHit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    monster_target: Option<(u32, u64)>,
    #[serde(default, skip_serializing_if = "mud_false")]
    target_bound: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    monster_hits: Vec<MudMonsterHit>,
    explosion_at_ms: Option<u64>,
    exploded: bool,
    corpse_expires_at_ms: Option<u64>,
    reward_owner: Option<SessionId>,
    reward_until_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MudHit {
    hit: PendingNativePlayerHit,
    accuracy: i32,
}

/// Native targets never borrow their owner's session as a synthetic victim.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MudMonsterHit {
    source_incarnation: u64,
    target: u32,
    incarnation: u64,
    due: u64,
    damage: i32,
    magic: bool,
    accuracy: i32,
}

#[derive(Clone)]
struct MudTarget {
    session: Option<SessionId>,
    id: u32,
    incarnation: u64,
    position: Point,
}
fn mud_false(value: &bool) -> bool {
    !*value
}

impl MudBoulderState {
    fn roll(&mut self, upper: u64) -> u64 {
        let upper = upper.max(1);
        let threshold = upper.wrapping_neg() % upper;
        loop {
            self.rng = self.rng.wrapping_add(0x9E3779B97F4A7C15);
            let mut v = self.rng;
            v = (v ^ (v >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            v = (v ^ (v >> 27)).wrapping_mul(0x94D049BB133111EB);
            v ^= v >> 31;
            if v >= threshold {
                return v % upper;
            }
        }
    }
}

pub(in crate::runtime::zone) fn initialize_mud_boulder(
    m: &mut ZoneNativeMonster,
    id: u32,
    now: u64,
) {
    if !matches!(m.ai, 108 | 170) {
        return;
    }
    let state = m.special_ai.get_or_insert_with(Default::default);
    if state.mud_boulder.is_some() {
        return;
    }
    let mut s = MudBoulderState {
        rng: now ^ (u64::from(id) << 32) ^ 0x108_170_5EED,
        cool_eye: false,
        target: None,
        hits: Vec::new(),
        monster_target: None,
        target_bound: false,
        monster_hits: Vec::new(),
        explosion_at_ms: None,
        exploded: false,
        corpse_expires_at_ms: None,
        reward_owner: None,
        reward_until_ms: 0,
    };
    s.cool_eye =
        crystal_monster_by_name(&m.name).is_some_and(|t| s.roll(100) < u64::from(t.cool_eye));
    state.mud_boulder = Some(s);
    if m.ai == 108 {
        // MonsterObject.Spawned ActionTime; Boulder ProcessAI bypasses CanAttack.
        m.next_ai_ready_at_ms = m.next_ai_ready_at_ms.max(now.saturating_add(2_000));
    }
}

pub(super) fn mud_boulder_on_damage(
    m: &mut ZoneNativeMonster,
    id: u32,
    owner: Option<&SessionId>,
    now: u64,
) {
    if m.ai != 170 {
        return;
    }
    initialize_mud_boulder(m, id, now);
    let Some(owner) = owner else {
        return;
    };
    let s = m.special_ai.as_mut().unwrap().mud_boulder.as_mut().unwrap();
    if s.reward_owner.is_none() || now > s.reward_until_ms {
        s.reward_owner = Some(owner.clone());
    }
    if s.reward_owner.as_ref() == Some(owner) {
        s.reward_until_ms = now.saturating_add(5_000);
    }
}

/// Central alive->dead hook, including a player/DoT kill before proximity fires.
pub(super) fn mud_boulder_on_death(m: &mut ZoneNativeMonster, id: u32, now: u64) {
    if !matches!(m.ai, 108 | 170) || !m.dead {
        return;
    }
    initialize_mud_boulder(m, id, now);
    let s = m.special_ai.as_mut().unwrap().mud_boulder.as_mut().unwrap();
    // Mud's already launched actions survive attacker death, as ActionList does.
    if m.ai == 170 && !s.exploded && s.explosion_at_ms.is_none() {
        s.explosion_at_ms = Some(now.saturating_add(300));
    }
    s.corpse_expires_at_ms
        .get_or_insert(now.saturating_add(180_000));
    m.control_poison = 0;
    m.control_until_ms = 0;
    m.damage_poison = 0;
    m.damage_poison_value = 0;
    m.damage_poison_next_damage_at_ms = 0;
    m.damage_poison_expires_at_ms = 0;
    m.damage_poison_owner_session_id = None;
    m.damage_poison_owner_object_id = 0;
}

fn mud_player_target(m: &ZoneNativeMonster, p: &ZonePlayer, sight: bool) -> bool {
    let cool_eye = m
        .special_ai
        .as_ref()
        .and_then(|s| s.mud_boulder.as_ref())
        .is_some_and(|s| s.cool_eye);
    !p.dead
        && p.hp > 0
        && !p.chat_profile.in_safe_zone
        && (!sight || !p.hidden || (cool_eye && m.level >= p.level))
        && !m
            .friendly_guild
            .as_deref()
            .zip(p.chat_profile.guild_name.as_deref())
            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
}

impl ZoneRuntime {
    /// Call from the same Leave/replacement-Join/death/revive hooks as other
    /// dedicated projectile modules, before a same-ID new player life exists.
    pub(super) fn clear_mud_boulder_targets(&mut self, object_id: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.mud_boulder.as_mut()) {
                s.hits.retain(|h| h.hit.target_object_id != object_id);
                s.monster_hits.retain(|h| h.target != object_id);
                if s.monster_target.is_some_and(|(id, _)| id == object_id) {
                    s.monster_target = None;
                }
                if s.target.as_ref().is_some_and(|(_, id)| *id == object_id) {
                    s.target = None;
                }
            }
        }
    }

    /// Prevent session refresh and scheduled respawn replacing a dying caster
    /// before its own delayed actions have completed. Despawn removes the state.
    pub(super) fn native_monster_has_pending_mud_boulder(&self, id: u32) -> bool {
        self.native_monsters
            .get(&id)
            .and_then(|m| m.special_ai.as_ref())
            .and_then(|s| s.mud_boulder.as_ref())
            .is_some_and(|s| {
                !s.hits.is_empty() || !s.monster_hits.is_empty() || s.explosion_at_ms.is_some()
            })
    }

    /// Before generic targeting. Always handles AI108/170, including cooldown;
    /// cooling MudZombie follows MoveTo rather than falling into generic melee.
    pub(super) fn try_tick_mud_boulder(&mut self, id: u32, now: u64) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if !matches!(m.ai, 108 | 170) {
            return None;
        }
        if m.ai == 170 || m.dead || !m.hostile_to_player || m.disposition.is_none() {
            return Some(Vec::new());
        }
        initialize_mud_boulder(self.native_monsters.get_mut(&id)?, id, now);
        let m = self.native_monsters[&id].clone();
        if now <= m.next_ai_ready_at_ms {
            return Some(Vec::new());
        }
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return Some(Vec::new());
        };
        let state = m.special_ai.as_ref()?.mud_boulder.as_ref()?;
        let global = self
            .selected_native_entity_target(id, now)
            .map(|t| match t.reference {
                ZoneCombatEntityRef::Player {
                    session_id,
                    object_id,
                    life_generation,
                } => MudTarget {
                    session: Some(session_id),
                    id: object_id,
                    incarnation: life_generation,
                    position: t.position,
                },
                ZoneCombatEntityRef::Monster {
                    object_id,
                    incarnation,
                } => MudTarget {
                    session: None,
                    id: object_id,
                    incarnation,
                    position: t.position,
                },
            });
        // Old local fields may migrate once. After binding, a cleared global
        // target must never be resurrected from an old session/object pair.
        let existing_player = state
            .target
            .as_ref()
            .filter(|_| !state.target_bound)
            .and_then(|(session, object_id)| {
                self.players
                    .get(session)
                    .filter(|p| {
                        p.object_id == *object_id
                            && p.life_generation == 0
                            && mud_player_target(&m, p, false)
                    })
                    .map(|p| MudTarget {
                        session: Some(session.clone()),
                        id: p.object_id,
                        incarnation: 0,
                        position: p.position.clone(),
                    })
            });
        let existing_monster = state
            .monster_target
            .filter(|_| !state.target_bound)
            .and_then(|(target, incarnation)| {
                let source = self.native_entity_monster_ref(id)?;
                let reference = ZoneCombatEntityRef::Monster {
                    object_id: target,
                    incarnation,
                };
                self.native_entity_can_attack(&source, &reference, EntityTargetPurpose::Impact, now)
                    .then(|| {
                        self.native_monsters.get(&target).map(|m| MudTarget {
                            session: None,
                            id: target,
                            incarnation,
                            position: m.position.clone(),
                        })
                    })
                    .flatten()
            });
        let retained = global
            .or(existing_player)
            .or(existing_monster)
            .filter(|target| {
                zone_tile_distance(&m.position, &target.position) <= 16
                    && self.mud_target_reference(target).is_some_and(|reference| {
                        self.native_entity_monster_ref(id).is_some_and(|source| {
                            self.native_entity_can_attack(
                                &source,
                                &reference,
                                EntityTargetPurpose::Impact,
                                now,
                            )
                        })
                    })
            });
        let target = retained.or_else(|| {
            self.mud_targets(id, &m.position, i32::from(t.view_range), true, now)
                .into_iter()
                .filter(|target| {
                    self.mud_target_reference(target).is_some_and(|reference| {
                        self.native_entity_monster_ref(id).is_some_and(|source| {
                            self.native_entity_can_attack(
                                &source,
                                &reference,
                                EntityTargetPurpose::Impact,
                                now,
                            )
                        })
                    })
                })
                .next()
        });
        self.native_monsters
            .get_mut(&id)?
            .special_ai
            .as_mut()?
            .mud_boulder
            .as_mut()?
            .target_bound = true;
        let Some(target) = target else {
            let state = self
                .native_monsters
                .get_mut(&id)?
                .special_ai
                .as_mut()?
                .mud_boulder
                .as_mut()?;
            state.target = None;
            state.monster_target = None;
            return Some(Vec::new());
        };
        let reference = self.mud_target_reference(&target)?;
        if !self.set_native_entity_target(id, &reference, now) {
            return Some(Vec::new());
        }
        let target_id = target.id;
        let target_position = target.position.clone();
        let state = self
            .native_monsters
            .get_mut(&id)?
            .special_ai
            .as_mut()?
            .mud_boulder
            .as_mut()?;
        state.target = target.session.clone().map(|session| (session, target_id));
        state.monster_target = target
            .session
            .is_none()
            .then_some((target.id, target.incarnation));
        let distance = zone_tile_distance(&m.position, &target_position);
        let direction = zone_direction_toward(&m.position, &target_position).unwrap_or(m.direction);
        if distance > i32::from(t.view_range) || now <= m.next_attack_ready_at_ms {
            let destination = offset_point(&m.position, direction, 1);
            if !self.can_native_monster_occupy(id, &destination) {
                return Some(self.turn_native_monster(id, direction, now));
            }
            let before: BTreeSet<_> = self
                .players
                .iter()
                .filter_map(|(sid, p)| p.visible_object_ids.contains(&id).then_some(sid.clone()))
                .collect();
            let live = self.native_monsters.get_mut(&id)?;
            live.position = destination.clone();
            live.direction = direction;
            live.next_ai_ready_at_ms = now.saturating_add(live.move_speed_ms);
            let packet = ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: id,
                    position: destination.clone(),
                    direction,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            let mut out = self.diff_all_zone_object_visibility();
            out.push(ZoneOutbound::ToMany {
                session_ids: self
                    .native_monster_visible_recipients(id, &destination)
                    .into_iter()
                    .filter(|s| before.contains(s))
                    .collect(),
                packets: vec![packet],
            });
            return Some(out);
        }
        let ranged = distance == 0 || distance > 2;
        let damage = self.mud_boulder_damage(id, ranged, now);
        let selected: Vec<_> = if ranged {
            vec![(target, 500)]
        } else {
            (1..=2)
                .filter_map(|step| {
                    let point = offset_point(&m.position, direction, step);
                    self.mud_targets(id, &point, 0, false, now)
                        .into_iter()
                        .next()
                        .map(|target| (target, 500 + step as u64 * 50))
                })
                .collect()
        };
        let live = self.native_monsters.get_mut(&id)?;
        live.direction = direction;
        live.next_ai_ready_at_ms = now.saturating_add(300);
        live.next_attack_ready_at_ms = now
            .saturating_add(live.attack_speed_ms)
            .saturating_add(if ranged { 500 } else { 0 });
        if damage > 0 {
            let state = live.special_ai.as_mut()?.mud_boulder.as_mut()?;
            for (target, delay) in selected {
                if let Some(target_session_id) = target.session {
                    state.hits.push(MudHit {
                        accuracy: t.accuracy,
                        hit: PendingNativePlayerHit {
                            ready_at_ms: now.saturating_add(delay),
                            attacker_object_id: id,
                            attacker_ai: 108,
                            target_session_id,
                            target_object_id: target.id,
                            damage,
                            magic: ranged,
                        },
                    });
                } else {
                    state.monster_hits.push(MudMonsterHit {
                        source_incarnation: m.incarnation,
                        target: target.id,
                        incarnation: target.incarnation,
                        due: now.saturating_add(delay),
                        damage,
                        magic: ranged,
                        accuracy: t.accuracy,
                    });
                }
            }
        }
        let packet = if ranged {
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id: id,
                    location: m.position,
                    direction,
                    target_id,
                    target: target_position.clone(),
                    attack_type: 0,
                    spell: 0,
                    level: 0,
                },
            }
        } else {
            ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: id,
                    location: m.position,
                    direction,
                    spell: 0,
                    level: 0,
                    attack_type: 0,
                },
            }
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        Some(vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(id, target_id, &target_position),
            packets: vec![packet],
        }])
    }

    /// Global Zone tick, before ordinary respawn. Dead actors still complete
    /// pending actions; committed take/flags make duplicate ticks idempotent.
    pub(super) fn tick_mud_boulder_lifecycles(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| matches!(m.ai, 108 | 170).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            initialize_mud_boulder(self.native_monsters.get_mut(&id).unwrap(), id, now);
            let m = self.native_monsters[&id].clone();
            if m.ai == 170 && !m.dead && m.hostile_to_player && m.disposition.is_some() {
                let range = crystal_monster_by_name(&m.name).map_or(0, |t| i32::from(t.view_range));
                if !self
                    .mud_targets(id, &m.position, range, true, now)
                    .is_empty()
                {
                    out.extend(self.kill_triggered_boulder(id, now));
                }
            }
            let s = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .mud_boulder
                .as_mut()
                .unwrap();
            let mut due = Vec::new();
            for h in std::mem::take(&mut s.hits) {
                if now >= h.hit.ready_at_ms {
                    due.push(h);
                } else {
                    s.hits.push(h);
                }
            }
            let mut monster_due = Vec::new();
            for h in std::mem::take(&mut s.monster_hits) {
                if now >= h.due {
                    monster_due.push(h);
                } else {
                    s.monster_hits.push(h);
                }
            }
            let explode = s.explosion_at_ms.is_some_and(|d| now >= d) && !s.exploded;
            if explode {
                s.exploded = true;
                s.explosion_at_ms = None;
            }
            for h in due {
                out.extend(self.resolve_mud_boulder_hit(h, now, true));
            }
            for h in monster_due {
                out.extend(self.resolve_mud_monster_hit(id, h, now, true));
            }
            if explode {
                out.extend(self.explode_boulder(id, now));
            }
            let expired = self.native_monsters[&id]
                .special_ai
                .as_ref()
                .unwrap()
                .mud_boulder
                .as_ref()
                .unwrap()
                .corpse_expires_at_ms
                .is_some_and(|d| now >= d);
            if expired {
                self.retire_native_source_object(id);
                self.objects.remove(&id);
                self.object_grid.remove(&id);
                self.dead_object_ids.remove(&id);
                self.revived_object_ids.remove(&id);
                self.harvested_object_ids.remove(&id);
                self.removed_object_ids.insert(id);
                if self.native_monster_respawns.contains_key(&id) {
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .mud_boulder
                        .as_mut()
                        .unwrap()
                        .corpse_expires_at_ms = None;
                } else {
                    self.native_monsters.remove(&id);
                }
                let recipients = self
                    .players
                    .iter_mut()
                    .filter_map(|(session, p)| {
                        p.visible_object_ids.remove(&id).then_some(session.clone())
                    })
                    .collect::<Vec<_>>();
                if !recipients.is_empty() {
                    out.push(ZoneOutbound::ToMany {
                        session_ids: recipients,
                        packets: vec![ServerPacket::ObjectRemove { object_id: id }],
                    });
                }
            }
        }
        out
    }

    fn mud_boulder_damage(&mut self, id: u32, magic: bool, now: u64) -> i32 {
        let m = &self.native_monsters[&id];
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return 0;
        };
        let (min, max, low, high) = if magic {
            (t.min_mc, t.max_mc, CRYSTAL_STAT_MIN_MC, CRYSTAL_STAT_MAX_MC)
        } else {
            (t.min_dc, t.max_dc, CRYSTAL_STAT_MIN_DC, CRYSTAL_STAT_MAX_DC)
        };
        let (min, max) = horned_stat_range(
            m,
            if magic {
                HornedStat::Mc
            } else {
                HornedStat::Dc
            },
            min,
            max,
            now,
        );
        let min = min
            .saturating_add(zone_native_monster_buff_stat_total(m, low))
            .max(0);
        let max = max
            .saturating_add(zone_native_monster_buff_stat_total(m, high))
            .max(min);
        min.saturating_add(
            self.mud_boulder_roll(id, (i64::from(max) - i64::from(min) + 1) as u64) as i32,
        )
    }

    fn mud_boulder_roll(&mut self, id: u32, upper: u64) -> u64 {
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .mud_boulder
            .as_mut()
            .unwrap()
            .roll(upper)
    }

    fn resolve_mud_boulder_hit(&mut self, h: MudHit, now: u64, poison: bool) -> Vec<ZoneOutbound> {
        let id = h.hit.attacker_object_id;
        let Some(m) = self.native_monsters.get(&id) else {
            return Vec::new();
        };
        let Some(p) = self.players.get(&h.hit.target_session_id) else {
            return Vec::new();
        };
        if p.object_id != h.hit.target_object_id || !mud_player_target(m, p, false) {
            return Vec::new();
        }
        let (session, target_id, resist, agility) = (
            p.session_id.clone(),
            p.object_id,
            p.combat_stats.poison_resist.clamp(0, 10),
            p.combat_stats.agility.max(0),
        );
        if !h.hit.magic && self.mud_boulder_roll(id, agility as u64 + 1) > h.accuracy.max(0) as u64
        {
            return Vec::new();
        }
        let mut out = self.resolve_pending_native_player_hit(h.hit, now);
        if poison && out.iter().any(|o| matches!(o,ZoneOutbound::PlayerDamaged { session_id, damage, .. } if session_id==&session && *damage>0)) {
            // MonsterObject.PoisonTarget: SC roll, resist, 1/5, then HumanObject
            // ApplyPoison's second resistance check. The admitted shared carrier
            // owns eight 2000ms SC damage ticks; it must not roll resistance again.
            let sc = crystal_monster_by_name(&self.native_monsters[&id].name)
                .map(|t| (t.min_sc.max(0), t.max_sc.max(t.min_sc).max(0)))
                .map_or(0, |(min,max)| min.saturating_add(self.mud_boulder_roll(id,(i64::from(max)-i64::from(min)+1) as u64) as i32));
            if self.mud_boulder_roll(id,10)>=resist as u64 && self.mud_boulder_roll(id,5)==0 && self.mud_boulder_roll(id,10)>=resist as u64 {
                out.extend(self.apply_native_player_green_poison(id,&session,target_id,sc,8,2_000,now));
            }
        }
        out
    }

    fn kill_triggered_boulder(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let m = &self.native_monsters[&id];
        let hp = m.hp;
        let owner = m.special_ai.as_ref().unwrap().mud_boulder.as_ref().unwrap();
        let owner = owner
            .reward_owner
            .as_ref()
            .filter(|_| now <= owner.reward_until_ms)
            .filter(|s| self.players.get(*s).is_some_and(|p| !p.dead && p.hp > 0))
            .cloned();
        let Some((_, _, killed, name, experience, position, direction, _, drops, boss_audit)) =
            self.apply_native_monster_damage(id, hp, owner.as_ref(), now)
        else {
            return Vec::new();
        };
        if !killed {
            return Vec::new();
        }
        let packets = vec![
            ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: id,
                    location: position.clone(),
                    direction,
                    kind: 0,
                },
            },
            ServerPacket::ObjectPoisoned {
                object_id: id,
                poison: 0,
            },
        ];
        self.apply_zone_object_packets(&packets, now);
        let mut out = vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &position),
            packets,
        }];
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
                    monster_object_id: id,
                    killed_at_ms: now,
                    monster_name: name,
                    experience,
                    drops,
                    boss_audit,
                },
            ));
        }
        out
    }

    fn explode_boulder(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return Vec::new();
        };
        // CompleteDeath needSight=false, targets/positions are selected now.
        let targets = self.mud_targets(id, &m.position, i32::from(t.view_range), false, now);
        let mut out = Vec::new();
        for target in targets {
            let damage = self.mud_boulder_damage(id, false, now);
            if damage == 0 {
                break;
            }
            let Some(session) = target.session else {
                out.extend(self.resolve_mud_monster_hit(
                    id,
                    MudMonsterHit {
                        source_incarnation: m.incarnation,
                        target: target.id,
                        incarnation: target.incarnation,
                        due: now,
                        damage,
                        magic: false,
                        accuracy: t.accuracy,
                    },
                    now,
                    false,
                ));
                continue;
            };
            let target_id = target.id;
            out.extend(self.resolve_mud_boulder_hit(
                MudHit {
                    accuracy: t.accuracy,
                    hit: PendingNativePlayerHit {
                        ready_at_ms: now,
                        attacker_object_id: id,
                        attacker_ai: 170,
                        target_session_id: session,
                        target_object_id: target_id,
                        damage,
                        magic: false,
                    },
                },
                now,
                false,
            ));
        }
        out
    }
    fn mud_targets(
        &self,
        id: u32,
        center: &Point,
        radius: i32,
        sight: bool,
        now: u64,
    ) -> Vec<MudTarget> {
        let m = &self.native_monsters[&id];
        let mut targets: Vec<_> = self
            .players
            .values()
            .filter(|p| {
                mud_player_target(m, p, sight) && zone_tile_distance(center, &p.position) <= radius
            })
            .map(|p| MudTarget {
                session: Some(p.session_id.clone()),
                id: p.object_id,
                incarnation: p.life_generation,
                position: p.position.clone(),
            })
            .collect();
        targets.extend(
            self.native_entity_monster_targets(
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
            .into_iter()
            .filter_map(|t| match t.reference {
                ZoneCombatEntityRef::Monster {
                    object_id,
                    incarnation,
                } => Some(MudTarget {
                    session: None,
                    id: object_id,
                    incarnation,
                    position: t.position,
                }),
                _ => None,
            }),
        );
        targets.sort_by_key(|t| {
            (
                zone_tile_distance(center, &t.position),
                t.position.y,
                t.position.x,
                t.id,
            )
        });
        targets
    }
    fn mud_target_reference(&self, target: &MudTarget) -> Option<ZoneCombatEntityRef> {
        if let Some(session) = &target.session {
            self.native_entity_player_ref(session).filter(|r|matches!(r,ZoneCombatEntityRef::Player{object_id,life_generation,..}if *object_id==target.id && *life_generation==target.incarnation))
        } else {
            Some(ZoneCombatEntityRef::Monster {
                object_id: target.id,
                incarnation: target.incarnation,
            })
        }
    }
    fn resolve_mud_monster_hit(
        &mut self,
        id: u32,
        h: MudMonsterHit,
        now: u64,
        poison: bool,
    ) -> Vec<ZoneOutbound> {
        let source = ZoneCombatEntityRef::Monster {
            object_id: id,
            incarnation: h.source_incarnation,
        };
        let target = ZoneCombatEntityRef::Monster {
            object_id: h.target,
            incarnation: h.incarnation,
        };
        if !self.native_entity_can_attack(&source, &target, EntityTargetPurpose::Impact, now) {
            return Vec::new();
        }
        let (damage, mut out) = self.resolve_native_entity_hit(
            &source,
            &target,
            h.damage,
            if h.magic {
                EntityDefence::MAC
            } else {
                EntityDefence::ACAgility
            },
            false,
            now,
        );
        if poison && damage > 0 {
            let resist = self.native_entity_poison_resist(&target);
            let sc = crystal_monster_by_name(&self.native_monsters[&id].name)
                .map(|t| (t.min_sc.max(0), t.max_sc.max(t.min_sc).max(0)))
                .map_or(0, |(a, b)| {
                    a + self.mud_boulder_roll(id, (i64::from(b) - i64::from(a) + 1) as u64) as i32
                });
            // Monster.ApplyPoison has no HumanObject second resistance gate.
            if self.mud_boulder_roll(id, 10) >= resist as u64 && self.mud_boulder_roll(id, 5) == 0 {
                out.extend(
                    self.apply_native_entity_green_poison(&source, &target, sc, 8, 2000, now),
                );
            }
        }
        out
    }
}
