//! Shared AI25 death/revival lifecycle.
//!
//! Reference: Crystal RevivingZombie.cs and MonsterObject.Die/Revive/Process.
//! RNG is a checkpointed per-instance stream, not Crystal's global RNG stream.
//! OPEN: the Zone's one-object-per-respawn-slot model cannot reproduce Crystal's
//! live-count replenishment while an older corpse is waiting to self-revive.
//! The caller must defer ordinary same-ID replacement while revival is pending.

use super::*;
use crate::runtime::drops::zone_ground_drop_snapshots_for_monster_at_tick;

const ZOMBIE_AI: u8 = 25;
const CORPSE_LIFETIME_MS: u64 = 180_000;
const REVIVAL_ACTION_DELAY_MS: u64 = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RevivingZombieState {
    pub life_count: u8,
    pub revival_count: u8,
    pub died_at_ms: Option<u64>,
    pub revive_at_ms: Option<u64>,
    pub corpse_expires_at_ms: Option<u64>,
    pub death_generation: u64,
    pub rng_state: u64,
    pub rng_draws: u64,
    pub last_drop_roll: Option<u64>,
}

impl RevivingZombieState {
    fn new(object_id: u32, born_at_ms: u64, key: &ZoneKey) -> Self {
        // Explicit byte order and stable source identity preserve cross-process
        // replay. No OS randomness or personal-session clock is used here.
        let mut hash = Sha256::new();
        hash.update(b"mir2.zone.reviving-zombie.v1\0");
        hash.update(serde_json::to_vec(key).expect("ZoneKey serialization"));
        hash.update(object_id.to_le_bytes());
        hash.update(born_at_ms.to_le_bytes());
        let digest = hash.finalize();
        let mut state = Self {
            life_count: 0,
            revival_count: 0,
            died_at_ms: None,
            revive_at_ms: None,
            corpse_expires_at_ms: None,
            death_generation: 0,
            rng_state: u64::from_le_bytes(digest[..8].try_into().expect("digest seed")),
            rng_draws: 0,
            last_drop_roll: None,
        };
        state.life_count = state.roll_below(3) as u8;
        state
    }

    fn next_random(&mut self) -> u64 {
        self.rng_state = self.rng_state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        self.rng_draws = self.rng_draws.saturating_add(1);
        let mut value = self.rng_state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn roll_below(&mut self, upper: u64) -> u64 {
        // Rejection avoids modulo bias in the 0..2 and 0..19 contracts.
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let roll = self.next_random();
            if roll >= threshold {
                return roll % upper;
            }
        }
    }
}

impl ZoneRuntime {
    /// Call only after inserting a fresh incarnation, not on metadata refresh.
    pub(super) fn initialize_native_monster_revival(&mut self, object_id: u32, now_ms: u64) {
        let Some(monster) = self.native_monsters.get_mut(&object_id) else {
            return;
        };
        if monster.ai != ZOMBIE_AI {
            return;
        }
        let special = monster.special_ai.get_or_insert_with(Default::default);
        if special.revival.is_none() {
            special.revival = Some(RevivingZombieState::new(object_id, now_ms, &self.key));
        }
    }

    /// Call once at the central alive -> dead transition, after releasing its
    /// mutable monster borrow. Overrides that death's EXP and cached loot.
    /// None on a repeated call means no second reward should be issued.
    pub(super) fn revival_death_reward(
        &mut self,
        object_id: u32,
        now_ms: u64,
    ) -> Option<(u32, Vec<GroundDropSnapshot>)> {
        if self.native_monsters.get(&object_id)?.ai != ZOMBIE_AI {
            return None;
        }
        // Also supports authenticated legacy checkpoints which predate this
        // optional state. New spawns initialize at construction instead.
        self.initialize_native_monster_revival(object_id, now_ms);
        let monster = self.native_monsters.get_mut(&object_id)?;
        if !monster.dead || monster.hp > 0 {
            return None;
        }
        let state = monster.special_ai.as_mut()?.revival.as_mut()?;
        if state.died_at_ms.is_some() {
            return None;
        }
        state.death_generation = state.death_generation.checked_add(1)?;
        let delay_ms = (4 + state.roll_below(20)) * 1_000;
        state.died_at_ms = Some(now_ms);
        // Keep the final death's random delay too: Crystal draws even when
        // RevivalCount has already reached LifeCount.
        state.revive_at_ms = Some(now_ms.saturating_add(delay_ms));
        state.corpse_expires_at_ms = Some(now_ms.saturating_add(CORPSE_LIFETIME_MS));
        let experience = (u64::from(monster.experience) * u64::from(100 - 25 * state.revival_count)
            / 100) as u32;
        let drop_roll = state.next_random();
        state.last_drop_roll = Some(drop_roll);
        let name = monster.name.clone();
        // Never let a poison from the previous life resume after resurrection.
        monster.damage_poison = 0;
        monster.damage_poison_value = 0;
        monster.damage_poison_next_damage_at_ms = 0;
        monster.damage_poison_expires_at_ms = 0;
        monster.damage_poison_owner_session_id = None;
        monster.damage_poison_owner_object_id = 0;
        monster.control_until_ms = 0;
        monster.control_poison = 0;
        self.revived_object_ids.remove(&object_id);

        // Regenerate from the monster table on EVERY death. Cached assigned
        // player-item carriers must never be cloned into another life. The
        // existing generator produces exact metadata with uid_assigned=false;
        // the established durable pickup allocator assigns fresh item UIDs.
        // OPEN: owner drop bonuses and runtime custom map drop-rule overrides
        // are not part of the existing Zone drop generator's contract.
        let no_drop = mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name)
            .is_some_and(|map| map.no_drop_monster);
        let drops = if no_drop {
            Vec::new()
        } else {
            zone_ground_drop_snapshots_for_monster_at_tick(object_id, &name, drop_roll)
        };
        Some((experience, drops))
    }

    pub(super) fn native_monster_has_pending_revival(&self, object_id: u32) -> bool {
        self.native_monsters.get(&object_id).is_some_and(|monster| {
            monster.ai == ZOMBIE_AI
                && monster.dead
                && monster
                    .special_ai
                    .as_ref()
                    .and_then(|s| s.revival.as_ref())
                    .is_some_and(|state| {
                        state.revival_count < state.life_count
                            && state.revive_at_ms.is_some()
                            && state.corpse_expires_at_ms.is_some()
                    })
        })
    }

    /// Run before ordinary same-ID respawns and before generic monster AI.
    pub(super) fn tick_native_monster_revivals(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, monster)| {
                (monster.ai == ZOMBIE_AI
                    && monster.dead
                    && monster
                        .special_ai
                        .as_ref()
                        .and_then(|s| s.revival.as_ref())
                        .is_some())
                .then_some(id)
            })
            .collect();
        let mut outbounds = Vec::new();
        for object_id in ids {
            // Crystal processes the now-empty PoisonList even on a corpse.
            // Clear its retained appearance on the death tick as well, so a
            // final-life corpse does not keep its old poison for 180 seconds.
            outbounds.extend(self.clear_reviving_zombie_corpse_poison(object_id, now_ms));
            let state = self.native_monsters[&object_id]
                .special_ai
                .as_ref()
                .and_then(|s| s.revival.as_ref())
                .expect("selected zombie state");
            // Crystal removes a 180s corpse BEFORE ProcessAI. A long offline
            // interval therefore cannot cause a belated resurrection.
            if state
                .corpse_expires_at_ms
                .is_some_and(|deadline| now_ms >= deadline)
            {
                outbounds.extend(self.expire_reviving_zombie_corpse(object_id));
                continue;
            }
            if state.revival_count >= state.life_count
                || !state.revive_at_ms.is_some_and(|deadline| now_ms > deadline)
            {
                continue;
            }
            self.invalidate_vampire_object(object_id);
            self.retire_entity_combat_life(object_id);
            let incarnation = self.allocate_monster_incarnation();
            let monster = self
                .native_monsters
                .get_mut(&object_id)
                .expect("reviving zombie");
            monster.incarnation = incarnation;
            monster.entity_poison = 0;
            let state = monster
                .special_ai
                .as_mut()
                .and_then(|s| s.revival.as_mut())
                .expect("reviving state");
            state.revival_count += 1;
            state.died_at_ms = None;
            state.revive_at_ms = None;
            state.corpse_expires_at_ms = None;
            monster.hp = ((i64::from(monster.max_hp) * i64::from(100 - 25 * state.revival_count))
                / 100) as i32;
            monster.dead = false;
            let action_ready_at = now_ms.saturating_add(REVIVAL_ACTION_DELAY_MS);
            // Death clears PoisonList in Crystal, and Revive replaces
            // ActionTime. Keeping a former paralysis deadline here would
            // silently preserve a control effect which no longer exists.
            monster.next_ai_ready_at_ms = action_ready_at;
            monster.next_attack_ready_at_ms = action_ready_at;
            let position = monster.position.clone();
            let health_percent = native_monster_health_percent(monster.hp, monster.max_hp);
            let health_expire = if monster.revelation_until_ms > now_ms {
                u8::try_from(
                    monster
                        .revelation_until_ms
                        .saturating_sub(now_ms)
                        .div_ceil(1_000)
                        .min(255),
                )
                .unwrap_or(255)
            } else {
                0
            };
            // Direct and delayed hits queued against the corpse's prior life
            // cannot be allowed to bypass the new life boundary.
            self.clear_reviving_zombie_life_actions(object_id);
            if let Some(respawn) = self.native_monster_respawns.get_mut(&object_id) {
                respawn.due_at_ms = None;
            }
            let packets = vec![
                ServerPacket::ObjectRevived {
                    info: ObjectRevivedInfo {
                        object_id,
                        effect: false,
                    },
                },
                ServerPacket::ObjectPoisoned {
                    object_id,
                    poison: 0,
                },
                ServerPacket::ObjectHealth {
                    info: ObjectHealthInfo {
                        object_id,
                        percent: health_percent,
                        expire: health_expire,
                    },
                },
            ];
            self.apply_zone_object_packets(&packets, now_ms);
            let recipients = self.native_monster_visible_recipients(object_id, &position);
            if !recipients.is_empty() {
                outbounds.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets,
                });
            }
        }
        outbounds
    }

    fn expire_reviving_zombie_corpse(&mut self, object_id: u32) -> Vec<ZoneOutbound> {
        // Remove the display body without deleting its future respawn rule.
        self.clear_reviving_zombie_life_actions(object_id);
        self.retire_native_source_object(object_id);
        self.objects.remove(&object_id);
        self.object_grid.remove(&object_id);
        self.dead_object_ids.remove(&object_id);
        self.revived_object_ids.remove(&object_id);
        self.harvested_object_ids.remove(&object_id);
        self.removed_object_ids.insert(object_id);
        if self.native_monster_respawns.contains_key(&object_id) {
            if let Some(state) = self
                .native_monsters
                .get_mut(&object_id)
                .and_then(|m| m.special_ai.as_mut())
                .and_then(|s| s.revival.as_mut())
            {
                state.revive_at_ms = None;
                state.corpse_expires_at_ms = None;
            }
        } else {
            self.native_monsters.remove(&object_id);
        }
        let recipients = self
            .players
            .iter_mut()
            .filter_map(|(session, player)| {
                player
                    .visible_object_ids
                    .remove(&object_id)
                    .then_some(session.clone())
            })
            .collect::<Vec<_>>();
        if recipients.is_empty() {
            Vec::new()
        } else {
            vec![ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![ServerPacket::ObjectRemove { object_id }],
            }]
        }
    }

    fn clear_reviving_zombie_life_actions(&mut self, object_id: u32) {
        self.pending_native_hits
            .retain(|hit| hit.object_id != object_id && hit.attacker_object_id != object_id);
        self.pending_native_player_hits
            .retain(|hit| hit.attacker_object_id != object_id);
        self.clear_shinsu_hits_for_target(object_id);
        self.pending_native_projectiles.retain(|projectile| {
            projectile.source_id != object_id && projectile.destination_id != object_id
        });
        // Ground spells are spatial effects with their own lifetimes, rather
        // than attacks aimed at this incarnation. They remain able to affect
        // a revived creature that is still standing inside their area.
    }

    fn clear_reviving_zombie_corpse_poison(
        &mut self,
        object_id: u32,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(position) = self.objects.get(&object_id).and_then(|object| {
            matches!(&object.packet, ServerPacket::ObjectMonster { info } if info.poison != 0)
                .then(|| object.position.clone())
        }) else {
            return Vec::new();
        };
        let packets = vec![ServerPacket::ObjectPoisoned {
            object_id,
            poison: 0,
        }];
        self.apply_zone_object_packets(&packets, now_ms);
        let session_ids = self.native_monster_visible_recipients(object_id, &position);
        if session_ids.is_empty() {
            Vec::new()
        } else {
            vec![ZoneOutbound::ToMany {
                session_ids,
                packets,
            }]
        }
    }
}
