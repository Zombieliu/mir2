//! Crystal GeneralMeowMeow AI123. The source's mass-thunder spawn is already expired.
use super::entity_combat::{EntityDefence, EntityTargetPurpose, ZoneCombatEntityRef};
use super::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct MeowState {
    spawn_ms: u64,
    thunder_ms: u64,
    action_ms: u64,
    sequence: u64,
    slaves: Vec<u32>,
    hits: Vec<MeowHit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entity_hits: Vec<MeowEntityHit>,
    thunder: Vec<MeowThunder>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MeowEntityHit {
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
    due_ms: u64,
    damage: i32,
    group: bool,
    defence: EntityDefence,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MeowHit {
    hit: PendingNativePlayerHit,
    group: bool,
    accuracy: Option<i32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MeowThunder {
    spawn_ms: u64,
    expire_ms: u64,
    point: Point,
    value: i32,
}
pub(in crate::runtime::zone) fn initialize_meow(m: &mut ZoneNativeMonster, now: u64) {
    if m.ai == 123 {
        m.special_ai
            .get_or_insert_with(Default::default)
            .meow
            .get_or_insert_with(|| MeowState {
                spawn_ms: now.saturating_add(60000),
                ..Default::default()
            });
    }
}
fn meow_roll(s: &mut MeowState, id: u32, now: u64, n: u64) -> u64 {
    s.sequence = s.sequence.wrapping_add(1);
    let mut x = now ^ u64::from(id) ^ s.sequence.wrapping_mul(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    (x ^ (x >> 31)) % n.max(1)
}
impl ZoneRuntime {
    pub(super) fn clear_meow_target_life(&mut self, target: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.meow.as_mut()) {
                s.hits.retain(|h| h.hit.target_object_id != target);
                s.entity_hits.retain(|h| h.target.object_id() != target);
            }
        }
    }
    fn meow_targets(
        &self,
        m: &ZoneNativeMonster,
        point: &Point,
        range: i32,
    ) -> Vec<NativeMonsterTarget> {
        self.players
            .values()
            .filter(|p| {
                !p.dead
                    && !p.hidden
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
    pub(super) fn tick_meow_states(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (m.ai == 123 && !m.dead).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            initialize_meow(self.native_monsters.get_mut(&id).unwrap(), now);
            let m = self.native_monsters[&id].clone();
            let mut s = m.special_ai.as_ref().unwrap().meow.clone().unwrap();
            if let Some(target) = self.selected_native_entity_target(id, now).or_else(|| {
                self.native_entity_targets(
                    id,
                    &m.position,
                    crystal_monster_by_name(&m.name).map_or(0, |t| i32::from(t.view_range)),
                    EntityTargetPurpose::Search,
                    now,
                )
                .into_iter()
                .next()
            }) {
                self.set_native_entity_target(id, &target.reference, now);
                if now > s.spawn_ms {
                    s.slaves
                        .retain(|id| self.native_monsters.get(id).is_some_and(|m| !m.dead));
                    for _ in 0..3.min(6usize.saturating_sub(s.slaves.len())) {
                        let names = ["StainHammerCat", "BlackHammerCat", "StrayCat", "CatShaman"];
                        let name = names[meow_roll(&mut s, id, now, 4) as usize];
                        let front = offset_point(&m.position, m.direction, 1);
                        let (child, events) =
                            self.spawn_special_relative(id, name, &front, &m, None, false, now);
                        out.extend(events);
                        if let Some(child) = child {
                            self.set_native_entity_target(child, &target.reference, now);
                            s.slaves.push(child);
                        }
                    }
                    s.spawn_ms = now.saturating_add(60000);
                }
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .meow = Some(s);
        }
        out
    }
    pub(super) fn try_tick_meow(&mut self, id: u32, now: u64) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if m.ai != 123 {
            return None;
        }
        if m.dead || !m.hostile_to_player {
            return Some(Vec::new());
        }
        initialize_meow(self.native_monsters.get_mut(&id)?, now);
        let mut s = self.native_monsters[&id]
            .special_ai
            .as_ref()?
            .meow
            .clone()?;
        let target = self.selected_native_entity_target(id, now).or_else(|| {
            self.native_entity_targets(
                id,
                &m.position,
                crystal_monster_by_name(&m.name).map_or(0, |t| i32::from(t.view_range)),
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
            || zone_tile_distance(&m.position, &target.position) > 12
        {
            return None;
        }
        let t = crystal_monster_by_name(&m.name)?;
        let direction = zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
        let mut out = Vec::new();
        let percent = m.hp.saturating_mul(100) / m.max_hp.max(1);
        if (70..=80).contains(&percent) || (40..=50).contains(&percent) || percent <= 20 {
            let buff = ClientBuff {
                buff_type: 52,
                visible: true,
                object_id: id,
                expire_time: 30000,
                infinite: false,
                paused: false,
                stats: vec![
                    UserItemStat {
                        stat: CRYSTAL_STAT_MIN_AC,
                        value: 100,
                    },
                    UserItemStat {
                        stat: CRYSTAL_STAT_MAX_AC,
                        value: 100,
                    },
                ],
                values: Vec::new(),
            };
            self.native_monsters.get_mut(&id)?.buffs.insert(
                52,
                super::super::types::ZonePlayerBuff {
                    buff: buff.clone(),
                    expires_at_ms: Some(now.saturating_add(30000)),
                    notify_owner_on_expiry: false,
                },
            );
            let packet = ServerPacket::AddBuff { buff };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_visible_recipients(id, &m.position),
                packets: vec![packet],
            });
            if now > s.thunder_ms {
                let targets = self.native_entity_targets(
                    id,
                    &target.position,
                    12,
                    EntityTargetPurpose::VisibleImpact,
                    now,
                );
                if !targets.is_empty() {
                    for target in targets {
                        let max = t.max_mc.saturating_add(zone_native_monster_buff_stat_total(
                            &m,
                            CRYSTAL_STAT_MAX_MC,
                        ));
                        let value = if max > t.min_mc {
                            t.min_mc.saturating_add(meow_roll(
                                &mut s,
                                id,
                                now,
                                (max - t.min_mc) as u64,
                            ) as i32)
                        } else {
                            t.min_mc
                        };
                        s.thunder.push(MeowThunder {
                            spawn_ms: now.saturating_add(2000),
                            expire_ms: now.saturating_add(1000),
                            point: target.position,
                            value,
                        });
                    }
                    s.thunder_ms = now.saturating_add(
                        meow_roll(&mut s, id, now, 2000).max(meow_roll(&mut s, id, now, 4000)),
                    );
                }
            }
        }
        let ranged =
            m.position == target.position || zone_tile_distance(&m.position, &target.position) > 2;
        let slam = !ranged && meow_roll(&mut s, id, now, 9) == 0;
        let damage = zone_roll_stat_range(
            if ranged { t.min_mc } else { t.min_dc },
            if ranged {
                t.max_mc
                    .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_MC))
            } else {
                t.max_dc
                    .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_DC))
            },
            now,
            id,
            meow_roll(&mut s, id, now, u64::MAX),
        )
        .saturating_mul(if slam { 3 } else { 1 });
        let packet = if ranged {
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
                    attack_type: if slam { 1 } else { 0 },
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
        if damage > 0 {
            s.entity_hits.push(MeowEntityHit {
                source,
                target: target.reference,
                due_ms: now.saturating_add(500),
                damage,
                group: ranged,
                defence: if ranged {
                    EntityDefence::MACAgility
                } else if slam {
                    EntityDefence::AC
                } else {
                    EntityDefence::ACAgility
                },
            });
        }
        s.action_ms = now.saturating_add(300);
        let live = self.native_monsters.get_mut(&id)?;
        live.direction = direction;
        live.next_ai_ready_at_ms = live.next_ai_ready_at_ms.max(s.action_ms);
        live.next_attack_ready_at_ms = now.saturating_add(live.attack_speed_ms);
        live.special_ai.as_mut()?.meow = Some(s);
        Some(out)
    }
    pub(super) fn tick_meow_delays(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let mut hits = Vec::new();
        let mut entity_hits = Vec::new();
        let mut thunder = Vec::new();
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.meow.as_mut()) {
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
                for t in std::mem::take(&mut s.thunder) {
                    if now >= t.spawn_ms {
                        thunder.push(t)
                    } else {
                        s.thunder.push(t)
                    }
                }
            }
        }
        let mut out = Vec::new();
        for h in entity_hits {
            // Validate the captured anchor life before looking up its CURRENT
            // position. A replacement pet/player must never inherit this cast.
            if !self.native_entity_can_attack(
                &h.source,
                &h.target,
                EntityTargetPurpose::Impact,
                now,
            ) {
                continue;
            }
            let point = match &h.target {
                ZoneCombatEntityRef::Player { session_id, .. } => {
                    self.players.get(session_id).map(|p| p.position.clone())
                }
                ZoneCombatEntityRef::Monster { object_id, .. } => self
                    .native_monsters
                    .get(object_id)
                    .map(|m| m.position.clone()),
            };
            let Some(point) = point else {
                continue;
            };
            let targets = if h.group {
                self.native_entity_targets(
                    h.source.object_id(),
                    &point,
                    2,
                    EntityTargetPurpose::VisibleImpact,
                    now,
                )
                .into_iter()
                .map(|t| t.reference)
                .collect::<Vec<_>>()
            } else {
                vec![h.target]
            };
            for target in targets {
                let (_, events) = self
                    .resolve_native_entity_hit(&h.source, &target, h.damage, h.defence, false, now);
                out.extend(events);
            }
        }
        // Compatibility only: old checkpoints retain their existing player-hit representation.
        for h in hits {
            let Some(m) = self.native_monsters.get(&h.hit.attacker_object_id).cloned() else {
                continue;
            };
            let Some(p) = self.players.get(&h.hit.target_session_id).filter(|p| {
                p.object_id == h.hit.target_object_id && !p.dead && !p.chat_profile.in_safe_zone
            }) else {
                continue;
            };
            let targets = if h.group {
                self.meow_targets(&m, &p.position, 2)
            } else {
                self.meow_targets(&m, &p.position, 0)
                    .into_iter()
                    .filter(|t| t.object_id == h.hit.target_object_id)
                    .collect()
            };
            for target in targets {
                let p = &self.players[&target.session_id];
                if h.accuracy.is_some_and(|a| {
                    crate::runtime::combat::crystal_accuracy_roll(
                        now,
                        h.hit.attacker_object_id,
                        p.object_id,
                        p.combat_stats.agility.max(0) as u64 + 1,
                    ) > a.max(0) as u64
                }) {
                    continue;
                }
                let mut hit = h.hit.clone();
                hit.target_session_id = target.session_id;
                hit.target_object_id = target.object_id;
                out.extend(self.resolve_pending_native_player_hit(hit, now));
            }
        }
        // Crystal: creation expiry=cast+1000, delayed Spawn=cast+2000.
        // Preserve the expired spell lifecycle; never invent the intended damage.
        for t in thunder {
            if now > t.expire_ms {
                let id = self.unique_object_id(0);
                let packet = ServerPacket::ObjectSpell {
                    info: ObjectSpellInfo {
                        object_id: id,
                        location: t.point.clone(),
                        spell: Spell::GeneralMeowMeowThunder,
                        direction: MirDirection::Up,
                        param: false,
                    },
                };
                out.push(ZoneOutbound::ToMany {
                    session_ids: self
                        .players
                        .values()
                        .filter(|p| points_visible(&p.position, &t.point))
                        .map(|p| p.session_id.clone())
                        .collect(),
                    packets: vec![packet, ServerPacket::ObjectRemove { object_id: id }],
                });
            }
        }
        out
    }
}
