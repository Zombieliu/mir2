//! Crystal VampireSpider60: immediate MACAgility bite and same-update death burst.
use super::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct VampireState {
    death_marked: bool,
    pending: Option<VampireDeath>,
}

#[cfg(test)]
mod death_tests {
    use super::*;
    use mir2_protocol::MirGender;
    fn fixture() -> (ZoneRuntime, SessionId, ZoneMonsterSpawn) {
        let mut z = ZoneRuntime::new(ZoneKey::for_map("vampire-central-death"));
        let owner = SessionId::new("owner");
        z.handle(ZoneCommand::Join(ZoneJoin {
            session_id: owner.clone(),
            account_id: "test".into(),
            character_index: 1,
            object_id: 101,
            name: "owner".into(),
            class: MirClass::Archer,
            gender: MirGender::Male,
            level: 50,
            hp: 100,
            max_hp: 100,
            mp: 0,
            map_file_name: "vampire-central-death".into(),
            position: Point { x: 19, y: 20 },
            direction: MirDirection::Right,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        let spawn = ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 60,
            name: "VampireSpider".into(),
            name_colour_argb: -1,
            image: 359,
            ai: 60,
            disposition: Some(crate::config::WorldEntityDisposition::Friendly),
            level: 10,
            max_hp: 200,
            hp: 200,
            experience: 0,
            move_speed_ms: 300,
            attack_speed_ms: 2500,
            friendly_guild: None,
            position: Point { x: 20, y: 20 },
            direction: MirDirection::Right,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        };
        assert!(z.spawn_authoritative_monster(&spawn, 0).0);
        let m = z.native_monsters.get_mut(&60).unwrap();
        m.owner_session_id = Some(owner.clone());
        m.master_object_id = 101;
        m.owner_player_object_id = 101;
        m.summon_skill_level = 2;
        initialize_vampire(m);
        let mut target = spawn.clone();
        target.object_id = 9001;
        target.name = "Scarecrow".into();
        target.ai = 0;
        target.disposition = Some(crate::config::WorldEntityDisposition::Hostile);
        target.position = Point { x: 21, y: 20 };
        target.hp = 100;
        target.max_hp = 100;
        assert!(z.spawn_authoritative_monster(&target, 0).0);
        (z, owner, target)
    }
    #[test]
    fn ordinary_fatal_damage_bursts_once_even_when_owner_is_dead() {
        let (mut z, owner, _) = fixture();
        z.handle(ZoneCommand::SyncPlayerVitals {
            session_id: owner,
            hp: 0,
            max_hp: 100,
            mp: 0,
        });
        assert!(
            z.apply_native_monster_direct_damage(60, 200, None, 100)
                .unwrap()
                .2
        );
        assert!(z.vampire_death_pending(60));
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(z.flush_vampire_deaths(), restored.flush_vampire_deaths());
        assert_eq!(z.native_monsters[&9001].hp, 80);
        assert!(z.flush_vampire_deaths().is_empty());
        assert!(
            z.pet_special_world
                .as_ref()
                .is_none_or(|w| w.vampire.is_empty()),
            "dead owner gets no healing reserve, but remains the burst's owner"
        );
    }
    #[test]
    fn captured_explosion_does_not_damage_a_revived_zombie_incarnation() {
        let (mut z, _, mut target) = fixture();
        target.ai = 25;
        target.name = "RevivingZombie".into();
        let mut selected = false;
        for born in 0..32 {
            z.despawn_world_event_monster(9001, born);
            assert!(z.spawn_authoritative_monster(&target, born).0);
            if z.native_monsters[&9001]
                .special_ai
                .as_ref()
                .unwrap()
                .revival
                .as_ref()
                .unwrap()
                .life_count
                > 0
            {
                selected = true;
                break;
            }
        }
        assert!(selected);
        let previous = z.native_monsters[&9001].incarnation;
        assert!(
            z.apply_native_monster_direct_damage(60, 200, None, 100)
                .unwrap()
                .2
        );
        let mut control = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        control.flush_vampire_deaths();
        assert_eq!(control.native_monsters[&9001].hp, 80);
        assert!(
            z.apply_native_monster_direct_damage(9001, 100, None, 101)
                .unwrap()
                .2
        );
        let due = z.native_monsters[&9001]
            .special_ai
            .as_ref()
            .unwrap()
            .revival
            .as_ref()
            .unwrap()
            .revive_at_ms
            .unwrap();
        z.tick_native_monster_revivals(due + 1);
        assert!(!z.native_monsters[&9001].dead);
        assert_ne!(z.native_monsters[&9001].incarnation, previous);
        let hp = z.native_monsters[&9001].hp;
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(z.flush_vampire_deaths(), restored.flush_vampire_deaths());
        assert_eq!(z.native_monsters[&9001].hp, hp);
    }
    #[test]
    fn lethal_explosion_awards_its_real_owner_and_drop_exactly_once() {
        let (mut z, owner, mut target) = fixture();
        z.despawn_world_event_monster(9001, 0);
        target.hp = 10;
        target.experience = 7;
        target.drops = vec![crate::GroundDropSnapshot {
            object_id: 0,
            name: "Gold".into(),
            name_colour_argb: -1,
            icon: 0,
            x: 0,
            y: 0,
            quantity: 1,
            source_monster: "Scarecrow".into(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: crate::GroundDropLootSnapshot::Gold { amount: 9 },
        }];
        assert!(z.spawn_authoritative_monster(&target, 0).0);
        assert!(
            z.apply_native_monster_direct_damage(60, 200, None, 100)
                .unwrap()
                .2
        );
        let out = z.flush_vampire_deaths();
        let awards: Vec<_> = out
            .iter()
            .filter_map(|o| match o {
                ZoneOutbound::MonsterKillAward { session_id, award } => Some((session_id, award)),
                _ => None,
            })
            .collect();
        assert_eq!(awards.len(), 1);
        assert_eq!(awards[0].0, &owner);
        assert_eq!(awards[0].1.experience, 7);
        assert_eq!(awards[0].1.drops.len(), 1);
        assert_eq!(z.ground_drops.len(), 1);
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert!(restored.flush_vampire_deaths().is_empty());
        restored.mark_vampire_death(60, 101);
        assert!(restored.flush_vampire_deaths().is_empty());
        assert_eq!(restored.ground_drops.len(), 1);
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VampireDeath {
    at: u64,
    targets: Vec<u32>,
}
pub(in crate::runtime::zone) fn initialize_vampire(m: &mut ZoneNativeMonster) {
    if m.ai == 60 {
        m.special_ai
            .get_or_insert_with(Default::default)
            .vampire
            .get_or_insert_with(Default::default);
    }
}
impl ZoneRuntime {
    fn vampire_target_eligible(
        &self,
        source: &ZoneNativeMonster,
        target: &ZoneNativeMonster,
    ) -> bool {
        target.hostile_to_player
            && !target.dead
            && target.hp > 0
            && !matches!(target.ai, 57 | 68)
            && monster_visibility_is_attackable(target)
            && horned_encounter_is_attack_target(target)
            && !source
                .owner_session_id
                .as_ref()
                .and_then(|s| self.players.get(s))
                .and_then(|p| p.chat_profile.guild_name.as_deref())
                .zip(target.friendly_guild.as_deref())
                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
    }
    /// Central alive->dead hook. Capture targets before other actors can move.
    pub(super) fn mark_vampire_death(&mut self, id: u32, now: u64) {
        let Some(m) = self
            .native_monsters
            .get(&id)
            .filter(|m| m.ai == 60 && m.dead)
            .cloned()
        else {
            return;
        };
        initialize_vampire(self.native_monsters.get_mut(&id).unwrap());
        if self.native_monsters[&id]
            .special_ai
            .as_ref()
            .unwrap()
            .vampire
            .as_ref()
            .unwrap()
            .death_marked
        {
            return;
        }
        let targets = self
            .native_monsters
            .iter()
            .filter_map(|(&other, t)| {
                (other != id
                    && zone_tile_distance(&m.position, &t.position) <= 1
                    && !self.collision.is_blocked(&t.position)
                    && self.vampire_target_eligible(&m, t))
                .then_some(other)
            })
            .collect();
        let s = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .vampire
            .as_mut()
            .unwrap();
        s.death_marked = true;
        s.pending = Some(VampireDeath { at: now, targets });
        if let Some(object) = self.objects.get_mut(&id) {
            object.expires_at_ms = Some(now.saturating_add(180000));
        }
    }
    pub(super) fn vampire_death_pending(&self, id: u32) -> bool {
        self.native_monsters
            .get(&id)
            .and_then(|m| m.special_ai.as_ref())
            .and_then(|s| s.vampire.as_ref())
            .is_some_and(|s| s.pending.is_some())
    }
    pub(super) fn invalidate_vampire_object(&mut self, id: u32) {
        for (&source, m) in self.native_monsters.iter_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.vampire.as_mut()) {
                if source == id {
                    s.pending = None;
                } else if let Some(p) = s.pending.as_mut() {
                    p.targets.retain(|target| *target != id);
                }
            }
        }
    }
    /// Flush before returning handle/tick output. Each living source can mark
    /// once; processing takes the mark before damage, so chains terminate in
    /// at most the number of native vampire lives present in this update.
    pub(super) fn flush_vampire_deaths(&mut self) -> Vec<ZoneOutbound> {
        let mut out = Vec::new();
        let budget = self.native_monsters.len();
        for _ in 0..budget {
            let Some(id) = self.native_monsters.iter().find_map(|(&id, m)| {
                m.special_ai
                    .as_ref()
                    .and_then(|s| s.vampire.as_ref())
                    .and_then(|s| s.pending.as_ref())
                    .map(|_| id)
            }) else {
                break;
            };
            let pending = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .vampire
                .as_mut()
                .unwrap()
                .pending
                .take()
                .unwrap();
            let source = self.native_monsters[&id].clone();
            let damage = i32::from(source.summon_skill_level).saturating_mul(10);
            for target in pending.targets {
                out.extend(self.vampire_damage_monster(id, target, damage, pending.at));
            }
        }
        out
    }
    pub(super) fn try_tick_vampire_attack(
        &mut self,
        id: u32,
        now: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if m.ai != 60 {
            return None;
        }
        if m.dead {
            return Some(Vec::new());
        }
        let owner = m
            .owner_session_id
            .as_ref()
            .and_then(|s| self.players.get(s))
            .filter(|p| !p.dead && p.object_id == zone_native_summon_owner_player_object_id(&m))?;
        let _ = owner;
        let target = self.nearest_native_summon_target(id, &m.position)?;
        if zone_tile_distance(&m.position, &target.position) != 1 {
            return None;
        }
        if super::entity_combat::native_entity_attack_blocked(&m, now) {
            return Some(Vec::new());
        }
        if now < m.next_ai_ready_at_ms || now < m.next_attack_ready_at_ms {
            return Some(Vec::new());
        }
        let direction = zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
        let t = crystal_monster_by_name(&m.name)?;
        let damage = zone_roll_stat_range(
            t.min_dc,
            t.max_dc
                .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_DC)),
            now,
            id,
            0x60,
        );
        let live = self.native_monsters.get_mut(&id)?;
        live.direction = direction;
        live.next_ai_ready_at_ms = now.saturating_add(300);
        live.next_attack_ready_at_ms = now.saturating_add(live.attack_speed_ms);
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
        let mut out = vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(
                id,
                target.object_id,
                &target.position,
            ),
            packets: vec![packet],
        }];
        if target.position == offset_point(&m.position, direction, 1) {
            out.extend(self.vampire_damage_monster(id, target.object_id, damage, now));
        }
        Some(out)
    }
    fn vampire_damage_monster(
        &mut self,
        id: u32,
        target_id: u32,
        raw: i32,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if raw <= 0 {
            return Vec::new();
        }
        let Some(source) = self.native_monsters.get(&id).cloned() else {
            return Vec::new();
        };
        let Some(target) = self
            .native_monsters
            .get(&target_id)
            .filter(|t| self.vampire_target_eligible(&source, t))
            .cloned()
        else {
            return Vec::new();
        };
        let Some(owner) = source.owner_session_id.clone().filter(|s| {
            self.players
                .get(s)
                .is_some_and(|p| p.object_id == zone_native_summon_owner_player_object_id(&source))
        }) else {
            return Vec::new();
        };
        let Some(t) = crystal_monster_by_name(&source.name) else {
            return Vec::new();
        };
        if crate::runtime::combat::crystal_accuracy_roll(
            now,
            id,
            target_id,
            target.defense.agility.max(0) as u64 + 1,
        ) > t.accuracy.max(0) as u64
        {
            return Vec::new();
        }
        let damage = zone_magic_damage_after_monster_armour(&target, raw, id, now);
        if damage <= 0 {
            return Vec::new();
        }
        let Some((
            damage,
            percent,
            killed,
            name,
            experience,
            position,
            direction,
            reward_owner,
            drops,
            boss_audit,
        )) = self.apply_native_monster_direct_damage(target_id, damage, Some(&owner), now)
        else {
            return Vec::new();
        };
        let mut packets = vec![
            ServerPacket::ObjectStruck {
                info: ObjectStruckInfo {
                    object_id: target_id,
                    attacker_id: id,
                    location: position.clone(),
                    direction,
                },
            },
            ServerPacket::DamageIndicator {
                damage,
                damage_type: 0,
                object_id: target_id,
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: target_id,
                    percent,
                    expire: 0,
                },
            },
        ];
        if killed {
            packets.push(ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: target_id,
                    location: position.clone(),
                    direction,
                    kind: 0,
                },
            });
        }
        if damage > 0 && self.queue_pet_vampire(id, damage, now) {
            packets.push(ServerPacket::ObjectEffect {
                info: ObjectEffectInfo {
                    object_id: target_id,
                    effect: CRYSTAL_SPELL_EFFECT_BLEEDING,
                    effect_type: 0,
                    delay_time: 0,
                    time: 0,
                },
            });
        }
        self.apply_zone_object_packets(&packets, now);
        let mut out = vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_action_recipients(&owner, id, target_id, &position),
            packets,
        }];
        if killed {
            let reward = reward_owner.unwrap_or(owner);
            if let Some(player) = self.players.get(&reward) {
                let owner_id = player.object_id;
                let drops = self.spawn_native_monster_drops(&name, &position, owner_id, drops, now);
                out.extend(self.diff_all_zone_object_visibility());
                out.extend(self.group_monster_kill_awards(
                    &reward,
                    ZoneMonsterKillAward {
                        source_receipt_key: None,
                        experience_selection: None,
                        monster_object_id: target_id,
                        killed_at_ms: now,
                        monster_name: name,
                        experience,
                        drops,
                        boss_audit,
                    },
                ));
            }
        }
        out
    }
}
