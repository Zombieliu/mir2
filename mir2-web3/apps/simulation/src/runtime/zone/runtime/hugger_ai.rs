//! Crystal Hugger70 / PoisonHugger69. No private-session World is advanced.
use super::entity_combat::{EntityDefence, EntityTargetPurpose, ZoneCombatEntityRef};
use super::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HuggerState {
    fuse_ms: u64,
    action_ms: u64,
    rng: u64,
    death_ms: Option<u64>,
    finished: bool,
    #[serde(default)]
    hits: Vec<HuggerHit>,
    #[serde(default)]
    excluded_lives: BTreeSet<u32>,
    #[serde(default)]
    reward_owner: Option<SessionId>,
    #[serde(default)]
    reward_until_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HuggerHit {
    due: u64,
    session: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reference: Option<ZoneCombatEntityRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<ZoneCombatEntityRef>,
    target: u32,
    damage: i32,
    poison: bool,
    #[serde(default)]
    roll_on_resolve: bool,
}
#[derive(Clone)]
struct HuggerTarget {
    session: Option<SessionId>,
    target: u32,
    reference: Option<ZoneCombatEntityRef>,
    position: Point,
}
impl HuggerState {
    fn roll(&mut self, n: u64) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E3779B97F4A7C15);
        let mut x = self.rng;
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
        (x ^ (x >> 31)) % n.max(1)
    }
}
pub(in crate::runtime::zone) fn initialize_hugger(m: &mut ZoneNativeMonster, id: u32, now: u64) {
    if !matches!(m.ai, 69 | 70) {
        return;
    }
    m.special_ai
        .get_or_insert_with(Default::default)
        .hugger
        .get_or_insert(HuggerState {
            fuse_ms: now.saturating_add(300000),
            action_ms: now.saturating_add(2000),
            rng: u64::from(id) ^ now,
            death_ms: None,
            finished: false,
            hits: Vec::new(),
            excluded_lives: BTreeSet::new(),
            reward_owner: None,
            reward_until_ms: 0,
        });
}
pub(super) fn hugger_on_damage(
    m: &mut ZoneNativeMonster,
    id: u32,
    owner: Option<&SessionId>,
    now: u64,
) {
    if !matches!(m.ai, 69 | 70) {
        return;
    }
    initialize_hugger(m, id, now);
    let Some(owner) = owner else {
        return;
    };
    let s = m.special_ai.as_mut().unwrap().hugger.as_mut().unwrap();
    if s.reward_owner.is_none() || now > s.reward_until_ms {
        s.reward_owner = Some(owner.clone());
    }
    if s.reward_owner.as_ref() == Some(owner) {
        s.reward_until_ms = now.saturating_add(5000);
    }
}
impl ZoneRuntime {
    fn hugger_targets(
        &self,
        id: u32,
        m: &ZoneNativeMonster,
        sight: bool,
        now: u64,
    ) -> Vec<HuggerTarget> {
        let mut players: Vec<_> = self
            .players
            .values()
            .filter(|p| {
                !p.dead
                    && !p.chat_profile.in_safe_zone
                    && (!sight || !p.hidden)
                    && zone_tile_distance(&p.position, &m.position) <= 1
                    && !m
                        .friendly_guild
                        .as_deref()
                        .zip(p.chat_profile.guild_name.as_deref())
                        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            })
            .collect();
        players.sort_by_key(|p| {
            (
                zone_tile_distance(&p.position, &m.position),
                p.position.y,
                p.position.x,
                p.object_id,
            )
        });
        let mut targets: Vec<_> = players
            .into_iter()
            .map(|p| HuggerTarget {
                session: Some(p.session_id.clone()),
                target: p.object_id,
                reference: None,
                position: p.position.clone(),
            })
            .collect();
        targets.extend(
            self.native_entity_monster_targets(
                id,
                &m.position,
                1,
                if sight {
                    EntityTargetPurpose::VisibleImpact
                } else {
                    EntityTargetPurpose::Impact
                },
                now,
            )
            .into_iter()
            .map(|t| HuggerTarget {
                session: None,
                target: t.object_id,
                reference: Some(t.reference),
                position: t.position,
            }),
        );
        targets.sort_by_key(|t| {
            (
                zone_tile_distance(&m.position, &t.position),
                t.position.y,
                t.position.x,
                t.target,
            )
        });
        targets
    }

    /// Root must call immediately after each authoritative alive->dead change,
    /// including player strikes and poison, before any player can move again.
    pub(super) fn hugger_on_death(&mut self, id: u32, now: u64) {
        let Some(m) = self.native_monsters.get(&id).cloned() else {
            return;
        };
        if !matches!(m.ai, 69 | 70) || !m.dead {
            return;
        }
        initialize_hugger(self.native_monsters.get_mut(&id).unwrap(), id, now);
        if self.native_monsters[&id]
            .special_ai
            .as_ref()
            .unwrap()
            .hugger
            .as_ref()
            .unwrap()
            .death_ms
            .is_some()
        {
            return;
        }
        let targets = if m.ai == 69 {
            self.hugger_targets(id, &m, true, now)
        } else {
            Vec::new()
        };
        let template = crystal_monster_by_name(&m.name);
        let s = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .hugger
            .as_mut()
            .unwrap();
        s.death_ms = Some(now.saturating_add(500));
        for target in targets {
            let damage = template.as_ref().map_or(0, |t| {
                zone_roll_stat_range(
                    t.min_dc,
                    t.max_dc.saturating_add(zone_native_monster_buff_stat_total(
                        &m,
                        CRYSTAL_STAT_MAX_DC,
                    )),
                    now,
                    id,
                    s.roll(u64::MAX),
                )
            });
            s.hits.push(HuggerHit {
                due: now.saturating_add(500),
                session: target.session,
                target: target.target,
                reference: target.reference,
                source: Some(ZoneCombatEntityRef::Monster {
                    object_id: id,
                    incarnation: m.incarnation,
                }),
                damage,
                poison: true,
                roll_on_resolve: false,
            });
        }
    }
    pub(super) fn native_monster_has_pending_hugger_death(&self, id: u32) -> bool {
        self.native_monsters
            .get(&id)
            .and_then(|m| m.special_ai.as_ref())
            .and_then(|s| s.hugger.as_ref())
            .is_some_and(|s| s.death_ms.is_some() && !s.finished)
    }
    pub(super) fn clear_hugger_target_life(&mut self, target: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.hugger.as_mut()) {
                s.hits.retain(|h| h.target != target);
                if s.reward_owner.as_ref().is_some_and(|owner| {
                    self.players
                        .get(owner)
                        .is_none_or(|p| p.object_id == target)
                }) {
                    s.reward_owner = None;
                    s.reward_until_ms = 0;
                }
                if s.death_ms.is_some() && !s.finished {
                    s.excluded_lives.insert(target);
                }
            }
        }
    }
    /// Dispatch before generic ranged/decoy handling, after shared controls.
    /// None allows AI70 melee or ordinary pursuit when appropriate.
    pub(super) fn try_tick_hugger(&mut self, id: u32, now: u64) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if !matches!(m.ai, 69 | 70) {
            return None;
        }
        if m.dead || !m.hostile_to_player {
            return Some(Vec::new());
        }
        initialize_hugger(self.native_monsters.get_mut(&id)?, id, now);
        let s = self.native_monsters[&id]
            .special_ai
            .as_ref()?
            .hugger
            .as_ref()?;
        if now <= s.action_ms || now <= m.next_attack_ready_at_ms {
            return Some(Vec::new());
        }
        let fuse_ms = s.fuse_ms;
        let chosen = self.selected_native_entity_target(id, now).or_else(|| {
            self.native_entity_targets(
                id,
                &m.position,
                crystal_monster_by_name(&m.name).map_or(0, |t| i32::from(t.view_range)),
                EntityTargetPurpose::Search,
                now,
            )
            .into_iter()
            .next()
        });
        if let Some(chosen) = &chosen {
            self.set_native_entity_target(id, &chosen.reference, now);
        }
        let has_target = chosen.is_some();
        let target = chosen.as_ref().and_then(|t| match &t.reference {
            ZoneCombatEntityRef::Player { session_id, .. } => Some(NativeMonsterTarget {
                session_id: session_id.clone(),
                object_id: t.object_id,
                position: t.position.clone(),
            }),
            _ => None,
        });
        let native = chosen.filter(|t| matches!(t.reference, ZoneCombatEntityRef::Monster { .. }));
        if now <= fuse_ms {
            if let Some(native) = native {
                if m.ai == 70 || zone_tile_distance(&m.position, &native.position) > 1 {
                    return Some(self.tick_hugger_monster_target(
                        id,
                        native.reference,
                        native.position,
                        now,
                    ));
                }
                return Some(self.kill_hugger_native_adjacent(id, now));
            }
        }
        if !has_target
            || now > fuse_ms
            || (m.ai == 69
                && target
                    .as_ref()
                    .is_some_and(|t| zone_tile_distance(&t.position, &m.position) <= 1))
        {
            let s = self.native_monsters[&id]
                .special_ai
                .as_ref()?
                .hugger
                .as_ref()?;
            let reward_owner = s
                .reward_owner
                .as_ref()
                .filter(|owner| {
                    now <= s.reward_until_ms
                        && self
                            .players
                            .get(*owner)
                            .is_some_and(|p| !p.dead && p.hp > 0)
                })
                .cloned();
            let Some((_, _, killed, name, experience, position, direction, _, drops, boss_audit)) =
                self.apply_native_monster_damage(id, m.hp, reward_owner.as_ref(), now)
            else {
                return Some(Vec::new());
            };
            if !killed {
                return Some(Vec::new());
            }
            self.hugger_on_death(id, now);
            let packet = ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: id,
                    location: position.clone(),
                    direction,
                    kind: 0,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            let mut out = vec![ZoneOutbound::ToMany {
                session_ids: self.native_monster_visible_recipients(id, &position),
                packets: vec![packet],
            }];
            if let Some(owner) = reward_owner {
                let player_id = self.players[&owner].object_id;
                let drops =
                    self.spawn_native_monster_drops(&name, &position, player_id, drops, now);
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
            return Some(out);
        }
        let target = target?;
        let distance = zone_tile_distance(&m.position, &target.position);
        if m.ai == 70 || distance > 5 {
            return None;
        }
        let s = self
            .native_monsters
            .get_mut(&id)?
            .special_ai
            .as_mut()?
            .hugger
            .as_mut()?;
        if s.roll(5) != 0 {
            return None;
        }
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return Some(Vec::new());
        };
        let damage = zone_roll_stat_range(
            t.min_dc,
            t.max_dc
                .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_DC)),
            now,
            id,
            s.roll(u64::MAX),
        );
        s.action_ms = now.saturating_add(300);
        if damage > 0 {
            s.hits.push(HuggerHit {
                due: now.saturating_add(500 + distance as u64 * 50),
                session: Some(target.session_id.clone()),
                reference: None,
                source: None,
                target: target.object_id,
                damage,
                poison: false,
                roll_on_resolve: false,
            });
        }
        let direction = zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
        let live = self.native_monsters.get_mut(&id)?;
        live.direction = direction;
        live.next_attack_ready_at_ms = now.saturating_add(live.attack_speed_ms);
        let packet = ServerPacket::ObjectRangeAttack {
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
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        Some(vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(
                id,
                target.object_id,
                &target.position,
            ),
            packets: vec![packet],
        }])
    }
    pub(super) fn tick_hugger_delays(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| matches!(m.ai, 69 | 70).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            let m = self.native_monsters[&id].clone();
            let Some(state) = m.special_ai.as_ref().and_then(|s| s.hugger.as_ref()) else {
                continue;
            };
            let death_due = !state.finished && state.death_ms.is_some_and(|d| now >= d);
            let template = crystal_monster_by_name(&m.name);
            let dynamic = if death_due && m.ai == 70 {
                self.hugger_targets(id, &m, false, now)
            } else {
                Vec::new()
            };
            let s = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .hugger
                .as_mut()
                .unwrap();
            if death_due {
                s.finished = true;
            }
            for target in dynamic {
                if s.excluded_lives.contains(&target.target) {
                    continue;
                }
                let damage = 0; // Draw only when this target is reached, after prior early-return checks.
                s.hits.push(HuggerHit {
                    due: now,
                    session: target.session,
                    target: target.target,
                    reference: target.reference,
                    source: Some(ZoneCombatEntityRef::Monster {
                        object_id: id,
                        incarnation: m.incarnation,
                    }),
                    damage,
                    poison: true,
                    roll_on_resolve: true,
                });
            }
            let pending = std::mem::take(&mut s.hits);
            let mut abort_explosion = false;
            for mut h in pending {
                if h.due > now {
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .hugger
                        .as_mut()
                        .unwrap()
                        .hits
                        .push(h);
                    continue;
                }
                if h.poison && abort_explosion {
                    continue;
                }
                if h.roll_on_resolve {
                    let s = self
                        .native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .hugger
                        .as_mut()
                        .unwrap();
                    h.damage = template.as_ref().map_or(0, |t| {
                        zone_roll_stat_range(
                            t.min_dc,
                            t.max_dc.saturating_add(zone_native_monster_buff_stat_total(
                                &m,
                                CRYSTAL_STAT_MAX_DC,
                            )),
                            now,
                            id,
                            s.roll(u64::MAX),
                        )
                    });
                }
                if let (Some(source), Some(target)) = (&h.source, &h.reference) {
                    let (actual, events) = self.resolve_native_entity_hit(
                        source,
                        target,
                        h.damage,
                        EntityDefence::ACAgility,
                        false,
                        now,
                    );
                    out.extend(events);
                    if h.poison && actual <= 0 && m.ai == 70 {
                        abort_explosion = true;
                    }
                    if h.poison && actual > 0 {
                        let resist = self.native_entity_poison_resist(target) as u64;
                        let s = self
                            .native_monsters
                            .get_mut(&id)
                            .unwrap()
                            .special_ai
                            .as_mut()
                            .unwrap()
                            .hugger
                            .as_mut()
                            .unwrap();
                        let value = template.as_ref().map_or(0, |t| {
                            zone_roll_stat_range(t.min_sc, t.max_sc, now, id, s.roll(u64::MAX))
                        });
                        if s.roll(10) >= resist && s.roll(5) == 0 {
                            out.extend(self.apply_native_entity_green_poison(
                                source, target, value, 5, 2000, now,
                            ));
                        }
                    }
                    continue;
                }
                let Some(session) = &h.session else {
                    continue;
                };
                let before = self
                    .players
                    .get(session)
                    .filter(|p| p.object_id == h.target)
                    .map(|p| p.hp);
                let hit = self.players.get(session).is_some_and(|p| {
                    crate::runtime::combat::crystal_accuracy_roll(
                        now,
                        id,
                        h.target,
                        p.combat_stats.agility.max(0) as u64 + 1,
                    ) <= template.as_ref().map_or(0, |t| t.accuracy.max(0)) as u64
                });
                if hit && h.damage > 0 {
                    out.extend(self.resolve_pending_native_player_hit(
                        PendingNativePlayerHit {
                            ready_at_ms: now,
                            attacker_object_id: id,
                            attacker_ai: m.ai,
                            target_session_id: session.clone(),
                            target_object_id: h.target,
                            damage: h.damage,
                            magic: false,
                        },
                        now,
                    ));
                }
                let damaged = before
                    .zip(self.players.get(session).map(|p| p.hp))
                    .is_some_and(|(a, b)| b < a);
                if h.poison && !damaged && m.ai == 70 {
                    abort_explosion = true;
                }
                if h.poison && damaged {
                    let resist = self
                        .players
                        .get(session)
                        .map_or(10, |p| p.combat_stats.poison_resist.clamp(0, 10))
                        as u64;
                    let s = self
                        .native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .hugger
                        .as_mut()
                        .unwrap();
                    // PoisonTarget resist gate, chance, then ApplyPoison's
                    // independent resist gate (noResist=false in Crystal).
                    if s.roll(10) >= resist && s.roll(5) == 0 && s.roll(10) >= resist {
                        let value = template.as_ref().map_or(0, |t| {
                            zone_roll_stat_range(t.min_sc, t.max_sc, now, id, s.roll(u64::MAX))
                        });
                        out.extend(self.apply_native_player_green_poison(
                            id, session, h.target, value, 5, 2000, now,
                        ));
                    }
                }
            }
        }
        out
    }
    fn kill_hugger_native_adjacent(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        let owner = m
            .special_ai
            .as_ref()
            .and_then(|s| s.hugger.as_ref())
            .and_then(|s| s.reward_owner.as_ref().filter(|_| now <= s.reward_until_ms))
            .filter(|s| self.players.get(*s).is_some_and(|p| !p.dead && p.hp > 0))
            .cloned();
        let Some((_, _, killed, name, experience, position, direction, _, drops, boss_audit)) =
            self.apply_native_monster_scripted_death_damage(id, owner.as_ref(), now)
        else {
            return Vec::new();
        };
        if !killed {
            return Vec::new();
        }
        self.hugger_on_death(id, now);
        let packet = ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: id,
                location: position.clone(),
                direction,
                kind: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        let mut out = vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &position),
            packets: vec![packet],
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
    fn tick_hugger_monster_target(
        &mut self,
        id: u32,
        target: ZoneCombatEntityRef,
        position: Point,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        let distance = zone_tile_distance(&m.position, &position);
        let direction = zone_direction_toward(&m.position, &position).unwrap_or(m.direction);
        let state = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .hugger
            .as_mut()
            .unwrap();
        let ranged = m.ai == 69 && distance <= 5 && state.roll(5) == 0;
        let melee = m.ai == 70 && distance <= 1;
        if !ranged && !melee {
            let destination = offset_point(&m.position, direction, 1);
            if !self.can_native_monster_occupy(id, &destination) {
                return self.turn_native_monster(id, direction, now);
            }
            let before: BTreeSet<_> = self
                .native_monster_visible_recipients(id, &m.position)
                .into_iter()
                .collect();
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.position = destination.clone();
            live.direction = direction;
            live.next_ai_ready_at_ms = now.saturating_add(live.move_speed_ms);
            live.special_ai
                .as_mut()
                .unwrap()
                .hugger
                .as_mut()
                .unwrap()
                .action_ms = live.next_ai_ready_at_ms;
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
            return out;
        }
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return Vec::new();
        };
        let damage = zone_roll_stat_range(
            t.min_dc,
            t.max_dc
                .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_DC)),
            now,
            id,
            state.roll(u64::MAX),
        );
        state.action_ms = now.saturating_add(300);
        if damage > 0 {
            state.hits.push(HuggerHit {
                due: now.saturating_add(if ranged {
                    500 + distance as u64 * 50
                } else {
                    300
                }),
                session: None,
                target: target.object_id(),
                reference: Some(target.clone()),
                source: Some(ZoneCombatEntityRef::Monster {
                    object_id: id,
                    incarnation: m.incarnation,
                }),
                damage,
                poison: false,
                roll_on_resolve: false,
            });
        }
        let live = self.native_monsters.get_mut(&id).unwrap();
        live.direction = direction;
        live.next_attack_ready_at_ms = now.saturating_add(live.attack_speed_ms);
        let packet = if ranged {
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id: id,
                    location: m.position,
                    direction,
                    target_id: target.object_id(),
                    target: position.clone(),
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
        vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(id, target.object_id(), &position),
            packets: vec![packet],
        }]
    }
}
