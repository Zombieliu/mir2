//! Source-defined supplements to the existing shared pet path, not a second pet world.
use super::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct PetSpecialState {
    pub search_at: u64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct PetSpecialWorld {
    pub vampire: Vec<PetVampirePool>,
    pub toad_hits: Vec<PetToadHit>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PetVampirePool {
    pub owner: SessionId,
    pub owner_id: u32,
    pub amount: u16,
    pub due: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PetToadHit {
    pub source: u32,
    pub target: u32,
    pub owner: SessionId,
    pub owner_id: u32,
    pub damage: i32,
    pub due: u64,
}
pub(in crate::runtime::zone) fn initialize_pet_special(
    m: &mut ZoneNativeMonster,
    id: u32,
    now: u64,
) {
    if matches!(m.ai, 60 | 61 | 62) {
        m.next_ai_ready_at_ms = m.next_ai_ready_at_ms.max(now.saturating_add(1000));
    }
    if matches!(m.ai, 62 | 255) {
        m.direction = MirDirection::Up;
    }
    if m.ai == 61 {
        m.special_ai
            .get_or_insert_with(Default::default)
            .pet_special
            .get_or_insert_with(|| PetSpecialState {
                search_at: now + zone_roll_stat_range(0, 2999, now, id, 61) as u64,
            });
    }
}
impl ZoneRuntime {
    pub(super) fn expire_shared_spitting_toads(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                if m.ai != 61 || m.dead || m.owner_session_id.is_none() {
                    return None;
                }
                let owner = m.owner_session_id.as_ref().unwrap();
                let absent = !self.players.get(owner).is_some_and(|p| {
                    p.object_id == zone_native_summon_owner_player_object_id(m)
                        && zone_tile_distance(&p.position, &m.position) <= 15
                });
                let expired = self
                    .objects
                    .get(&id)
                    .and_then(|o| o.expires_at_ms)
                    .is_some_and(|due| now > due);
                (absent || expired).then_some(id)
            })
            .collect();
        ids.into_iter()
            .flat_map(|id| self.finish_shared_spitting_toad(id, now))
            .collect()
    }
    pub(super) fn clear_pet_special_owner(&mut self, id: u32) {
        if let Some(w) = self.pet_special_world.as_mut() {
            w.vampire.retain(|v| v.owner_id != id);
            w.toad_hits.retain(|h| h.owner_id != id);
        }
    }
    pub(super) fn invalidate_pet_special_source(&mut self, id: u32) {
        if let Some(w) = self.pet_special_world.as_mut() {
            w.toad_hits.retain(|h| h.source != id && h.target != id);
        }
    }
    // Return whether MasterVampire's Bleeding visual is required, even if integer
    // truncation produces zero reserve. A zero reserve resets the next arrival.
    pub(super) fn queue_pet_vampire(&mut self, source: u32, damage: i32, now: u64) -> bool {
        let Some(m) = self.native_monsters.get(&source).filter(|m| m.ai == 60) else {
            return false;
        };
        if damage <= 0 {
            return false;
        }
        let Some(owner) = m.owner_session_id.clone() else {
            return false;
        };
        let owner_id = zone_native_summon_owner_player_object_id(m);
        if !self
            .players
            .get(&owner)
            .is_some_and(|p| p.object_id == owner_id && !p.dead)
        {
            return false;
        }
        let level = m.summon_skill_level;
        let amount = (damage as f32 * (f32::from(level) + 1.0) * 0.25) as u64 as u16;
        let w = self.pet_special_world.get_or_insert_with(Default::default);
        if let Some(pool) = w
            .vampire
            .iter_mut()
            .find(|p| p.owner == owner && p.owner_id == owner_id)
        {
            if pool.amount == 0 {
                pool.due = now + 1000;
            }
            pool.amount = pool.amount.wrapping_add(amount);
        } else {
            w.vampire.push(PetVampirePool {
                owner,
                owner_id,
                amount,
                due: now + 1000,
            });
        }
        true
    }
    pub(super) fn tick_pet_special_world(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let Some(mut w) = self.pet_special_world.take() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        w.vampire.retain_mut(|p| {
            if !self
                .players
                .get(&p.owner)
                .is_some_and(|owner| owner.object_id == p.owner_id && !owner.dead)
            {
                return false;
            }
            if now > p.due {
                p.due = now + 500;
                let amount = p.amount.min(10);
                p.amount -= amount;
                if amount > 0 {
                    out.extend(self.apply_native_player_heal(p.owner.clone(), i32::from(amount)));
                }
            }
            p.amount > 0
        });
        w.toad_hits.retain(|h| {
            if now < h.due {
                return true;
            }
            if !self.objects.contains_key(&h.source)
                || !self
                    .players
                    .get(&h.owner)
                    .is_some_and(|p| p.object_id == h.owner_id && !p.dead)
            {
                return false;
            }
            let Some(target) = self
                .native_monsters
                .get(&h.target)
                .filter(|m| !m.dead && m.hostile_to_player && monster_visibility_is_attackable(m))
            else {
                return false;
            };
            let damage = zone_magic_damage_after_monster_armour(target, h.damage, h.source, now);
            if damage > 0 {
                out.extend(self.resolve_pending_native_monster_hit(
                    PendingNativeMonsterHit {
                        ready_at_ms: now,
                        session_id: h.owner.clone(),
                        attacker_object_id: h.source,
                        object_id: h.target,
                        damage,
                        fire_bounce: None,
                    },
                    now,
                ));
            }
            false
        });
        self.pet_special_world = Some(w);
        out
    }
    pub(super) fn tick_shared_spitting_toad(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let Some(m) = self
            .native_monsters
            .get(&id)
            .filter(|m| m.ai == 61)
            .cloned()
        else {
            return Vec::new();
        };
        if m.dead || now < m.next_ai_ready_at_ms || native_monster_control_active(&m, now) {
            return Vec::new();
        }
        let Some(owner) = m.owner_session_id.clone() else {
            return Vec::new();
        };
        let owner_id = zone_native_summon_owner_player_object_id(&m);
        let valid_owner = self.players.get(&owner).is_some_and(|p| {
            !p.dead && p.object_id == owner_id && zone_tile_distance(&p.position, &m.position) <= 15
        });
        let expired = self
            .objects
            .get(&id)
            .and_then(|o| o.expires_at_ms)
            .is_some_and(|due| now > due);
        if !valid_owner || expired {
            return self.finish_shared_spitting_toad(id, now);
        }
        let search_at = m
            .special_ai
            .as_ref()
            .and_then(|s| s.pet_special.as_ref())
            .map_or(0, |s| s.search_at);
        if now < search_at {
            return Vec::new();
        }
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .get_or_insert_with(Default::default)
            .pet_special
            .get_or_insert_with(Default::default)
            .search_at = now + 3000;
        let Some(target) = self
            .nearest_native_summon_target(id, &m.position)
            .filter(|t| {
                let d = zone_tile_distance(&m.position, &t.position);
                d > 0 && d <= 12
            })
        else {
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .next_ai_ready_at_ms = now + 300;
            return Vec::new();
        };
        if super::entity_combat::native_entity_attack_blocked(&m, now)
            || now <= m.next_attack_ready_at_ms
        {
            return Vec::new();
        }
        let dir = zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
        let t = crystal_monster_by_name(&m.name);
        let damage = t.map_or(0, |t| zone_roll_stat_range(t.min_dc, t.max_dc, now, id, 61));
        let live = self.native_monsters.get_mut(&id).unwrap();
        live.direction = dir;
        live.next_ai_ready_at_ms = now + 300;
        live.next_attack_ready_at_ms = now + live.attack_speed_ms + 500;
        if damage > 0 {
            self.pet_special_world
                .get_or_insert_with(Default::default)
                .toad_hits
                .push(PetToadHit {
                    source: id,
                    target: target.object_id,
                    owner: owner.clone(),
                    owner_id,
                    damage,
                    due: now + zone_tile_distance(&m.position, &target.position) as u64 * 50 + 500,
                });
        }
        let packet = ServerPacket::ObjectRangeAttack {
            info: ObjectRangeAttackInfo {
                object_id: id,
                location: m.position.clone(),
                direction: dir,
                target_id: target.object_id,
                target: target.position.clone(),
                attack_type: 0,
                spell: 0,
                level: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_action_recipients(
                &owner,
                id,
                target.object_id,
                &target.position,
            ),
            packets: vec![packet],
        }]
    }
    fn finish_shared_spitting_toad(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let Some((_, _, killed, _, _, position, direction, _, _, _)) =
            self.apply_native_monster_scripted_death_damage(id, None, now)
        else {
            return Vec::new();
        };
        if !killed {
            return Vec::new();
        }
        if let Some(object) = self.objects.get_mut(&id) {
            object.expires_at_ms = Some(now.saturating_add(180000));
        }
        let packet = ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: id,
                location: position.clone(),
                direction,
                kind: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &position),
            packets: vec![packet],
        }]
    }
}
