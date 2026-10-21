//! DB-clock capabilities for the existing newcomer daily quest board. This
//! module never accepts a client date, reward, account or character generation.
use super::*;

const DAILY_IDS: [i32; 4] = [2_100_001, 2_100_002, 2_100_003, 2_100_004];
const UTC8_MS: i64 = 28_800_000;
const DAY_MS: i64 = 86_400_000;

/// Opaque, process-local proof of one server-read daily period and source base.
/// It is not serializable and cannot authorize a receipt or asset on its own.
#[derive(Clone)]
pub(crate) struct HuntingDailyQuestPeriod {
    binding: SharedAccountStore,
    database_binding: String,
    realm: String,
    owner: WalletOwner,
    quest_id: i32,
    period: i64,
    database_time_ms: u64,
    runtime_before_hash: String,
    canonical_hash: String,
    account_version: i64,
    save_version: i64,
    freeze_epoch: u64,
}
impl fmt::Debug for HuntingDailyQuestPeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HuntingDailyQuestPeriod")
            .field("quest_id", &self.quest_id)
            .field("period", &self.period)
            .finish_non_exhaustive()
    }
}
impl HuntingDailyQuestPeriod {
    pub(crate) fn database_time_ms(&self) -> u64 {
        self.database_time_ms
    }
    pub(crate) fn period(&self) -> i64 {
        self.period
    }
}

pub(super) fn period_at(time_ms: i64) -> Result<i64, EconomyError> {
    if time_ms < 0 {
        return Err(EconomyError::SourceRejected);
    }
    time_ms
        .checked_add(UTC8_MS)
        .map(|time| time.div_euclid(DAY_MS))
        .ok_or(EconomyError::Overflow("daily_quest_clock"))
}

pub(super) fn database_time_ms(tx: &mut Transaction<'_>) -> Result<i64, EconomyError> {
    tx.query_one(
        "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
        &[],
    )
    .map(|row| row.get(0))
    .map_err(source_db)
}

#[derive(Deserialize)]
struct CadenceState {
    quest_id: i32,
    stage: QuestStage,
    #[serde(default)]
    cadence_last_claimed_period: Option<u64>,
    #[serde(default)]
    cadence_high_watermark_period: Option<u64>,
}

fn states(save: &CharacterSaveRecord) -> Result<BTreeMap<i32, CadenceState>, String> {
    let mut states = BTreeMap::new();
    for row in &save.quest_states_json {
        let row: CadenceState =
            serde_json::from_str(row).map_err(|_| "daily quest state is invalid")?;
        if states.insert(row.quest_id, row).is_some() {
            return Err("daily quest checkpoint has duplicate quests".into());
        }
    }
    Ok(states)
}

fn validate_claimable(save: &CharacterSaveRecord, quest: i32, period: i64) -> Result<(), String> {
    if !DAILY_IDS.contains(&quest) || period < 0 {
        return Err("unsupported daily quest source".into());
    }
    let states = states(save)?;
    let period = period as u64;
    for (&id, row) in &states {
        // Native Daily 141 shares the same legacy daily high-watermark group.
        if !DAILY_IDS.contains(&id) && id != 141 {
            continue;
        }
        if row
            .cadence_high_watermark_period
            .is_some_and(|high| high > period)
            || row
                .cadence_last_claimed_period
                .is_some_and(|last| last > period)
        {
            return Err("daily quest checkpoint is ahead of the database period".into());
        }
        if DAILY_IDS.contains(&id)
            && row.stage == QuestStage::Completed
            && row.cadence_last_claimed_period.is_none()
        {
            // Ordinary recurrence initialization and a normal full save can
            // adopt legacy metadata. A read capability never rewrites it.
            return Err("legacy daily completion requires a normal canonical save".into());
        }
    }
    if states.get(&quest).is_some_and(|row| {
        row.cadence_last_claimed_period == Some(period)
            || (row.stage == QuestStage::Completed && row.cadence_last_claimed_period.is_none())
    }) {
        return Err("daily quest has already been claimed in this database period".into());
    }
    let completed = DAILY_IDS[..3]
        .iter()
        .filter(|id| {
            states.get(id).is_some_and(|row| {
                row.stage == QuestStage::Completed
                    && row.cadence_last_claimed_period == Some(period)
            })
        })
        .count();
    if quest == DAILY_IDS[3] {
        if completed < 2 {
            return Err("daily bonus requires two canonical same-period commissions".into());
        }
    } else if completed >= 2 {
        return Err("daily commissions are limited to two in this database period".into());
    }
    Ok(())
}

pub(super) fn validate_completed_period(
    save: &CharacterSaveRecord,
    quest: i32,
    period: i64,
) -> Result<(), String> {
    let states = states(save)?;
    let row = states.get(&quest).ok_or("daily completion missing")?;
    if period < 0
        || row.stage != QuestStage::Completed
        || row.cadence_last_claimed_period != Some(period as u64)
        || row.cadence_high_watermark_period != Some(period as u64)
    {
        return Err("daily completion does not own the captured database period".into());
    }
    Ok(())
}

fn canonical(
    tx: &mut Transaction<'_>,
    image: &LockedImage,
) -> Result<CharacterSaveRecord, EconomyError> {
    let save = image
        .account
        .saves
        .get(&image.owner.character_index)
        .ok_or(EconomyError::CorruptHistory("daily_current_checkpoint"))?;
    let mut roster = image
        .account
        .characters
        .iter()
        .filter(|character| character.index == image.owner.character_index);
    if roster.next() != Some(&save.character) || roster.next().is_some() {
        return Err(EconomyError::CorruptHistory("daily_current_roster"));
    }
    let row = tx.query_opt(
        "SELECT s.snapshot_json,c.character_name FROM character_saves s JOIN characters c USING(account_id,character_index) WHERE s.account_id=$1 AND s.character_index=$2",
        &[&image.owner.account_id,&image.owner.character_index],
    ).map_err(source_db)?.ok_or(EconomyError::CorruptHistory("daily_current_checkpoint"))?;
    if row.get::<_, Value>("snapshot_json")
        != to_json(save).map_err(|_| EconomyError::SourceRejected)?
        || row.get::<_, String>("character_name") != save.character.name
    {
        return Err(EconomyError::CorruptHistory("daily_current_checkpoint"));
    }
    Ok(save.clone())
}

pub(super) fn validate_first_write(
    tx: &mut Transaction<'_>,
    config: &SimulationConfig,
    cap: &HuntingDailyQuestPeriod,
    image: &LockedImage,
    before: &CharacterSaveRecord,
    after: &CharacterSaveRecord,
) -> Result<(), EconomyError> {
    let current = canonical(tx, image)?;
    if config.account_store_freeze_epoch.load(Ordering::Acquire) != cap.freeze_epoch
        || image.owner != cap.owner
        || image.versions.accounts.get(&cap.owner.account_id) != Some(&cap.account_version)
        || image
            .versions
            .saves
            .get(&cap.owner.account_id)
            .and_then(|saves| saves.get(&cap.owner.character_index))
            != Some(&cap.save_version)
        || canonical_hash(&to_json(&current).map_err(|_| EconomyError::SourceRejected)?)
            .map_err(|_| EconomyError::SourceRejected)?
            != cap.canonical_hash
        || canonical_hash(&to_json(before).map_err(|_| EconomyError::SourceRejected)?)
            .map_err(|_| EconomyError::SourceRejected)?
            != cap.runtime_before_hash
    {
        return Err(EconomyError::SourceRejected);
    }
    validate_claimable(&current, cap.quest_id, cap.period)
        .map_err(|_| EconomyError::SourceRejected)?;
    validate_claimable(before, cap.quest_id, cap.period)
        .map_err(|_| EconomyError::SourceRejected)?;
    validate_completed_period(after, cap.quest_id, cap.period)
        .map_err(|_| EconomyError::SourceRejected)
}

impl SimulationConfig {
    pub(crate) fn read_hunting_daily_quest_period(
        &self,
        account: &str,
        incarnation: &str,
        quest_id: i32,
        runtime_before: &CharacterSaveRecord,
    ) -> Result<HuntingDailyQuestPeriod, String> {
        if !DAILY_IDS.contains(&quest_id) {
            return Err("unsupported daily quest source".into());
        }
        validate_token(account, 256)?;
        validate_token(incarnation, 256)?;
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "daily period persistence lock unavailable")?;
        self.ensure_account_store_writable()?;
        let (realm, url) = self.hunting_source_binding()?;
        let mirror = self
            .account_store
            .lock()
            .map_err(|_| "daily period mirror unavailable")?
            .clone();
        let index = runtime_before.character.index;
        let image = with_account_store_postgres_client(url, |client| {
            let mut tx = client
                .transaction()
                .map_err(|_| "daily period transaction unavailable")?;
            tx.query_one("SELECT set_config('lock_timeout','2000ms',true),set_config('statement_timeout','10000ms',true)", &[])
                .map_err(|_| "daily period timeouts unavailable")?;
            economy::lock_realm_and_read_database_clock(&mut tx, realm)
                .map_err(|_| "daily period realm unavailable")?;
            let image = lock_and_load(
                &mut tx,
                account,
                index,
                Some(&runtime_before.character),
                std::iter::empty(),
                false,
            )
            .map_err(|_| "daily period owner unavailable")?
            .ok_or("daily period owner unavailable")?;
            if image.owner.incarnation != incarnation {
                return Err("daily period requires the original StartGame generation".into());
            }
            let current = canonical(&mut tx, &image)
                .map_err(|_| "daily period canonical state unavailable")?;
            if current.revision != runtime_before.revision
                || mirror.source_account_versions.get(account)
                    != image.versions.accounts.get(account)
                || mirror
                    .source_save_versions
                    .get(account)
                    .and_then(|saves| saves.get(&index))
                    != image
                        .versions
                        .saves
                        .get(account)
                        .and_then(|saves| saves.get(&index))
                || mirror
                    .accounts
                    .get(account)
                    .and_then(|account| account.saves.get(&index))
                    .map(to_json)
                    .transpose()?
                    != Some(to_json(&current)?)
            {
                return Err("daily period canonical source changed; reload required".into());
            }
            // Fresh AFTER account/save locks, including a wait over midnight.
            let time = database_time_ms(&mut tx).map_err(|_| "daily period clock unavailable")?;
            let period = period_at(time).map_err(|_| "daily period clock invalid")?;
            validate_claimable(&current, quest_id, period)?;
            validate_claimable(runtime_before, quest_id, period)?;
            let cap = HuntingDailyQuestPeriod {
                binding: self.account_store.clone(),
                database_binding: database_binding(url),
                realm: realm.into(),
                owner: image.owner,
                quest_id,
                period,
                database_time_ms: u64::try_from(time).map_err(|_| "daily period clock invalid")?,
                runtime_before_hash: canonical_hash(&to_json(runtime_before)?)?,
                canonical_hash: canonical_hash(&to_json(&current)?)?,
                account_version: *image
                    .versions
                    .accounts
                    .get(account)
                    .ok_or("daily account version missing")?,
                save_version: *image
                    .versions
                    .saves
                    .get(account)
                    .and_then(|saves| saves.get(&index))
                    .ok_or("daily save version missing")?,
                freeze_epoch: self.account_store_freeze_epoch.load(Ordering::Acquire),
            };
            tx.commit()
                .map_err(|_| "daily period read confirmation unavailable")?;
            Ok(cap)
        })?;
        Ok(image)
    }

    pub(crate) fn prepare_hunting_daily_quest_source<F>(
        &self,
        cap: &HuntingDailyQuestPeriod,
        digest: &str,
        amount: u64,
        runtime_before: CharacterSaveRecord,
        stage: F,
    ) -> Result<PreparedHuntingQuestSource, String>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        let (realm, url) = self.hunting_source_binding()?;
        if !Arc::ptr_eq(&self.account_store, &cap.binding)
            || realm != cap.realm
            || database_binding(url) != cap.database_binding
            || self.account_store_freeze_epoch.load(Ordering::Acquire) != cap.freeze_epoch
            || canonical_hash(&to_json(&runtime_before)?)? != cap.runtime_before_hash
        {
            return Err("daily period capability belongs to another source".into());
        }
        let prepared = self.prepare_hunting_quest_source_for_slot(
            &cap.owner.account_id,
            &cap.owner.incarnation,
            cap.quest_id,
            digest,
            amount,
            runtime_before,
            stage,
            QuestSourceSlot::Daily { period: cap.period },
            Some(cap.clone()),
        )?;
        validate_completed_period(prepared.checkpoint(), cap.quest_id, cap.period)?;
        Ok(prepared)
    }
}

#[cfg(test)]
#[path = "config_hunting_daily_source_tests.rs"]
mod tests;
