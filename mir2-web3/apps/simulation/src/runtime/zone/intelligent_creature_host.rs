//! Zone-owned Creature projection and lifecycle. Public methods are internal
//! server APIs, not wire commands; callers supply acknowledged personal state.
use super::super::intelligent_creatures::{
    CreatureEvent, CreatureOperation, CreatureOwner, CreatureOwnerPose, CreaturePickupIntent,
    CreatureTarget, ZoneIntelligentCreature,
};
use super::*;
use mir2_protocol::ClientIntelligentCreature;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CreatureHost {
    actor: ZoneIntelligentCreature,
    record: ClientIntelligentCreature,
    allowed: BTreeSet<u32>,
    group_members: Vec<String>,
    map_allows: bool,
    spawn_ms: u64,
    operation_sequence: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::MirGender;
    fn join(s: &str, id: u32, x: i32) -> ZoneJoin {
        ZoneJoin {
            session_id: SessionId::new(s),
            account_id: format!("{s}-account"),
            character_index: id as i32,
            object_id: id,
            name: s.into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 7,
            hp: 60,
            max_hp: 60,
            mp: 100,
            map_file_name: "0".into(),
            position: Point { x, y: 10 },
            direction: MirDirection::Right,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }
    }
    fn record() -> ClientIntelligentCreature {
        ClientIntelligentCreature {
            pet_type: 0,
            icon: 500,
            custom_name: "Pig".into(),
            fullness: 7500,
            slot_index: 0,
            expire_binary_datetime: 0,
            blackstone_time: 0,
            pet_mode: 1,
            maintain_food_time: 0,
            pickup_grade: 0,
            creature_rules: crate::runtime::resources::intelligent_creature_default_rules(0),
            filter: mir2_protocol::IntelligentCreatureItemFilter {
                pet_pickup_all: true,
                pet_pickup_gold: false,
                pet_pickup_weapons: false,
                pet_pickup_armours: false,
                pet_pickup_helmets: false,
                pet_pickup_boots: false,
                pet_pickup_belts: false,
                pet_pickup_accessories: false,
                pet_pickup_others: false,
            },
        }
    }
    fn zone() -> ZoneRuntime {
        let mut z =
            ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
        z.handle(ZoneCommand::Join(join("owner", 100, 10)));
        z.handle(ZoneCommand::Join(join("observer", 101, 12)));
        z
    }
    fn summon(z: &mut ZoneRuntime, allowed: BTreeSet<u32>) -> Vec<ZoneOutbound> {
        z.sync_intelligent_creature(
            &SessionId::new("owner"),
            Some(record()),
            allowed,
            vec![],
            true,
            0,
        )
    }
    fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
        out.iter()
            .flat_map(|o| match o {
                ZoneOutbound::ToSession { packets, .. } | ZoneOutbound::ToMany { packets, .. } => {
                    packets.iter().collect::<Vec<_>>()
                }
                _ => vec![],
            })
            .collect()
    }
    fn drop_at(z: &mut ZoneRuntime, owner: Option<u32>, x: i32) {
        z.handle(ZoneCommand::SyncGroundDrops {
            session_id: SessionId::new("owner"),
            drops: vec![GroundDropSnapshot {
                object_id: 300,
                name: "Gold".into(),
                name_colour_argb: -1,
                icon: 0,
                x,
                y: 10,
                quantity: 1,
                source_monster: "".into(),
                owner_object_id: owner,
                ownership_remaining_ticks: Some(10000),
                loot: GroundDropLootSnapshot::Gold { amount: 1 },
            }],
            now_ms: 0,
        });
    }
    #[test]
    fn creature_host_two_clients_receive_real_nonblocking_aoi_actor() {
        let mut z = zone();
        let output = summon(&mut z, BTreeSet::new());
        let id = z
            .intelligent_creature_object_id(&SessionId::new("owner"))
            .unwrap();
        assert!(!z.native_monsters.contains_key(&id));
        assert!(z
            .players
            .values()
            .all(|p| p.visible_object_ids.contains(&id)));
        assert!(packets(&output).iter().any(|p|matches!(p,ServerPacket::ObjectMonster{info} if info.object_id==id&&info.ai==64&&info.effect==0&&info.image==10000&&info.extra&&info.master_object_id==100)));
        assert!(
            z.can_player_movement_occupy(&Point { x: 11, y: 10 }, Some(&SessionId::new("owner")))
        );
        assert!(z
            .canonical_observer_zone_object_packet(ServerPacket::ObjectRemove { object_id: id })
            .is_none());
        let late = z.handle(ZoneCommand::Join(join("late", 102, 14)));
        assert!(packets(&late)
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectMonster{info} if info.object_id==id)));
    }
    #[test]
    fn creature_host_follows_owner_and_forks_actor_state() {
        let mut z = zone();
        summon(&mut z, BTreeSet::new());
        z.players
            .get_mut(&SessionId::new("owner"))
            .unwrap()
            .position = Point { x: 15, y: 10 };
        let mut fork = z.transaction_fork();
        z.tick_intelligent_creatures(1000);
        fork.tick_intelligent_creatures(1000);
        let id = z
            .intelligent_creature_object_id(&SessionId::new("owner"))
            .unwrap();
        assert_eq!(z.objects[&id].position, fork.objects[&id].position);
        assert_ne!(z.objects[&id].position, Point { x: 11, y: 10 });
    }
    #[test]
    fn creature_host_death_leave_and_restriction_remove_actor() {
        for reason in 0..3 {
            let mut z = zone();
            summon(&mut z, BTreeSet::new());
            let id = z
                .intelligent_creature_object_id(&SessionId::new("owner"))
                .unwrap();
            let output = match reason {
                0 => {
                    z.players.get_mut(&SessionId::new("owner")).unwrap().dead = true;
                    z.tick_intelligent_creatures(1000)
                }
                1 => z.handle(ZoneCommand::Leave {
                    session_id: SessionId::new("owner"),
                }),
                _ => z.sync_intelligent_creature(
                    &SessionId::new("owner"),
                    Some(record()),
                    BTreeSet::new(),
                    vec![],
                    false,
                    1000,
                ),
            };
            assert!(!z.objects.contains_key(&id));
            assert!(z.intelligent_creatures.is_empty());
            assert!(packets(&output)
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectRemove{object_id} if *object_id==id)));
        }
    }
    #[test]
    fn creature_host_forged_candidate_cannot_bypass_ownership_or_filter() {
        let mut z = zone();
        drop_at(&mut z, Some(999), 11);
        summon(&mut z, BTreeSet::from([300, 9999]));
        assert!(!z.request_intelligent_creature_pickup(
            &SessionId::new("owner"),
            false,
            Point { x: 11, y: 10 }
        ));
        assert!(z.drain_intelligent_creature_pickup_intents().is_empty());
        z.ground_drops.get_mut(&300).unwrap().drop.owner_object_id = None;
        let mut pet = record();
        pet.filter.pet_pickup_all = false;
        z.sync_intelligent_creature(
            &SessionId::new("owner"),
            Some(pet),
            BTreeSet::from([300]),
            vec![],
            true,
            0,
        );
        assert!(!z.request_intelligent_creature_pickup(
            &SessionId::new("owner"),
            false,
            Point { x: 11, y: 10 }
        ));
    }
    #[test]
    fn creature_host_one_operation_and_intent_until_settlement_then_dismiss_invalidates() {
        let mut z = zone();
        drop_at(&mut z, None, 11);
        summon(&mut z, BTreeSet::from([300]));
        assert!(z.request_intelligent_creature_pickup(
            &SessionId::new("owner"),
            false,
            Point { x: 11, y: 10 }
        ));
        z.tick_intelligent_creatures(1000);
        let operations = z.drain_intelligent_creature_operations();
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].owner.account_id, "owner-account");
        assert!(z.settle_intelligent_creature_operation(
            &operations[0].owner.session_id,
            &operations[0].operation_id
        ));
        assert!(z.drain_intelligent_creature_pickup_intents().is_empty());
        z.tick_intelligent_creatures(1500);
        let intents = z.drain_intelligent_creature_pickup_intents();
        assert_eq!(intents.len(), 1);
        assert!(z.intelligent_creature_intent_is_current(&intents[0]));
        z.tick_intelligent_creatures(20_000);
        assert!(z.drain_intelligent_creature_pickup_intents().is_empty());
        assert!(z.drain_intelligent_creature_operations().is_empty());
        z.sync_intelligent_creature(
            &SessionId::new("owner"),
            None,
            BTreeSet::new(),
            vec![],
            true,
            21_000,
        );
        assert!(!z.intelligent_creature_intent_is_current(&intents[0]));
    }

    #[test]
    fn creature_host_checkpoint_preserves_pending_timer_rng_and_in_flight_receipt() {
        let mut z = zone();
        drop_at(&mut z, None, 11);
        summon(&mut z, BTreeSet::from([300]));
        z.collision = ZoneCollision::for_map("0");
        z.request_intelligent_creature_pickup(
            &SessionId::new("owner"),
            false,
            Point { x: 11, y: 10 },
        );
        z.tick_intelligent_creatures(1000);
        let bytes = z.checkpoint_bytes().unwrap();
        let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
        assert_eq!(
            z.canonical_state_root().unwrap(),
            restored.canonical_state_root().unwrap()
        );
        let ops = restored.drain_intelligent_creature_operations();
        assert_eq!(ops.len(), 1);
        restored
            .settle_intelligent_creature_operation(&ops[0].owner.session_id, &ops[0].operation_id);
        restored.tick_intelligent_creatures(1499);
        assert!(restored
            .drain_intelligent_creature_pickup_intents()
            .is_empty());
        restored.tick_intelligent_creatures(1500);
        let intent = restored
            .drain_intelligent_creature_pickup_intents()
            .pop()
            .unwrap();
        let mut resumed =
            ZoneRuntime::restore_checkpoint(&restored.checkpoint_bytes().unwrap()).unwrap();
        resumed.tick_intelligent_creatures(30_000);
        assert!(resumed
            .drain_intelligent_creature_pickup_intents()
            .is_empty());
        assert!(resumed.drain_intelligent_creature_operations().is_empty());
        assert!(resumed.settle_intelligent_creature_pickup(
            &intent.owner.session_id,
            intent.creature_object_id,
            intent.target.object_id
        ));
    }
    #[test]
    fn creature_host_checkpoint_commits_actor_seed_and_rejects_tampering() {
        let mut z = zone();
        summon(&mut z, BTreeSet::new());
        z.collision = ZoneCollision::for_map("0");
        let mut checkpoint: serde_json::Value =
            serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
        checkpoint["intelligent_creatures"]["owner"]["actor"]["rng_state"] =
            serde_json::json!(99999);
        assert!(
            ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&checkpoint).unwrap()).is_err()
        );
    }
    #[test]
    fn creature_host_checkpoint_cleans_stale_owner_and_retained_phantom() {
        let mut z = zone();
        summon(&mut z, BTreeSet::new());
        z.collision = ZoneCollision::for_map("0");
        let id = z
            .intelligent_creature_object_id(&SessionId::new("owner"))
            .unwrap();
        z.players.remove(&SessionId::new("owner"));
        let restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert!(restored.intelligent_creatures.is_empty());
        assert!(!restored.objects.contains_key(&id));
        let mut z = zone();
        summon(&mut z, BTreeSet::new());
        z.collision = ZoneCollision::for_map("0");
        let id = z
            .intelligent_creature_object_id(&SessionId::new("owner"))
            .unwrap();
        z.intelligent_creatures.clear();
        let restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert!(!restored.objects.contains_key(&id));
    }

    #[test]
    fn creature_host_manager_owner_scoped_delivery_preserves_other_owner_and_offline_receipts() {
        let mut manager = crate::runtime::zone::ZoneManager::new();
        for (session, id, x, drop_id) in [("owner", 100, 10, 300), ("observer", 101, 14, 301)] {
            let mut joined = join(session, id, x);
            joined.map_file_name = "creature-host-test".into();
            manager.join(joined);
            manager.handle(ZoneCommand::SyncGroundDrops {
                session_id: SessionId::new(session),
                drops: vec![GroundDropSnapshot {
                    object_id: drop_id,
                    name: "Gold".into(),
                    name_colour_argb: -1,
                    icon: 0,
                    x: x + 1,
                    y: 10,
                    quantity: 1,
                    source_monster: "".into(),
                    owner_object_id: Some(id),
                    ownership_remaining_ticks: Some(10000),
                    loot: GroundDropLootSnapshot::Gold { amount: 1 },
                }],
                now_ms: 0,
            });
            manager.sync_intelligent_creature(
                &SessionId::new(session),
                Some(record()),
                BTreeSet::from([drop_id]),
                vec![],
                true,
                0,
            );
            assert!(manager.request_intelligent_creature_pickup(
                &SessionId::new(session),
                false,
                Point { x: x + 1, y: 10 }
            ));
        }
        manager.tick_all(1000);
        let owner = SessionId::new("owner");
        let observer = SessionId::new("observer");
        let owned = manager.drain_intelligent_creature_operations(&owner);
        assert_eq!(owned.len(), 1);
        assert_eq!(manager.drain_intelligent_creature_operations(&owner), owned);
        let other = manager.drain_intelligent_creature_operations(&observer);
        assert_eq!(other.len(), 1);
        assert_ne!(owned[0].operation_id, other[0].operation_id);
        assert!(!manager.settle_intelligent_creature_operation(&observer, &owned[0].operation_id));
        manager.handle(ZoneCommand::Leave {
            session_id: owner.clone(),
        });
        let mut manager = crate::runtime::zone::ZoneManager::restore_checkpoint(
            &manager.checkpoint_bytes().unwrap(),
        )
        .unwrap();
        assert_eq!(manager.drain_intelligent_creature_operations(&owner), owned);
        assert!(manager.settle_intelligent_creature_operation(&owner, &owned[0].operation_id));
        assert!(manager
            .drain_intelligent_creature_operations(&owner)
            .is_empty());
        manager.tick_all(1500);
        assert!(manager
            .drain_intelligent_creature_pickup_intents(&owner)
            .is_empty());
        let intents = manager.drain_intelligent_creature_pickup_intents(&observer);
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].owner.session_id, observer);
        assert_eq!(
            manager.drain_intelligent_creature_operations(&observer),
            other
        );
    }
    #[test]
    fn creature_host_reconnected_identity_recovers_old_session_operation_without_cross_character_access(
    ) {
        let mut manager = crate::runtime::zone::ZoneManager::new();
        let old_session = SessionId::new("owner");
        let mut old = join("owner", 100, 10);
        old.map_file_name = "creature-host-test".into();
        manager.join(old);
        manager.handle(ZoneCommand::SyncGroundDrops {
            session_id: old_session.clone(),
            drops: vec![GroundDropSnapshot {
                object_id: 300,
                name: "Gold".into(),
                name_colour_argb: -1,
                icon: 0,
                x: 11,
                y: 10,
                quantity: 1,
                source_monster: "".into(),
                owner_object_id: Some(100),
                ownership_remaining_ticks: Some(10000),
                loot: GroundDropLootSnapshot::Gold { amount: 1 },
            }],
            now_ms: 0,
        });
        manager.sync_intelligent_creature(
            &old_session,
            Some(record()),
            BTreeSet::from([300]),
            vec![],
            true,
            0,
        );
        assert!(manager.request_intelligent_creature_pickup(
            &old_session,
            false,
            Point { x: 11, y: 10 }
        ));
        manager.tick_all(1000);
        let original = manager.drain_intelligent_creature_operations(&old_session);
        assert_eq!(original.len(), 1);
        manager.handle(ZoneCommand::Leave {
            session_id: old_session.clone(),
        });
        let mut manager = crate::runtime::zone::ZoneManager::restore_checkpoint(
            &manager.checkpoint_bytes().unwrap(),
        )
        .unwrap();
        let mut fresh = join("new-session", 200, 10);
        fresh.account_id = "owner-account".into();
        fresh.character_index = 100;
        fresh.map_file_name = "creature-host-test".into();
        manager.join(fresh);
        assert!(manager
            .drain_intelligent_creature_operations(&SessionId::new("new-session"))
            .is_empty());
        assert_eq!(
            manager.peek_intelligent_creature_operations_for_identity("owner-account", 100),
            original
        );
        assert_eq!(
            manager.peek_intelligent_creature_operations_for_identity("owner-account", 100),
            original
        );
        assert!(manager
            .peek_intelligent_creature_operations_for_identity("owner-account", 101)
            .is_empty());
        assert!(manager
            .peek_intelligent_creature_operations_for_identity("other-account", 100)
            .is_empty());
        assert!(!manager.settle_intelligent_creature_operation(
            &SessionId::new("new-session"),
            &original[0].operation_id
        ));
        assert!(manager.settle_intelligent_creature_operation(
            &original[0].owner.session_id,
            &original[0].operation_id
        ));
        assert!(manager
            .peek_intelligent_creature_operations_for_identity("owner-account", 100)
            .is_empty());
    }
}

impl ZoneRuntime {
    /// Called only AFTER checkpoint root verification. Actor timers use the
    /// same absolute zone-clock convention as other pending zone actions.
    pub(super) fn validate_intelligent_creature_checkpoint(&mut self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        let mut stale = vec![];
        for (session, host) in &self.intelligent_creatures {
            let id = host.actor.object_id();
            if !ids.insert(id)
                || self.native_monsters.contains_key(&id)
                || host.actor.owner().session_id != *session
                || host.actor.pet_type() != host.record.pet_type
            {
                return Err("invalid controlled creature identity in checkpoint".into());
            }
            let owner = self.creature_owner_pose(session, host.map_allows);
            if !owner
                .as_ref()
                .is_some_and(|o| o.may_operate && &o.identity == host.actor.owner())
            {
                stale.push(session.clone());
                continue;
            }
            let valid_object = self.objects.get(&id).is_some_and(|object|
                object.position == *host.actor.position() && matches!(&object.packet,
                    ServerPacket::ObjectMonster{info} if info.ai==64 && info.extra
                    && info.effect==host.actor.pet_type() && info.master_object_id==host.actor.owner().object_id));
            if !valid_object {
                return Err("controlled creature checkpoint has no matching retained actor".into());
            }
        }
        for session in stale {
            self.remove_intelligent_creature(&session, 0);
        }
        // A pre-feature checkpoint can retain an appearance without an actor;
        // never restore it as a blocking, immortal pseudo-monster.
        let orphans:Vec<_> = self.objects.values().filter(|object|
            !self.is_intelligent_creature_object(object.object_id)
            && !self.native_monsters.contains_key(&object.object_id)
            && matches!(&object.packet,ServerPacket::ObjectMonster{info} if info.ai==64 && info.extra && info.master_object_id!=0))
            .map(|o|o.object_id).collect();
        for id in orphans {
            self.remove_retained_zone_object(id);
        }
        let actors = &self.intelligent_creatures;
        self.intelligent_creature_intents.retain(|intent| {
            actors.get(&intent.owner.session_id).is_some_and(|h| {
                h.actor.owner() == &intent.owner
                    && h.actor.object_id() == intent.creature_object_id
                    && h.actor.pet_type() == intent.pet_type
            })
        });
        Ok(())
    }
    /// Synchronize only from authenticated session state. `allowed_object_ids`
    /// is the server's capacity preflight, not a client permission list. Every
    /// tick independently verifies the current drop, grade/filter and ownership.
    pub fn sync_intelligent_creature(
        &mut self,
        session_id: &SessionId,
        creature: Option<ClientIntelligentCreature>,
        allowed_object_ids: BTreeSet<u32>,
        group_members: Vec<String>,
        map_allows: bool,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let map_allows = map_allows
            && !mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name)
                .is_some_and(|map| map.no_intelligent_creatures);
        let Some(owner) = self.creature_owner_pose(session_id, map_allows) else {
            return self.remove_intelligent_creature(session_id, now_ms);
        };
        let Some(creature) = creature.filter(|c| c.pet_type <= 14 && owner.may_operate) else {
            return self.remove_intelligent_creature(session_id, now_ms);
        };
        let replace = self.intelligent_creatures.get(session_id).is_some_and(|h| {
            h.actor.owner() != &owner.identity || h.actor.pet_type() != creature.pet_type
        });
        let mut output = if replace {
            self.remove_intelligent_creature(session_id, now_ms)
        } else {
            vec![]
        };
        let display_name = format!(
            "{}_{}'s Pet",
            creature.custom_name, self.players[session_id].name
        );
        if let Some(host) = self.intelligent_creatures.get_mut(session_id) {
            host.actor.synchronize(&owner.identity, creature.clone());
            host.record = creature;
            host.allowed = allowed_object_ids;
            host.group_members = group_members;
            host.map_allows = map_allows;
            let events = host
                .actor
                .set_display_name(&owner.identity, display_name)
                .into_iter()
                .collect();
            output.extend(self.apply_creature_events(events, now_ms));
            return output;
        }
        // Cache the immutable catalog once; never parse/clone the full monster
        // manifest on every player tick or synthesize a placeholder appearance.
        static CATALOG: std::sync::OnceLock<BTreeMap<u8, mir2_game_data::CrystalMonsterTemplate>> =
            std::sync::OnceLock::new();
        let catalog = CATALOG.get_or_init(|| {
            mir2_game_data::crystal_monster_manifest()
                .monsters
                .into_iter()
                .filter(|m| m.ai == 64 && m.effect <= 14)
                .map(|m| (m.effect, m))
                .collect()
        });
        let Some(template) = catalog.get(&creature.pet_type) else {
            return output;
        };
        let object_id = self.unique_object_id(0);
        let spawn = ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id,
            name: display_name,
            name_colour_argb: -1,
            image: template.image,
            ai: 64,
            disposition: None,
            level: template.level,
            hp: template.hp.max(1),
            max_hp: template.hp.max(1),
            experience: 0,
            move_speed_ms: u64::from(template.move_speed),
            attack_speed_ms: u64::from(template.attack_speed),
            friendly_guild: None,
            position: owner.position.clone(),
            direction: owner.direction,
            defense: Default::default(),
            respawn: None,
            drops: vec![],
        };
        let Ok((actor, event)) = ZoneIntelligentCreature::summon(
            &owner,
            creature.clone(),
            spawn,
            template.effect,
            now_ms,
            |p| !self.collision.is_blocked(p),
        ) else {
            return output;
        };
        self.intelligent_creatures.insert(
            session_id.clone(),
            CreatureHost {
                actor,
                record: creature,
                allowed: allowed_object_ids,
                group_members,
                map_allows,
                spawn_ms: now_ms,
                operation_sequence: 0,
            },
        );
        output.extend(self.apply_creature_events(vec![event], now_ms));
        output
    }

    pub fn request_intelligent_creature_pickup(
        &mut self,
        session_id: &SessionId,
        mouse_mode: bool,
        location: Point,
    ) -> bool {
        let Some(host) = self.intelligent_creatures.get(session_id) else {
            return false;
        };
        let Some(owner) = self
            .creature_owner_pose(session_id, host.map_allows)
            .filter(|p| p.may_operate)
        else {
            return false;
        };
        let candidates = self.creature_candidates(host);
        self.intelligent_creatures
            .get_mut(session_id)
            .unwrap()
            .actor
            .request_pickup(&owner.identity, mouse_mode, location, &candidates)
    }

    /// Draining is scoped to this Zone. The gateway must dispatch each intent
    /// through its matching session/character, never the current tick caller.
    pub fn drain_intelligent_creature_pickup_intents(&mut self) -> Vec<CreaturePickupIntent> {
        std::mem::take(&mut self.intelligent_creature_intents)
    }
    pub fn drain_intelligent_creature_operations(&mut self) -> Vec<CreatureOperation> {
        self.intelligent_creature_operations.clone()
    }
    pub fn peek_intelligent_creature_operations_for_identity(
        &self,
        account_id: &str,
        character_index: i32,
    ) -> Vec<CreatureOperation> {
        self.intelligent_creature_operations
            .iter()
            .filter(|operation| {
                operation.owner.account_id == account_id
                    && operation.owner.character_index == character_index
            })
            .cloned()
            .collect()
    }
    pub fn drain_intelligent_creature_pickup_intents_for(
        &mut self,
        session_id: &SessionId,
    ) -> Vec<CreaturePickupIntent> {
        let (owned, others) = std::mem::take(&mut self.intelligent_creature_intents)
            .into_iter()
            .partition(|intent| &intent.owner.session_id == session_id);
        self.intelligent_creature_intents = others;
        owned
    }
    pub fn drain_intelligent_creature_operations_for(
        &mut self,
        session_id: &SessionId,
    ) -> Vec<CreatureOperation> {
        self.intelligent_creature_operations
            .iter()
            .filter(|operation| &operation.owner.session_id == session_id)
            .cloned()
            .collect()
    }
    /// Ack only AFTER personal wallet plus dedupe receipt are durable. Delivery
    /// batches intentionally remain retryable across actor removal/checkpoints.
    pub fn settle_intelligent_creature_operation(
        &mut self,
        session_id: &SessionId,
        operation_id: &str,
    ) -> bool {
        let before = self.intelligent_creature_operations.len();
        self.intelligent_creature_operations
            .retain(|op| &op.owner.session_id != session_id || op.operation_id != operation_id);
        before != self.intelligent_creature_operations.len()
    }
    pub fn settle_intelligent_creature_pickup(
        &mut self,
        session_id: &SessionId,
        creature_object_id: u32,
        drop_id: u32,
    ) -> bool {
        self.intelligent_creatures
            .get_mut(session_id)
            .is_some_and(|host| {
                host.actor.object_id() == creature_object_id && host.actor.settle_pickup(drop_id)
            })
    }
    pub fn intelligent_creature_intent_is_current(&self, intent: &CreaturePickupIntent) -> bool {
        self.intelligent_creatures
            .get(&intent.owner.session_id)
            .is_some_and(|host| {
                host.actor.owner() == &intent.owner
                    && host.actor.object_id() == intent.creature_object_id
                    && host.actor.pet_type() == intent.pet_type
                    && host.actor.is_alive()
                    && self
                        .creature_owner_pose(&intent.owner.session_id, host.map_allows)
                        .is_some_and(|p| p.may_operate)
                    && self.creature_candidates(host).contains(&intent.target)
            })
    }
    pub fn intelligent_creature_object_id(&self, session_id: &SessionId) -> Option<u32> {
        self.intelligent_creatures
            .get(session_id)
            .map(|h| h.actor.object_id())
    }
    pub(super) fn is_intelligent_creature_object(&self, object_id: u32) -> bool {
        self.intelligent_creatures
            .values()
            .any(|h| h.actor.object_id() == object_id)
    }

    fn creature_owner_pose(
        &self,
        session_id: &SessionId,
        map_allows: bool,
    ) -> Option<CreatureOwnerPose> {
        let p = self.players.get(session_id)?;
        Some(CreatureOwnerPose {
            identity: CreatureOwner {
                session_id: session_id.clone(),
                account_id: p.account_id.clone(),
                character_index: p.character_index,
                object_id: p.object_id,
            },
            position: p.position.clone(),
            direction: p.direction,
            may_operate: map_allows && p.hp > 0 && !p.dead,
        })
    }
    fn creature_candidates(&self, host: &CreatureHost) -> Vec<CreatureTarget> {
        self.ground_drops
            .values()
            .filter(|stored| {
                host.allowed.contains(&stored.drop.object_id)
                    && !self
                        .claimed_ground_drops
                        .contains_key(&stored.drop.object_id)
                    && self.ground_drop_ownership_allows(
                        &stored.drop,
                        host.actor.owner().object_id,
                        &host.group_members,
                    )
                    && crate::runtime::intelligent_creature_allows_ground_drop(
                        &host.record,
                        &stored.drop,
                    )
            })
            .map(|stored| CreatureTarget {
                object_id: stored.drop.object_id,
                position: Point {
                    x: stored.drop.x,
                    y: stored.drop.y,
                },
            })
            .collect()
    }
    pub(super) fn remove_intelligent_creature(
        &mut self,
        session_id: &SessionId,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        self.intelligent_creature_intents
            .retain(|i| &i.owner.session_id != session_id);
        // Started operations remain billable once even if their actor despawns.
        let Some(mut host) = self.intelligent_creatures.remove(session_id) else {
            return vec![];
        };
        self.apply_creature_events(host.actor.dismiss().into_iter().collect(), now_ms)
    }
    pub(super) fn tick_intelligent_creatures(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let sessions: Vec<_> = self.intelligent_creatures.keys().cloned().collect();
        let mut output = vec![];
        for session_id in sessions {
            let host = &self.intelligent_creatures[&session_id];
            let owner = self.creature_owner_pose(&session_id, host.map_allows);
            if !owner.as_ref().is_some_and(|p| p.may_operate) {
                output.extend(self.remove_intelligent_creature(&session_id, now_ms));
                continue;
            }
            let candidates = self.creature_candidates(host);
            // Temporarily remove only the actor, retaining a nonblocking ID set
            // for the collision closure so it cannot collide with itself.
            let controlled_ids: BTreeSet<_> = self
                .intelligent_creatures
                .values()
                .map(|h| h.actor.object_id())
                .collect();
            let mut host = self.intelligent_creatures.remove(&session_id).unwrap();
            let events = host.actor.tick(now_ms, owner.as_ref(), &candidates, |p| {
                !self.collision.is_blocked(p)
                    && !self.gate_blocks_tile(p)
                    && !self.occupancy.contains_key(&tile_key(p))
                    && !self.objects.values().any(|o| {
                        !controlled_ids.contains(&o.object_id)
                            && retained_zone_object_blocks_tile(o, p)
                    })
            });
            self.intelligent_creatures.insert(session_id, host);
            output.extend(self.apply_creature_events(events, now_ms));
        }
        output
    }
    fn apply_creature_events(
        &mut self,
        events: Vec<CreatureEvent>,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let mut output = vec![];
        for event in events {
            let packet = match event {
                CreatureEvent::PickupIntent(intent) => {
                    self.intelligent_creature_intents.push(intent);
                    continue;
                }
                CreatureEvent::Appear {
                    owner,
                    pet_type,
                    monster,
                } => {
                    self.removed_object_ids.remove(&monster.object_id);
                    let mut packet = native_monster_spawn_packet(&monster, monster.object_id);
                    if let ServerPacket::ObjectMonster { info } = &mut packet {
                        info.effect = pet_type;
                        info.extra = true;
                        info.master_object_id = owner.object_id;
                    }
                    packet
                }
                CreatureEvent::Move {
                    object_id,
                    position,
                    direction,
                } => ServerPacket::ObjectWalk {
                    movement: ObjectMovement {
                        object_id,
                        position,
                        direction,
                    },
                },
                CreatureEvent::Turn {
                    object_id,
                    direction,
                } => ServerPacket::ObjectTurn {
                    movement: ObjectMovement {
                        object_id,
                        position: self
                            .objects
                            .get(&object_id)
                            .map(|o| o.position.clone())
                            .unwrap_or(Point { x: 0, y: 0 }),
                        direction,
                    },
                },
                CreatureEvent::Name { object_id, name } => {
                    ServerPacket::ObjectName { object_id, name }
                }
                CreatureEvent::Remove { object_id } => ServerPacket::ObjectRemove { object_id },
                CreatureEvent::Attack {
                    object_id,
                    position,
                    direction,
                    attack_type,
                    pickup_operation,
                } => {
                    if pickup_operation {
                        if let Some(host) = self
                            .intelligent_creatures
                            .values_mut()
                            .find(|h| h.actor.object_id() == object_id)
                        {
                            host.operation_sequence = host.operation_sequence.saturating_add(1);
                            self.intelligent_creature_operations
                                .push(CreatureOperation {
                                    owner: host.actor.owner().clone(),
                                    pet_type: host.actor.pet_type(),
                                    operation_id: format!(
                                        "creature-operation:{}:{}:{}:{}:{}:{}:{}",
                                        self.key.shard_id,
                                        self.key.map_file_name,
                                        self.key.channel_id,
                                        self.key.instance_id,
                                        host.spawn_ms,
                                        object_id,
                                        host.operation_sequence
                                    ),
                                });
                        }
                    }
                    ServerPacket::ObjectAttack {
                        info: ObjectAttackInfo {
                            object_id,
                            location: position,
                            direction,
                            spell: 0,
                            level: 0,
                            attack_type,
                        },
                    }
                }
            };
            let object_id = match &packet {
                ServerPacket::ObjectMonster { info } => info.object_id,
                ServerPacket::ObjectWalk { movement } | ServerPacket::ObjectTurn { movement } => {
                    movement.object_id
                }
                ServerPacket::ObjectAttack { info } => info.object_id,
                ServerPacket::ObjectName { object_id, .. }
                | ServerPacket::ObjectRemove { object_id } => *object_id,
                _ => unreachable!(),
            };
            let old_visible: Vec<_> = self
                .players
                .iter()
                .filter(|(_, p)| p.visible_object_ids.contains(&object_id))
                .map(|(s, _)| s.clone())
                .collect();
            let is_remove = matches!(packet, ServerPacket::ObjectRemove { .. });
            let is_appear = matches!(packet, ServerPacket::ObjectMonster { .. });
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
            output.extend(self.diff_all_zone_object_visibility());
            if !is_appear {
                let recipients = old_visible
                    .into_iter()
                    .filter(|s| {
                        is_remove
                            || self
                                .players
                                .get(s)
                                .is_some_and(|p| p.visible_object_ids.contains(&object_id))
                    })
                    .collect::<Vec<_>>();
                if !recipients.is_empty() {
                    output.push(ZoneOutbound::ToMany {
                        session_ids: recipients,
                        packets: vec![packet],
                    });
                }
            }
        }
        output
    }
}
