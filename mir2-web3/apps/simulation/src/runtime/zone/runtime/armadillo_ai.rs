//! Crystal Armadillo.cs and ArmadilloElder.cs: shared entity attack branches.
//! Separate from generic damage selection to avoid doubling Elder damage twice.
use super::entity_combat::{EntityDefence, EntityTargetPurpose, ZoneCombatEntityRef};
use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ArmadilloState {
    fleeing: bool,
    attack_sequence: u64,
    hit_sequence: u64,
    retreat: Option<RetreatHit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entity_hits: Vec<ArmadilloEntityHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RetreatHit {
    due_ms: u64,
    target_session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_ref: Option<ZoneCombatEntityRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_ref: Option<ZoneCombatEntityRef>,
    target_object_id: u32,
    damage: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArmadilloEntityHit {
    due_ms: u64,
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
    damage: i32,
}

pub(super) fn armadillo_attacked(monster: &mut ZoneNativeMonster, object_id: u32, now_ms: u64) {
    if !matches!(monster.ai, 124 | 125) {
        return;
    }
    let s = monster
        .special_ai
        .get_or_insert_with(Default::default)
        .armadillo
        .get_or_insert_with(Default::default);
    if !s.fleeing {
        return;
    }
    s.hit_sequence = s.hit_sequence.wrapping_add(1);
    if armadillo_roll(
        now_ms,
        object_id as usize,
        s.hit_sequence as usize ^ 0xA124,
        4,
    ) == 0
    {
        s.fleeing = false;
    }
}

impl ZoneRuntime {
    pub(super) fn clear_armadillo_target_life(&mut self, target: u32) {
        for monster in self.native_monsters.values_mut() {
            if let Some(state) = monster
                .special_ai
                .as_mut()
                .and_then(|s| s.armadillo.as_mut())
            {
                state.entity_hits.retain(|h| h.target.object_id() != target);
                if state
                    .retreat
                    .as_ref()
                    .is_some_and(|hit| hit.target_object_id == target)
                {
                    state.retreat = None;
                }
            }
        }
    }
    /// Called before ordinary hostile player targeting. None retains generic
    /// chase/turn; Some always consumes the decision (including cooldown).
    pub(super) fn tick_armadillo_player_ai(
        &mut self,
        id: u32,
        now_ms: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let monster = self.native_monsters.get(&id)?.clone();
        if !matches!(monster.ai, 124 | 125) {
            return None;
        }
        let target = self.selected_native_entity_target(id, now_ms).or_else(|| {
            self.native_entity_targets(
                id,
                &monster.position,
                crystal_monster_by_name(&monster.name).map_or(7, |t| i32::from(t.view_range)),
                EntityTargetPurpose::Search,
                now_ms,
            )
            .into_iter()
            .next()
        })?;
        self.set_native_entity_target(id, &target.reference, now_ms);
        let source = self.native_entity_monster_ref(id)?;
        let direction = zone_direction_toward(&monster.position, &target.position)?;
        let fleeing = monster
            .special_ai
            .as_ref()
            .and_then(|s| s.armadillo.as_ref())
            .is_some_and(|s| s.fleeing);
        if fleeing {
            if !great_fox_movement_ready(&monster, now_ms) {
                return Some(Vec::new());
            }
            let away = zone_rotated_direction(direction, 4);
            let sign = if armadillo_roll(now_ms, id as usize, 0xAF, 2) == 0 {
                1
            } else {
                -1
            };
            for step in 0..8 {
                let d = zone_rotated_direction(away, step * sign);
                let destination = offset_point(&monster.position, d, 1);
                if self.can_native_monster_occupy(id, &destination) {
                    return Some(self.armadillo_move(id, destination, d, None, now_ms));
                }
            }
            self.native_monsters.get_mut(&id)?.next_ai_ready_at_ms =
                now_ms.saturating_add(monster.move_speed_ms);
            return Some(Vec::new());
        }
        if !native_monster_adjacent_to(&monster.position, &target.position) {
            if !great_fox_movement_ready(&monster, now_ms) {
                return Some(Vec::new());
            }
            let destination = offset_point(&monster.position, direction, 1);
            return Some(if self.can_native_monster_occupy(id, &destination) {
                self.armadillo_move(id, destination, direction, None, now_ms)
            } else {
                self.turn_native_monster(id, direction, now_ms)
            });
        }
        if super::entity_combat::native_entity_attack_blocked(&monster, now_ms)
            || now_ms < monster.next_attack_ready_at_ms
        {
            return Some(Vec::new());
        }
        let m = self.native_monsters.get_mut(&id)?;
        m.direction = direction;
        m.next_ai_ready_at_ms = now_ms.saturating_add(300);
        m.next_attack_ready_at_ms = now_ms.saturating_add(m.attack_speed_ms);
        let s = m
            .special_ai
            .get_or_insert_with(Default::default)
            .armadillo
            .get_or_insert_with(Default::default);
        s.attack_sequence = s.attack_sequence.wrapping_add(1);
        let sequence = s.attack_sequence;
        let branch = armadillo_roll(now_ms, id as usize, sequence as usize ^ 0xA6, 6);
        let (min_dc, max_dc) = crystal_monster_by_name(&monster.name).map(|t| {
            (
                t.min_dc,
                t.max_dc.saturating_add(zone_native_monster_buff_stat_total(
                    &monster,
                    CRYSTAL_STAT_MAX_DC,
                )),
            )
        })?;
        if branch == 0 {
            if monster.ai == 125 {
                s.fleeing = true;
            }
            let away = zone_rotated_direction(direction, 4);
            let intermediate = offset_point(&monster.position, away, 1);
            let destination = offset_point(&monster.position, away, 2);
            // Crystal's first validation loop accidentally checks the first
            // tile twice. Validate both here so a retreat cannot enter walls.
            if !self.can_native_monster_occupy(id, &intermediate)
                || !self.can_native_monster_occupy(id, &destination)
            {
                return Some(Vec::new());
            }
            if monster.ai == 124 && max_dc > 0 {
                self.native_monsters
                    .get_mut(&id)?
                    .special_ai
                    .as_mut()?
                    .armadillo
                    .as_mut()?
                    .retreat = Some(RetreatHit {
                    due_ms: now_ms.saturating_add(900),
                    target_session_id: None,
                    source_ref: Some(source.clone()),
                    target_ref: Some(target.reference.clone()),
                    target_object_id: target.object_id,
                    damage: max_dc,
                });
            }
            return Some(self.armadillo_move(id, destination, direction, Some(2), now_ms));
        }
        let packet = ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: id,
                location: monster.position.clone(),
                direction,
                spell: 0,
                level: 0,
                attack_type: if branch == 1 { 1 } else { 0 },
            },
        };
        let recipients =
            self.native_monster_combat_recipients(id, target.object_id, &target.position);
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let mut out = vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![packet],
        }];
        let count = if branch == 1 && monster.ai == 124 {
            3
        } else {
            1
        };
        for index in 0..count {
            let damage =
                zone_roll_stat_range(min_dc, max_dc, now_ms, id, 0xA000 + sequence + index);
            if damage == 0 {
                break;
            }
            if branch == 1 && monster.ai == 125 {
                out.extend(match &target.reference {
                    ZoneCombatEntityRef::Player { session_id, .. } => self.armadillo_push_player(
                        &NativeMonsterTarget {
                            session_id: session_id.clone(),
                            object_id: target.object_id,
                            position: target.position.clone(),
                        },
                        direction,
                        now_ms,
                    ),
                    ZoneCombatEntityRef::Monster { object_id, .. } => {
                        self.great_fox_pull_monster(*object_id, direction, 2, now_ms)
                    }
                });
                break;
            }
            let damage = if branch == 1 {
                damage / 2
            } else if monster.ai == 125 {
                damage.saturating_mul(2)
            } else {
                damage
            };
            self.native_monsters
                .get_mut(&id)?
                .special_ai
                .as_mut()?
                .armadillo
                .as_mut()?
                .entity_hits
                .push(ArmadilloEntityHit {
                    due_ms: now_ms.saturating_add(400 + index * 200),
                    source: source.clone(),
                    target: target.reference.clone(),
                    damage,
                });
        }
        Some(out)
    }

    fn armadillo_move(
        &mut self,
        id: u32,
        destination: Point,
        direction: MirDirection,
        backstep: Option<u8>,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let before: BTreeSet<_> = self
            .players
            .iter()
            .filter_map(|(session, p)| {
                p.visible_object_ids
                    .contains(&id)
                    .then_some(session.clone())
            })
            .collect();
        let m = self.native_monsters.get_mut(&id).expect("armadillo");
        m.position = destination.clone();
        m.direction = direction;
        m.next_ai_ready_at_ms = now_ms.saturating_add(if backstep.is_some() {
            300
        } else {
            m.move_speed_ms
        });
        let movement = ObjectMovement {
            object_id: id,
            position: destination.clone(),
            direction,
        };
        let packet = if let Some(distance) = backstep {
            ServerPacket::ObjectBackStep {
                movement,
                distance: i32::from(distance),
            }
        } else {
            ServerPacket::ObjectWalk { movement }
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let mut out = self.diff_all_zone_object_visibility();
        out.push(ZoneOutbound::ToMany {
            session_ids: self
                .native_monster_visible_recipients(id, &destination)
                .into_iter()
                .filter(|s| before.contains(s))
                .collect(),
            packets: vec![packet],
        });
        out
    }

    fn armadillo_push_player(
        &mut self,
        target: &NativeMonsterTarget,
        direction: MirDirection,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let mut position = target.position.clone();
        let mut packets = Vec::new();
        let mut owner_packets = Vec::new();
        let before: BTreeSet<_> = self
            .players
            .iter()
            .filter_map(|(session, p)| {
                p.visible_object_ids
                    .contains(&target.object_id)
                    .then_some(session.clone())
            })
            .collect();
        let reverse = zone_rotated_direction(direction, 4);
        for _ in 0..2 {
            let destination = offset_point(&position, direction, 1);
            if !self.can_player_movement_occupy(&destination, Some(&target.session_id)) {
                break;
            }
            self.occupancy.remove(&tile_key(&position));
            self.occupancy
                .insert(tile_key(&destination), target.session_id.clone());
            let p = self.players.get_mut(&target.session_id).expect("target");
            p.position = destination.clone();
            p.direction = reverse;
            // Shared intent adapter: discard movement authored before the push.
            p.movement_actions.clear();
            p.run_step_until_ms = 0;
            if let Some(map) = mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name) {
                p.chat_profile.in_safe_zone = map.safe_zones.iter().any(|zone| {
                    zone_tile_distance(&zone.location, &destination) <= i32::from(zone.size)
                });
            }
            self.player_grid.moved(&target.session_id, &destination);
            self.ecs.move_player(&target.session_id, &destination);
            packets.push(ServerPacket::ObjectPushed {
                object_id: target.object_id,
                location: destination.clone(),
                direction: reverse,
            });
            owner_packets.push(ServerPacket::Pushed {
                location: destination.clone(),
                direction: reverse,
            });
            position = destination;
        }
        // HumanObject.Pushed finishes with ActionTime = now + 500, including
        // an occupied first tile that prevents displacement. The Zone keeps
        // movement, physical attack and spell admission on separate clocks.
        // Preserve any longer cooldown while applying the shared action lock.
        let player = self.players.get_mut(&target.session_id).expect("target");
        let action_ready_at_ms = now_ms.saturating_add(500);
        player.movement_actions.clear();
        player.run_step_until_ms = 0;
        player.movement_ready_at_ms = player.movement_ready_at_ms.max(action_ready_at_ms);
        player.next_attack_ready_at_ms = player.next_attack_ready_at_ms.max(action_ready_at_ms);
        player.next_spell_ready_at_ms = player.next_spell_ready_at_ms.max(action_ready_at_ms);
        if packets.is_empty() {
            return Vec::new();
        }
        let mut out = self.diff_visibility_for(&target.session_id);
        out.extend(self.diff_all_zone_object_visibility());
        out.push(ZoneOutbound::ToMany {
            session_ids: self
                .player_status_recipients(target.object_id, &position)
                .into_iter()
                .filter(|s| {
                    s != &target.session_id
                        && before.contains(s)
                        && self
                            .players
                            .get(s)
                            .is_some_and(|p| p.visible_object_ids.contains(&target.object_id))
                })
                .collect(),
            packets,
        });
        owner_packets.push(user_location_packet(&self.players[&target.session_id]));
        out.push(ZoneOutbound::ToSession {
            session_id: target.session_id.clone(),
            packets: owner_packets,
        });
        out
    }

    pub(super) fn tick_armadillo_retreat_hits(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let mut due = Vec::new();
        for m in self.native_monsters.values_mut() {
            if let Some(state) = m.special_ai.as_mut().and_then(|s| s.armadillo.as_mut()) {
                let mut future = Vec::new();
                for hit in std::mem::take(&mut state.entity_hits) {
                    if now_ms >= hit.due_ms {
                        due.push(hit);
                    } else {
                        future.push(hit);
                    }
                }
                state.entity_hits = future;
            }
        }
        let mut out = Vec::new();
        for h in due {
            out.extend(
                self.resolve_native_entity_hit(
                    &h.source,
                    &h.target,
                    h.damage,
                    EntityDefence::ACAgility,
                    false,
                    now_ms,
                )
                .1,
            );
        }
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                m.special_ai
                    .as_ref()
                    .and_then(|s| s.armadillo.as_ref())
                    .and_then(|s| s.retreat.as_ref())
                    .filter(|r| now_ms >= r.due_ms)
                    .map(|_| id)
            })
            .collect();
        for id in ids {
            let m = self.native_monsters.get_mut(&id).expect("pending attacker");
            let pending = m
                .special_ai
                .as_mut()
                .unwrap()
                .armadillo
                .as_mut()
                .unwrap()
                .retreat
                .take()
                .unwrap();
            let position = m.position.clone();
            let friendly_guild = m.friendly_guild.clone();
            if let (Some(source), Some(target)) = (&pending.source_ref, &pending.target_ref) {
                if !self.native_entity_can_attack(
                    source,
                    target,
                    EntityTargetPurpose::Impact,
                    now_ms,
                ) {
                    continue;
                }
                let targets = self.native_entity_targets(
                    id,
                    &position,
                    2,
                    EntityTargetPurpose::VisibleImpact,
                    now_ms,
                );
                for victim in targets {
                    let (damage, events) = self.resolve_native_entity_hit(
                        source,
                        &victim.reference,
                        pending.damage,
                        EntityDefence::AC,
                        false,
                        now_ms,
                    );
                    out.extend(events);
                    if damage <= 0 {
                        if let Some(state) = self
                            .native_monsters
                            .get_mut(&id)
                            .and_then(|m| m.special_ai.as_mut())
                            .and_then(|s| s.armadillo.as_mut())
                        {
                            state.fleeing = true;
                        }
                    }
                }
                continue;
            }
            let Some(legacy_session) = pending.target_session_id else {
                continue;
            };

            let friendly = |p: &ZonePlayer| {
                friendly_guild
                    .as_deref()
                    .zip(p.chat_profile.guild_name.as_deref())
                    .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            };
            if !self.players.get(&legacy_session).is_some_and(|p| {
                p.object_id == pending.target_object_id
                    && !p.dead
                    && !p.chat_profile.in_safe_zone
                    && !friendly(p)
            }) {
                continue;
            }
            let targets: Vec<_> = self
                .players
                .values()
                .filter(|p| {
                    !p.dead
                        && !p.chat_profile.in_safe_zone
                        && !p.hidden
                        && !friendly(p)
                        && zone_tile_distance(&p.position, &position) <= 2
                })
                .map(|p| (p.session_id.clone(), p.object_id, p.hp))
                .collect();
            for (session, object_id, before) in targets {
                out.extend(self.resolve_pending_native_player_hit(
                    PendingNativePlayerHit {
                        ready_at_ms: now_ms,
                        attacker_object_id: id,
                        attacker_ai: 124,
                        target_session_id: session.clone(),
                        target_object_id: object_id,
                        damage: pending.damage,
                        magic: false,
                    },
                    now_ms,
                ));
                if self.players.get(&session).is_some_and(|p| p.hp >= before) {
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .armadillo
                        .as_mut()
                        .unwrap()
                        .fleeing = true;
                }
            }
        }
        out
    }
}

// Avalanche the input before reduction: the legacy linear helper has only two
// outcomes modulo six when only time varies, suppressing entire attack branches.
fn armadillo_roll(now_ms: u64, object_id: usize, sequence: usize, upper: u64) -> u64 {
    let mut state = now_ms
        ^ (object_id as u64).rotate_left(32)
        ^ (sequence as u64).wrapping_mul(0xD1B54A32D192ED03);
    let threshold = upper.wrapping_neg() % upper;
    loop {
        state = state.wrapping_add(0x9E3779B97F4A7C15);
        let mut value = state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
        value ^= value >> 31;
        if value >= threshold {
            return value % upper;
        }
    }
}
