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
        // Prepare an AC-only target so MACAgility deterministically hits
        // agility zero and subtracts zero MAC in this lifecycle fixture.
        target.defense.min_ac = 1;
        assert!(z.spawn_authoritative_monster(&target, 0).0);
        (z, owner, target)
    }
    #[test]
    fn ordinary_fatal_damage_releases_master_and_cannot_hit_uncontrolled_wild() {
        let (mut z, owner, _) = fixture();
        let death = z.handle(ZoneCommand::SyncPlayerVitals {
            session_id: owner,
            hp: 0,
            max_hp: 100,
            mp: 0,
        });
        assert!(
            z.native_monsters[&60].dead,
            "actual Human death kills his pets"
        );
        assert!(death.iter().any(|out| matches!(out, ZoneOutbound::ToMany { packets, .. }
            if packets.iter().any(|packet| matches!(packet, ServerPacket::ObjectDied { info } if info.object_id == 60)))));
        assert!(
            z.apply_native_monster_direct_damage(60, 200, None, 100)
                .is_none()
        );
        assert!(
            !z.vampire_death_pending(60),
            "death burst flushed in the same handle"
        );
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(z.flush_vampire_deaths(), restored.flush_vampire_deaths());
        assert_eq!(z.native_monsters[&9001].hp, 100);
        assert_eq!(z.native_monsters[&60].master_object_id, 0);
        assert!(z.native_monsters[&60].owner_session_id.is_none());
        assert!(z.flush_vampire_deaths().is_empty());
        assert!(
            z.pet_special_world
                .as_ref()
                .is_none_or(|w| w.vampire.is_empty()),
            "Master-null death burst has no Human healing authority"
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
        z.native_monsters
            .get_mut(&60)
            .unwrap()
            .hallucination_until_ms = 1000;
        let previous = z.native_monsters[&9001].incarnation;
        assert!(
            z.apply_native_monster_direct_damage(60, 200, None, 100)
                .unwrap()
                .2
        );
        let proof = z.native_monsters[&60]
            .special_ai
            .as_ref()
            .unwrap()
            .vampire
            .as_ref()
            .unwrap()
            .pending
            .as_ref()
            .unwrap();
        assert!(proof.source.as_ref().unwrap().can_attack_wild);
        assert!(
            proof.victims.iter().any(|v| v.life.object_id() == 9001),
            "source targets: {:?}; target position {:?}",
            proof.victims,
            z.native_monsters[&9001].position
        );
        assert_eq!(z.native_monsters[&9001].defense.agility, 0);
        assert_eq!(z.native_monsters[&9001].defense.max_mac, 0);
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
        let live_out = z.flush_vampire_deaths();
        let cold_out = restored.flush_vampire_deaths();
        assert_eq!(z.native_monsters[&9001].hp, hp);
        assert_eq!(restored.native_monsters[&9001].hp, hp);
        for output in [&live_out, &cold_out] {
            assert!(output.iter().all(|o| !matches!(o, ZoneOutbound::ToMany { packets, .. }
                if packets.iter().any(|p| matches!(p, ServerPacket::DamageIndicator { object_id: 9001, .. })))));
        }
        assert_eq!(
            z.players[&SessionId::new("owner")].hp,
            80,
            "live original Human Node still receives its own burst"
        );
        assert_eq!(
            restored.players[&SessionId::new("owner")].hp,
            100,
            "cold recovery revokes that Human Node"
        );
    }
    #[test]
    fn lethal_released_explosion_preserves_only_a_preexisting_exp_owner_once() {
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
        assert_eq!(
            z.apply_native_monster_direct_damage(9001, 1, Some(&owner), 90)
                .unwrap()
                .0,
            1
        );
        z.native_monsters
            .get_mut(&60)
            .unwrap()
            .hallucination_until_ms = 1000;
        assert!(
            z.apply_native_monster_direct_damage(60, 200, None, 100)
                .unwrap()
                .2
        );
        let proof = z.native_monsters[&60]
            .special_ai
            .as_ref()
            .unwrap()
            .vampire
            .as_ref()
            .unwrap()
            .pending
            .as_ref()
            .unwrap();
        assert!(proof.source.as_ref().unwrap().can_attack_wild);
        assert!(
            proof.victims.iter().any(|v| v.life.object_id() == 9001),
            "source targets: {:?}; target position {:?}",
            proof.victims,
            z.native_monsters[&9001].position
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

    #[test]
    fn released_burst_can_hit_former_safe_master_without_human_pk_or_vampirism() {
        let (mut z, owner, _) = fixture();
        let mut profile = z.players[&owner].chat_profile.clone();
        profile.in_safe_zone = true;
        z.handle(ZoneCommand::UpdateChatProfile {
            session_id: owner.clone(),
            profile,
        });
        assert!(
            z.apply_native_monster_direct_damage(60, 200, None, 100)
                .unwrap()
                .2
        );
        let out = z.flush_vampire_deaths();
        assert_eq!(z.players[&owner].hp, 80);
        assert!(out.iter().any(|o| matches!(o, ZoneOutbound::PlayerDamaged { session_id, damage: 20, .. } if session_id == &owner)));
        assert!(out.iter().all(|o| !matches!(
            o,
            ZoneOutbound::OwnedPetPlayerKill { .. } | ZoneOutbound::MonsterKillAward { .. }
        )));
        assert!(z.flush_vampire_deaths().is_empty());
        assert!(z.tick_owned_human_vampirism(2000).is_empty());
        assert_eq!(z.players[&owner].hp, 80);
    }

    #[test]
    fn released_burst_rejects_cold_revoked_or_reborn_human_and_untyped_legacy_queue() {
        for change in ["cold", "revive", "legacy"] {
            let (mut z, owner, _) = fixture();
            assert!(
                z.apply_native_monster_direct_damage(60, 200, None, 100)
                    .unwrap()
                    .2
            );
            match change {
                "cold" => {
                    z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap()
                }
                "revive" => {
                    z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                        session_id: owner.clone(),
                        hp: 0,
                        max_hp: 100,
                        mp: 0,
                        dead: true,
                    });
                    z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                        session_id: owner.clone(),
                        hp: 100,
                        max_hp: 100,
                        mp: 0,
                        dead: false,
                    });
                }
                _ => {
                    let state = z
                        .native_monsters
                        .get_mut(&60)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .vampire
                        .as_mut()
                        .unwrap()
                        .pending
                        .as_mut()
                        .unwrap();
                    state.source = None;
                    state.victims.clear();
                }
            }
            z.flush_vampire_deaths();
            assert_eq!(z.players[&owner].hp, 100, "{change}");
            assert_eq!(z.native_monsters[&9001].hp, 100, "{change}");
            assert!(z.flush_vampire_deaths().is_empty());
        }
    }

    #[test]
    fn fresh_authoritative_spawn_reuses_removed_id_but_rejects_old_burst_target_life() {
        let (mut z, _, target) = fixture();
        z.native_monsters
            .get_mut(&60)
            .unwrap()
            .hallucination_until_ms = 1000;
        assert!(
            z.apply_native_monster_direct_damage(60, 200, None, 100)
                .unwrap()
                .2
        );
        let pending = z.native_monsters[&60]
            .special_ai
            .as_ref()
            .unwrap()
            .vampire
            .as_ref()
            .unwrap()
            .pending
            .as_ref()
            .unwrap()
            .clone();
        let source = pending.source.unwrap();
        let old = pending
            .victims
            .into_iter()
            .find(|v| v.life.object_id() == target.object_id)
            .unwrap();
        z.despawn_world_event_monster(target.object_id, 101);
        assert!(z.removed_object_ids.contains(&target.object_id));
        let (spawned, out) = z.spawn_world_event_monster(&target, 102);
        assert!(spawned);
        assert!(out.iter().any(|o| matches!(o, ZoneOutbound::ToMany { packets, .. } | ZoneOutbound::ToSession { packets, .. }
            if packets.iter().any(|p| matches!(p, ServerPacket::ObjectMonster { info } if info.object_id == target.object_id)))));
        let fresh = z.native_entity_monster_ref(target.object_id).unwrap();
        assert_ne!(fresh, old.life);
        assert!(z.entity_ref_exists(&fresh, false));
        assert!(!z.entity_ref_exists(&old.life, false));
        assert!(
            z.resolve_released_vampire_burst(&source, &old, 102)
                .is_empty()
        );
        assert_eq!(z.native_monsters[&target.object_id].hp, 100);
        assert!(!z.removed_object_ids.contains(&target.object_id));
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VampireDeath {
    at: u64,
    // Preserve the legacy checkpoint shape; old untyped queued IDs cannot
    // acquire the authority of a newly issued death burst on cold recovery.
    targets: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<VampireBurstSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    victims: Vec<VampireBurstVictim>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VampireBurstSource {
    life: super::entity_combat::ZoneCombatEntityRef,
    can_attack_wild: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VampireBurstVictim {
    life: super::entity_combat::ZoneCombatEntityRef,
    online: Option<super::super::online_identity::OnlineOwner>,
    had_master: bool,
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
    /// MonsterObject.Die settles Master-dependent rewards before clearing
    /// Master. This method runs after that decision and before subclass Die.
    pub(super) fn release_vampire_master_after_death(&mut self, id: u32) {
        if !self
            .native_monsters
            .get(&id)
            .is_some_and(|m| m.ai == 60 && m.dead)
        {
            return;
        }
        self.retire_native_source_object(id);
        let m = self.native_monsters.get_mut(&id).unwrap();
        m.owner_session_id = None;
        m.master_object_id = 0;
        m.owner_player_object_id = 0;
        if let Some(object) = self.objects.get_mut(&id) {
            if let ServerPacket::ObjectMonster { info } = &mut object.packet {
                info.master_object_id = 0;
                info.name = m.name.clone();
            }
        }
    }

    /// Capture the real source/victim lives before retirement clears controls.
    /// A corpse's burst has no former Human authority; Human/owned targets
    /// still require their original live online Node at impact.
    pub(super) fn mark_vampire_death(&mut self, id: u32, now: u64) {
        use super::entity_combat::ZoneCombatEntityRef;
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
        let Some(life) = self.native_entity_monster_ref(id) else {
            return;
        };
        let can_attack_wild =
            now < m.hallucination_until_ms || self.owned_monster_rage_active(id, now);
        let candidates = self
            .native_monsters
            .keys()
            .filter_map(|other| self.native_entity_monster_ref(*other))
            .chain(
                self.players
                    .keys()
                    .filter_map(|sid| self.native_entity_player_ref(sid)),
            );
        let victims: Vec<_> = candidates
            .filter_map(|target| {
                if target.object_id() == id || !self.entity_ref_exists(&target, false) {
                    return None;
                }
                let (position, had_master, eligible) = match &target {
                    ZoneCombatEntityRef::Player { session_id, .. } => {
                        let p = &self.players[session_id];
                        (p.position.clone(), false, !p.combat_stats.gm_never_die)
                    }
                    ZoneCombatEntityRef::Monster { object_id, .. } => {
                        let t = &self.native_monsters[object_id];
                        let mastered = t.master_object_id != 0 || t.owner_session_id.is_some();
                        (
                            t.position.clone(),
                            mastered,
                            !matches!(t.ai, 57 | 68)
                                && monster_visibility_is_attackable(t)
                                && horned_encounter_is_attack_target(t)
                                && (mastered || can_attack_wild),
                        )
                    }
                };
                let online = self.owned_pet_target_online(&target);
                if !eligible
                    || zone_tile_distance(&m.position, &position) > 1
                    || self.collision.is_blocked(&position)
                    || ((had_master || matches!(&target, ZoneCombatEntityRef::Player { .. }))
                        && online.is_none())
                {
                    return None;
                }
                Some(VampireBurstVictim {
                    life: target,
                    online,
                    had_master,
                })
            })
            .collect();
        let targets = victims.iter().map(|v| v.life.object_id()).collect();
        let state = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .vampire
            .as_mut()
            .unwrap();
        state.death_marked = true;
        state.pending = Some(VampireDeath {
            at: now,
            targets,
            source: Some(VampireBurstSource {
                life,
                can_attack_wild,
            }),
            victims,
        });
        if let Some(object) = self.objects.get_mut(&id) {
            object.expires_at_ms = Some(now.saturating_add(180_000));
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
                // A newly born death burst survives source retirement; its
                // typed corpse proof still fails if the object is removed.
                if source == id && !m.dead {
                    s.pending = None;
                } else if source != id {
                    if let Some(p) = s.pending.as_mut() {
                        p.targets.retain(|target| *target != id);
                        p.victims.retain(|target| target.life.object_id() != id);
                    }
                }
            }
        }
    }

    /// Take before damage. A chain can process each source corpse only once.
    pub(super) fn flush_vampire_deaths(&mut self) -> Vec<ZoneOutbound> {
        let mut out = Vec::new();
        for _ in 0..self.native_monsters.len() {
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
            let Some(source) = pending.source else {
                continue;
            };
            self.release_vampire_master_after_death(id);
            for target in pending.victims {
                out.extend(self.resolve_released_vampire_burst(&source, &target, pending.at));
            }
        }
        out
    }

    /// This is the only safe-zone exception for ordinary native player damage.
    /// Its private issuer proves a dead, Master-null Vampire and both typed
    /// lives. It never relaxes generic owned-entity combat admission.
    fn resolve_released_vampire_burst(
        &mut self,
        source: &VampireBurstSource,
        victim: &VampireBurstVictim,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        use super::entity_combat::{EntityDefence, ZoneCombatEntityRef};
        let id = source.life.object_id();
        if !self.entity_ref_exists(&source.life, true)
            || !self.entity_ref_exists(&victim.life, false)
            || self.owned_pet_target_online(&victim.life) != victim.online
        {
            return Vec::new();
        }
        let m = self.native_monsters[&id].clone();
        if m.ai != 60 || !m.dead || m.master_object_id != 0 || m.owner_session_id.is_some() {
            return Vec::new();
        }
        let (position, agility, resist) = match &victim.life {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let p = &self.players[session_id];
                if p.combat_stats.gm_never_die || victim.online.is_none() {
                    return Vec::new();
                }
                (
                    p.position.clone(),
                    p.combat_stats.agility,
                    p.combat_stats.magic_resist,
                )
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let t = &self.native_monsters[object_id];
                let mastered = t.master_object_id != 0 || t.owner_session_id.is_some();
                if mastered != victim.had_master
                    || (!mastered && !source.can_attack_wild)
                    || matches!(t.ai, 57 | 68)
                    || !monster_visibility_is_attackable(t)
                    || !horned_encounter_is_attack_target(t)
                {
                    return Vec::new();
                }
                (
                    t.position.clone(),
                    t.defense
                        .agility
                        .saturating_add(zone_native_monster_buff_stat_total(t, 11)),
                    zone_native_monster_buff_stat_total(t, 30),
                )
            }
        };
        if zone_tile_distance(&m.position, &position) > 1 || self.collision.is_blocked(&position) {
            return Vec::new();
        }
        let raw = i32::from(m.summon_skill_level) * 10;
        if raw <= 0 {
            return Vec::new();
        }
        let accuracy = crystal_monster_by_name(&m.name)
            .map_or(0, |t| t.accuracy)
            .saturating_add(zone_native_monster_buff_stat_total(&m, 10))
            .max(0);
        if crate::runtime::combat::crystal_accuracy_roll(now, id, victim.life.object_id(), 10)
            < resist.clamp(0, 10) as u64
            || crate::runtime::combat::crystal_accuracy_roll(
                now,
                id,
                victim.life.object_id().wrapping_add(0xEA),
                agility.max(0) as u64 + 1,
            ) > accuracy as u64
        {
            return self.entity_miss_packet(id, &victim.life);
        }
        match &victim.life {
            ZoneCombatEntityRef::Player {
                session_id,
                object_id,
                ..
            } => self.resolve_native_player_damage_kind(
                PendingNativePlayerHit {
                    ready_at_ms: now,
                    attacker_object_id: id,
                    attacker_ai: 60,
                    target_session_id: session_id.clone(),
                    target_object_id: *object_id,
                    damage: raw,
                    magic: true,
                },
                now,
                false,
                true,
                false,
                true,
            ),
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let damage = super::entity_combat::entity_monster_defended_damage(
                    &self.native_monsters[object_id],
                    *object_id,
                    raw,
                    EntityDefence::MACAgility,
                    id,
                    now,
                );
                if damage <= 0 {
                    return self.entity_miss_packet(id, &victim.life);
                }
                let Some(result) = self.apply_native_monster_damage_internal(
                    *object_id,
                    damage,
                    None,
                    now,
                    NativeMonsterDamageCause::Direct,
                    Some(NativeExperienceActor::Object {
                        object_id: id,
                        force_owner: false,
                    }),
                ) else {
                    return Vec::new();
                };
                let (actual, mut out) =
                    self.entity_monster_damage_outcome(id, *object_id, result, Some(id), now);
                if actual > 0 {
                    out.extend(self.release_owned_monster_shock(*object_id, now));
                }
                out
            }
        }
    }
}
