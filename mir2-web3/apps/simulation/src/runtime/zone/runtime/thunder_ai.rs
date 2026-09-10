//! Crystal AI49 ThunderElement. AI50 is GreatFoxSpirit, a separate family.
use super::entity_combat::{EntityDefence, EntityTargetPurpose};
use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ThunderAiState {
    pub rng_state: u64,
    pub pending_bursts: Vec<ThunderBurst>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_repulsions: Vec<ThunderRepulsion>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ThunderRepulsion {
    session_id: SessionId,
    attacker_object_id: u32,
    damage: i32,
    due_at_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ThunderBurst {
    pub due_at_ms: u64,
    pub damage: i32,
}

pub(super) fn thunder_accepts_damage(m: &ZoneNativeMonster, is_repulsion: bool) -> bool {
    m.ai != 49 || is_repulsion
}

impl ZoneRuntime {
    pub(super) fn queue_thunder_repulsion(
        &mut self,
        id: u32,
        session: &SessionId,
        requested: i32,
        moved: bool,
        now: u64,
    ) {
        let Some(attacker) = self.players.get(session).map(|p| p.object_id) else {
            return;
        };
        let Some(damage) = self.thunder_repulsion_damage(id, requested, moved, now) else {
            return;
        };
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .thunder
            .as_mut()
            .unwrap()
            .pending_repulsions
            .push(ThunderRepulsion {
                session_id: session.clone(),
                attacker_object_id: attacker,
                damage,
                due_at_ms: now,
            });
    }

    fn resolve_thunder_repulsions(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                m.special_ai
                    .as_ref()
                    .and_then(|s| s.thunder.as_ref())
                    .is_some_and(|s| !s.pending_repulsions.is_empty())
                    .then_some(id)
            })
            .collect();
        let mut out = Vec::new();
        for id in ids {
            let pending = std::mem::take(
                &mut self
                    .native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .thunder
                    .as_mut()
                    .unwrap()
                    .pending_repulsions,
            );
            for hit in pending {
                if now < hit.due_at_ms {
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .thunder
                        .as_mut()
                        .unwrap()
                        .pending_repulsions
                        .push(hit);
                    continue;
                }
                let Some((
                    damage,
                    percent,
                    killed,
                    name,
                    experience,
                    position,
                    direction,
                    owner,
                    drops,
                    audit,
                )) = self.apply_native_monster_repulsion_damage(
                    id,
                    hit.damage,
                    Some(&hit.session_id),
                    now,
                )
                else {
                    continue;
                };
                let mut packets = vec![
                    ServerPacket::ObjectStruck {
                        info: ObjectStruckInfo {
                            object_id: id,
                            attacker_id: hit.attacker_object_id,
                            location: position.clone(),
                            direction,
                        },
                    },
                    ServerPacket::DamageIndicator {
                        damage,
                        damage_type: 0,
                        object_id: id,
                    },
                    ServerPacket::ObjectHealth {
                        info: ObjectHealthInfo {
                            object_id: id,
                            percent,
                            expire: 0,
                        },
                    },
                ];
                if killed {
                    packets.push(ServerPacket::ObjectDied {
                        info: ObjectDiedInfo {
                            object_id: id,
                            location: position.clone(),
                            direction,
                            kind: 0,
                        },
                    });
                }
                self.apply_zone_object_packets(&packets, now);
                let recipients = self.native_monster_action_recipients(
                    &hit.session_id,
                    hit.attacker_object_id,
                    id,
                    &position,
                );
                if !recipients.is_empty() {
                    out.push(ZoneOutbound::ToMany {
                        session_ids: recipients,
                        packets,
                    });
                }
                if killed {
                    let owner = owner.unwrap_or(hit.session_id);
                    let owner_id = self
                        .players
                        .get(&owner)
                        .map_or(hit.attacker_object_id, |p| p.object_id);
                    let drops =
                        self.spawn_native_monster_drops(&name, &position, owner_id, drops, now);
                    out.extend(self.diff_all_zone_object_visibility());
                    out.extend(self.group_monster_kill_awards(
                        &owner,
                        ZoneMonsterKillAward {
                            source_receipt_key: None,
                            experience_selection: None,
                            monster_object_id: id,
                            killed_at_ms: now,
                            monster_name: name,
                            experience,
                            drops,
                            boss_audit: audit,
                        },
                    ));
                }
            }
        }
        out
    }
    fn thunder_roll(&mut self, id: u32, now: u64, upper: u64) -> u64 {
        let s = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .get_or_insert_with(Default::default)
            .thunder
            .get_or_insert_with(Default::default);
        if s.rng_state == 0 {
            s.rng_state = (u64::from(id) << 32) ^ now ^ 0x49AD;
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

    /// Called after a successful push. Crystal scales by REQUESTED distance,
    /// not the actual successful steps; a fully blocked push returns no damage.
    pub(super) fn thunder_repulsion_damage(
        &mut self,
        id: u32,
        requested_distance: i32,
        moved: bool,
        now: u64,
    ) -> Option<i32> {
        let m = self.native_monsters.get(&id)?;
        if m.ai != 49 || m.dead || m.hp <= 0 || !moved || requested_distance <= 0 {
            return None;
        }
        let max = m.max_hp.max(1) as u64;
        let base = (self.thunder_roll(id, now, max) / 5)
            .max(50)
            .min(i32::MAX as u64) as i32;
        Some(requested_distance.saturating_mul(base))
    }

    pub(super) fn try_tick_thunder_monster(
        &mut self,
        id: u32,
        m: &ZoneNativeMonster,
        target: &NativeMonsterTarget,
        now: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        self.try_tick_thunder_at_position(id, m, &target.position, now)
    }

    /// Invoke before generic player-only selection, after normal action/control
    /// gates. A real owned monster can be the sole nearby attack target.
    pub(super) fn try_tick_thunder_entity(
        &mut self,
        id: u32,
        now: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if m.ai != 49 {
            return None;
        }
        if m.dead || now < m.next_ai_ready_at_ms || native_monster_control_active(&m, now) {
            return Some(Vec::new());
        }
        let range = crystal_monster_by_name(&m.name).map_or(0, |t| i32::from(t.view_range));
        let Some(target) = self.selected_native_entity_target(id, now).or_else(|| {
            self.native_entity_targets(id, &m.position, range, EntityTargetPurpose::Search, now)
                .into_iter()
                .next()
        }) else {
            return Some(Vec::new());
        };
        self.set_native_entity_target(id, &target.reference, now);
        if let Some(out) = self.try_tick_thunder_at_position(id, &m, &target.position, now) {
            return Some(out);
        }
        let Some(direction) = zone_direction_toward(&m.position, &target.position) else {
            return Some(Vec::new());
        };
        let point = offset_point(&m.position, direction, 1);
        Some(self.thunder_move_to_cell(id, point, direction, now))
    }

    fn thunder_move_to_cell(
        &mut self,
        id: u32,
        point: Point,
        direction: MirDirection,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.can_native_monster_occupy(id, &point) {
            return Vec::new();
        }
        let old = self.native_monster_visible_recipients(id, &self.native_monsters[&id].position);
        let m = self.native_monsters.get_mut(&id).unwrap();
        m.position = point.clone();
        m.direction = direction;
        m.next_ai_ready_at_ms = now.saturating_add(m.move_speed_ms.max(300));
        let packet = ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id: id,
                position: point.clone(),
                direction,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        let mut out = self.diff_all_zone_object_visibility();
        let recipients = self
            .native_monster_visible_recipients(id, &point)
            .into_iter()
            .filter(|s| old.contains(s))
            .collect::<Vec<_>>();
        if !recipients.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![packet],
            });
        }
        out
    }

    fn try_tick_thunder_at_position(
        &mut self,
        id: u32,
        m: &ZoneNativeMonster,
        target: &Point,
        now: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        if m.ai != 49 {
            return None;
        }
        if zone_tile_distance(&m.position, target) > 1 {
            return None;
        }
        if now <= m.next_attack_ready_at_ms {
            return Some(Vec::new());
        }
        let mut out = Vec::new();
        if self.thunder_roll(id, now, 3) == 1 {
            let dx = self.thunder_roll(id, now, 3) as i32 - 1;
            let dy = self.thunder_roll(id, now, 3) as i32 - 1;
            if let (Some(x), Some(y)) = (target.x.checked_add(dx), target.y.checked_add(dy)) {
                let desired = Point { x, y };
                if let Some(direction) = zone_direction_toward(&m.position, &desired) {
                    let dirs = [
                        MirDirection::Up,
                        MirDirection::UpRight,
                        MirDirection::Right,
                        MirDirection::DownRight,
                        MirDirection::Down,
                        MirDirection::DownLeft,
                        MirDirection::Left,
                        MirDirection::UpLeft,
                    ];
                    let index = dirs.iter().position(|d| *d == direction).unwrap();
                    let cw = self.thunder_roll(id, now, 2) == 0;
                    for n in 0..8 {
                        let direction = dirs[if cw {
                            (index + n) % 8
                        } else {
                            (index + 8 - n) % 8
                        }];
                        let point = offset_point(&m.position, direction, 1);
                        if !self.can_native_monster_occupy(id, &point) {
                            continue;
                        }
                        out.extend(self.thunder_move_to_cell(id, point, direction, now));
                        break;
                    }
                }
            }
        }
        let template = crystal_monster_by_name(&m.name);
        let damage = template.map_or(0, |t| {
            zone_roll_stat_range(
                t.min_dc,
                t.max_dc
                    .saturating_add(zone_native_monster_buff_stat_total(m, CRYSTAL_STAT_MAX_DC)),
                now,
                id,
                0x49DC,
            )
        });
        let monster = self.native_monsters.get_mut(&id).unwrap();
        monster.next_ai_ready_at_ms = monster.next_ai_ready_at_ms.max(now.saturating_add(300));
        monster.next_attack_ready_at_ms = now.saturating_add(monster.attack_speed_ms);
        if damage > 0 {
            monster
                .special_ai
                .get_or_insert_with(Default::default)
                .thunder
                .get_or_insert_with(Default::default)
                .pending_bursts
                .push(ThunderBurst {
                    due_at_ms: now.saturating_add(300),
                    damage,
                });
        }
        Some(out)
    }

    /// Resolve against the caster's CURRENT position at impact, not a target
    /// list captured at cast time. This is the original CompleteAttack contract.
    pub(super) fn tick_thunder_effects(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let mut out = self.resolve_thunder_repulsions(now);
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                m.special_ai
                    .as_ref()
                    .and_then(|s| s.thunder.as_ref())
                    .is_some_and(|s| !s.pending_bursts.is_empty())
                    .then_some(id)
            })
            .collect();
        for id in ids {
            let bursts = std::mem::take(
                &mut self
                    .native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .thunder
                    .as_mut()
                    .unwrap()
                    .pending_bursts,
            );
            let mut future = Vec::new();
            for burst in bursts {
                if now < burst.due_at_ms {
                    future.push(burst);
                    continue;
                }
                let m = self.native_monsters[&id].clone();
                let Some(source) = self.native_entity_monster_ref(id) else {
                    continue;
                };
                let targets = self.native_entity_targets(
                    id,
                    &m.position,
                    2,
                    EntityTargetPurpose::Impact,
                    now,
                );
                if targets.is_empty() {
                    continue;
                }
                for target in targets {
                    let (_, events) = self.resolve_native_entity_hit(
                        &source,
                        &target.reference,
                        burst.damage,
                        EntityDefence::MAC,
                        false,
                        now,
                    );
                    out.extend(events);
                }
                let packet = ServerPacket::ObjectAttack {
                    info: ObjectAttackInfo {
                        object_id: id,
                        location: m.position.clone(),
                        direction: m.direction,
                        spell: 0,
                        level: 0,
                        attack_type: 0,
                    },
                };
                self.apply_zone_object_packets(&[packet.clone()], now);
                let recipients = self.native_monster_visible_recipients(id, &m.position);
                if !recipients.is_empty() {
                    out.push(ZoneOutbound::ToMany {
                        session_ids: recipients,
                        packets: vec![packet],
                    });
                }
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .thunder
                .as_mut()
                .unwrap()
                .pending_bursts = future;
        }
        out
    }
}
