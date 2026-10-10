//! One manager-owned, ephemeral read of its admitted map Players lists.
//! This is neither a loaded Source map catalogue nor an arena reservation.
use super::*;
use super::super::online_identity::OnlineOwner;

/// Never serialized, restored, or admitted from a client payload. A Gateway
/// must additionally bind its complete owner lease and invocation/transport
/// epoch before installing this read in a personal NPC interpreter.
#[derive(Debug, Clone)]
pub struct ZoneNpcPopulationReadSet {
    owner: OnlineOwner,
    actor_key: ZoneKey,
    actor_life: u64,
    counts: BTreeMap<ZoneKey, Option<usize>>,
}

impl ZoneNpcPopulationReadSet {
    pub fn actor_key(&self) -> &ZoneKey { &self.actor_key }
    pub fn actor_life(&self) -> u64 { self.actor_life }
    pub fn actor_object_id(&self) -> u32 { self.owner.object_id }
    pub fn matches_actor(&self, account_id: &str, character_index: i32, object_id: u32) -> bool {
        self.owner.account_id == account_id && self.owner.character_index == character_index
            && self.owner.object_id == object_id
    }
    /// None includes an absent/lazy Zone and an inconsistent population. It
    /// must not become zero or a failed CHECKHUM branch. The Source map loader
    /// and catalogue establish separately whether a queried map exists.
    pub fn count_for_key(&self, key: &ZoneKey) -> Option<usize> {
        self.counts.get(key).copied().flatten()
    }
}

impl ZoneManager {
    /// The caller owns the manager's existing single-writer boundary. Death
    /// and AOI do not remove a Node; an actual Leave does. Every counted Node
    /// must still match the Book and the manager's reverse session route. An
    /// orphan Book owner cannot silently vanish from the map population.
    pub fn capture_npc_population(
        &self, session_id: &SessionId, encoded_owner: &str, expected_key: &ZoneKey,
        expected_life: u64, expected_object_id: u32,
    ) -> Option<ZoneNpcPopulationReadSet> {
        let owner = self.online_identities.owner(session_id)?;
        if owner.encoded() != encoded_owner || owner.object_id != expected_object_id
            || self.session_zones.get(session_id) != Some(expected_key)
            || self.player_life_generation(session_id) != Some(expected_life)
        { return None; }
        let zone = self.zones.get(expected_key)?;
        if zone.player_object_id(session_id)?.0 != expected_object_id
            || zone.player_identity(session_id)? !=
                (session_id.clone(), owner.account_id.clone(), owner.character_index)
        { return None; }
        if self.online_identities.owners().any(|(session, owner)|
            session != &owner.session_id || !self.session_zones.contains_key(session))
        { return None; }
        let counts = self.zones.iter().map(|(key, zone)|
            (key.clone(), zone.npc_population_count(&self.online_identities, &self.session_zones)))
            .collect::<BTreeMap<_, _>>();
        counts.get(expected_key).copied().flatten()?;
        Some(ZoneNpcPopulationReadSet {
            owner: owner.clone(), actor_key: expected_key.clone(), actor_life: expected_life, counts,
        })
    }

    /// This checks the actor binding only. Counts are point-in-time, and a
    /// retained A -> B -> A transfer can match this binding again. The Gateway
    /// must bind its own invocation and presence epoch and recapture for every
    /// read. This predicate is not freshness or reservation authority.
    pub fn npc_population_actor_binding_matches(&self, read: &ZoneNpcPopulationReadSet) -> bool {
        self.online_identities.owner(&read.owner.session_id) == Some(&read.owner)
            && self.session_zones.get(&read.owner.session_id) == Some(&read.actor_key)
            && self.player_life_generation(&read.owner.session_id) == Some(read.actor_life)
            && self.zones.get(&read.actor_key).is_some_and(|zone|
                zone.player_object_id(&read.owner.session_id).map(|id| id.0) == Some(read.owner.object_id)
                && zone.player_identity(&read.owner.session_id) == Some((read.owner.session_id.clone(),
                    read.owner.account_id.clone(), read.owner.character_index)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::{MirClass, MirGender};
    use super::super::super::types::{ZoneChatProfile, ZonePlayerCombatStats};

    fn join(map: &str, session: &str, object_id: u32, x: i32) -> ZoneJoin {
        ZoneJoin {
            session_id: SessionId::new(session), account_id: format!("{session}-account"),
            character_index: 0, object_id, name: session.into(), class: MirClass::Warrior,
            gender: MirGender::Male, level: 7, hp: 60, max_hp: 60, mp: 0,
            map_file_name: map.into(), position: Point { x, y: 270 }, direction: MirDirection::Down,
            chat_profile: ZoneChatProfile::default(), combat_stats: ZonePlayerCombatStats::default(),
        }
    }
    fn capture(manager: &ZoneManager, session: &str) -> ZoneNpcPopulationReadSet {
        let session = SessionId::new(session);
        let key = manager.zone_key_for_session(&session).unwrap();
        let owner = manager.online_owner_proof_for_session(&session).unwrap();
        let object_id = manager.zones[&key].player_object_id(&session).unwrap().0;
        manager.capture_npc_population(&session, &owner, &key,
            manager.player_life_generation(&session).unwrap(), object_id).unwrap()
    }

    #[test]
    fn population_keeps_dead_and_out_of_aoi_players_until_actual_leave() {
        let mut manager = ZoneManager::new();
        manager.join(join("isolated-population", "actor", 1, 330));
        let remote_outbounds = manager.join(join("isolated-population", "remote", 2, 900));
        manager.join(join("other-population", "other", 3, 331));
        let actor = SessionId::new("actor");
        let key = manager.zone_key_for_session(&actor).unwrap();
        assert_eq!(manager.player_life_generation(&actor), Some(0));
        assert!(manager.online_owner_proof_for_session(&SessionId::new("remote")).is_some());
        assert!(!remote_outbounds.iter().filter_map(|outbound| match outbound {
            ZoneOutbound::ToSession { session_id, packets } if session_id == &actor => Some(packets),
            ZoneOutbound::ToMany { session_ids, packets } if session_ids.contains(&actor) => Some(packets),
            ZoneOutbound::ToAll { packets } => Some(packets),
            _ => None,
        }).flatten().any(|packet| matches!(packet, mir2_protocol::ServerPacket::ObjectPlayer { info }
            if info.object_id == 2)));
        let remote = SessionId::new("remote");
        manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
            session_id: remote.clone(), hp: 0, max_hp: 60, mp: 0, dead: true,
        });
        assert_eq!(manager.player_is_dead(&remote), Some(true));
        let read = capture(&manager, "actor");
        assert_eq!(read.count_for_key(&key), Some(2));
        assert_eq!(read.count_for_key(&ZoneKey::for_map("other-population")), Some(1));
        assert_eq!(read.count_for_key(&ZoneKey::for_map("absent-population")), None);
        manager.handle(ZoneCommand::Leave { session_id: remote });
        assert_eq!(capture(&manager, "actor").count_for_key(&key), Some(1));
        // A read is point-in-time; the old value is not presented as fresh.
        assert_eq!(read.count_for_key(&key), Some(2));
    }

    #[test]
    fn actor_binding_rejects_wrong_node_map_life_and_a_new_login_epoch() {
        let mut manager = ZoneManager::new();
        let original = join("isolated-population", "actor", 1, 330);
        manager.join(original.clone());
        let read = capture(&manager, "actor");
        assert!(manager.npc_population_actor_binding_matches(&read));
        let session = SessionId::new("actor");
        let owner = manager.online_owner_proof_for_session(&session).unwrap();
        for (encoded, key, life, object) in [
            ("{}".to_string(), read.actor_key.clone(), 0, 1),
            (owner.clone(), ZoneKey::for_map("other-map"), 0, 1),
            (owner.clone(), read.actor_key.clone(), 1, 1),
            (owner.clone(), read.actor_key.clone(), 0, 2),
        ] {
            assert!(manager.capture_npc_population(&session, &encoded, &key, life, object).is_none());
        }
        manager.handle(ZoneCommand::Leave { session_id: session.clone() });
        assert!(!manager.npc_population_actor_binding_matches(&read));
        assert!(manager.capture_npc_population(&session, &owner, &read.actor_key, 0, 1).is_none());
        manager.join(original);
        assert_eq!(manager.player_life_generation(&session), Some(0));
        assert!(!manager.npc_population_actor_binding_matches(&read));
        assert!(manager.npc_population_actor_binding_matches(&capture(&manager, "actor")));
    }

    #[test]
    fn an_inconsistent_counted_node_is_unavailable_instead_of_disappearing() {
        let mut manager = ZoneManager::new();
        manager.join(join("isolated-population", "actor", 1, 330));
        manager.join(join("isolated-population", "remote", 2, 331));
        manager.online_identities.revoke(&SessionId::new("remote"));
        let session = SessionId::new("actor");
        let owner = manager.online_owner_proof_for_session(&session).unwrap();
        let key = manager.zone_key_for_session(&session).unwrap();
        assert_eq!(manager.zones[&key].player_count(), 2);
        assert!(manager.capture_npc_population(&session, &owner, &key, 0, 1).is_none());
    }

    #[test]
    fn reverse_session_route_and_missing_life_cannot_be_replaced_by_zero() {
        let mut manager = ZoneManager::new();
        manager.join(join("isolated-population", "actor", 1, 330));
        manager.join(join("isolated-population", "remote", 2, 331));
        let actor = SessionId::new("actor");
        let owner = manager.online_owner_proof_for_session(&actor).unwrap();
        let key = manager.zone_key_for_session(&actor).unwrap();
        manager.session_zones.remove(&SessionId::new("remote"));
        assert!(manager.online_owner_proof_for_session(&SessionId::new("remote")).is_some());
        assert!(manager.capture_npc_population(&actor, &owner, &key, 0, 1).is_none());
        manager.session_zones.insert(SessionId::new("remote"), key.clone());
        assert!(manager.capture_npc_population(&actor, &owner, &key, 0, 1).is_some());
        manager.session_zones.remove(&actor);
        assert_eq!(manager.player_life_generation(&actor), None);
        assert!(manager.capture_npc_population(&actor, &owner, &key, 0, 1).is_none());
    }

    #[test]
    fn a_real_map_change_invalidates_the_old_actor_read_and_keeps_the_departed_map_zero() {
        let mut manager = ZoneManager::new();
        manager.join(join("isolated-population", "actor", 1, 330));
        let old = capture(&manager, "actor");
        manager.join(join("new-population", "actor", 1, 330));
        assert!(!manager.npc_population_actor_binding_matches(&old));
        let current = capture(&manager, "actor");
        assert_eq!(current.actor_key(), &ZoneKey::for_map("new-population"));
        assert_eq!(current.count_for_key(old.actor_key()), Some(0));
        assert_eq!(current.count_for_key(current.actor_key()), Some(1));
    }

    #[test]
    fn another_manager_namespace_and_cold_bytes_do_not_authorize_an_old_read() {
        let mut original = ZoneManager::new();
        original.join(join("isolated-population", "actor", 1, 330));
        let read = capture(&original, "actor");
        let mut another = ZoneManager::new();
        another.join(join("isolated-population", "actor", 1, 330));
        assert!(!another.npc_population_actor_binding_matches(&read));
        let actor = SessionId::new("actor");
        assert!(another.capture_npc_population(&actor, &read.owner.encoded(), read.actor_key(), 0, 1).is_none());
        let mut cold = ZoneManager::restore_checkpoint(&original.checkpoint_bytes().unwrap()).unwrap();
        assert!(!cold.npc_population_actor_binding_matches(&read));
        assert!(cold.capture_npc_population(&actor, &read.owner.encoded(), read.actor_key(), 0, 1).is_none());
        cold.join(join("isolated-population", "actor", 1, 330));
        assert!(!cold.npc_population_actor_binding_matches(&read));
        assert!(cold.npc_population_actor_binding_matches(&capture(&cold, "actor")));
    }

    #[test]
    fn orphan_owner_and_route_are_unavailable_instead_of_known_zero() {
        let mut manager = ZoneManager::new();
        manager.join(join("isolated-population", "actor", 1, 330));
        let actor = SessionId::new("actor");
        let owner = manager.online_owner_proof_for_session(&actor).unwrap();
        let key = manager.zone_key_for_session(&actor).unwrap();
        let orphan = join("other-population", "orphan", 2, 331);
        manager.online_identities.admit(&orphan, false).unwrap();
        assert!(manager.capture_npc_population(&actor, &owner, &key, 0, 1).is_none());
        manager.session_zones.insert(orphan.session_id.clone(), key.clone());
        assert!(manager.capture_npc_population(&actor, &owner, &key, 0, 1).is_none());
        manager.online_identities.revoke(&orphan.session_id);
        assert!(manager.capture_npc_population(&actor, &owner, &key, 0, 1).is_none());
        manager.session_zones.remove(&orphan.session_id);
        assert_eq!(capture(&manager, "actor").count_for_key(&key), Some(1));
        manager.join(orphan.clone());
        let other_key = manager.zone_key_for_session(&orphan.session_id).unwrap();
        // A present but empty Zone with a dangling reverse route is not an
        // owned empty Source map. Reject the census without manufacturing zero.
        manager.zones.get_mut(&other_key).unwrap().handle(ZoneCommand::Leave {
            session_id: orphan.session_id,
        });
        assert_eq!(manager.zones[&other_key].player_count(), 0);
        assert_eq!(capture(&manager, "actor").count_for_key(&other_key), None);
    }

    #[test]
    fn returning_to_a_retained_map_does_not_refresh_an_old_read() {
        let mut manager = ZoneManager::new();
        manager.join(join("isolated-population", "actor", 1, 330));
        let read = capture(&manager, "actor");
        manager.join(join("new-population", "actor", 1, 330));
        assert!(!manager.npc_population_actor_binding_matches(&read));
        manager.join(join("isolated-population", "remote", 2, 331));
        manager.join(join("isolated-population", "actor", 1, 330));
        assert!(manager.npc_population_actor_binding_matches(&read));
        assert_eq!(read.count_for_key(read.actor_key()), Some(1));
        assert_eq!(capture(&manager, "actor").count_for_key(read.actor_key()), Some(2));
    }
}
