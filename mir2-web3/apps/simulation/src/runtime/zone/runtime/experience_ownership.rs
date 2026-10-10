//! Crystal EXPOwner under one manager's global online Node lifetime.
use super::*;
use crate::runtime::zone::types::ZoneExperienceOwner;

const EXPERIENCE_OWNER_DELAY_MS: u64 = 5_000;
const MASTER_CREDIT_RANGE: i32 = 16;

#[derive(Debug, Clone, Copy)]
pub(super) enum NativeExperienceActor {
    Object { object_id: u32, force_owner: bool },
    PeriodicPoison { owner_object_id: u32 },
}

pub(super) struct NativeExperienceImpact {
    claimant: Option<ZoneExperienceOwner>,
    clears_owner: bool,
    force_owner: bool,
    current_owner_dead: bool,
    current_owner_mismatched: bool,
}

fn claimant(player: &ZonePlayer, online_owner: Option<OnlineOwner>, now_ms: u64) -> ZoneExperienceOwner {
    ZoneExperienceOwner {
        session_id: player.session_id.clone(),
        account_id: player.account_id.clone(),
        character_index: player.character_index,
        object_id: player.object_id,
        expires_at_ms: now_ms.saturating_add(EXPERIENCE_OWNER_DELAY_MS),
        online_owner,
    }
}

impl ZoneRuntime {
    pub(crate) fn online_player_snapshot(&self, book: &OnlineIdentityBook) -> OnlinePresenceSnapshot {
        self.players.iter().filter_map(|(sid, player)| {
            let owner = book.owner(sid).filter(|owner|owner.matches_player(player))?.clone();
            Some((sid.clone(), OnlinePresence { owner, key: self.key.clone(), dead: player.dead,
                owned_pet_clock: self.player_owned_pet_clock(sid),
                life_generation: player.life_generation, guild_name: player.chat_profile.guild_name.clone(),
                chat_profile: Some(player.chat_profile.clone()), brown_until_ms: self.owned_pet_brown_until(player),
                position: player.position.clone(), level: player.level,
                experience_profile: player.experience_profile.clone(),
                name: player.name.clone(), group_members: player.chat_profile.group_members.clone(),
                mentor_bank: player.mentor_bank.clone() }))
        }).collect()
    }
    pub(super) fn refresh_local_online_presence(&mut self) {
        if !self.managed_online_identity {
            let snapshot = self.online_player_snapshot(&self.local_online_identities);
            self.ingest_online_presence(snapshot, false);
        }
    }
    pub(crate) fn ingest_online_presence(&mut self, snapshot: OnlinePresenceSnapshot, managed: bool) {
        self.online_presence = snapshot;
        self.managed_online_identity = managed;
        for monster in self.native_monsters.values_mut() {
            if monster.experience_owner.as_ref().is_some_and(|owner|
                owner.online_owner.as_ref().is_none_or(|proof|self.online_presence.get(&proof.session_id)
                    .is_none_or(|presence|presence.owner != *proof))) {
                monster.experience_owner = None;
            }
            if monster.damage_poison_owner_session_id.as_ref().is_some_and(|sid|
                self.online_presence.get(sid).is_none_or(|p|p.owner.object_id != monster.damage_poison_owner_object_id)) {
                monster.damage_poison_owner_session_id = None;
                monster.damage_poison_owner_object_id = 0;
            }
        }
        for stored in self.ground_drops.values_mut() {
            if stored.native_owner.as_ref().is_some_and(|owner|
                self.online_presence.get(&owner.owner.session_id).is_none_or(|p|p.owner != owner.owner)) {
                stored.owner_expires_at_ms = None;
                stored.drop.ownership_remaining_ticks = None;
            }
        }
    }
    pub(crate) fn revoke_online_session(&mut self, session_id: &SessionId) {
        self.online_presence.remove(session_id);
        self.issued_native_monster_awards.retain(|_, owner| &owner.session_id != session_id);
        // Clear historical poison before a reused session/object can be admitted.
        for monster in self.native_monsters.values_mut() {
            if monster.experience_owner.as_ref().is_some_and(|o|&o.session_id == session_id) {
                monster.experience_owner = None;
            }
            if monster.damage_poison_owner_session_id.as_ref() == Some(session_id) {
                monster.damage_poison_owner_session_id = None;
                monster.damage_poison_owner_object_id = 0;
            }
        }
        self.pending_native_hits.retain(|hit|&hit.session_id != session_id);
        let snapshot = self.online_presence.clone();
        self.ingest_online_presence(snapshot, self.managed_online_identity);
    }
    pub(crate) fn clear_online_authority(&mut self) {
        self.issued_native_monster_awards.clear();
        for monster in self.native_monsters.values_mut() {
            monster.experience_owner = None;
            monster.damage_poison_owner_session_id = None;
            monster.damage_poison_owner_object_id = 0;
        }
        self.ingest_online_presence(BTreeMap::new(), self.managed_online_identity);
        // Legacy owner ids never authenticate a fresh spawned Node either.
        // This does not add an owner to manual/player-death loot.
        for stored in self.ground_drops.values_mut().filter(|s|s.drop.owner_object_id.is_some()) {
            stored.owner_expires_at_ms = None;
            stored.drop.ownership_remaining_ticks = None;
        }
        self.local_online_identities = OnlineIdentityBook::default();
    }
    fn player_claimant(&self, player: &ZonePlayer, now_ms: u64) -> Option<ZoneExperienceOwner> {
        let owner = self.online_presence.get(&player.session_id)
            .filter(|p|p.owner.matches_player(player))?.owner.clone();
        Some(claimant(player, Some(owner), now_ms))
    }
    fn online_claimant(&self, session_id: &SessionId, object_id: u32, now_ms: u64) -> Option<ZoneExperienceOwner> {
        let owner = &self.online_presence.get(session_id).filter(|p|p.owner.object_id == object_id)?.owner;
        Some(ZoneExperienceOwner { session_id: owner.session_id.clone(), account_id: owner.account_id.clone(),
            character_index: owner.character_index, object_id: owner.object_id,
            expires_at_ms: now_ms.saturating_add(EXPERIENCE_OWNER_DELAY_MS), online_owner: Some(owner.clone()) })
    }
    pub(super) fn native_experience_impact(
        &self,
        target_id: u32,
        session_id: Option<&SessionId>,
        actor: Option<NativeExperienceActor>,
        now_ms: u64,
    ) -> NativeExperienceImpact {
        let old = self
            .native_monsters
            .get(&target_id)
            .and_then(|m| m.experience_owner.as_ref());
        let old_player = old.and_then(|o| self.online_presence.get(&o.session_id).map(|p| (o, p)));
        let mut impact = NativeExperienceImpact {
            claimant: None,
            clears_owner: false,
            force_owner: false,
            current_owner_dead: old_player.is_some_and(|(owner, presence)| {
                if owner.online_owner.as_ref() != Some(&presence.owner)
                    || owner.session_id != presence.owner.session_id
                    || owner.account_id != presence.owner.account_id
                    || owner.character_index != presence.owner.character_index
                    || owner.object_id != presence.owner.object_id
                {
                    return false;
                }
                // Crystal reads EXPOwner.Dead at the impact, after earlier
                // damage in this tick. The global snapshot is batch-start
                // state and remains the fallback only for another Zone.
                match self.players.get(&owner.session_id) {
                    Some(player) => presence.owner.matches_player(player) && player.dead,
                    None => presence.key != self.key && presence.dead,
                }
            }),
            current_owner_mismatched: old.is_some_and(|o|o.online_owner.as_ref().is_none_or(|proof|!self.online_owner_current(proof))),
        };
        match actor {
            Some(NativeExperienceActor::Object {
                object_id,
                force_owner,
            }) => {
                if let Some(monster) = self.native_monsters.get(&object_id) {
                    // These source species clear EXPOwner even when Master exists.
                    if matches!(monster.ai, 6 | 58 | 113) {
                        impact.clears_owner = true;
                    } else if monster.owner_session_id.is_some()
                        && monster.master_object_id != zone_native_summon_owner_player_object_id(monster)
                    {
                        // MonsterObject.Attacked(MonsterObject): EXPOwner is
                        // the immediate Master, not the causal Human. A real
                        // CharmedSnake's Master is its incarnation-bound Totem;
                        // MapObject.WinExp is empty for that Monster. Do not
                        // mint/renew Human XP or loot from owner_session_id.
                        // An earlier living Human first hitter keeps custody.
                        if let Some(master) = self
                            .owned_pet_immediate_monster_master(object_id)
                            .and_then(|reference| self.native_monsters.get(&reference.object_id()))
                        {
                            impact.clears_owner = impact.current_owner_dead
                                || !points_within_action_range(
                                    &master.position,
                                    &monster.position,
                                    MASTER_CREDIT_RANGE,
                                );
                        }
                    } else if monster.master_object_id != 0 || monster.owner_session_id.is_some() {
                        let master = monster
                            .owner_session_id
                            .as_ref()
                            .and_then(|sid| self.players.get(sid))
                            .filter(|p| {
                                p.object_id == zone_native_summon_owner_player_object_id(monster)
                            });
                        if let Some(master) = master.filter(|p| {
                            points_within_action_range(
                                &p.position,
                                &monster.position,
                                MASTER_CREDIT_RANGE,
                            )
                        }) {
                            impact.claimant = self.player_claimant(master, now_ms);
                        } else {
                            // One Zone implies the same map. Missing/different
                            // master or out-of-range pet still applies damage.
                            impact.clears_owner = true;
                        }
                    }
                } else if let Some(player) = session_id
                    .and_then(|sid| self.players.get(sid))
                    .filter(|p| p.object_id == object_id)
                {
                    impact.claimant = self.player_claimant(player, now_ms);
                    impact.force_owner = force_owner;
                }
            }
            Some(NativeExperienceActor::PeriodicPoison { owner_object_id }) => {
                // Source periodic poison renews before PoisonDamage and does
                // not reject a dead owner that remains a spawned object.
                impact.claimant = session_id.and_then(|sid|self.online_claimant(sid, owner_object_id, now_ms));
            }
            None => {
                // Internal player spells and scripted special-AI trackers use
                // their trusted acting session; no target ID invents credit.
                impact.claimant = session_id
                    .and_then(|sid| self.players.get(sid))
                    .and_then(|p| self.player_claimant(p, now_ms));
            }
        }
        impact
    }

    pub(super) fn native_experience_reward_owner(
        &self,
        monster_id: u32,
        now_ms: u64,
    ) -> Option<SessionId> {
        let owner = self
            .native_monsters
            .get(&monster_id)?
            .experience_owner
            .as_ref()?;
        if now_ms > owner.expires_at_ms {
            return None;
        }
        // No alive gate here: MonsterObject.Die permits a spawned dead owner.
        owner.online_owner.as_ref().filter(|proof|self.online_owner_current(proof))
            .map(|_| owner.session_id.clone())
    }

    pub(super) fn expire_native_experience_owners(&mut self, now_ms: u64) {
        for monster in self.native_monsters.values_mut() {
            if monster
                .experience_owner
                .as_ref()
                .is_some_and(|o| now_ms > o.expires_at_ms)
            {
                monster.experience_owner = None;
            }
        }
    }

    pub(super) fn revoke_same_map_experience_owner_on_join(
        &mut self,
        session_id: &SessionId,
        object_id: u32,
    ) {
        for monster in self.native_monsters.values_mut() {
            if monster
                .experience_owner
                .as_ref()
                .is_some_and(|o| &o.session_id == session_id || o.object_id == object_id)
            {
                monster.experience_owner = None;
            }
            // Reused same-map IDs cannot make an old despawned poison's owner
            // become the fresh spawned object. Normal map transfer is not yet
            // represented by this fresh-Join security boundary.
            if monster.damage_poison_owner_session_id.as_ref() == Some(session_id)
                || (monster.damage_poison_owner_object_id != 0
                    && monster.damage_poison_owner_object_id == object_id)
            {
                monster.damage_poison_owner_session_id = None;
                monster.damage_poison_owner_object_id = 0;
                monster.damage_poison_expires_at_ms = 0;
                monster.damage_poison_value = 0;
            }
        }
    }

    /// Consume the common death arbitration once, without a fatal-actor
    /// fallback. Ground ownership and the durable personal award envelope
    /// continue to use their existing contracts and drop duration.
    pub(super) fn native_monster_kill_outbounds(
        &mut self,
        owner_session_id: Option<SessionId>,
        position: &Point,
        mut award: ZoneMonsterKillAward,
    ) -> Vec<ZoneOutbound> {
        let Some(owner_session_id) = owner_session_id else {
            return Vec::new();
        };
        if self
            .native_monsters
            .get(&award.monster_object_id)
            .is_some_and(|m| m.master_object_id != 0 || m.owner_session_id.is_some())
        {
            return Vec::new();
        }
        let Some(owner_object_id) = self.online_presence.get(&owner_session_id).map(|p| p.owner.object_id) else {
            return Vec::new();
        };
        award.drops = self.spawn_native_monster_drops(
            &award.monster_name,
            position,
            owner_object_id,
            award.drops,
            award.killed_at_ms,
        );
        let mut out = self.diff_all_zone_object_visibility();
        out.extend(self.group_monster_kill_awards(&owner_session_id, award));
        out
    }
}

pub(super) fn apply_native_experience_impact(
    monster: &mut ZoneNativeMonster,
    impact: NativeExperienceImpact,
    now_ms: u64,
) {
    if impact.clears_owner || impact.current_owner_mismatched {
        monster.experience_owner = None;
    }
    if impact.clears_owner {
        return;
    }
    if monster
        .experience_owner
        .as_ref()
        .is_some_and(|o| now_ms > o.expires_at_ms)
    {
        monster.experience_owner = None;
    }
    let Some(new_owner) = impact.claimant else {
        return;
    };
    if impact.force_owner || monster.experience_owner.is_none() || impact.current_owner_dead {
        monster.experience_owner = Some(new_owner);
    } else if monster.experience_owner.as_ref().is_some_and(|old| {
        old.session_id == new_owner.session_id
            && old.account_id == new_owner.account_id
            && old.character_index == new_owner.character_index
            && old.object_id == new_owner.object_id
            && old.online_owner == new_owner.online_owner
    }) {
        monster.experience_owner.as_mut().unwrap().expires_at_ms = new_owner.expires_at_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldEntityDisposition;
    use mir2_protocol::MirGender;

    // Internal ownership-rule fixture only. It deliberately does not claim
    // that generic AI6/58/113 target selection can choose a wild monster.
    fn rule_fixture(source_ai: u8) -> ZoneRuntime {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map("ownership-rule-fixture"));
        let sid = SessionId::new("owner");
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: sid.clone(),
            account_id: "owner-account".into(),
            character_index: 3,
            object_id: 101,
            name: "Owner".into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 40,
            hp: 100,
            max_hp: 100,
            mp: 100,
            map_file_name: "ownership-rule-fixture".into(),
            position: Point { x: 9, y: 10 },
            direction: MirDirection::Right,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        for (id, ai, position) in [
            (201, 3, Point { x: 10, y: 10 }),
            (301, source_ai, Point { x: 11, y: 10 }),
        ] {
            zone.handle(ZoneCommand::SpawnMonster {
                session_id: sid.clone(),
                now_ms: 0,
                monster: ZoneMonsterSpawn {
                    crystal_drop_seed: None,
                    object_id: id,
                    name: "Skeleton".into(),
                    name_colour_argb: -1,
                    image: 3,
                    ai,
                    disposition: Some(WorldEntityDisposition::Hostile),
                    level: 20,
                    hp: 100,
                    max_hp: 100,
                    experience: 123,
                    move_speed_ms: 100_000,
                    attack_speed_ms: 100_000,
                    friendly_guild: None,
                    defense: Default::default(),
                    respawn: None,
                    position,
                    direction: MirDirection::Left,
                    drops: Vec::new(),
                },
            });
        }
        let hit = zone
            .apply_native_monster_direct_damage(201, 20, Some(&sid), 100)
            .unwrap();
        assert_eq!(hit.0, 20);
        assert!(zone.native_monsters[&201].experience_owner.is_some());
        zone
    }

    fn assert_clearing_source_rule(ai: u8) {
        let mut zone = rule_fixture(ai);
        let result = zone
            .apply_native_monster_damage_internal(
                201,
                80,
                None,
                200,
                NativeMonsterDamageCause::Direct,
                Some(NativeExperienceActor::Object {
                    object_id: 301,
                    force_owner: false,
                }),
            )
            .unwrap();
        assert_eq!(
            result.0, 80,
            "clearing credit cannot suppress actual HP damage"
        );
        assert!(result.2);
        assert!(result.7.is_none());
        assert!(zone.native_monsters[&201].experience_owner.is_none());
    }

    #[test]
    fn source_ai6_clears_prior_exp_owner_rule() {
        assert_clearing_source_rule(6);
    }
    #[test]
    fn source_ai58_clears_prior_exp_owner_rule() {
        assert_clearing_source_rule(58);
    }
    #[test]
    fn source_ai113_clears_prior_exp_owner_rule() {
        assert_clearing_source_rule(113);
    }

    #[test]
    fn ordinary_unowned_source_retains_prior_exp_owner_rule() {
        let mut zone = rule_fixture(0);
        let result = zone
            .apply_native_monster_damage_internal(
                201,
                80,
                None,
                200,
                NativeMonsterDamageCause::Direct,
                Some(NativeExperienceActor::Object {
                    object_id: 301,
                    force_owner: false,
                }),
            )
            .unwrap();
        assert_eq!(result.0, 80);
        assert_eq!(result.7, Some(SessionId::new("owner")));
    }

    #[test]
    fn zero_direct_damage_does_not_claim_but_periodic_poison_rule_claims_and_renews() {
        let mut zone = rule_fixture(0);
        let sid = SessionId::new("owner");
        zone.native_monsters.get_mut(&201).unwrap().experience_owner = None;
        let before = zone.native_monsters[&201].hp;
        let direct = zone
            .apply_native_monster_direct_damage(201, 0, Some(&sid), 200)
            .unwrap();
        assert_eq!(direct.0, 0);
        assert!(zone.native_monsters[&201].experience_owner.is_none());
        let poison = zone
            .apply_native_monster_damage_internal(
                201,
                0,
                Some(&sid),
                200,
                NativeMonsterDamageCause::Indirect,
                Some(NativeExperienceActor::PeriodicPoison {
                    owner_object_id: 101,
                }),
            )
            .unwrap();
        assert_eq!(poison.0, 0);
        assert_eq!(zone.native_monsters[&201].hp, before);
        assert_eq!(
            zone.native_monsters[&201]
                .experience_owner
                .as_ref()
                .unwrap()
                .expires_at_ms,
            5_200
        );
        zone.apply_native_monster_direct_damage(201, 0, Some(&sid), 5_000);
        assert_eq!(
            zone.native_monsters[&201]
                .experience_owner
                .as_ref()
                .unwrap()
                .expires_at_ms,
            5_200
        );
        zone.apply_native_monster_damage_internal(
            201,
            0,
            Some(&sid),
            5_000,
            NativeMonsterDamageCause::Indirect,
            Some(NativeExperienceActor::PeriodicPoison {
                owner_object_id: 101,
            }),
        );
        assert_eq!(zone.native_monsters[&201].hp, before);
        assert_eq!(
            zone.native_monsters[&201]
                .experience_owner
                .as_ref()
                .unwrap()
                .expires_at_ms,
            10_000
        );
        // This is an internal rule test, not evidence that current public
        // player green-poison producers create a zero-valued tick.
    }

    #[test]
    fn legacy_v4_forward_claim_and_forced_impact_are_neutralized() {
        let mut zone = rule_fixture(0);
        zone.pending_native_hits.push(PendingNativeMonsterHit {
            ready_at_ms: 1_000,
            session_id: SessionId::new("owner"),
            attacker_object_id: 101,
            object_id: 201,
            damage: 20,
            force_experience_owner: false,
            fire_bounce: None,
            soulfire_practice: None,
            journey_event: None,
        });
        let root = zone.legacy_v4_canonical_state_root().unwrap();
        let mut state: serde_json::Value =
            serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
        state["version"] = serde_json::json!(4);
        state["state_root"] = serde_json::json!(root);
        state["native_monsters"]["201"]["experience_owner"]["expires_at_ms"] =
            serde_json::json!(99_999);
        state["pending_native_hits"][0]["force_experience_owner"] = serde_json::json!(true);
        let restored =
            ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(restored.native_monsters[&201].experience_owner.is_none());
        assert!(!restored.pending_native_hits[0].force_experience_owner);
        assert_eq!(restored.pending_native_hits[0].ready_at_ms, 1_000);
        assert_eq!(restored.native_monsters[&201].hp, 80);
        let reanchored: serde_json::Value =
            serde_json::from_slice(&restored.checkpoint_bytes().unwrap()).unwrap();
        // The current checkpoint contract includes mining (v7). Reanchoring
        // a v4 file must still retire legacy claim authority and forced hits.
        assert_eq!(reanchored["version"], 7);
        assert!(reanchored["native_monsters"]["201"]
            .get("experience_owner")
            .is_none());
        let current =
            ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&reanchored).unwrap()).unwrap();
        assert!(current.native_monsters[&201].experience_owner.is_none());
        assert!(!current.pending_native_hits[0].force_experience_owner);
        assert_eq!(current.pending_native_hits[0].ready_at_ms, 1_000);
    }
}
