//! Synchronous repository clock pump. Gateway owns scheduling; no session or Zone
//! tick can independently advance guild time. Lock order is local coordinator,
//! repository publication mutex, account image, then PostgreSQL clock/guild rows.
use super::guild_clock::*;
use super::*;
const LEASE_MS: u64 = 30_000;
#[derive(Debug, Default)]
pub(super) struct ClockDriverState {
    owner: Option<String>,
    retained_lease: Option<GuildClockLease>,
    observed_guild_versions: Option<BTreeMap<String, i64>>,
    file_started: bool,
    frozen: Option<String>,
}
impl SimulationConfig {
    /// Called by one server-level scheduler even when there are no players.
    /// Returns stable guild IDs whose persisted business state changed.
    pub fn shared_guild_clock_generation(&self) -> u64 {
        self.guild_clock_updates
            .load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn tick_shared_guild_clock(&self) -> Result<Vec<String>, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("guild clock wall time invalid: {e}"))?
            .as_millis();
        let changed = self.tick_shared_guild_clock_at(
            u64::try_from(now).map_err(|_| "guild clock time exhausted")?,
        )?;
        if !changed.is_empty() {
            self.guild_clock_updates
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }
        Ok(changed)
    }
    fn tick_shared_guild_clock_at(&self, now_ms: u64) -> Result<Vec<String>, String> {
        let mut driver = self
            .guild_clock_driver
            .lock()
            .map_err(|_| "guild clock coordinator poisoned")?;
        if let Some(error) = &driver.frozen {
            return Err(error.clone());
        }
        self.ensure_account_store_writable()?;
        self.ensure_file_writer_binding()?;
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "account store persist mutex poisoned")?;
        self.ensure_account_store_writable()?;
        if self.account_store_database_url.is_some()
            && self.account_store_database_mode == AccountStoreDatabaseMode::Mirror
            && self.account_store_path.is_none()
        {
            return Err(
                "guild clock mirror requires a File authority before any PostgreSQL write".into(),
            );
        }
        let mut live = self
            .account_store
            .lock()
            .map_err(|_| "account store mutex poisoned")?;
        if driver.observed_guild_versions.is_none() {
            driver.observed_guild_versions = Some(live.source_guild_versions.clone());
        }
        if driver.owner.is_none() {
            let mut bytes = [0u8; 16];
            OsRng
                .try_fill_bytes(&mut bytes)
                .map_err(|e| format!("guild clock token unavailable: {e}"))?;
            driver.owner = Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect());
        }
        let owner = driver.owner.as_ref().unwrap().clone();
        if let Some(url) = &self.account_store_database_url {
            let mirror = (self.account_store_database_mode == AccountStoreDatabaseMode::Mirror)
                .then(|| live.clone());
            let mirror_input = mirror.clone();
            let pool = postgres_account_store_pool(url);
            let retained_lease = driver.retained_lease.clone();
            let observe =
                self.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth;
            let publication = self.begin_file_publication()?;
            let receipt = std::thread::spawn(move || {
                let mut client = pool
                    .connection()
                    .map_err(GuildClockTransactionError::from)?;
                pool.ensure_migrated(&mut client)
                    .map_err(GuildClockTransactionError::from)?;
                let receipt = transactions::commit_postgres_tick_checked_with_lease(
                    &mut client,
                    &owner,
                    LEASE_MS,
                    mirror_input.as_ref(),
                    retained_lease.as_ref(),
                )?;
                let observation = if observe {
                    observe_postgres_clock(&mut client).map(Some)
                } else {
                    Ok(None)
                };
                Ok((receipt, observation))
            })
            .join()
            .unwrap_or_else(|_| {
                Err(GuildClockTransactionError::CommitOutcomeUnknown(
                    "guild clock worker panicked".into(),
                ))
            });
            let (receipt, observation) = match receipt {
                Ok(result) => result,
                Err(error) => {
                    if matches!(error, GuildClockTransactionError::CommitOutcomeUnknown(_)) {
                        driver.frozen = Some(error.to_string());
                        if self.account_store_database_mode == AccountStoreDatabaseMode::Mirror {
                            return Err(self.freeze_account_store_writes(error.to_string()));
                        }
                    } else if let Some(publication) = publication {
                        publication.settle()?;
                    }
                    return Err(error.to_string());
                }
            };
            let observation = match observation {
                Ok(observation) => observation,
                Err(error) => {
                    // The write receipt remains authoritative even when the
                    // subsequent read fails. Retain its generation for a late
                    // retry instead of treating a known commit as a takeover.
                    if let Some(receipt) = receipt {
                        driver.retained_lease = receipt.after.lease();
                        if !receipt.after_guilds.is_empty() {
                            self.guild_clock_updates
                                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                        }
                        adopt_receipt(&mut live, &receipt);
                    }
                    return Err(error.to_string());
                }
            };
            if let Some(observation) = observation {
                let previous = driver.observed_guild_versions.as_ref().unwrap();
                let changed = previous
                    .keys()
                    .chain(observation.versions.keys())
                    .filter(|id| previous.get(*id) != observation.versions.get(*id))
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                driver.observed_guild_versions = Some(observation.versions.clone());
                if let Some(receipt) = receipt {
                    driver.retained_lease = receipt.after.lease();
                }
                live.guild_clock = Some(observation.clock.record);
                live.source_guild_clock_version = Some(observation.clock.version);
                live.shared_guilds = observation.guilds;
                live.source_guild_versions = observation.versions;
                return Ok(changed);
            }
            let Some(receipt) = receipt else {
                if let Some(publication) = publication {
                    publication.settle()?;
                }
                return Ok(vec![]);
            };
            if let Some(original) = mirror {
                let mut staged = original.clone();
                adopt_receipt(&mut staged, &receipt);
                let Some(path) = self.account_store_path.as_deref() else {
                    let error = "guild clock mirror requires a File authority".to_string();
                    driver.frozen = Some(error.clone());
                    return Err(error);
                };
                if let Err(error) = self.save_file_account_store(path, &staged) {
                    if error.is_commit_outcome_unknown() {
                        let error = self.map_file_commit_error(error);
                        driver.frozen = Some(error.clone());
                        return Err(error);
                    }
                    let first_error = error.to_string();
                    let pool = postgres_account_store_pool(url);
                    let rollback_receipt = receipt.clone();
                    let compensation = std::thread::spawn(move || {
                        let mut client = pool
                            .connection()
                            .map_err(GuildClockTransactionError::from)?;
                        transactions::compensate_postgres_tick(&mut client, &rollback_receipt)
                    })
                    .join()
                    .unwrap_or_else(|_| {
                        Err(GuildClockTransactionError::CommitOutcomeUnknown(
                            "guild clock compensation worker panicked".into(),
                        ))
                    });
                    match compensation {
                        Ok(compensation) => {
                            let mut restored = original;
                            adopt_receipt(&mut restored, &compensation);
                            if let Err(error) = self.save_file_account_store(path, &restored) {
                                let reason = format!("guild clock compensated PostgreSQL but File publication failed: {error}");
                                driver.frozen = Some(reason.clone());
                                return Err(self.freeze_account_store_writes(reason));
                            }
                            *live = restored;
                            if let Some(publication) = publication {
                                publication.settle()?;
                            }
                            return Err(first_error);
                        }
                        Err(error) => {
                            let reason = format!("guild clock File publication failed ({first_error}); compensation failed: {error}");
                            driver.frozen = Some(reason.clone());
                            return Err(self.freeze_account_store_writes(reason));
                        }
                    }
                }
                let ids = receipt.after_guilds.keys().cloned().collect();
                driver.retained_lease = receipt.after.lease();
                *live = staged;
                if let Some(publication) = publication {
                    publication.settle()?;
                }
                return Ok(ids);
            }
            let ids = receipt.after_guilds.keys().cloned().collect();
            driver.retained_lease = receipt.after.lease();
            live.guild_clock = Some(receipt.after);
            live.source_guild_clock_version = Some(receipt.clock_version);
            for (id, guild) in receipt.after_guilds {
                live.shared_guilds.insert(id, guild);
            }
            live.source_guild_versions.extend(receipt.guild_versions);
            return Ok(ids);
        }
        let mut staged = live.clone();
        let before = staged.guild_clock.clone().unwrap_or_default();
        // FileAuthority owns the OS writer lock. Its first pump after a real
        // process restart fences the previous process regardless of wall time.
        let (next, minutes) = if !driver.file_started {
            (
                before
                    .invalidate_preserving_anchor(before.generation)?
                    .acquire(&owner, now_ms, LEASE_MS)?,
                0,
            )
        } else if before.owner_token.as_deref() == Some(owner.as_str())
            && before.lease() == driver.retained_lease
        {
            let advance = before.advance_retained(
                driver
                    .retained_lease
                    .as_ref()
                    .ok_or("guild clock lease missing")?,
                now_ms,
                LEASE_MS,
            )?;
            (advance.next, advance.admitted_minutes)
        } else {
            (before.acquire(&owner, now_ms, LEASE_MS)?, 0)
        };
        let mut ids = Vec::new();
        if minutes > 0 {
            for (id, guild) in &mut staged.shared_guilds {
                let before = guild.clone();
                crate::runtime::advance_shared_guild_minutes(guild, minutes);
                if *guild != before {
                    guild.revision = guild
                        .revision
                        .checked_add(1)
                        .ok_or("guild revision exhausted")?;
                    ids.push(id.clone());
                }
            }
        }
        staged.guild_clock = Some(next);
        shared_guild_store::validate_complete_guild_state(&staged)?;
        let publication = self.begin_file_publication()?;
        if let Some(path) = &self.account_store_path {
            if let Err(error) = self.save_file_account_store(path, &staged) {
                if error.is_commit_outcome_unknown() {
                    let error = self.map_file_commit_error(error);
                    driver.frozen = Some(error.clone());
                    return Err(error);
                }
                if let Some(publication) = publication {
                    publication.settle()?;
                }
                return Err(error.to_string());
            }
        }
        driver.retained_lease = staged
            .guild_clock
            .as_ref()
            .and_then(GuildClockRecord::lease);
        *live = staged;
        driver.file_started = true;
        if let Some(publication) = publication {
            publication.settle()?;
        }
        Ok(ids)
    }
}

struct ClockObservation {
    clock: GuildClockSourceVersion,
    guilds: BTreeMap<String, SharedGuildRecord>,
    versions: BTreeMap<String, i64>,
}
fn observe_postgres_clock(
    client: &mut Client,
) -> Result<ClockObservation, GuildClockTransactionError> {
    // Standbys observe a consistent source view without locking or changing the
    // clock lease. A File mirror must never adopt this path.
    let mut transaction = client
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .map_err(|e| e.to_string())?;
    let clock = guild_clock::load_postgres(&mut transaction)?;
    let mut guilds = BTreeMap::new();
    let mut versions = BTreeMap::new();
    for row in transaction
        .query(
            "SELECT guild_id,raw_json,store_version FROM shared_guilds ORDER BY guild_id",
            &[],
        )
        .map_err(|e| e.to_string())?
    {
        let id: String = row.get(0);
        guilds.insert(
            id.clone(),
            serde_json::from_value(row.get(1)).map_err(|e| e.to_string())?,
        );
        versions.insert(id, row.get(2));
    }
    let mut view = AccountStore::new(CharacterRecord {
        index: 0,
        name: String::new(),
        level: 1,
        class: mir2_protocol::MirClass::Warrior,
        gender: mir2_protocol::MirGender::Male,
    });
    view.accounts.clear();
    view.shared_guilds = guilds;
    shared_guild_store::validate_guild_state(&view)?;
    transaction
        .commit()
        .map_err(|e| format!("guild observer read transaction failed: {e}"))?;
    Ok(ClockObservation {
        clock,
        guilds: view.shared_guilds,
        versions,
    })
}

fn adopt_receipt(store: &mut AccountStore, receipt: &transactions::GuildClockCommitReceipt) {
    store.guild_clock = Some(receipt.after.clone());
    store.source_guild_clock_version = Some(receipt.clock_version);
    store.shared_guilds.extend(receipt.after_guilds.clone());
    store
        .source_guild_versions
        .extend(receipt.guild_versions.clone());
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (SimulationConfig, PathBuf, String, i32) {
        let path = std::env::temp_dir()
            .join(format!(
                "mir2-guild-pump-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
            .join("accounts.json");
        let config = SimulationConfig::default().with_account_store_path(&path);
        let mut store = config.account_store.lock().unwrap();
        let mut guild = guild_clock::schema_tests::fixture_guild(&store);
        let buff = mir2_game_data::crystal_guild_buff_definitions()
            .iter()
            .find(|buff| buff.time_limit > 0)
            .unwrap();
        guild.buffs.insert(
            buff.id,
            SharedGuildBuff {
                id: buff.id,
                active: true,
                remaining_minutes: 2,
            },
        );
        let id = guild.id.clone();
        store.shared_guilds.insert(id.clone(), guild);
        drop(store);
        config.save_account_store().unwrap();
        (config, path, id, buff.id)
    }
    #[test]
    fn guild_clock_file_late_same_owner_keeps_anchor_and_charges_one_minute() {
        let (config, path, id, buff) = fixture();
        for now in [1000, 11000, 21000] {
            config.tick_shared_guild_clock_at(now).unwrap();
        }
        let before = config
            .account_store
            .lock()
            .unwrap()
            .guild_clock
            .clone()
            .unwrap();
        assert!(before.lease_expires_ms < 61000);
        config
            .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
        assert!(config.tick_shared_guild_clock_at(61000).is_err());
        assert_eq!(
            config.account_store.lock().unwrap().guild_clock.as_ref(),
            Some(&before)
        );
        assert_eq!(
            config.tick_shared_guild_clock_at(61000).unwrap(),
            vec![id.clone()]
        );
        let store = config.account_store.lock().unwrap();
        assert_eq!(
            store.guild_clock.as_ref().unwrap().generation,
            before.generation
        );
        assert_eq!(store.guild_clock.as_ref().unwrap().minute_anchor_ms, 61000);
        assert_eq!(store.shared_guilds[&id].buffs[&buff].remaining_minutes, 1);
        drop(store);
        assert!(config.tick_shared_guild_clock_at(61000).unwrap().is_empty());
        // Even a very delayed live loop charges only the original one interval.
        assert_eq!(
            config.tick_shared_guild_clock_at(900000).unwrap(),
            vec![id.clone()]
        );
        assert_eq!(
            config.account_store.lock().unwrap().shared_guilds[&id].buffs[&buff].remaining_minutes,
            0
        );
        drop(config);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn guild_clock_file_pump_runs_without_players_and_duplicate_factories_share_one_anchor() {
        let (first, path, id, buff) = fixture();
        let second = SimulationConfig::default().with_account_store_path(&path);
        first.tick_shared_guild_clock_at(1000).unwrap();
        for now in (11000..=61000).step_by(10000) {
            first.tick_shared_guild_clock_at(now).unwrap();
            assert!(second.tick_shared_guild_clock_at(now).unwrap().is_empty());
        }
        assert_eq!(
            first.account_store.lock().unwrap().shared_guilds[&id].buffs[&buff].remaining_minutes,
            1
        );
        let generation = first
            .account_store
            .lock()
            .unwrap()
            .guild_clock
            .as_ref()
            .unwrap()
            .generation;
        drop(second);
        drop(first);
        let restarted = SimulationConfig::default().with_account_store_path(&path);
        restarted.tick_shared_guild_clock_at(900000).unwrap();
        let store = restarted.account_store.lock().unwrap();
        assert_eq!(store.shared_guilds[&id].buffs[&buff].remaining_minutes, 1);
        assert!(store.guild_clock.as_ref().unwrap().generation > generation);
        assert_eq!(store.guild_clock.as_ref().unwrap().minute_anchor_ms, 900000);
        drop(store);
        drop(restarted);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn guild_clock_file_pump_publication_failure_preserves_anchor_and_retries_once() {
        let (config, path, id, buff) = fixture();
        config.tick_shared_guild_clock_at(1000).unwrap();
        for now in (11000..=51000).step_by(10000) {
            config.tick_shared_guild_clock_at(now).unwrap();
        }
        let before = fs::read(&path).unwrap();
        config
            .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
        assert!(config.tick_shared_guild_clock_at(61000).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(
            config
                .account_store
                .lock()
                .unwrap()
                .guild_clock
                .as_ref()
                .unwrap()
                .minute_anchor_ms,
            1000
        );
        assert_eq!(
            config.tick_shared_guild_clock_at(61000).unwrap(),
            vec![id.clone()]
        );
        assert_eq!(
            config.account_store.lock().unwrap().shared_guilds[&id].buffs[&buff].remaining_minutes,
            1
        );
        assert!(config.tick_shared_guild_clock_at(61000).unwrap().is_empty());
        drop(config);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
