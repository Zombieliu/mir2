//! The server clock observes live primary Zones, commits siege authority, then
//! projects that committed authority back into the shared world. Client NPC
//! dialogs and cached player/chat snapshots are never siege authority.
use super::{
    shared_gateway_now_ms, GatewayConfig, ServerPacket, SessionId, SharedInProcessZoneResources,
    SharedInProcessZoneRuntimeFactory, SharedInProcessZoneState, ZoneId, ZonePresenceKey,
};
use mir2_protocol::ChatType;
use mir2_simulation::conquest::{
    ConquestEvent, ConquestEventKind, ConquestPalacePresence, ConquestPolicy, SharedConquestRecord,
};
use mir2_simulation::{ConquestDefenseSample, ZoneConquestMembership, ZoneKey};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

type PrimaryConquestZone = (ZoneId, Arc<Mutex<SharedInProcessZoneState>>);

struct ConquestObservation {
    presences: Vec<ConquestPalacePresence>,
    defenses: Vec<ConquestDefenseSample>,
}

pub(super) struct CommittedConquestProjection {
    records: Vec<(ConquestPolicy, SharedConquestRecord)>,
    memberships: BTreeMap<ZonePresenceKey, String>,
    guild_names: BTreeMap<String, String>,
}

impl SharedInProcessZoneRuntimeFactory {
    /// Called only by a server-owned clock with its stable process lease token.
    /// Every lock needed to observe the world is released before durable IO.
    pub fn advance_shared_conquest(
        &self,
        config: &GatewayConfig,
        now_ms: u64,
        clock_owner: &str,
    ) -> Result<Vec<ConquestEvent>, String> {
        if config.conquest_policies.is_empty() {
            return Ok(Vec::new());
        }
        let observation_started = std::time::Instant::now();
        let zones = self.primary_conquest_zones()?;
        let observed = observe_conquest_zones(&zones, &config.conquest_policies)?;
        // A restore, demotion or newly joined Zone must not turn an incomplete
        // population observation into a successful castle capture.
        self.ensure_conquest_topology_unchanged(&zones)?;
        let events = config.advance_shared_conquests_with_defenses(
            now_ms,
            clock_owner,
            &observed.presences,
            &observed.defenses,
        )?;

        // advance_shared_conquests refreshed and durably committed both shared
        // domains. Take one coherent copy, without any Zone/registry lock.
        let projection = committed_projection(config)?;
        let notices = committed_notices(&events, &projection)?;
        let current_zones = self.primary_conquest_zones()?;
        let mut failures = Vec::new();
        for (zone_id, zone) in current_zones {
            let mut state = match zone.lock() {
                Ok(state) => state,
                Err(_) => {
                    failures.push(format!("conquest primary Zone {zone_id} mutex poisoned"));
                    continue;
                }
            };
            let memberships: Vec<_> = state
                .zone_manager
                .conquest_player_samples()
                .into_iter()
                .filter_map(|sample| {
                    let key = active_sample_key(&state, &sample.session_id)?;
                    if key.account_id != sample.account_id
                        || key.character_index != sample.character_index
                    {
                        return None;
                    }
                    Some(ZoneConquestMembership {
                        session_id: sample.session_id,
                        account_id: sample.account_id,
                        character_index: sample.character_index,
                        guild_id: projection.memberships.get(&key).cloned(),
                    })
                })
                .collect();
            for (policy, record) in &projection.records {
                let project_epoch = now_ms.saturating_add(
                    u64::try_from(observation_started.elapsed().as_millis()).unwrap_or(u64::MAX),
                );
                match state.zone_manager.apply_conquest_projection(
                    policy,
                    record,
                    &memberships,
                    project_epoch,
                    shared_gateway_now_ms(),
                ) {
                    Ok(outbounds) => {
                        state.dispatch_zone_outbounds(outbounds, None);
                    }
                    Err(error) => failures.push(format!(
                        "conquest {} committed but Zone {zone_id} projection failed: {error}",
                        policy.index
                    )),
                }
            }
            // These packets can reach sockets only after the transaction above
            // returned its durable receipt. They also reach players on other maps.
            let recipients: Vec<_> = state
                .zone_sessions
                .iter()
                .filter(|(key, session)| {
                    !state.teardown_fences.contains(*key)
                        && state.zone_session_keys.get(*session) == Some(*key)
                        && state.zone_manager.zone_key_for_session(session).is_some()
                        && state.zone_manager.player_transform(session).is_some()
                })
                .map(|(key, _)| key.clone())
                .collect();
            for key in recipients {
                state.queue_zone_packets(key, notices.clone());
            }
        }
        if failures.is_empty() {
            Ok(events)
        } else {
            // The durable event is never rolled back or replayed as a fresh
            // capture because a world projection failed. The next clock retries
            // the committed projection; the error remains operationally visible.
            Err(failures.join("; "))
        }
    }

    fn primary_conquest_zones(&self) -> Result<Vec<PrimaryConquestZone>, String> {
        // The existing factory's fixed lock order is replicas, then registry.
        let replicas = self
            .replica_zone_ids
            .lock()
            .map_err(|_| "conquest replica registry mutex poisoned")?;
        let zones = self
            .zones
            .lock()
            .map_err(|_| "conquest primary registry mutex poisoned")?;
        Ok(zones
            .iter()
            .filter(|(zone_id, _)| !replicas.contains(*zone_id))
            .map(|(zone_id, resources)| (zone_id.clone(), resources.zone_state.clone()))
            .collect())
    }

    fn ensure_conquest_topology_unchanged(
        &self,
        observed: &[PrimaryConquestZone],
    ) -> Result<(), String> {
        let current = self.primary_conquest_zones()?;
        if current.len() != observed.len()
            || current
                .iter()
                .zip(observed)
                .any(|((id, zone), (old_id, old_zone))| {
                    id != old_id || !Arc::ptr_eq(zone, old_zone)
                })
        {
            return Err("conquest primary topology changed; retry clock".into());
        }
        Ok(())
    }
}

fn active_sample_key(
    state: &SharedInProcessZoneState,
    session: &SessionId,
) -> Option<ZonePresenceKey> {
    let key = state.zone_session_keys.get(session)?;
    if state.teardown_fences.contains(key) || state.zone_sessions.get(key) != Some(session) {
        return None;
    }
    Some(key.clone())
}

fn observe_conquest_zones(
    zones: &[PrimaryConquestZone],
    policies: &[ConquestPolicy],
) -> Result<ConquestObservation, String> {
    let mut identities = BTreeSet::new();
    let mut presences = Vec::new();
    let mut defenses = BTreeMap::new();
    for (zone_id, zone) in zones {
        let state = zone
            .lock()
            .map_err(|_| format!("conquest primary Zone {zone_id} mutex poisoned"))?;
        for sample in state.zone_manager.conquest_player_samples() {
            let Some(key) = active_sample_key(&state, &sample.session_id) else {
                continue;
            };
            let Some(zone_key) = state.zone_manager.zone_key_for_session(&sample.session_id) else {
                continue;
            };
            let Some((position, _)) = state.zone_manager.player_transform(&sample.session_id)
            else {
                return Err(format!(
                    "conquest live player transform unavailable in {zone_id}"
                ));
            };
            let Some((hp, _, _)) = state.zone_manager.player_vitals(&sample.session_id) else {
                return Err(format!(
                    "conquest live player vitals unavailable in {zone_id}"
                ));
            };
            if key.account_id.is_empty()
                || key.character_index < 0
                || key.account_id != sample.account_id
                || key.character_index != sample.character_index
                || zone_key.map_file_name != sample.map_file_name
                || position != sample.position
                || hp != sample.hp
            {
                return Err(format!(
                    "conquest live player identity/state mismatch in {zone_id}"
                ));
            }
            if !identities.insert(key.clone()) {
                return Err("conquest duplicate account/character in primary Zones".into());
            }
            if !sample.alive || hp <= 0 {
                continue;
            }
            // Castle ownership is one shared world, not an unrelated parallel
            // channel/instance carrying the same display map name.
            let is_castle_map = policies.iter().any(|policy| {
                policy
                    .map_file_name
                    .eq_ignore_ascii_case(&sample.map_file_name)
                    || policy
                        .palace_file_name
                        .eq_ignore_ascii_case(&sample.map_file_name)
            });
            if is_castle_map && zone_key != ZoneKey::for_map(sample.map_file_name.clone()) {
                continue;
            }
            presences.push(ConquestPalacePresence {
                account_id: key.account_id,
                character_index: key.character_index,
                map_file_name: zone_key.map_file_name,
                alive: true,
                // The Config transaction binds this from current durable Guild
                // membership. A stale cached chat profile cannot authorize it.
                guild_id: None,
            });
        }
        for sample in state.zone_manager.conquest_defense_samples() {
            let key = (sample.conquest_index, sample.key.clone());
            if defenses.contains_key(&key) {
                return Err("conquest defense has multiple primary Zone authorities".into());
            }
            defenses.insert(
                key,
                ConquestDefenseSample {
                    conquest_index: sample.conquest_index,
                    record_revision: sample.record_revision,
                    key: sample.key,
                    hp: sample.hp,
                    committed_hp: sample.committed_hp,
                    battle_day: sample.battle_day,
                    war_ends_ms: sample.war_ends_ms,
                    clock_generation: sample.clock_generation,
                    damage_was_admitted_before_cutoff: sample.damage_was_admitted_before_cutoff,
                },
            );
        }
    }
    Ok(ConquestObservation {
        presences,
        defenses: defenses.into_values().collect(),
    })
}

fn committed_projection(config: &GatewayConfig) -> Result<CommittedConquestProjection, String> {
    read_projection(config, false)
}

pub(super) fn bootstrap_projection(
    config: &GatewayConfig,
) -> Result<CommittedConquestProjection, String> {
    config.refresh_shared_conquest_authority()?;
    read_projection(config, true)
}

fn read_projection(
    config: &GatewayConfig,
    allow_initial: bool,
) -> Result<CommittedConquestProjection, String> {
    let store = config
        .account_store
        .lock()
        .map_err(|_| "conquest committed authority mutex poisoned")?;
    let mut records = Vec::new();
    for policy in &config.conquest_policies {
        let mut initial = SharedConquestRecord::new(policy.index);
        initial.defenses = mir2_simulation::conquest::default_sabuk_defenses();
        let record = match store.shared_conquests.get(&policy.index) {
            Some(record) => record,
            None if allow_initial => &initial,
            None => return Err("conquest committed record unavailable".into()),
        };
        record.validate()?;
        records.push((policy.clone(), record.clone()));
    }
    let mut memberships = BTreeMap::new();
    let mut guild_names = BTreeMap::new();
    for guild in store.shared_guilds.values() {
        guild_names.insert(guild.id.clone(), guild.name.clone());
        for member in &guild.members {
            if !store
                .accounts
                .get(&member.identity.account_id)
                .is_some_and(|account| {
                    account
                        .characters
                        .iter()
                        .any(|character| character.index == member.identity.character_index)
                })
            {
                continue;
            }
            let key = ZonePresenceKey {
                account_id: member.identity.account_id.clone(),
                character_index: member.identity.character_index,
            };
            if memberships.insert(key, guild.id.clone()).is_some() {
                return Err("conquest ambiguous durable Guild membership".into());
            }
        }
    }
    Ok(CommittedConquestProjection {
        records,
        memberships,
        guild_names,
    })
}

impl SharedInProcessZoneState {
    pub(super) fn project_bootstrap_conquest(
        &mut self,
        projection: &CommittedConquestProjection,
    ) -> Result<Vec<mir2_simulation::ZoneOutbound>, String> {
        let memberships = self
            .zone_manager
            .conquest_player_samples()
            .into_iter()
            .filter_map(|sample| {
                let key = active_sample_key(self, &sample.session_id)?;
                if key.account_id != sample.account_id
                    || key.character_index != sample.character_index
                {
                    return None;
                }
                Some(ZoneConquestMembership {
                    session_id: sample.session_id,
                    account_id: sample.account_id,
                    character_index: sample.character_index,
                    guild_id: projection.memberships.get(&key).cloned(),
                })
            })
            .collect::<Vec<_>>();
        let epoch_now = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "conquest bootstrap time unavailable")?
                .as_millis(),
        )
        .map_err(|_| "conquest bootstrap time exhausted")?;
        let mut outbounds = Vec::new();
        for (policy, record) in &projection.records {
            outbounds.extend(self.zone_manager.apply_conquest_projection(
                policy,
                record,
                &memberships,
                epoch_now,
                shared_gateway_now_ms(),
            )?);
        }
        Ok(outbounds)
    }
}

/// Ordinary castle NPC changes are projected immediately. This only reads the
/// committed state; it cannot open a war or advance palace capture cadence.
pub(super) fn project_registry_committed(
    config: &GatewayConfig,
    registry: &Arc<Mutex<BTreeMap<ZoneId, SharedInProcessZoneResources>>>,
    replicas: &Arc<Mutex<BTreeSet<ZoneId>>>,
) -> Result<(), String> {
    let projection = bootstrap_projection(config)?;
    let zones = {
        let replica_ids = replicas
            .lock()
            .map_err(|_| "conquest replica registry mutex poisoned")?;
        let registry = registry
            .lock()
            .map_err(|_| "conquest primary registry mutex poisoned")?;
        registry
            .iter()
            .filter(|(id, _)| !replica_ids.contains(*id))
            .map(|(_, resources)| resources.zone_state.clone())
            .collect::<Vec<_>>()
    };
    for zone in zones {
        let mut state = zone
            .lock()
            .map_err(|_| "conquest primary Zone mutex poisoned")?;
        let outbounds = state.project_bootstrap_conquest(&projection)?;
        state.dispatch_zone_outbounds(outbounds, None);
    }
    Ok(())
}

fn committed_notices(
    events: &[ConquestEvent],
    projection: &CommittedConquestProjection,
) -> Result<Vec<ServerPacket>, String> {
    let mut notices = Vec::new();
    for (_, record) in &projection.records {
        for event in record.events.iter().filter(|event| events.contains(event)) {
            let message = if event.kind == ConquestEventKind::WarEnded && event.guild_id.is_none() {
                "Siege ended. The castle is unowned.".into()
            } else {
                let guild_id = event
                    .guild_id
                    .as_ref()
                    .ok_or("conquest persisted notice Guild identity unavailable")?;
                let guild = projection
                    .guild_names
                    .get(guild_id)
                    .ok_or("conquest persisted notice Guild name unavailable")?;
                match event.kind {
                    ConquestEventKind::Requested => format!("Siege requested by {guild}."),
                    ConquestEventKind::WarStarted => format!("Siege started. Attacker: {guild}"),
                    ConquestEventKind::Captured => format!("Castle captured by {guild}."),
                    ConquestEventKind::WarEnded => format!("Siege ended. Castle owner: {guild}."),
                }
            };
            notices.push(ServerPacket::Chat {
                message,
                chat_type: ChatType::Announcement,
            });
        }
    }
    if events.iter().any(|event| {
        !projection
            .records
            .iter()
            .any(|(_, record)| record.events.contains(event))
    }) {
        return Err("conquest event missing from committed authority".into());
    }
    Ok(notices)
}

#[cfg(test)]
mod tests {
    use super::super::{SharedInProcessZoneResources, ZoneCommand};
    use super::*;
    use mir2_protocol::{MirClass, MirDirection, MirGender, Point};
    use mir2_simulation::conquest::sabuk_policy;
    use mir2_simulation::{
        AccountStoreTransactionFault, SharedGuildMember, SharedGuildRank, SharedGuildRecord,
        Stage5FriendIdentity, ZoneChatProfile, ZoneJoin, ZonePlayerCombatStats,
    };
    use std::sync::atomic::{AtomicBool, AtomicU64};

    const GUILD: &str = "0123456789abcdef0123456789abcdef";
    const CLOCK: &str = "2123456789abcdef0123456789abcdef";
    const START: u64 = 86_400_000 + 18 * 60 * 60_000;

    fn fixture() -> (GatewayConfig, i32) {
        let mut config = GatewayConfig::default();
        let mut policy = sabuk_policy();
        policy.utc_offset_minutes = 0;
        config.conquest_policies = vec![policy];
        let mut store = config.account_store.lock().unwrap();
        let character = store.accounts["demo"].characters[0].clone();
        store.shared_guilds.insert(
            GUILD.into(),
            SharedGuildRecord {
                id: GUILD.into(),
                name: "Durable Knights".into(),
                revision: 1,
                level: 0,
                experience: 0,
                spare_points: 0,
                gold: 10_000,
                ranks: vec![
                    SharedGuildRank {
                        index: 0,
                        name: "Leader".into(),
                        options: 255,
                    },
                    SharedGuildRank {
                        index: 1,
                        name: "Member".into(),
                        options: 0,
                    },
                ],
                members: vec![SharedGuildMember {
                    membership_epoch: 0,
                    identity: Stage5FriendIdentity {
                        account_id: "demo".into(),
                        character_index: character.index,
                    },
                    name: character.name,
                    rank_index: 0,
                }],
                notice: Vec::new(),
                storage: BTreeMap::new(),
                buffs: BTreeMap::new(),
                last_buff_tick_ms: 0,
                experience_receipts: BTreeSet::new(),
                experience_receipt_payloads: BTreeMap::new(),
            },
        );
        drop(store);
        (config, character.index)
    }

    fn player(account: &str, index: i32, map: &str, x: i32, hp: i32) -> ZoneJoin {
        ZoneJoin {
            session_id: SessionId::new(format!("{account}:{index}")),
            account_id: account.into(),
            character_index: index,
            object_id: (index as u32).saturating_add(10_000),
            name: account.into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 30,
            hp,
            max_hp: 100,
            mp: 20,
            map_file_name: map.into(),
            position: Point { x, y: 13 },
            direction: MirDirection::Down,
            chat_profile: ZoneChatProfile {
                guild_name: Some("Untrusted cached Guild".into()),
                ..ZoneChatProfile::default()
            },
            combat_stats: ZonePlayerCombatStats::default(),
        }
    }

    fn install_zone(
        factory: &SharedInProcessZoneRuntimeFactory,
        zone_id: &str,
        players: Vec<ZoneJoin>,
    ) -> Arc<Mutex<SharedInProcessZoneState>> {
        let mut state = SharedInProcessZoneState::new();
        for player in players {
            let key = ZonePresenceKey {
                account_id: player.account_id.clone(),
                character_index: player.character_index,
            };
            state
                .zone_sessions
                .insert(key.clone(), player.session_id.clone());
            state
                .zone_session_keys
                .insert(player.session_id.clone(), key);
            state.zone_manager.join(player);
        }
        let state = Arc::new(Mutex::new(state));
        let (movement_sender, _receiver) = std::sync::mpsc::sync_channel(1);
        factory.zones.lock().unwrap().insert(
            ZoneId::new(zone_id),
            SharedInProcessZoneResources {
                zone_state: state.clone(),
                movement_sender,
                tick_count: Arc::new(AtomicU64::new(0)),
                autonomous_ticks_enabled: Arc::new(AtomicBool::new(false)),
            },
        );
        state
    }

    fn notices(state: &Arc<Mutex<SharedInProcessZoneState>>) -> Vec<String> {
        state
            .lock()
            .unwrap()
            .pending_zone_packets
            .values()
            .flatten()
            .filter_map(|packet| {
                if let ServerPacket::Chat {
                    message,
                    chat_type: ChatType::Announcement,
                } = packet
                {
                    Some(message.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn conquest_observation_uses_live_zone_state_and_excludes_dead_offline_fenced_replicas() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let state = install_zone(
            &factory,
            "main",
            vec![
                player("live", 1, "0150", 10, 100),
                player("dead", 2, "0150", 11, 0),
                player("offline", 3, "0150", 12, 100),
                player("fenced", 4, "0150", 13, 100),
            ],
        );
        {
            let mut state = state.lock().unwrap();
            state.zone_manager.handle(ZoneCommand::SyncPlayerTransform {
                session_id: SessionId::new("live:1"),
                position: Point { x: 20, y: 13 },
                direction: MirDirection::Right,
            });
            state.zone_sessions.remove(&ZonePresenceKey {
                account_id: "offline".into(),
                character_index: 3,
            });
            state.teardown_fences.insert(ZonePresenceKey {
                account_id: "fenced".into(),
                character_index: 4,
            });
        }
        install_zone(
            &factory,
            "replica",
            vec![player("replica", 5, "0150", 14, 100)],
        );
        factory.mark_zone_as_replica(&ZoneId::new("replica"));
        let observed = observe_conquest_zones(
            &factory.primary_conquest_zones().unwrap(),
            &[sabuk_policy()],
        )
        .unwrap();
        assert_eq!(
            observed.presences,
            vec![ConquestPalacePresence {
                account_id: "live".into(),
                character_index: 1,
                map_file_name: "0150".into(),
                alive: true,
                guild_id: None,
            }]
        );
    }

    #[test]
    fn conquest_duplicate_primary_identity_fails_before_durable_clock_or_broadcast() {
        let (config, index) = fixture();
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let first = install_zone(
            &factory,
            "first",
            vec![player("demo", index, "0150", 10, 100)],
        );
        let second = install_zone(
            &factory,
            "second",
            vec![player("demo", index, "0", 11, 100)],
        );
        assert!(factory
            .advance_shared_conquest(&config, START, CLOCK)
            .unwrap_err()
            .contains("duplicate"));
        assert!(config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .is_empty());
        assert!(notices(&first).is_empty());
        assert!(notices(&second).is_empty());
    }

    #[test]
    fn conquest_parallel_palace_channel_cannot_contest_or_capture_primary_castle() {
        let (config, index) = fixture();
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        let factory = SharedInProcessZoneRuntimeFactory::new();
        install_zone(
            &factory,
            "palace",
            vec![player("demo", index, "0150", 10, 100)],
        );
        let parallel = install_zone(&factory, "parallel", Vec::new());
        {
            let mut state = parallel.lock().unwrap();
            let player = player("unknown", 55, "0150", 11, 100);
            let key = ZonePresenceKey {
                account_id: "unknown".into(),
                character_index: 55,
            };
            state
                .zone_sessions
                .insert(key.clone(), player.session_id.clone());
            state
                .zone_session_keys
                .insert(player.session_id.clone(), key);
            state.zone_manager.handle_for_key(
                ZoneKey::new("primary", "0150", 2, "main"),
                ZoneCommand::Join(player),
            );
        }
        assert!(factory
            .advance_shared_conquest(&config, START, CLOCK)
            .unwrap()
            .iter()
            .any(|event| event.kind == ConquestEventKind::Captured));
    }

    #[test]
    fn conquest_topology_change_requires_fresh_observation_before_commit() {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        install_zone(&factory, "palace", vec![player("live", 1, "0150", 10, 100)]);
        let before = factory.primary_conquest_zones().unwrap();
        factory.mark_zone_as_replica(&ZoneId::new("palace"));
        assert!(factory
            .ensure_conquest_topology_unchanged(&before)
            .unwrap_err()
            .contains("topology changed"));
    }

    #[test]
    fn conquest_identity_mismatch_fails_before_durable_clock() {
        let (config, index) = fixture();
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let state = install_zone(
            &factory,
            "main",
            vec![player("demo", index, "0150", 10, 100)],
        );
        {
            let mut state = state.lock().unwrap();
            let session = SessionId::new(format!("demo:{index}"));
            let original = state.zone_session_keys.remove(&session).unwrap();
            state.zone_sessions.remove(&original);
            let forged = ZonePresenceKey {
                account_id: "forged".into(),
                character_index: index,
            };
            state
                .zone_session_keys
                .insert(session.clone(), forged.clone());
            state.zone_sessions.insert(forged, session);
        }
        assert!(factory
            .advance_shared_conquest(&config, START, CLOCK)
            .unwrap_err()
            .contains("mismatch"));
        assert!(config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .is_empty());
    }

    #[test]
    fn conquest_capture_binds_durable_guild_and_broadcasts_once_to_other_maps() {
        let (config, index) = fixture();
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let palace = install_zone(
            &factory,
            "palace",
            vec![player("demo", index, "0150", 10, 100)],
        );
        let listener = install_zone(&factory, "town", vec![player("listener", 55, "0", 11, 100)]);
        let events = factory
            .advance_shared_conquest(&config, START, CLOCK)
            .unwrap();
        assert!(events
            .iter()
            .any(|event| event.kind == ConquestEventKind::Captured));
        assert_eq!(
            config
                .shared_conquest_snapshot_checked(1)
                .unwrap()
                .unwrap()
                .owner_guild_id
                .as_deref(),
            Some(GUILD)
        );
        let first = notices(&listener);
        assert_eq!(first.len(), 2);
        assert!(first
            .iter()
            .all(|notice| notice.contains("Durable Knights")));
        assert!(!first
            .iter()
            .any(|notice| notice.contains("Untrusted cached Guild")));
        assert_eq!(notices(&palace), first);
        assert!(factory
            .advance_shared_conquest(&config, START + 10_000, CLOCK)
            .unwrap()
            .is_empty());
        assert_eq!(notices(&listener), first);
    }

    #[test]
    fn conquest_unknown_living_occupant_contests_until_leaving_real_zone() {
        let (config, index) = fixture();
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let palace = install_zone(
            &factory,
            "palace",
            vec![
                player("demo", index, "0150", 10, 100),
                player("unknown", 55, "0150", 11, 100),
            ],
        );
        let events = factory
            .advance_shared_conquest(&config, START, CLOCK)
            .unwrap();
        assert!(events
            .iter()
            .all(|event| event.kind != ConquestEventKind::Captured));
        assert!(config
            .shared_conquest_snapshot_checked(1)
            .unwrap()
            .unwrap()
            .owner_guild_id
            .is_none());
        palace
            .lock()
            .unwrap()
            .zone_manager
            .handle(ZoneCommand::Leave {
                session_id: SessionId::new("unknown:55"),
            });
        assert!(factory
            .advance_shared_conquest(&config, START + 10_000, CLOCK)
            .unwrap()
            .iter()
            .any(|event| event.kind == ConquestEventKind::Captured));
    }

    #[test]
    fn conquest_persistence_failure_never_projects_or_announces_capture() {
        let (config, index) = fixture();
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        let before = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let palace = install_zone(
            &factory,
            "palace",
            vec![player("demo", index, "0150", 10, 100)],
        );
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
        assert!(factory
            .advance_shared_conquest(&config, START, CLOCK)
            .is_err());
        assert_eq!(
            config.shared_conquest_snapshot_checked(1).unwrap().unwrap(),
            before
        );
        assert!(notices(&palace).is_empty());
    }

    #[test]
    fn conquest_unowned_notice_is_distinct_from_opaque_guild_named_none() {
        let unowned = ConquestEvent {
            sequence: 1,
            at_ms: START,
            kind: ConquestEventKind::WarEnded,
            guild_id: None,
        };
        let mut record = SharedConquestRecord::new(1);
        record.events = vec![unowned.clone()];
        let mut projection = CommittedConquestProjection {
            records: vec![(sabuk_policy(), record)],
            memberships: BTreeMap::new(),
            guild_names: BTreeMap::from([(GUILD.into(), "none".into())]),
        };
        assert!(
            matches!(committed_notices(&[unowned], &projection).unwrap().as_slice(),
            [ServerPacket::Chat { message, .. }] if message == "Siege ended. The castle is unowned.")
        );
        let named = ConquestEvent {
            sequence: 1,
            at_ms: START,
            kind: ConquestEventKind::WarEnded,
            guild_id: Some(GUILD.into()),
        };
        projection.records[0].1.events = vec![named.clone()];
        assert!(
            matches!(committed_notices(&[named], &projection).unwrap().as_slice(),
            [ServerPacket::Chat { message, .. }] if message == "Siege ended. Castle owner: none.")
        );
    }

    #[test]
    fn conquest_replica_registry_failure_is_visible_and_cannot_become_empty_palace() {
        let (config, _) = fixture();
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let replicas = factory.replica_zone_ids.clone();
        let _ = std::thread::spawn(move || {
            let _guard = replicas.lock().unwrap();
            panic!("test poison replica registry");
        })
        .join();
        assert!(factory
            .advance_shared_conquest(&config, START, CLOCK)
            .unwrap_err()
            .contains("replica registry"));
        assert!(config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .is_empty());
    }
}
