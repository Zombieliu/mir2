//! Durable server-owned siege operations. NPC admission and live Zone sampling
//! happen before these calls; no client packet deserializes an operation here.
use super::{
    postgres_account_store_pool, shared_conquest_store, shared_guild_store, AccountStore,
    AccountStoreDatabaseMode, AccountStoreMutationScope, SharedGuildRecord, SimulationConfig,
    Stage5FriendIdentity,
};
use crate::conquest::{
    default_sabuk_defenses, valid_guild_id, ConquestEvent, ConquestPalacePresence, ConquestPolicy,
    SharedConquestRecord,
};
use std::collections::BTreeMap;

/// Trusted ordinary-NPC intent. The wrapper always rechecks the actor against
/// the current owner's stable Guild record before applying it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConquestManagementAction {
    SetTax { rate: u8 },
    SetGate { key: String, open: bool },
    Repair { key: String },
    WithdrawTax,
}

/// Produced only from the primary authoritative Zone's defense state. The
/// committed projection revision/HP fences samples taken before a paid repair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConquestDefenseSample {
    pub conquest_index: i32,
    pub record_revision: u64,
    pub key: String,
    pub hp: i32,
    pub committed_hp: i32,
    pub battle_day: Option<i64>,
    pub war_ends_ms: u64,
    pub clock_generation: u64,
    pub damage_was_admitted_before_cutoff: bool,
}

impl SimulationConfig {
    /// Refresh the two shared authority domains in one consistent read. Mirror
    /// reopening may recover source versions, but can never replace File business
    /// state with different PostgreSQL state or discard an observed version fence.
    pub fn refresh_shared_conquest_authority(&self) -> Result<(), String> {
        self.ensure_account_store_writable()?;
        let Some(database_url) = self.account_store_database_url.clone() else {
            return Ok(());
        };
        if self.account_store_database_mode == AccountStoreDatabaseMode::Mirror
            && self.account_store_path.is_none()
        {
            return Err("conquest mirror requires a File authority".into());
        }
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "conquest refresh persist mutex poisoned")?;
        self.ensure_account_store_writable()?;
        self.ensure_file_writer_binding()?;
        let original = self
            .account_store
            .lock()
            .map_err(|_| "conquest refresh account mutex poisoned")?
            .clone();
        let mut projection = original.clone();
        let (projection, guild_images, conquest_images) = std::thread::spawn(move || {
            let pool = postgres_account_store_pool(&database_url);
            let mut client = pool.connection()?;
            pool.ensure_migrated(&mut client)?;
            let mut transaction = client
                .build_transaction()
                .isolation_level(postgres::IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .map_err(|error| format!("conquest authority snapshot failed: {error}"))?;
            projection.shared_guilds.clear();
            projection.source_guild_versions.clear();
            projection.shared_conquests.clear();
            projection.source_conquest_versions.clear();
            let mut guild_images = BTreeMap::<String, serde_json::Value>::new();
            let mut conquest_images = BTreeMap::<i32, serde_json::Value>::new();
            for row in transaction
                .query(
                    "SELECT guild_id,raw_json,store_version FROM shared_guilds ORDER BY guild_id",
                    &[],
                )
                .map_err(|error| format!("conquest guild authority refresh failed: {error}"))?
            {
                let id: String = row.get(0);
                let image: serde_json::Value = row.get(1);
                let version: i64 = row.get(2);
                if version <= 0 {
                    return Err(format!("invalid conquest Guild {id} source version"));
                }
                let guild = serde_json::from_value(image.clone())
                    .map_err(|error| format!("invalid conquest Guild {id}: {error}"))?;
                projection.shared_guilds.insert(id.clone(), guild);
                projection.source_guild_versions.insert(id.clone(), version);
                guild_images.insert(id, image);
            }
            for row in transaction
                .query(
                    "SELECT conquest_index,raw_json,store_version FROM shared_conquests ORDER BY conquest_index",
                    &[],
                )
                .map_err(|error| format!("conquest authority refresh failed: {error}"))?
            {
                let index: i32 = row.get(0);
                let image: serde_json::Value = row.get(1);
                let version: i64 = row.get(2);
                if version <= 0 {
                    return Err(format!("invalid conquest {index} source version"));
                }
                let record = serde_json::from_value(image.clone())
                    .map_err(|error| format!("invalid conquest {index}: {error}"))?;
                projection.shared_conquests.insert(index, record);
                projection.source_conquest_versions.insert(index, version);
                conquest_images.insert(index, image);
            }
            shared_guild_store::validate_guild_state(&projection)?;
            // Validate membership against this same database snapshot, rather
            // than a coordinator's older personal-account cache.
            let mut indexed_members = std::collections::BTreeMap::<String, std::collections::BTreeSet<(String, i32)>>::new();
            for row in transaction.query("SELECT guild_id,account_id,character_index FROM shared_guild_members ORDER BY guild_id,account_id,character_index", &[]).map_err(|e| format!("conquest membership authority read failed: {e}"))? {
                indexed_members.entry(row.get(0)).or_default().insert((row.get(1), row.get(2)));
            }
            for (id, guild) in &projection.shared_guilds {
                let expected = guild.members.iter().map(|member| (member.identity.account_id.clone(), member.identity.character_index)).collect::<std::collections::BTreeSet<_>>();
                if indexed_members.remove(id).unwrap_or_default() != expected { return Err("conquest Guild membership authority mismatch".into()); }
            }
            if !indexed_members.is_empty() { return Err("conquest orphan Guild membership index".into()); }
            shared_conquest_store::validate_guild_bindings(&projection)?;
            shared_conquest_store::validate_complete_state(&projection)?;
            transaction
                .commit()
                .map_err(|error| format!("conquest authority read failed: {error}"))?;
            Ok::<_, String>((projection, guild_images, conquest_images))
        })
        .join()
        .map_err(|_| "conquest authority refresh worker panicked")??;
        if self.account_store_database_mode == AccountStoreDatabaseMode::Mirror {
            validate_mirror_observation(
                &original.shared_guilds,
                &original.source_guild_versions,
                &projection.source_guild_versions,
                &guild_images,
                "Guild",
            )?;
            validate_mirror_observation(
                &original.shared_conquests,
                &original.source_conquest_versions,
                &projection.source_conquest_versions,
                &conquest_images,
                "conquest",
            )?;
        }
        self.ensure_account_store_writable()?;
        let mut live = self
            .account_store
            .lock()
            .map_err(|_| "conquest refresh account mutex poisoned")?;
        if self.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth {
            if live.shared_guilds != projection.shared_guilds {
                self.guild_clock_updates
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            }
            live.shared_guilds = projection.shared_guilds;
            live.shared_conquests = projection.shared_conquests;
        }
        live.source_guild_versions = projection.source_guild_versions;
        live.source_conquest_versions = projection.source_conquest_versions;
        Ok(())
    }

    /// Convenience inspection. Authority-sensitive callers use the checked
    /// version so a poisoned or unavailable repository cannot become "unowned".
    pub fn shared_conquest_snapshot(&self, index: i32) -> Option<SharedConquestRecord> {
        self.shared_conquest_snapshot_checked(index).ok().flatten()
    }

    pub fn shared_conquest_snapshot_checked(
        &self,
        index: i32,
    ) -> Result<Option<SharedConquestRecord>, String> {
        self.configured_conquest_policy(index)?;
        self.refresh_shared_conquest_authority()?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "conquest snapshot account mutex poisoned")?;
        shared_conquest_store::validate_complete_state(&store)?;
        Ok(store.shared_conquests.get(&index).cloned())
    }

    /// One server clock calls this even when nobody is online. Guild identities
    /// come from the shared store, never the sampled presence's claimed guild.
    pub fn advance_shared_conquests(
        &self,
        now_ms: u64,
        clock_owner: &str,
        presences: &[ConquestPalacePresence],
    ) -> Result<Vec<ConquestEvent>, String> {
        self.advance_shared_conquests_with_defenses(now_ms, clock_owner, presences, &[])
    }

    /// A graceful server stop releases only its own committed clock grants.
    /// The next process can resume immediately without reviving the old token.
    pub fn release_shared_conquest_clock(&self, clock_owner: &str) -> Result<(), String> {
        if !valid_guild_id(clock_owner) {
            return Err("invalid conquest clock identity".into());
        }
        let policies = self.configured_conquest_policies()?;
        if policies.is_empty() {
            return Ok(());
        }
        self.refresh_shared_conquest_authority()?;
        let ids = policies
            .iter()
            .map(|policy| policy.index)
            .collect::<Vec<_>>();
        self.commit_account_store_transaction_inner(
            AccountStoreMutationScope::AccountsWithGuildsAndConquests {
                account_ids: &[],
                guild_ids: &[],
                conquest_ids: &ids,
            },
            |store| {
                validate_authority(store)?;
                for index in &ids {
                    if let Some(record) = store.shared_conquests.get_mut(index) {
                        if record
                            .lease
                            .as_ref()
                            .is_some_and(|lease| lease.owner == clock_owner)
                        {
                            record.lease = None;
                            record.changed()?;
                        }
                    }
                }
                Ok(())
            },
        )
    }

    /// Damage observation, palace capture and deadline settlement share the
    /// same durable commit. Only a successfully committed result is projected.
    pub fn advance_shared_conquests_with_defenses(
        &self,
        now_ms: u64,
        clock_owner: &str,
        presences: &[ConquestPalacePresence],
        defenses: &[ConquestDefenseSample],
    ) -> Result<Vec<ConquestEvent>, String> {
        let policies = self.configured_conquest_policies()?;
        if policies.is_empty() {
            return if defenses.is_empty() {
                Ok(Vec::new())
            } else {
                Err("conquest defense policy unavailable".into())
            };
        }
        if !valid_guild_id(clock_owner) || now_ms > i64::MAX as u64 {
            return Err("invalid conquest clock identity/time".into());
        }
        self.refresh_shared_conquest_authority()?;
        let conquest_ids: Vec<_> = policies.iter().map(|policy| policy.index).collect();
        let defenses = validate_defense_samples(defenses, &conquest_ids)?;
        let guild_ids = {
            let store = self
                .account_store
                .lock()
                .map_err(|_| "conquest clock account mutex poisoned")?;
            validate_authority(&store)?;
            let bound = bind_presences(&store, presences);
            if any_capture_possible(&store, &policies, now_ms, clock_owner, &bound)? {
                store.shared_guilds.keys().cloned().collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        };
        self.commit_account_store_transaction_inner(
            AccountStoreMutationScope::AccountsWithGuildsAndConquests {
                account_ids: &[],
                guild_ids: &guild_ids,
                conquest_ids: &conquest_ids,
            },
            |store| {
                validate_authority(store)?;
                let bound = bind_presences(store, presences);
                // Guild creation/deletion between scope selection and commit is
                // an explicit retry, not permission to widen the fixed scope.
                if any_capture_possible(store, &policies, now_ms, clock_owner, &bound)?
                    && !store.shared_guilds.keys().eq(guild_ids.iter())
                {
                    return Err("conquest presence authority changed; retry clock".into());
                }
                let mut events = Vec::new();
                for policy in &policies {
                    let mut record = record_or_new(store, policy)?;
                    validate_observation(&record, policy, now_ms)?;
                    apply_defense_samples(&mut record, policy, now_ms, clock_owner, &defenses)?;
                    events.extend(record.tick(policy, now_ms, clock_owner, &bound)?);
                    store.shared_conquests.insert(policy.index, record);
                }
                Ok(events)
            },
        )
    }

    /// Ordinary registrar request. Crystal's executable request has no material
    /// or personal-gold fee. NPC range and the trusted index are caller-owned.
    pub fn request_shared_conquest(
        &self,
        account_id: &str,
        character_index: i32,
        index: i32,
        now_ms: u64,
    ) -> Result<SharedConquestRecord, String> {
        let policy = self.configured_conquest_policy(index)?;
        policy.window(now_ms)?;
        self.refresh_shared_conquest_authority()?;
        let identity = actor_identity(account_id, character_index)?;
        let guild_id = self.conquest_actor_guild(&identity)?;
        let account_ids = [account_id.to_owned()];
        let guild_ids = [guild_id.clone()];
        let conquest_ids = [index];
        self.commit_account_store_transaction_inner(
            AccountStoreMutationScope::AccountsWithGuildsAndConquests {
                account_ids: &account_ids,
                guild_ids: &guild_ids,
                conquest_ids: &conquest_ids,
            },
            |store| {
                validate_authority(store)?;
                require_current_leader(store, &identity, &guild_id)?;
                let mut record = record_or_new(store, &policy)?;
                validate_observation(&record, &policy, now_ms)?;
                let window = policy.window(now_ms)?;
                if !record.war_active && window.open && record.battle_day == Some(window.day) {
                    return Err("conquest battle already settled for this window".into());
                }
                record.request(&guild_id, now_ms)?;
                store.shared_conquests.insert(index, record.clone());
                Ok(record)
            },
        )
    }

    /// Management uses the current owner's Guild bank. Conquest and bank writes
    /// share one bounded commit, including rollback and strict version fencing.
    pub fn manage_shared_conquest(
        &self,
        account_id: &str,
        character_index: i32,
        index: i32,
        now_ms: u64,
        action: ConquestManagementAction,
    ) -> Result<SharedConquestRecord, String> {
        let policy = self.configured_conquest_policy(index)?;
        policy.window(now_ms)?;
        self.refresh_shared_conquest_authority()?;
        let identity = actor_identity(account_id, character_index)?;
        let guild_id = self.conquest_actor_guild(&identity)?;
        let account_ids = [account_id.to_owned()];
        let guild_ids = [guild_id.clone()];
        let conquest_ids = [index];
        self.commit_account_store_transaction_inner(
            AccountStoreMutationScope::AccountsWithGuildsAndConquests {
                account_ids: &account_ids,
                guild_ids: &guild_ids,
                conquest_ids: &conquest_ids,
            },
            |store| {
                validate_authority(store)?;
                require_current_leader(store, &identity, &guild_id)?;
                let mut record = store
                    .shared_conquests
                    .get(&index)
                    .cloned()
                    .ok_or("conquest has no owner authority")?;
                validate_observation(&record, &policy, now_ms)?;
                if record.owner_guild_id.as_deref() != Some(&guild_id) {
                    return Err("only current castle owner's Guild may manage conquest".into());
                }
                // The original bound officer is hidden during war. A delayed
                // scheduler must not leave an already-open war window editable.
                if record.war_active
                    || (policy.window(now_ms)?.open && record.attacker_guild_id.is_some())
                {
                    return Err("conquest management unavailable during war".into());
                }
                let guild = store
                    .shared_guilds
                    .get_mut(&guild_id)
                    .ok_or("conquest owner Guild no longer exists")?;
                let original_bank = guild.gold;
                match &action {
                    ConquestManagementAction::SetTax { rate } => {
                        record.set_tax(*rate)?;
                    }
                    ConquestManagementAction::SetGate { key, open } => {
                        record.set_gate_open(key, *open)?;
                    }
                    ConquestManagementAction::Repair { key } => {
                        record.repair(key, &mut guild.gold)?;
                    }
                    ConquestManagementAction::WithdrawTax => {
                        record.withdraw_tax(&mut guild.gold)?;
                    }
                }
                if guild.gold != original_bank {
                    guild.revision = guild
                        .revision
                        .checked_add(1)
                        .ok_or("conquest owner Guild revision exhausted")?;
                }
                store.shared_conquests.insert(index, record.clone());
                Ok(record)
            },
        )
    }

    fn configured_conquest_policies(&self) -> Result<Vec<ConquestPolicy>, String> {
        let mut policies = BTreeMap::new();
        for policy in &self.conquest_policies {
            policy.validate()?;
            if policies.insert(policy.index, policy.clone()).is_some() {
                return Err("duplicate conquest policy index".into());
            }
        }
        Ok(policies.into_values().collect())
    }

    fn configured_conquest_policy(&self, index: i32) -> Result<ConquestPolicy, String> {
        self.configured_conquest_policies()?
            .into_iter()
            .find(|policy| policy.index == index)
            .ok_or_else(|| "conquest policy unavailable".into())
    }

    fn conquest_actor_guild(&self, identity: &Stage5FriendIdentity) -> Result<String, String> {
        let store = self
            .account_store
            .lock()
            .map_err(|_| "conquest applicant account mutex poisoned")?;
        validate_authority(&store)?;
        require_character(&store, identity)?;
        let guild = guild_for_identity(&store, identity)
            .ok_or("conquest requires current shared Guild membership")?;
        require_current_leader(&store, identity, &guild.id)?;
        Ok(guild.id.clone())
    }
}

fn validate_authority(store: &AccountStore) -> Result<(), String> {
    shared_guild_store::validate_guild_state(store)?;
    shared_conquest_store::validate_guild_bindings(store)?;
    shared_conquest_store::validate_complete_state(store)
}

fn actor_identity(account_id: &str, character_index: i32) -> Result<Stage5FriendIdentity, String> {
    if account_id.is_empty() || character_index < 0 {
        return Err("invalid conquest actor identity".into());
    }
    Ok(Stage5FriendIdentity {
        account_id: account_id.to_owned(),
        character_index,
    })
}

fn require_character(store: &AccountStore, identity: &Stage5FriendIdentity) -> Result<(), String> {
    if identity.account_id.is_empty()
        || identity.character_index < 0
        || !store
            .accounts
            .get(&identity.account_id)
            .is_some_and(|account| {
                account
                    .characters
                    .iter()
                    .any(|character| character.index == identity.character_index)
            })
    {
        return Err("conquest actor character no longer exists".into());
    }
    Ok(())
}

fn guild_for_identity<'a>(
    store: &'a AccountStore,
    identity: &Stage5FriendIdentity,
) -> Option<&'a SharedGuildRecord> {
    store
        .shared_guilds
        .values()
        .find(|guild| guild.member(identity).is_some())
}

fn require_current_leader(
    store: &AccountStore,
    identity: &Stage5FriendIdentity,
    expected_guild_id: &str,
) -> Result<(), String> {
    require_character(store, identity)?;
    let guild = guild_for_identity(store, identity)
        .ok_or("conquest requires current shared Guild membership")?;
    if guild.id != expected_guild_id || !valid_guild_id(expected_guild_id) {
        return Err("conquest actor's stable Guild changed".into());
    }
    if guild
        .member(identity)
        .is_none_or(|member| member.rank_index != 0)
    {
        return Err("conquest requires current Guild leader permission".into());
    }
    Ok(())
}

fn record_or_new(
    store: &AccountStore,
    policy: &ConquestPolicy,
) -> Result<SharedConquestRecord, String> {
    if let Some(record) = store.shared_conquests.get(&policy.index) {
        if record.index != policy.index {
            return Err("conquest stable record index mismatch".into());
        }
        record.validate()?;
        return Ok(record.clone());
    }
    let mut record = SharedConquestRecord::new(policy.index);
    if policy.map_file_name.eq_ignore_ascii_case("3")
        && policy.palace_file_name.eq_ignore_ascii_case("0150")
    {
        record.defenses = default_sabuk_defenses();
    }
    record.validate()?;
    Ok(record)
}

fn validate_observation(
    record: &SharedConquestRecord,
    policy: &ConquestPolicy,
    now_ms: u64,
) -> Result<(), String> {
    if record.index != policy.index || now_ms < record.last_observed_ms {
        return Err("stale conquest server observation".into());
    }
    if record
        .events
        .last()
        .is_some_and(|event| event.at_ms > now_ms)
    {
        return Err("conquest observation predates committed event".into());
    }
    policy.window(now_ms)?;
    record.validate()
}

fn bind_presences(
    store: &AccountStore,
    presences: &[ConquestPalacePresence],
) -> Vec<ConquestPalacePresence> {
    presences
        .iter()
        .map(|presence| {
            let identity = Stage5FriendIdentity {
                account_id: presence.account_id.clone(),
                character_index: presence.character_index,
            };
            let mut bound = presence.clone();
            bound.guild_id = if require_character(store, &identity).is_ok() {
                guild_for_identity(store, &identity).map(|guild| guild.id.clone())
            } else {
                // Unknown/deleted living occupants still contest. Dropping them
                // would turn uncertain authority into an attacker's capture.
                None
            };
            bound
        })
        .collect()
}

fn any_capture_possible(
    store: &AccountStore,
    policies: &[ConquestPolicy],
    now_ms: u64,
    clock_owner: &str,
    presences: &[ConquestPalacePresence],
) -> Result<bool, String> {
    for policy in policies {
        let record = record_or_new(store, policy)?;
        validate_observation(&record, policy, now_ms)?;
        let window = policy.window(now_ms)?;
        if !window.open
            || record
                .lease
                .as_ref()
                .is_some_and(|lease| lease.owner != clock_owner && lease.expires_ms > now_ms)
            || record.attacker_guild_id.is_none()
            || (record.war_active
                && (now_ms >= record.war_ends_ms || record.battle_day != Some(window.day)))
            || (!record.war_active && record.battle_day == Some(window.day))
            || (record.war_active
                && record.last_capture_ms != 0
                && now_ms.saturating_sub(record.last_capture_ms) < policy.capture_interval_ms)
        {
            continue;
        }
        let mut living = presences.iter().filter(|presence| {
            presence.alive
                && presence
                    .map_file_name
                    .eq_ignore_ascii_case(&policy.palace_file_name)
        });
        let Some(first) = living.next() else { continue };
        let Some(guild_id) = first.guild_id.as_deref() else {
            continue;
        };
        if record.owner_guild_id.as_deref() != Some(guild_id)
            && record.attacker_guild_id.as_deref() == Some(guild_id)
            && living.all(|presence| presence.guild_id.as_deref() == Some(guild_id))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_mirror_observation<K, V>(
    business: &BTreeMap<K, V>,
    expected: &BTreeMap<K, i64>,
    observed: &BTreeMap<K, i64>,
    images: &BTreeMap<K, serde_json::Value>,
    domain: &str,
) -> Result<(), String>
where
    K: Ord + std::fmt::Display,
    V: serde::Serialize,
{
    for (key, version) in expected {
        if observed.get(key) != Some(version) {
            return Err(format!(
                "stale conquest Mirror {domain} {key} source version"
            ));
        }
    }
    for (key, image) in images {
        let original = business
            .get(key)
            .ok_or_else(|| format!("unobserved conquest Mirror {domain} {key}"))?;
        if serde_json::to_value(original).map_err(|error| error.to_string())? != *image {
            return Err(format!(
                "unequal conquest Mirror {domain} {key} business state"
            ));
        }
    }
    Ok(())
}

fn validate_defense_samples(
    samples: &[ConquestDefenseSample],
    conquest_ids: &[i32],
) -> Result<BTreeMap<(i32, String), ConquestDefenseSample>, String> {
    let mut result = BTreeMap::new();
    for sample in samples {
        let key_is_valid = sample.key.split_once(':').is_some_and(|(kind, slot)| {
            matches!(kind, "gate" | "wall" | "archer")
                && slot.parse::<u8>().is_ok_and(|slot| slot > 0)
        });
        if !conquest_ids.contains(&sample.conquest_index)
            || sample.record_revision == 0
            || !key_is_valid
            || sample.hp < 0
            || sample.committed_hp < 0
            || (sample.damage_was_admitted_before_cutoff
                && (sample.battle_day.is_none()
                    || sample.war_ends_ms == 0
                    || sample.clock_generation == 0))
        {
            return Err("invalid authoritative conquest defense sample".into());
        }
        let key = (sample.conquest_index, sample.key.clone());
        if result.get(&key).is_some_and(|previous| previous != sample) {
            return Err("conflicting authoritative conquest defense samples".into());
        }
        result.insert(key, sample.clone());
    }
    Ok(result)
}

fn apply_defense_samples(
    record: &mut SharedConquestRecord,
    policy: &ConquestPolicy,
    now_ms: u64,
    clock_owner: &str,
    samples: &BTreeMap<(i32, String), ConquestDefenseSample>,
) -> Result<(), String> {
    let revision = record.revision;
    let clock_owned = record
        .lease
        .as_ref()
        .is_none_or(|lease| lease.owner == clock_owner || lease.expires_ms <= now_ms);
    let window = policy.window(now_ms)?;
    let damage_admitted = clock_owned
        && record.war_active
        && now_ms < record.war_ends_ms
        && record.battle_day == Some(window.day)
        && window.open;
    for ((index, _), sample) in samples {
        if *index != record.index || sample.record_revision > revision {
            continue;
        }
        let defense = record
            .defenses
            .get(&sample.key)
            .ok_or("unknown authoritative conquest defense")?;
        if sample.hp > defense.max_hp || sample.committed_hp > defense.max_hp {
            return Err("authoritative conquest defense HP exceeds maximum".into());
        }
        if sample.committed_hp != defense.hp || sample.hp == defense.hp || !clock_owned {
            continue;
        }
        if sample.hp > defense.hp {
            if defense.kind == crate::conquest::ConquestDefenseKind::Archer
                && defense.hp > 0
                && sample.clock_generation == record.clock_generation
                && sample.battle_day == record.battle_day
                && sample.war_ends_ms == record.war_ends_ms
            {
                record
                    .defenses
                    .get_mut(&sample.key)
                    .expect("validated defense")
                    .hp = sample.hp;
                record.changed()?;
            }
            continue;
        }
        let admitted_hit = sample.damage_was_admitted_before_cutoff
            && record.war_active
            && sample.battle_day == record.battle_day
            && sample.war_ends_ms == record.war_ends_ms
            && sample.clock_generation == record.clock_generation;
        if !admitted_hit || (!damage_admitted && now_ms < record.war_ends_ms) {
            continue;
        }
        // The trusted Zone already admitted this hit before its cutoff. Flush
        // the final interval before the same transaction settles WarEnded.
        record
            .defenses
            .get_mut(&sample.key)
            .expect("validated defense")
            .hp = sample.hp;
        record.changed()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AccountStoreTransactionFault, SharedGuildMember, SharedGuildRank};
    use crate::conquest::{sabuk_policy, ConquestEventKind};
    use std::collections::BTreeSet;

    const GUILD: &str = "0123456789abcdef0123456789abcdef";
    const OTHER: &str = "1123456789abcdef0123456789abcdef";
    const CLOCK: &str = "2123456789abcdef0123456789abcdef";
    const START: u64 = 86_400_000 + 18 * 60 * 60_000;

    fn fixture() -> (SimulationConfig, i32) {
        let mut config = SimulationConfig::default();
        config.conquest_policies = vec![sabuk_policy()];
        let mut store = config.account_store.lock().unwrap();
        let index = store.accounts["demo"].characters[0].index;
        let character = &store.accounts["demo"].characters[0];
        let guild = SharedGuildRecord {
            id: GUILD.into(),
            name: "Knights".into(),
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
                    character_index: index,
                },
                name: character.name.clone(),
                rank_index: 0,
            }],
            notice: Vec::new(),
            storage: BTreeMap::new(),
            buffs: BTreeMap::new(),
            last_buff_tick_ms: 0,
            experience_receipts: BTreeSet::new(),
            experience_receipt_payloads: BTreeMap::new(),
        };
        store.shared_guilds.insert(GUILD.into(), guild);
        drop(store);
        (config, index)
    }

    fn add_peer(config: &SimulationConfig, guild_member: bool) -> i32 {
        let mut store = config.account_store.lock().unwrap();
        let mut peer = store.accounts["demo"].clone();
        let old_index = peer.characters[0].index;
        let index = old_index + 1;
        peer.characters[0].index = index;
        peer.characters[0].name = "Peer".into();
        if let Some(mut save) = peer.saves.remove(&old_index) {
            save.character = peer.characters[0].clone();
            peer.saves.insert(index, save);
        }
        store.accounts.insert("peer".into(), peer);
        if guild_member {
            store
                .shared_guilds
                .get_mut(GUILD)
                .unwrap()
                .members
                .push(SharedGuildMember {
                    membership_epoch: 0,
                    identity: Stage5FriendIdentity {
                        account_id: "peer".into(),
                        character_index: index,
                    },
                    name: "Peer".into(),
                    rank_index: 1,
                });
        }
        index
    }

    fn owned(config: &SimulationConfig) {
        let mut store = config.account_store.lock().unwrap();
        let mut record = SharedConquestRecord::new(1);
        record.owner_guild_id = Some(GUILD.into());
        record.defenses = default_sabuk_defenses();
        store.shared_conquests.insert(1, record);
    }

    fn add_other_guild(config: &SimulationConfig) {
        let index = add_peer(config, false);
        let mut store = config.account_store.lock().unwrap();
        let mut guild = store.shared_guilds[GUILD].clone();
        guild.id = OTHER.into();
        guild.name = "Rivals".into();
        guild.members = vec![SharedGuildMember {
            membership_epoch: 0,
            identity: Stage5FriendIdentity {
                account_id: "peer".into(),
                character_index: index,
            },
            name: "Peer".into(),
            rank_index: 0,
        }];
        store.shared_guilds.insert(OTHER.into(), guild);
    }

    fn presence(account_id: &str, index: i32) -> ConquestPalacePresence {
        ConquestPalacePresence {
            account_id: account_id.into(),
            character_index: index,
            map_file_name: "0150".into(),
            alive: true,
            guild_id: Some(OTHER.into()),
        }
    }

    #[test]
    fn conquest_request_requires_available_unique_policy_and_real_current_leader() {
        let (mut config, index) = fixture();
        assert!(config
            .request_shared_conquest("missing", index, 1, START - 1)
            .is_err());
        assert!(config
            .request_shared_conquest("demo", -1, 1, START - 1)
            .is_err());
        assert!(config
            .request_shared_conquest("demo", index + 99, 1, START - 1)
            .is_err());
        assert!(config
            .request_shared_conquest("demo", index, 99, START - 1)
            .is_err());
        let peer = add_peer(&config, true);
        assert!(config
            .request_shared_conquest("peer", peer, 1, START - 1)
            .unwrap_err()
            .contains("leader"));
        config.conquest_policies.push(sabuk_policy());
        assert!(config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap_err()
            .contains("duplicate"));
        assert!(config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .is_empty());
    }

    #[test]
    fn conquest_request_is_free_and_duplicate_business_is_idempotent() {
        let (config, index) = fixture();
        let before =
            serde_json::to_value(&config.account_store.lock().unwrap().accounts["demo"]).unwrap();
        let record = config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        assert_eq!(record.attacker_guild_id.as_deref(), Some(GUILD));
        assert_eq!(record.events.len(), 1);
        assert!(!record.defenses.is_empty());
        assert_eq!(
            config
                .request_shared_conquest("demo", index, 1, START - 1)
                .unwrap(),
            record
        );
        assert_eq!(
            serde_json::to_value(&config.account_store.lock().unwrap().accounts["demo"]).unwrap(),
            before
        );
    }

    #[test]
    fn conquest_owner_and_busy_request_are_rejected_without_changing_business() {
        let (config, index) = fixture();
        add_other_guild(&config);
        owned(&config);
        assert!(config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap_err()
            .contains("own castle"));
        config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .get_mut(&1)
            .unwrap()
            .owner_guild_id = None;
        config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .get_mut(&1)
            .unwrap()
            .attacker_guild_id = Some(OTHER.into());
        let before = config.shared_conquest_snapshot_checked(1).unwrap();
        assert!(config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap_err()
            .contains("already applied"));
        assert_eq!(config.shared_conquest_snapshot_checked(1).unwrap(), before);
    }

    #[test]
    fn conquest_management_revalidates_identity_rank_owner_and_time() {
        let (config, index) = fixture();
        let peer = add_peer(&config, true);
        owned(&config);
        let action = ConquestManagementAction::SetTax { rate: 10 };
        assert!(config
            .manage_shared_conquest("peer", peer, 1, START - 1, action.clone())
            .is_err());
        config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .get_mut(&1)
            .unwrap()
            .owner_guild_id = Some(OTHER.into());
        assert!(config
            .manage_shared_conquest("demo", index, 1, START - 1, action.clone())
            .is_err());
        config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .get_mut(&1)
            .unwrap()
            .owner_guild_id = Some(GUILD.into());
        config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .get_mut(&1)
            .unwrap()
            .last_observed_ms = START;
        assert!(config
            .manage_shared_conquest("demo", index, 1, START - 1, action)
            .unwrap_err()
            .contains("stale"));
    }

    #[test]
    fn conquest_tax_and_gate_retry_do_not_charge_or_duplicate_revisions() {
        let (config, index) = fixture();
        owned(&config);
        let taxed = config
            .manage_shared_conquest(
                "demo",
                index,
                1,
                START - 1,
                ConquestManagementAction::SetTax { rate: 15 },
            )
            .unwrap();
        assert_eq!(
            config
                .manage_shared_conquest(
                    "demo",
                    index,
                    1,
                    START - 1,
                    ConquestManagementAction::SetTax { rate: 15 }
                )
                .unwrap(),
            taxed
        );
        assert!(config
            .manage_shared_conquest(
                "demo",
                index,
                1,
                START - 1,
                ConquestManagementAction::SetTax { rate: 0 }
            )
            .is_err());
        let opened = config
            .manage_shared_conquest(
                "demo",
                index,
                1,
                START - 1,
                ConquestManagementAction::SetGate {
                    key: "gate:1".into(),
                    open: true,
                },
            )
            .unwrap();
        assert!(opened.defenses["gate:1"].open);
        assert_eq!(
            config
                .manage_shared_conquest(
                    "demo",
                    index,
                    1,
                    START - 1,
                    ConquestManagementAction::SetGate {
                        key: "gate:1".into(),
                        open: true
                    }
                )
                .unwrap(),
            opened
        );
        assert_eq!(
            config.account_store.lock().unwrap().shared_guilds[GUILD].gold,
            10_000
        );
    }

    #[test]
    fn conquest_repairs_and_withdrawals_commit_both_domains_once() {
        let (config, index) = fixture();
        owned(&config);
        {
            let mut store = config.account_store.lock().unwrap();
            let record = store.shared_conquests.get_mut(&1).unwrap();
            record.defenses.get_mut("gate:1").unwrap().hp = 0;
            record.gold = 2_000;
        }
        let repaired = config
            .manage_shared_conquest(
                "demo",
                index,
                1,
                START - 1,
                ConquestManagementAction::Repair {
                    key: "gate:1".into(),
                },
            )
            .unwrap();
        assert_eq!(repaired.defenses["gate:1"].hp, 5_000);
        assert_eq!(
            config.account_store.lock().unwrap().shared_guilds[GUILD].gold,
            9_000
        );
        assert_eq!(
            config
                .manage_shared_conquest(
                    "demo",
                    index,
                    1,
                    START - 1,
                    ConquestManagementAction::Repair {
                        key: "gate:1".into()
                    }
                )
                .unwrap(),
            repaired
        );
        let withdrawn = config
            .manage_shared_conquest(
                "demo",
                index,
                1,
                START - 1,
                ConquestManagementAction::WithdrawTax,
            )
            .unwrap();
        assert_eq!(withdrawn.gold, 0);
        assert_eq!(
            config.account_store.lock().unwrap().shared_guilds[GUILD].gold,
            11_000
        );
        assert_eq!(
            config
                .manage_shared_conquest(
                    "demo",
                    index,
                    1,
                    START - 1,
                    ConquestManagementAction::WithdrawTax
                )
                .unwrap(),
            withdrawn
        );
        assert_eq!(
            config.account_store.lock().unwrap().shared_guilds[GUILD].revision,
            3
        );
    }

    #[test]
    fn conquest_insufficient_bank_and_persist_failure_leave_both_domains_unchanged() {
        let (config, index) = fixture();
        owned(&config);
        {
            let mut store = config.account_store.lock().unwrap();
            store
                .shared_conquests
                .get_mut(&1)
                .unwrap()
                .defenses
                .get_mut("gate:1")
                .unwrap()
                .hp = 0;
            store.shared_guilds.get_mut(GUILD).unwrap().gold = 999;
        }
        let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
        assert!(config
            .manage_shared_conquest(
                "demo",
                index,
                1,
                START - 1,
                ConquestManagementAction::Repair {
                    key: "gate:1".into()
                }
            )
            .is_err());
        assert_eq!(
            serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
            before
        );
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
        assert!(config
            .manage_shared_conquest(
                "demo",
                index,
                1,
                START - 1,
                ConquestManagementAction::SetTax { rate: 10 }
            )
            .is_err());
        assert_eq!(
            serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
            before
        );
    }

    #[test]
    fn conquest_zero_online_clock_starts_and_settles_with_management_closed_during_war() {
        let (config, index) = fixture();
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        let events = config.advance_shared_conquests(START, CLOCK, &[]).unwrap();
        assert!(events
            .iter()
            .any(|event| event.kind == ConquestEventKind::WarStarted));
        let active = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
        assert!(active.war_active);
        assert_eq!(active.owner_guild_id, None);
        let events = config
            .advance_shared_conquests(START + 30 * 60_000, CLOCK, &[])
            .unwrap();
        assert!(events
            .iter()
            .any(|event| event.kind == ConquestEventKind::WarEnded));
        assert!(
            !config
                .shared_conquest_snapshot_checked(1)
                .unwrap()
                .unwrap()
                .war_active
        );
    }

    #[test]
    fn conquest_presence_binds_fresh_membership_and_unknown_living_identity_contests() {
        let (config, index) = fixture();
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        let attacker = presence("demo", index);
        let missing = presence("deleted-character", 777);
        config
            .advance_shared_conquests(START, CLOCK, &[attacker.clone(), missing])
            .unwrap();
        assert_eq!(
            config
                .shared_conquest_snapshot_checked(1)
                .unwrap()
                .unwrap()
                .owner_guild_id,
            None
        );
        let events = config
            .advance_shared_conquests(START + 10_000, CLOCK, &[attacker])
            .unwrap();
        assert!(events
            .iter()
            .any(|event| event.kind == ConquestEventKind::Captured
                && event.guild_id.as_deref() == Some(GUILD)));
        assert_eq!(
            config
                .shared_conquest_snapshot_checked(1)
                .unwrap()
                .unwrap()
                .owner_guild_id
                .as_deref(),
            Some(GUILD)
        );
    }

    #[test]
    fn conquest_unguilded_player_contests_even_when_sample_claims_attacker() {
        let (config, index) = fixture();
        let peer = add_peer(&config, false);
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        let mut contestant = presence("peer", peer);
        contestant.guild_id = Some(GUILD.into());
        config
            .advance_shared_conquests(START, CLOCK, &[presence("demo", index), contestant])
            .unwrap();
        assert_eq!(
            config
                .shared_conquest_snapshot_checked(1)
                .unwrap()
                .unwrap()
                .owner_guild_id,
            None
        );
    }

    #[test]
    fn conquest_existing_defenses_are_never_reseeded_by_clock() {
        let (config, _) = fixture();
        config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .insert(1, SharedConquestRecord::new(1));
        config
            .advance_shared_conquests(START - 1, CLOCK, &[])
            .unwrap();
        assert!(config
            .shared_conquest_snapshot_checked(1)
            .unwrap()
            .unwrap()
            .defenses
            .is_empty());
    }

    #[test]
    fn conquest_stale_registration_and_late_same_day_restart_are_rejected() {
        let (config, index) = fixture();
        config.advance_shared_conquests(START, CLOCK, &[]).unwrap();
        assert!(config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap_err()
            .contains("stale"));
        {
            let mut store = config.account_store.lock().unwrap();
            let record = store.shared_conquests.get_mut(&1).unwrap();
            record.battle_day = Some(sabuk_policy().window(START).unwrap().day);
            record.war_ends_ms = START + 1_000;
        }
        assert!(config
            .request_shared_conquest("demo", index, 1, START + 2_000)
            .unwrap_err()
            .contains("already settled"));
        assert!(config
            .request_shared_conquest("demo", index, 1, START + 30 * 60_000)
            .is_ok());
    }

    #[test]
    fn conquest_management_closes_before_delayed_scheduler_opens_registered_war() {
        let (config, index) = fixture();
        add_other_guild(&config);
        owned(&config);
        config
            .account_store
            .lock()
            .unwrap()
            .shared_conquests
            .get_mut(&1)
            .unwrap()
            .attacker_guild_id = Some(OTHER.into());
        assert!(config
            .manage_shared_conquest(
                "demo",
                index,
                1,
                START,
                ConquestManagementAction::SetTax { rate: 10 }
            )
            .unwrap_err()
            .contains("during war"));
    }

    #[test]
    fn conquest_mirror_reopen_accepts_only_exact_images_and_keeps_known_cas_fence() {
        let business = BTreeMap::from([(1, SharedConquestRecord::new(1))]);
        let images = BTreeMap::from([(1, serde_json::to_value(&business[&1]).unwrap())]);
        let observed = BTreeMap::from([(1, 9)]);
        assert!(validate_mirror_observation(
            &business,
            &BTreeMap::new(),
            &observed,
            &images,
            "conquest"
        )
        .is_ok());
        assert!(validate_mirror_observation(
            &business,
            &BTreeMap::from([(1, 8)]),
            &observed,
            &images,
            "conquest"
        )
        .is_err());
        let mut extra = images.clone();
        extra.get_mut(&1).unwrap()["unrecognizedAuthority"] = serde_json::json!(true);
        assert!(validate_mirror_observation(
            &business,
            &BTreeMap::new(),
            &observed,
            &extra,
            "conquest"
        )
        .is_err());
        assert!(validate_mirror_observation(
            &BTreeMap::<i32, SharedConquestRecord>::new(),
            &BTreeMap::new(),
            &observed,
            &images,
            "conquest"
        )
        .is_err());
    }

    #[test]
    fn conquest_combined_clock_commits_authoritative_damage_without_healing() {
        let (config, index) = fixture();
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        config.advance_shared_conquests(START, CLOCK, &[]).unwrap();
        let before = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
        let samples = ["gate:1", "wall:1"].map(|key| ConquestDefenseSample {
            conquest_index: 1,
            record_revision: before.revision,
            battle_day: before.battle_day,
            war_ends_ms: before.war_ends_ms,
            clock_generation: before.clock_generation,
            damage_was_admitted_before_cutoff: true,
            key: key.into(),
            hp: 1_000,
            committed_hp: before.defenses[key].hp,
        });
        config
            .advance_shared_conquests_with_defenses(START + 1_000, CLOCK, &[], &samples)
            .unwrap();
        let damaged = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
        assert_eq!(damaged.defenses["gate:1"].hp, 1_000);
        assert_eq!(damaged.defenses["wall:1"].hp, 1_000);
        let healing = ConquestDefenseSample {
            conquest_index: 1,
            record_revision: damaged.revision,
            battle_day: damaged.battle_day,
            war_ends_ms: damaged.war_ends_ms,
            clock_generation: damaged.clock_generation,
            damage_was_admitted_before_cutoff: true,
            key: "gate:1".into(),
            hp: 5_000,
            committed_hp: 1_000,
        };
        config
            .advance_shared_conquests_with_defenses(START + 2_000, CLOCK, &[], &[healing])
            .unwrap();
        assert_eq!(
            config
                .shared_conquest_snapshot_checked(1)
                .unwrap()
                .unwrap()
                .defenses["gate:1"]
                .hp,
            1_000
        );
    }

    #[test]
    fn conquest_combined_clock_ignores_stale_repairs_and_rejects_conflicting_samples() {
        let (config, index) = fixture();
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        config.advance_shared_conquests(START, CLOCK, &[]).unwrap();
        let record = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
        let stale = ConquestDefenseSample {
            conquest_index: 1,
            record_revision: record.revision - 1,
            battle_day: record.battle_day,
            war_ends_ms: record.war_ends_ms,
            clock_generation: record.clock_generation,
            damage_was_admitted_before_cutoff: false,
            key: "gate:1".into(),
            hp: 0,
            committed_hp: 5_000,
        };
        config
            .advance_shared_conquests_with_defenses(START + 1_000, CLOCK, &[], &[stale.clone()])
            .unwrap();
        assert_eq!(
            config
                .shared_conquest_snapshot_checked(1)
                .unwrap()
                .unwrap()
                .defenses["gate:1"]
                .hp,
            5_000
        );
        let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
        let mut conflict = stale.clone();
        conflict.hp = 1;
        assert!(config
            .advance_shared_conquests_with_defenses(START + 2_000, CLOCK, &[], &[stale, conflict])
            .unwrap_err()
            .contains("conflicting"));
        assert_eq!(
            serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
            before
        );
    }

    #[test]
    fn conquest_combined_damage_and_capture_fail_as_one_transaction() {
        let (config, index) = fixture();
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        config.advance_shared_conquests(START, CLOCK, &[]).unwrap();
        let record = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
        let sample = ConquestDefenseSample {
            conquest_index: 1,
            record_revision: record.revision,
            battle_day: record.battle_day,
            war_ends_ms: record.war_ends_ms,
            clock_generation: record.clock_generation,
            damage_was_admitted_before_cutoff: true,
            key: "gate:1".into(),
            hp: 0,
            committed_hp: 5_000,
        };
        let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
        assert!(config
            .advance_shared_conquests_with_defenses(
                START + 10_000,
                CLOCK,
                &[presence("demo", index)],
                &[sample]
            )
            .is_err());
        assert_eq!(
            serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
            before
        );
    }

    #[test]
    fn conquest_final_interval_damage_is_flushed_before_settlement_only_with_admitted_proof() {
        for admitted in [false, true] {
            let (config, index) = fixture();
            config
                .request_shared_conquest("demo", index, 1, START - 1)
                .unwrap();
            config.advance_shared_conquests(START, CLOCK, &[]).unwrap();
            let before = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
            // Unrelated tax commerce may advance the revision between samples.
            config
                .account_store
                .lock()
                .unwrap()
                .shared_conquests
                .get_mut(&1)
                .unwrap()
                .revision += 1;
            let samples = ["gate:1", "archer:1"].map(|key| ConquestDefenseSample {
                conquest_index: 1,
                record_revision: before.revision,
                key: key.into(),
                hp: 0,
                committed_hp: before.defenses[key].hp,
                battle_day: before.battle_day,
                war_ends_ms: before.war_ends_ms,
                clock_generation: before.clock_generation,
                damage_was_admitted_before_cutoff: admitted,
            });
            config
                .advance_shared_conquests_with_defenses(before.war_ends_ms, CLOCK, &[], &samples)
                .unwrap();
            let settled = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
            assert!(!settled.war_active);
            for key in ["gate:1", "archer:1"] {
                assert_eq!(
                    settled.defenses[key].hp,
                    if admitted { 0 } else { before.defenses[key].hp }
                );
            }
            let roundtrip: SharedConquestRecord =
                serde_json::from_value(serde_json::to_value(&settled).unwrap()).unwrap();
            assert_eq!(roundtrip, settled);
        }
    }

    #[test]
    fn conquest_graceful_clock_release_is_fenced_and_new_process_resumes_without_replaying_capture()
    {
        let (config, index) = fixture();
        config
            .request_shared_conquest("demo", index, 1, START - 1)
            .unwrap();
        config
            .advance_shared_conquests(START, CLOCK, &[presence("demo", index)])
            .unwrap();
        let before = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
        config.release_shared_conquest_clock(OTHER).unwrap();
        assert_eq!(
            config.shared_conquest_snapshot_checked(1).unwrap().unwrap(),
            before
        );
        config.release_shared_conquest_clock(CLOCK).unwrap();
        let released = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
        assert!(released.lease.is_none());
        assert_eq!(released.owner_guild_id, before.owner_guild_id);
        assert!(config
            .advance_shared_conquests(START + 1, OTHER, &[presence("demo", index)])
            .unwrap()
            .is_empty());
        let restarted = config.shared_conquest_snapshot_checked(1).unwrap().unwrap();
        assert!(restarted.clock_generation > before.clock_generation);
        assert_eq!(restarted.events, before.events);
    }
}
