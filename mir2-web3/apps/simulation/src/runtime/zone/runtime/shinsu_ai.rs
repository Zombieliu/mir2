//! Crystal Shinsu.cs: visible small/large form plus two-cell breath attack.
use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ShinsuState {
    mode: bool,
    mode_until_ms: u64,
    action_until_ms: u64,
    #[serde(default)]
    hits: Vec<ShinsuHit>,
}

#[cfg(test)]
mod expiry_tests {
    use super::*;

    #[test]
    fn shinsu_expiry_blocks_mode_attack_and_both_pending_hit_queues() {
        let mut z = ZoneRuntime::new(ZoneKey::for_map("shinsu-expiry"));
        let spawn = ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 18,
            name: "Shinsu".into(),
            name_colour_argb: -1,
            image: 79,
            ai: 18,
            disposition: Some(crate::config::WorldEntityDisposition::Hostile),
            level: 1,
            max_hp: 100,
            hp: 100,
            experience: 0,
            move_speed_ms: 300,
            attack_speed_ms: 500,
            friendly_guild: None,
            position: Point { x: 20, y: 20 },
            direction: MirDirection::Right,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        };
        let mut m = ZoneNativeMonster::from_spawn(&spawn, 18);
        initialize_shinsu(&mut m, 0);
        let s = m.special_ai.as_mut().unwrap().shinsu.as_mut().unwrap();
        s.mode = true;
        s.hits.push(ShinsuHit {
            due_ms: 4000,
            owner: SessionId::new("owner"),
            target: 99,
            damage: 10,
            accuracy: 25,
        });
        z.native_monsters.insert(18, m);
        let packet = native_monster_spawn_packet(&spawn, 18);
        z.apply_zone_object_packets(&[packet], 0);
        z.objects.get_mut(&18).unwrap().expires_at_ms = Some(4000);
        z.pending_native_player_hits.push(PendingNativePlayerHit {
            ready_at_ms: 4000,
            attacker_object_id: 18,
            attacker_ai: 18,
            target_session_id: SessionId::new("target"),
            target_object_id: 101,
            damage: 10,
            magic: false,
        });
        // Generate a genuine committed checkpoint after setting the real expiry
        // field. No serialized state or signature is rewritten by this test.
        let mut z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert!(z.shinsu_not_expired(18, 3999));
        z.tick_shinsu_hits(3999);
        assert_eq!(
            z.native_monsters[&18]
                .special_ai
                .as_ref()
                .unwrap()
                .shinsu
                .as_ref()
                .unwrap()
                .hits
                .len(),
            1
        );
        assert_eq!(z.pending_native_player_hits.len(), 1);
        assert!(!z.shinsu_not_expired(18, 4000));
        assert!(
            z.tick_shinsu_states(4000).is_empty(),
            "expired mode cannot emit Hide"
        );
        assert!(
            z.try_shinsu_line(18, &Point { x: 21, y: 20 }, 4000)
                .unwrap()
                .is_empty(),
            "expired source cannot launch"
        );
        assert!(z.tick_shinsu_hits(4000).is_empty());
        assert!(z.native_monsters[&18]
            .special_ai
            .as_ref()
            .unwrap()
            .shinsu
            .as_ref()
            .unwrap()
            .hits
            .is_empty());
        assert!(z.pending_native_player_hits.is_empty());
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShinsuHit {
    due_ms: u64,
    owner: SessionId,
    target: u32,
    damage: i32,
    accuracy: i32,
}

pub(in crate::runtime::zone) fn initialize_shinsu(m: &mut ZoneNativeMonster, now: u64) {
    if m.ai == 18 {
        m.special_ai
            .get_or_insert_with(Default::default)
            .shinsu
            .get_or_insert(ShinsuState {
                // MonsterObject.Spawned overrides the constructor lock with 2000 ms.
                action_until_ms: now.saturating_add(2000),
                ..Default::default()
            });
    }
}
pub(super) fn shinsu_can_act(m: &ZoneNativeMonster, now: u64) -> bool {
    m.ai != 18
        || m.special_ai
            .as_ref()
            .and_then(|s| s.shinsu.as_ref())
            .is_some_and(|s| now > s.action_until_ms)
}
pub(super) fn sync_shinsu_packet(m: &ZoneNativeMonster, packet: &mut ServerPacket) {
    if m.ai != 18 {
        return;
    }
    if let ServerPacket::ObjectMonster { info } = packet {
        info.image = if m
            .special_ai
            .as_ref()
            .and_then(|s| s.shinsu.as_ref())
            .is_some_and(|s| s.mode)
        {
            80
        } else {
            79
        };
        info.extra = true;
        // ObjectHide changes form; Shinsu remains an ordinary visible blocker.
        info.hidden = false;
    }
}
fn shinsu_range(source: &Point, target: &Point) -> bool {
    let x = (i64::from(source.x) - i64::from(target.x)).abs();
    let y = (i64::from(source.y) - i64::from(target.y)).abs();
    x.max(y) > 0 && x.max(y) <= 2 && (x.max(y) <= 1 || x == y || x % 2 == y % 2)
}

impl ZoneRuntime {
    fn shinsu_not_expired(&self, id: u32, now: u64) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|object| object.expires_at_ms.is_none_or(|deadline| now < deadline))
    }
    pub(super) fn clear_shinsu_hits_for_target(&mut self, target: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.shinsu.as_mut()) {
                s.hits.retain(|h| h.target != target);
            }
        }
    }
    pub(super) fn tick_shinsu_states(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (m.ai == 18 && !m.dead).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            if !self.shinsu_not_expired(id, now) {
                continue;
            }
            initialize_shinsu(self.native_monsters.get_mut(&id).unwrap(), now);
            let m = self.native_monsters[&id].clone();
            let has_target = if let Some(owner) = m.owner_session_id.as_ref() {
                self.players.get(owner).is_some_and(|p| {
                    !p.dead && p.object_id == zone_native_summon_owner_player_object_id(&m)
                }) && self.nearest_native_summon_target(id, &m.position).is_some()
            } else {
                m.hostile_to_player
                    && self
                        .nearest_native_monster_target(&m.position, m.friendly_guild.as_deref())
                        .is_some()
            };
            let s = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .shinsu
                .as_mut()
                .unwrap();
            if now <= s.action_until_ms {
                continue;
            }
            if has_target {
                s.mode_until_ms = now.saturating_add(30_000);
            }
            let next = if !s.mode && now < s.mode_until_ms {
                true
            } else if s.mode && now > s.mode_until_ms {
                false
            } else {
                s.mode
            };
            if next == s.mode {
                continue;
            }
            s.mode = next;
            s.action_until_ms = now.saturating_add(1000);
            let m = &self.native_monsters[&id];
            if let Some(o) = self.objects.get_mut(&id) {
                sync_shinsu_packet(m, &mut o.packet);
            }
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_visible_recipients(id, &m.position),
                packets: vec![if next {
                    ServerPacket::ObjectShow { object_id: id }
                } else {
                    ServerPacket::ObjectHide { object_id: id }
                }],
            });
        }
        out
    }

    /// Call after the existing owned-pet lifecycle/target checks or the wild
    /// target choice, before generic melee. None permits ordinary chase.
    pub(super) fn try_shinsu_line(
        &mut self,
        id: u32,
        target: &Point,
        now: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if m.ai != 18 {
            return None;
        }
        if !self.shinsu_not_expired(id, now) {
            return Some(Vec::new());
        }
        let state = m.special_ai.as_ref()?.shinsu.as_ref()?;
        if !state.mode || !shinsu_can_act(&m, now) {
            return Some(Vec::new());
        }
        if !shinsu_range(&m.position, target) {
            if m.position == *target {
                return Some(Vec::new());
            }
            return None;
        }
        if super::entity_combat::native_entity_attack_blocked(&m, now) {
            return Some(Vec::new());
        }
        if now < m.next_attack_ready_at_ms {
            return Some(Vec::new());
        }
        let direction = zone_direction_toward(&m.position, target)?;
        let template = crystal_monster_by_name(&m.name);
        let damage = template
            .as_ref()
            .map(|t| {
                zone_roll_stat_range(
                    t.min_dc,
                    t.max_dc.saturating_add(zone_native_monster_buff_stat_total(
                        &m,
                        CRYSTAL_STAT_MAX_DC,
                    )),
                    now,
                    id,
                    0x518,
                )
            })
            .unwrap_or(7);
        // MonsterObject.RefreshBase copies Info.Stats; MapObject.GetArmour's
        // AC Agility defence compares against Stat.Accuracy (manifest stat 10).
        let accuracy = template.as_ref().map_or(0, |t| t.accuracy);
        let live = self.native_monsters.get_mut(&id)?;
        live.direction = direction;
        live.next_ai_ready_at_ms = now.saturating_add(300);
        live.next_attack_ready_at_ms = now.saturating_add(live.attack_speed_ms);
        live.special_ai.as_mut()?.shinsu.as_mut()?.action_until_ms = now.saturating_add(300);
        let packet = ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: id,
                location: m.position.clone(),
                direction,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        let out = vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &m.position),
            packets: vec![packet],
        }];
        if damage <= 0 {
            return Some(out);
        }
        for step in 1..=2 {
            let cell = offset_point(&m.position, direction, step);
            if self.collision.is_blocked(&cell) {
                continue;
            }
            let due = now.saturating_add(500 + step as u64 * 50);
            if let Some(owner) = m.owner_session_id.as_ref() {
                let target = self.native_monsters.iter().find_map(|(&other, t)| {
                    (other != id
                        && !t.dead
                        && t.hp > 0
                        && t.position == cell
                        && t.hostile_to_player
                        && t.ai != 57
                        && monster_visibility_is_attackable(t)
                        && trap_rock_visible(t))
                    .then_some(other)
                });
                if let Some(target) = target {
                    self.native_monsters
                        .get_mut(&id)?
                        .special_ai
                        .as_mut()?
                        .shinsu
                        .as_mut()?
                        .hits
                        .push(ShinsuHit {
                            due_ms: due,
                            owner: owner.clone(),
                            target,
                            damage,
                            accuracy,
                        });
                }
            } else if let Some(t) = self.players.values().find(|p| {
                !p.dead
                    && !p.chat_profile.in_safe_zone
                    && p.position == cell
                    && !m
                        .friendly_guild
                        .as_deref()
                        .zip(p.chat_profile.guild_name.as_deref())
                        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            }) {
                self.pending_native_player_hits
                    .push(PendingNativePlayerHit {
                        ready_at_ms: due,
                        attacker_object_id: id,
                        attacker_ai: 18,
                        target_session_id: t.session_id.clone(),
                        target_object_id: t.object_id,
                        damage,
                        magic: false,
                    });
            }
        }
        Some(out)
    }

    pub(super) fn tick_shinsu_hits(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let expired: BTreeSet<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (m.ai == 18 && !self.shinsu_not_expired(id, now)).then_some(id))
            .collect();
        // Wild Shinsu uses the common player-hit queue. This tick precedes its
        // resolver, so expired sources lose those queued attacks as well.
        self.pending_native_player_hits
            .retain(|hit| !expired.contains(&hit.attacker_object_id));
        let ids: Vec<_> = self.native_monsters.keys().copied().collect();
        let mut out = Vec::new();
        for id in ids {
            let expired = !self.shinsu_not_expired(id, now);
            let Some(s) = self
                .native_monsters
                .get_mut(&id)
                .and_then(|m| m.special_ai.as_mut())
                .and_then(|s| s.shinsu.as_mut())
            else {
                continue;
            };
            let pending = std::mem::take(&mut s.hits);
            if expired {
                continue;
            }
            let mut remaining = Vec::new();
            for h in pending {
                if now < h.due_ms {
                    remaining.push(h);
                    continue;
                }
                if !self.players.get(&h.owner).is_some_and(|p| {
                    !p.dead
                        && self.native_monsters.get(&id).is_some_and(|m| {
                            m.owner_session_id.as_ref() == Some(&h.owner)
                                && zone_native_summon_owner_player_object_id(m) == p.object_id
                        })
                }) {
                    continue;
                }
                let Some(t) = self.native_monsters.get(&h.target).filter(|t| {
                    !t.dead
                        && t.hp > 0
                        && t.hostile_to_player
                        && t.ai != 57
                        && monster_visibility_is_attackable(t)
                        && trap_rock_visible(t)
                }) else {
                    continue;
                };
                let agility = t.defense.agility.max(0);
                if agility > 0
                    && crate::runtime::combat::crystal_accuracy_roll(
                        now,
                        id,
                        h.target,
                        agility as u64 + 1,
                    ) > h.accuracy.max(0) as u64
                {
                    continue;
                }
                let armour = zone_roll_stat_range(
                    t.defense.min_ac,
                    t.defense
                        .max_ac
                        .saturating_add(zone_native_monster_buff_stat_total(
                            t,
                            CRYSTAL_STAT_MAX_AC,
                        )),
                    now,
                    id,
                    u64::from(h.target),
                );
                out.extend(self.resolve_pending_native_monster_hit(
                    PendingNativeMonsterHit {
                        ready_at_ms: now,
                        session_id: h.owner,
                        attacker_object_id: id,
                        object_id: h.target,
                        damage: h.damage.saturating_sub(armour).max(0),
                        fire_bounce: None,
                    },
                    now,
                ));
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .shinsu
                .as_mut()
                .unwrap()
                .hits = remaining;
        }
        out
    }
}
