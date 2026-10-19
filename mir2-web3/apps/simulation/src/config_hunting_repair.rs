//! Trusted repair quotes, complete source CAS and bound payment in one PG commit.
use super::hunting_source::{
    canonical_hash, database_binding, lock_and_load, source_db, validate_token, LockedImage,
};
use super::*;
use mir2_economy_service::{
    self as economy, BoundDebitBurnReceipt, BurnLayer, EconomyError, Idempotent, SourceContext,
    TrustedBurn, WalletOwner, ZeroFeeSourceReceipt,
};
use postgres::GenericClient;
use std::cell::RefCell;
use std::sync::atomic::Ordering;

const KIND: &str = "npc_repair";
const QUOTE_TTL_MS: i64 = 30_000;

#[derive(Debug, Clone)]
pub(crate) struct HuntingRepairQuote {
    pub quote_id: String,
    pub cost: u64,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HuntingRepairReceipt {
    Charged(BoundDebitBurnReceipt),
    Free(ZeroFeeSourceReceipt),
}

#[derive(Debug, Clone)]
pub(crate) struct HuntingRepairCommit {
    /// Immutable original after-image, never a replacement for a later save.
    pub checkpoint: CharacterSaveRecord,
    pub current_revision: Option<u64>,
    pub receipt: Idempotent<HuntingRepairReceipt>,
}

#[derive(Debug)]
pub(crate) enum HuntingRepairSourceError {
    Rejected(String),
    OutcomeUnknown(String),
    KnownCommitted {
        commit: Box<HuntingRepairCommit>,
        detail: String,
    },
}
impl From<String> for HuntingRepairSourceError {
    fn from(value: String) -> Self {
        Self::Rejected(value)
    }
}
impl fmt::Display for HuntingRepairSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(s) | Self::OutcomeUnknown(s) => f.write_str(s),
            Self::KnownCommitted { detail, .. } => write!(f, "repair committed; {detail}"),
        }
    }
}
impl std::error::Error for HuntingRepairSourceError {}

#[derive(Debug, Clone)]
pub(super) struct HuntingRepairFence {
    realm: String,
    database_binding: String,
    owner: WalletOwner,
    quote_id: String,
    source_digest: String,
    cost: u64,
    guilds: Vec<String>,
    reason: String,
    epoch: u64,
}

#[derive(Clone)]
struct QuoteRow {
    view: HuntingRepairQuote,
    owner: WalletOwner,
    source_digest: String,
    revision: u64,
    json: Value,
}

struct Prepared {
    quote: QuoteRow,
    before: CharacterSaveRecord,
    original: AccountStore,
    staged: AccountStore,
    plan: AccountStoreMutationPlan,
}
impl Prepared {
    fn after(&self) -> &CharacterSaveRecord {
        &self.staged.accounts[&self.quote.owner.account_id].saves[&self.quote.owner.character_index]
    }
}

fn valid_quote_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate_actor(account: &str, witness: &str) -> Result<(), String> {
    validate_token(account, 256)?;
    validate_token(witness, 256)?;
    if !witness.starts_with("pg-character-v1:") {
        return Err("repair requires the original StartGame incarnation".into());
    }
    Ok(())
}

pub(crate) fn repair_quote_digest(value: &Value) -> Result<String, String> {
    canonical_hash(value)
}

fn validate_quote_json(value: &Value, cost: u64, digest: &str) -> Result<(), String> {
    if value.get("version").and_then(Value::as_str) != Some("hunting-repair-v1")
        || value.get("costBoundGold").and_then(Value::as_str) != Some(cost.to_string().as_str())
        || repair_quote_digest(value)? != digest
        || i64::try_from(cost).is_err()
    {
        return Err("invalid trusted repair quote payload".into());
    }
    Ok(())
}

fn read_quote<C: GenericClient>(
    db: &mut C,
    realm: &str,
    id: &str,
    locked: bool,
) -> Result<Option<QuoteRow>, EconomyError> {
    let sql = if locked {
        "SELECT * FROM hunting_repair_quotes WHERE realm_id=$1 AND quote_id=$2 FOR UPDATE"
    } else {
        "SELECT * FROM hunting_repair_quotes WHERE realm_id=$1 AND quote_id=$2"
    };
    let Some(row) = db.query_opt(sql, &[&realm, &id]).map_err(source_db)? else {
        return Ok(None);
    };
    let cost = u64::try_from(row.get::<_, i64>("cost_bound_gold"))
        .map_err(|_| EconomyError::CorruptHistory("repair_quote_cost"))?;
    let revision = u64::try_from(row.get::<_, i64>("quoted_save_revision"))
        .map_err(|_| EconomyError::CorruptHistory("repair_quote_revision"))?;
    let json: Value = row.get("quote_json");
    let digest: String = row.get("source_digest");
    if !valid_quote_id(id)
        || row.get::<_, String>("quote_digest") != digest
        || validate_quote_json(&json, cost, &digest).is_err()
    {
        return Err(EconomyError::CorruptHistory("repair_quote_digest"));
    }
    Ok(Some(QuoteRow {
        view: HuntingRepairQuote {
            quote_id: id.into(),
            cost,
            created_at_ms: row.get("created_at_ms"),
            expires_at_ms: row.get("expires_at_ms"),
        },
        owner: WalletOwner {
            account_id: row.get("account_id"),
            character_index: row.get("character_index"),
            incarnation: row.get("character_incarnation"),
        },
        source_digest: digest,
        revision,
        json,
    }))
}

fn source_key(quote_id: &str) -> String {
    format!("npc-repair-v1:{quote_id}")
}
fn burn(quote: &QuoteRow) -> TrustedBurn {
    TrustedBurn {
        source_kind: KIND.into(),
        source_key: source_key(&quote.view.quote_id),
        layer: BurnLayer::Daily,
        gold_equivalent: quote.view.cost,
        valuation_version: economy::BOUND_GOLD_FACE_VALUE_VALUATION_VERSION.into(),
    }
}
fn identity(realm: &str, quote: &QuoteRow) -> Result<(String, String), EconomyError> {
    if quote.view.cost == 0 {
        economy::zero_fee_source_operation_identity(
            realm,
            &quote.owner,
            KIND,
            &source_key(&quote.view.quote_id),
            &quote.source_digest,
        )
    } else {
        economy::bound_debit_burn_operation_identity(
            realm,
            &quote.owner,
            &burn(quote),
            &quote.source_digest,
        )
    }
}

fn read_result<C: GenericClient>(
    db: &mut C,
    realm: &str,
    quote: &QuoteRow,
) -> Result<Option<CharacterSaveRecord>, EconomyError> {
    let Some(row) = db
        .query_opt(
            "SELECT * FROM hunting_repair_results WHERE realm_id=$1 AND quote_id=$2",
            &[&realm, &quote.view.quote_id],
        )
        .map_err(source_db)?
    else {
        return Ok(None);
    };
    let (key, _) = identity(realm, quote)?;
    let data: Value = row.get("checkpoint_json");
    if row.get::<_, String>("account_id") != quote.owner.account_id
        || row.get::<_, i32>("character_index") != quote.owner.character_index
        || row.get::<_, String>("character_incarnation") != quote.owner.incarnation
        || row.get::<_, String>("source_digest") != quote.source_digest
        || row.get::<_, i64>("cost_bound_gold") != quote.view.cost as i64
        || row.get::<_, String>("operation_key") != key
        || row.get::<_, String>("payment_kind")
            != if quote.view.cost == 0 {
                "free"
            } else {
                "charged"
            }
        || row.get::<_, i64>("before_revision") != quote.revision as i64
        || canonical_hash(&data).map_err(|_| EconomyError::CorruptHistory("repair_checkpoint"))?
            != row.get::<_, String>("checkpoint_digest")
    {
        return Err(EconomyError::CorruptHistory("repair_source_identity"));
    }
    let checkpoint: CharacterSaveRecord = serde_json::from_value(data)
        .map_err(|_| EconomyError::CorruptHistory("repair_checkpoint"))?;
    if checkpoint.character.index != quote.owner.character_index
        || Some(checkpoint.revision) != quote.revision.checked_add(1)
        || i64::try_from(checkpoint.revision).ok() != Some(row.get("after_revision"))
    {
        return Err(EconomyError::CorruptHistory("repair_checkpoint_revision"));
    }
    Ok(Some(checkpoint))
}

fn verify_current(tx: &mut Transaction<'_>, image: &LockedImage) -> Result<(), EconomyError> {
    let save = image
        .account
        .saves
        .get(&image.owner.character_index)
        .ok_or(EconomyError::CharacterNotFound)?;
    let row=tx.query_opt("SELECT s.snapshot_json,c.character_name FROM character_saves s JOIN characters c USING(account_id,character_index) WHERE s.account_id=$1 AND s.character_index=$2",&[&image.owner.account_id,&image.owner.character_index]).map_err(source_db)?.ok_or(EconomyError::CharacterNotFound)?;
    if row.get::<_, Value>("snapshot_json")
        != to_json(save).map_err(|_| EconomyError::CorruptHistory("repair_current_checkpoint"))?
        || row.get::<_, String>("character_name") != save.character.name
    {
        return Err(EconomyError::CorruptHistory("repair_current_checkpoint"));
    }
    Ok(())
}

fn timeouts(tx: &mut Transaction<'_>) -> Result<(), EconomyError> {
    tx.query_one("SELECT set_config('lock_timeout','2000ms',true),set_config('statement_timeout','10000ms',true)",&[]).map_err(source_db)?;
    Ok(())
}

fn require_open(
    tx: &mut Transaction<'_>,
    realm: &str,
    clock: SourceContext,
) -> Result<(), EconomyError> {
    let row=tx.query_opt("SELECT p.genesis_day,p.settled_through_day,v.policy_json,v.policy_digest FROM hunting_economy_pools p JOIN hunting_economy_policies v USING(realm_id,policy_version) WHERE p.realm_id=$1 FOR UPDATE OF p",&[&realm]).map_err(source_db)?.ok_or(EconomyError::RealmMissing)?;
    let value: Value = row.get("policy_json");
    if canonical_hash(&value).map_err(|_| EconomyError::InvalidPolicy)?
        != row.get::<_, String>("policy_digest")
    {
        return Err(EconomyError::InvalidPolicy);
    }
    if value.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Err(EconomyError::Disabled);
    }
    let next = row
        .get::<_, i64>("settled_through_day")
        .checked_add(1)
        .ok_or(EconomyError::Overflow("settlement_day"))?;
    if row.get::<_, i64>("genesis_day") > clock.business_day || next > clock.business_day {
        return Err(EconomyError::CorruptHistory("future_settlement_cursor"));
    }
    if next != clock.business_day {
        return Err(EconomyError::SettlementRequired {
            next_day: next,
            current_day: clock.business_day,
        });
    }
    Ok(())
}

fn validate_receipt(
    realm: &str,
    quote: &QuoteRow,
    receipt: &HuntingRepairReceipt,
) -> Result<(), EconomyError> {
    let (key, hash) = identity(realm, quote)?;
    let valid = match receipt {
        HuntingRepairReceipt::Charged(r) => {
            quote.view.cost > 0
                && r.realm_id == realm
                && r.owner == quote.owner
                && r.operation_key == key
                && r.request_digest == hash
                && r.bound_gold_debit == quote.view.cost
                && r.source_kind == KIND
                && r.source_key == source_key(&quote.view.quote_id)
                && r.layer == BurnLayer::Daily
                && r.valuation_version == economy::BOUND_GOLD_FACE_VALUE_VALUATION_VERSION
        }
        HuntingRepairReceipt::Free(r) => {
            quote.view.cost == 0
                && r.realm_id == realm
                && r.owner == quote.owner
                && r.operation_key == key
                && r.request_digest == hash
                && r.source_kind == KIND
                && r.source_key == source_key(&quote.view.quote_id)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(EconomyError::CorruptHistory("repair_receipt_identity"))
    }
}

fn read_receipt(
    tx: &mut Transaction<'_>,
    realm: &str,
    quote: &QuoteRow,
    has_result: bool,
) -> Result<Option<HuntingRepairReceipt>, EconomyError> {
    let (key, hash) = identity(realm, quote)?;
    let Some(row)=tx.query_opt("SELECT operation_kind,request_digest,receipt_json,receipt_digest,business_day,committed_at_ms FROM hunting_economy_operations WHERE realm_id=$1 AND operation_key=$2",&[&realm,&key]).map_err(source_db)? else {
        return if has_result { Err(EconomyError::CorruptHistory("repair_result_without_receipt")) } else { Ok(None) };
    };
    if !has_result {
        return Err(EconomyError::CorruptHistory(
            "repair_receipt_without_result",
        ));
    }
    if row.get::<_, String>("request_digest") != hash
        || row.get::<_, String>("operation_kind")
            != if quote.view.cost == 0 {
                "zero_fee_source"
            } else {
                "bound_debit_burn"
            }
    {
        return Err(EconomyError::RequestConflict);
    }
    let value: Value = row.get("receipt_json");
    if canonical_hash(&value).map_err(|_| EconomyError::CorruptHistory("repair_receipt"))?
        != row.get::<_, String>("receipt_digest")
    {
        return Err(EconomyError::CorruptHistory("repair_receipt_digest"));
    }
    let receipt = if quote.view.cost == 0 {
        HuntingRepairReceipt::Free(
            serde_json::from_value(value)
                .map_err(|_| EconomyError::CorruptHistory("repair_receipt"))?,
        )
    } else {
        HuntingRepairReceipt::Charged(
            serde_json::from_value(value)
                .map_err(|_| EconomyError::CorruptHistory("repair_receipt"))?,
        )
    };
    validate_receipt(realm, quote, &receipt)?;
    let (day, time) = match &receipt {
        HuntingRepairReceipt::Charged(r) => (r.business_day, r.committed_at_ms),
        HuntingRepairReceipt::Free(r) => (r.business_day, r.committed_at_ms),
    };
    let result=tx.query_one("SELECT business_day,committed_at_ms FROM hunting_repair_results WHERE realm_id=$1 AND quote_id=$2",&[&realm,&quote.view.quote_id]).map_err(source_db)?;
    if row.get::<_, i64>("business_day") != day
        || row.get::<_, i64>("committed_at_ms") != time
        || result.get::<_, i64>("business_day") != day
        || result.get::<_, i64>("committed_at_ms") != time
    {
        return Err(EconomyError::CorruptHistory("repair_receipt_clock"));
    }
    Ok(Some(receipt))
}

fn lock_guilds(tx: &mut Transaction<'_>, guilds: &[String]) -> Result<(), EconomyError> {
    for id in guilds {
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,735891))",
            &[id],
        )
        .map_err(source_db)?;
        tx.query_opt(
            "SELECT guild_id FROM shared_guilds WHERE guild_id=$1 FOR UPDATE",
            &[id],
        )
        .map_err(source_db)?;
    }
    Ok(())
}

fn prepare<F>(
    original: AccountStore,
    quote: QuoteRow,
    before: CharacterSaveRecord,
    stage: F,
) -> Result<Prepared, String>
where
    F: FnOnce(&mut AccountStore) -> Result<(), String>,
{
    let account = &quote.owner.account_id;
    let index = quote.owner.character_index;
    let durable = original
        .accounts
        .get(account)
        .and_then(|a| a.saves.get(&index))
        .ok_or("repair durable checkpoint missing")?;
    if before.character.index != index
        || before.character.name != durable.character.name
        || before.revision != quote.revision
        || durable.revision != before.revision
    {
        return Err("repair quote or complete save revision changed".into());
    }
    let mut staged = original.clone();
    stage(&mut staged)?;
    let accounts = [account.clone()];
    validate_account_store_transaction_scope(
        &original,
        &staged,
        AccountStoreMutationScope::Accounts(&accounts),
    )
    .map_err(|e| e.to_string())?;
    let guilds =
        guild_experience::settle_authorized_sources(&original, &mut staged, &accounts, None)?
            .into_iter()
            .collect::<Vec<_>>();
    let scope = AccountStoreMutationScope::AccountsWithGuilds {
        account_ids: &accounts,
        guild_ids: &guilds,
    };
    validate_account_store_transaction_scope(&original, &staged, scope)
        .map_err(|e| e.to_string())?;
    let after = staged
        .accounts
        .get(account)
        .and_then(|a| a.saves.get(&index))
        .ok_or("repair staged checkpoint missing")?;
    if Some(after.revision) != before.revision.checked_add(1)
        || i64::try_from(after.revision).is_err()
        || after.character.index != index
        || after.character.name != before.character.name
        || after.gold != before.gold
    {
        return Err("repair must CAS one complete save without changing legacy gold".into());
    }
    let mut expected = original.accounts[account].clone();
    expected.saves.insert(index, after.clone());
    if let Some(character) = expected.characters.iter_mut().find(|c| c.index == index) {
        *character = after.character.clone();
    }
    if to_json(&expected)? != to_json(&staged.accounts[account])? {
        return Err("repair changed unrelated account state".into());
    }
    let plan = build_account_store_mutation_plan(&original, &staged, scope, false);
    if plan.global_stock.is_some()
        || plan.clock.is_some()
        || !plan.heroes.is_empty()
        || plan.hero_allocator.is_some()
    {
        return Err("repair changed unrelated shared state".into());
    }
    serde_json::to_vec(&staged).map_err(|_| "repair checkpoint encoding failed")?;
    Ok(Prepared {
        quote,
        before,
        original,
        staged,
        plan,
    })
}

impl SimulationConfig {
    pub(crate) fn register_hunting_repair_quote(
        &self,
        account: &str,
        incarnation: &str,
        quote_id: &str,
        source_digest: &str,
        cost: u64,
        before_revision: u64,
        quote_json: Value,
    ) -> Result<HuntingRepairQuote, String> {
        self.register_hunting_repair_quote_inner(
            account,
            incarnation,
            quote_id,
            source_digest,
            cost,
            before_revision,
            quote_json,
            QUOTE_TTL_MS,
        )
    }

    #[cfg(test)]
    pub(crate) fn register_hunting_repair_quote_with_ttl_for_test(
        &self,
        account: &str,
        incarnation: &str,
        quote_id: &str,
        source_digest: &str,
        cost: u64,
        before_revision: u64,
        quote_json: Value,
        ttl_ms: i64,
    ) -> Result<HuntingRepairQuote, String> {
        self.register_hunting_repair_quote_inner(
            account,
            incarnation,
            quote_id,
            source_digest,
            cost,
            before_revision,
            quote_json,
            ttl_ms,
        )
    }

    fn register_hunting_repair_quote_inner(
        &self,
        account: &str,
        incarnation: &str,
        quote_id: &str,
        source_digest: &str,
        cost: u64,
        before_revision: u64,
        quote_json: Value,
        ttl_ms: i64,
    ) -> Result<HuntingRepairQuote, String> {
        self.ensure_account_store_writable()?;
        let (realm, url) = self.hunting_source_binding()?;
        validate_actor(account, incarnation)?;
        validate_token(source_digest, 256)?;
        if !valid_quote_id(quote_id)
            || !(1..=QUOTE_TTL_MS).contains(&ttl_ms)
            || i64::try_from(before_revision).is_err()
        {
            return Err("invalid trusted repair quote identity or TTL".into());
        }
        validate_quote_json(&quote_json, cost, source_digest)?;
        with_account_store_postgres_client(url, |client| {
            let mut tx = client
                .transaction()
                .map_err(|_| "repair quote transaction unavailable")?;
            timeouts(&mut tx).map_err(|_| "repair quote timeouts unavailable")?;
            let clock = economy::lock_realm_and_read_database_clock(&mut tx, realm)
                .map_err(|_| "repair quote realm unavailable")?;
            // This unlocked hint only locates the expected UUID; authorization
            // and generation are checked again under account/character locks.
            let uuid = incarnation
                .strip_prefix("pg-character-v1:")
                .ok_or("repair incarnation unavailable")?;
            let index:i32=tx.query_opt("SELECT character_index FROM characters WHERE account_id=$1 AND hunting_incarnation::text=$2",&[&account,&uuid]).map_err(|_|"repair character unavailable")?.ok_or("repair character unavailable")?.get(0);
            let image = lock_and_load(&mut tx, account, index, None, std::iter::empty(), false)
                .map_err(|_| "repair owner unavailable")?
                .ok_or("repair owner unavailable")?;
            if image.owner.incarnation != incarnation {
                return Err("repair character incarnation changed".into());
            }
            verify_current(&mut tx, &image)
                .map_err(|_| "repair complete checkpoint unavailable")?;
            if let Some(old) = read_quote(&mut tx, realm, quote_id, true)
                .map_err(|_| "repair quote unavailable")?
            {
                if old.owner != image.owner
                    || old.source_digest != source_digest
                    || old.view.cost != cost
                    || old.revision != before_revision
                    || old.json != quote_json
                {
                    return Err("repair quote identity was already used for another payload".into());
                }
                tx.commit()
                    .map_err(|_| "repair quote confirmation unavailable")?;
                return Ok(old.view);
            }
            require_open(&mut tx, realm, clock)
                .map_err(|_| "repair source is disabled or settlement is behind")?;
            if image
                .account
                .saves
                .get(&index)
                .is_none_or(|save| save.revision != before_revision)
            {
                return Err("repair quote save revision changed".into());
            }
            let expires = clock
                .database_time_ms
                .checked_add(ttl_ms)
                .ok_or("repair quote expiry exhausted")?;
            tx.execute("INSERT INTO hunting_repair_quotes(realm_id,quote_id,account_id,character_index,character_incarnation,source_digest,cost_bound_gold,quoted_save_revision,quote_json,quote_digest,created_at_ms,expires_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$6,$10,$11)",
                &[&realm,&quote_id,&account,&index,&incarnation,&source_digest,&(cost as i64),&(before_revision as i64),&quote_json,&clock.database_time_ms,&expires]).map_err(|_|"repair quote persistence failed")?;
            tx.commit()
                .map_err(|_| "repair quote confirmation unavailable")?;
            Ok(HuntingRepairQuote {
                quote_id: quote_id.into(),
                cost,
                created_at_ms: clock.database_time_ms,
                expires_at_ms: expires,
            })
        })
        .map_err(|_| "repair quote registration unavailable".into())
    }

    fn fence_hunting_repair(
        &self,
        realm: &str,
        url: &str,
        quote: &QuoteRow,
        guilds: Vec<String>,
        detail: String,
    ) -> String {
        let (reason, epoch, newly_frozen) = self.freeze_account_store_writes_with_epoch(detail);
        if !newly_frozen {
            return reason;
        }
        if let Ok(mut slot) = self.hunting_repair_fence.lock() {
            *slot = Some(HuntingRepairFence {
                realm: realm.into(),
                database_binding: database_binding(url),
                owner: quote.owner.clone(),
                quote_id: quote.view.quote_id.clone(),
                source_digest: quote.source_digest.clone(),
                cost: quote.view.cost,
                guilds,
                reason: reason.clone(),
                epoch,
            });
        }
        reason
    }

    pub(crate) fn publish_hunting_repair_source<F>(
        &self,
        account: &str,
        incarnation: &str,
        quote_id: &str,
        before: CharacterSaveRecord,
        stage: F,
    ) -> Result<HuntingRepairCommit, HuntingRepairSourceError>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        self.publish_hunting_repair_source_inner(account, incarnation, quote_id, before, stage, 0)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn publish_hunting_repair_source_with_test_fault<F>(
        &self,
        account: &str,
        incarnation: &str,
        quote_id: &str,
        before: CharacterSaveRecord,
        stage: F,
        fault: u8,
    ) -> Result<HuntingRepairCommit, HuntingRepairSourceError>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        self.publish_hunting_repair_source_inner(
            account,
            incarnation,
            quote_id,
            before,
            stage,
            fault,
        )
    }

    fn publish_hunting_repair_source_inner<F>(
        &self,
        account: &str,
        incarnation: &str,
        quote_id: &str,
        before: CharacterSaveRecord,
        stage: F,
        test_fault: u8,
    ) -> Result<HuntingRepairCommit, HuntingRepairSourceError>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        validate_actor(account, incarnation)?;
        if !valid_quote_id(quote_id) {
            return Err("invalid repair quote identity".to_string().into());
        }
        // Existing committed requests never depend on a fresh NPC quote, TTL or
        // replanning the item. This also resolves this quote's own Unknown fence.
        if let Some(commit) = self.lookup_hunting_repair_source(
            account,
            incarnation,
            before.character.index,
            quote_id,
        )? {
            return Ok(commit);
        }
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "repair persistence lock poisoned".to_string())?;
        self.ensure_account_store_writable()?;
        let (realm, url) = self.hunting_source_binding()?;
        let quote = with_account_store_postgres_client(url, |client| {
            read_quote(client, realm, quote_id, false)
                .map_err(|_| "repair quote unavailable".into())
        })
        .map_err(|_| "repair quote repository unavailable".to_string())?
        .ok_or_else(|| "repair quote not registered".to_string())?;
        if quote.owner.account_id != account
            || quote.owner.incarnation != incarnation
            || quote.owner.character_index != before.character.index
        {
            return Err("repair quote owner mismatch".to_string().into());
        }
        let mut live = self
            .account_store
            .lock()
            .map_err(|_| "repair account image poisoned".to_string())?;
        let prepared = prepare(live.clone(), quote, before, stage)?;
        let locked = RefCell::new(None::<LockedImage>);
        let original_result = RefCell::new(None::<CharacterSaveRecord>);
        let written = RefCell::new(None::<AccountStoreSourceVersions>);
        let quote = &prepared.quote;
        let delivered = with_account_store_postgres_client(url, |client| {
            let authorize = |tx: &mut Transaction<'_>, _clock: SourceContext| {
                prepared
                    .plan
                    .lock_guild_plan(tx)
                    .map_err(|_| EconomyError::SourceRejected)?;
                let image = lock_and_load(
                    tx,
                    account,
                    quote.owner.character_index,
                    Some(&prepared.before.character),
                    prepared.plan.guilds.keys().cloned(),
                    false,
                )?
                .ok_or(EconomyError::CharacterNotFound)?;
                if image.owner != quote.owner {
                    return Err(EconomyError::CharacterNotFound);
                }
                verify_current(tx, &image)?;
                let current =
                    read_quote(tx, realm, quote_id, true)?.ok_or(EconomyError::SourceRejected)?;
                if current.owner != quote.owner
                    || current.source_digest != quote.source_digest
                    || current.revision != quote.revision
                    || current.view.cost != quote.view.cost
                    || current.json != quote.json
                {
                    return Err(EconomyError::RequestConflict);
                }
                *original_result.borrow_mut() = read_result(tx, realm, &current)?;
                let owner = image.owner.clone();
                *locked.borrow_mut() = Some(image);
                Ok(owner)
            };
            let source = |tx: &mut Transaction<'_>, clock: SourceContext| {
                if clock.database_time_ms >= quote.view.expires_at_ms
                    || quote.revision != prepared.before.revision
                {
                    return Err(EconomyError::SourceRejected);
                }
                {
                    let image = locked.borrow();
                    let image = image.as_ref().ok_or(EconomyError::SourceRejected)?;
                    if to_json(&image.account).map_err(|_| EconomyError::SourceRejected)?
                        != to_json(&prepared.original.accounts[account])
                            .map_err(|_| EconomyError::SourceRejected)?
                    {
                        return Err(EconomyError::SourceRejected);
                    }
                    for id in prepared.plan.guilds.keys() {
                        if to_json(&image.guilds.get(id).cloned().flatten())
                            .map_err(|_| EconomyError::SourceRejected)?
                            != to_json(&prepared.original.shared_guilds.get(id))
                                .map_err(|_| EconomyError::SourceRejected)?
                        {
                            return Err(EconomyError::SourceRejected);
                        }
                    }
                }
                let versions = write_account_store_mutation_plan_in_transaction(
                    tx,
                    &prepared.plan,
                    AccountStoreDatabaseMode::SourceOfTruth,
                )
                .map_err(|_| EconomyError::SourceRejected)?;
                let (operation, _) = identity(realm, quote)?;
                let checkpoint =
                    to_json(prepared.after()).map_err(|_| EconomyError::SourceRejected)?;
                let hash = canonical_hash(&checkpoint).map_err(|_| EconomyError::SourceRejected)?;
                let payment = if quote.view.cost == 0 {
                    "free"
                } else {
                    "charged"
                };
                tx.execute("INSERT INTO hunting_repair_results(realm_id,quote_id,account_id,character_index,character_incarnation,source_digest,cost_bound_gold,payment_kind,before_revision,after_revision,checkpoint_json,checkpoint_digest,operation_key,business_day,committed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
                    &[&realm,&quote_id,&account,&quote.owner.character_index,&incarnation,&quote.source_digest,&(quote.view.cost as i64),&payment,&(prepared.before.revision as i64),&(prepared.after().revision as i64),&checkpoint,&hash,&operation,&clock.business_day,&clock.database_time_ms]).map_err(source_db)?;
                *written.borrow_mut() = Some(versions);
                #[cfg(any(test, feature = "test-support"))]
                if test_fault == 1 || test_fault == 4 {
                    return Err(EconomyError::SourceRejected);
                }
                Ok(())
            };
            let result = if quote.view.cost == 0 {
                economy::record_zero_fee_source_authorized_with_source_with_timeouts(
                    client,
                    realm,
                    KIND,
                    &source_key(quote_id),
                    &quote.source_digest,
                    2000,
                    10000,
                    authorize,
                    source,
                )
                .map(|r| Idempotent {
                    receipt: HuntingRepairReceipt::Free(r.receipt),
                    replayed: r.replayed,
                })
            } else {
                economy::debit_bound_and_burn_authorized_with_source_with_timeouts(
                    client,
                    realm,
                    &burn(quote),
                    &quote.source_digest,
                    2000,
                    10000,
                    authorize,
                    source,
                )
                .map(|r| Idempotent {
                    receipt: HuntingRepairReceipt::Charged(r.receipt),
                    replayed: r.replayed,
                })
            };
            Ok(result)
        });
        #[cfg(any(test, feature = "test-support"))]
        let delivered = if test_fault == 5 && delivered.as_ref().is_ok_and(|r| r.is_ok()) {
            *locked.borrow_mut() = None;
            Err("injected repair worker delivery failure".into())
        } else {
            delivered
        };
        let outcome = match delivered {
            Ok(outcome) => outcome,
            Err(_) => Err(EconomyError::OutcomeUnknown),
        };
        #[cfg(any(test, feature = "test-support"))]
        let outcome = if (test_fault == 2 && outcome.is_ok()) || test_fault == 4 {
            Err(EconomyError::OutcomeUnknown)
        } else {
            outcome
        };
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = test_fault;
        let receipt = match outcome {
            Ok(receipt) => receipt,
            Err(EconomyError::OutcomeUnknown) => {
                let reason = self.fence_hunting_repair(
                    realm,
                    url,
                    quote,
                    prepared.plan.guilds.keys().cloned().collect(),
                    "repair COMMIT outcome unknown; query the original quote".into(),
                );
                return Err(HuntingRepairSourceError::OutcomeUnknown(reason));
            }
            Err(_) => {
                return Err(HuntingRepairSourceError::Rejected(
                    "repair source rejected without committing".into(),
                ))
            }
        };
        let guilds = prepared.plan.guilds.keys().cloned().collect::<Vec<_>>();
        if validate_receipt(realm, quote, &receipt.receipt).is_err() {
            let reason = self.fence_hunting_repair(
                realm,
                url,
                quote,
                guilds,
                "repair committed receipt failed validation".into(),
            );
            return Err(HuntingRepairSourceError::OutcomeUnknown(reason));
        }
        let Some(mut image) = locked.into_inner() else {
            let reason = self.fence_hunting_repair(
                realm,
                url,
                quote,
                guilds,
                "repair committed source image unavailable".into(),
            );
            return Err(HuntingRepairSourceError::OutcomeUnknown(reason));
        };
        let original = if receipt.replayed {
            original_result.into_inner()
        } else {
            Some(prepared.after().clone())
        };
        let Some(checkpoint) = original else {
            let reason = self.fence_hunting_repair(
                realm,
                url,
                quote,
                guilds,
                "repair receipt lacks original complete source outcome".into(),
            );
            return Err(HuntingRepairSourceError::OutcomeUnknown(reason));
        };
        let current_revision = if receipt.replayed {
            image
                .account
                .saves
                .get(&quote.owner.character_index)
                .map(|s| s.revision)
        } else {
            Some(checkpoint.revision)
        };
        let commit = HuntingRepairCommit {
            checkpoint,
            current_revision,
            receipt,
        };
        #[cfg(any(test, feature = "test-support"))]
        if test_fault == 3 {
            let detail = self.fence_hunting_repair(
                realm,
                url,
                quote,
                guilds,
                "injected repair publication failure after known COMMIT".into(),
            );
            return Err(HuntingRepairSourceError::KnownCommitted {
                commit: Box::new(commit),
                detail,
            });
        }
        let versions = if commit.receipt.replayed {
            image.versions
        } else {
            let Some(versions) = written.into_inner() else {
                let detail = self.fence_hunting_repair(
                    realm,
                    url,
                    quote,
                    guilds,
                    "repair committed source versions unavailable".into(),
                );
                return Err(HuntingRepairSourceError::KnownCommitted {
                    commit: Box::new(commit),
                    detail,
                });
            };
            image.account = prepared.staged.accounts[account].clone();
            image.guilds = prepared
                .plan
                .guilds
                .keys()
                .map(|id| (id.clone(), prepared.staged.shared_guilds.get(id).cloned()))
                .collect();
            versions
        };
        let write_state = self.account_store_write_state.lock().map_err(|_| {
            HuntingRepairSourceError::KnownCommitted {
                commit: Box::new(commit.clone()),
                detail: "repair write state unavailable after COMMIT".into(),
            }
        })?;
        if !matches!(&*write_state, AccountStoreWriteState::Writable) {
            return Err(HuntingRepairSourceError::KnownCommitted {
                commit: Box::new(commit),
                detail: "another publication fence prevents repair mirror publication".into(),
            });
        }
        live.accounts.insert(account.into(), image.account);
        for (id, guild) in image.guilds {
            if let Some(guild) = guild {
                live.shared_guilds.insert(id, guild);
            } else {
                live.shared_guilds.remove(&id);
            }
        }
        apply_account_store_mutation_source_versions(&mut live, &prepared.plan, versions);
        self.guild_clock_updates.fetch_add(1, Ordering::AcqRel);
        Ok(commit)
    }

    /// Trusted login/refresh preflight, not a player receipt/query endpoint.
    /// Only the private original fence supplies the historic owner and source.
    /// A lost Session, ban or a newer character incarnation must not strand this
    /// source's process-wide freeze after its original outcome is established.
    pub(crate) fn reconcile_hunting_repair_before_account_refresh(
        &self,
        account: &str,
    ) -> Result<(), String> {
        let fence = self
            .hunting_repair_fence
            .lock()
            .map_err(|_| "repair refresh recovery fence unavailable")?
            .clone();
        let Some(fence) = fence else {
            return Ok(());
        };
        if fence.owner.account_id != account
            || self.hunting_economy_realm.as_deref() != Some(fence.realm.as_str())
            || self
                .account_store_database_url
                .as_deref()
                .is_none_or(|url| database_binding(url) != fence.database_binding)
        {
            return Ok(());
        }
        {
            let state = self
                .account_store_write_state
                .lock()
                .map_err(|_| "repair refresh recovery write state unavailable")?;
            if self.account_store_freeze_epoch.load(Ordering::Acquire) != fence.epoch
                || !matches!(&*state,AccountStoreWriteState::Frozen{reason} if reason==&fence.reason)
            {
                return Ok(());
            }
        }
        // All probe guards above are released before entering the shared
        // persistence order. Never call public lookup/refresh while holding it.
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "repair refresh recovery persistence lock unavailable")?;
        let (realm, url) = self.hunting_source_binding()?;
        if realm != fence.realm || database_binding(url) != fence.database_binding {
            return Ok(());
        }
        let mut state = self
            .account_store_write_state
            .lock()
            .map_err(|_| "repair refresh recovery write state unavailable")?;
        if self.account_store_freeze_epoch.load(Ordering::Acquire) != fence.epoch
            || !matches!(&*state,AccountStoreWriteState::Frozen{reason} if reason==&fence.reason)
        {
            return Ok(());
        }
        let delivered = with_account_store_postgres_client(url, |client| {
            let mut tx = client
                .transaction()
                .map_err(|_| "repair refresh recovery transaction unavailable")?;
            timeouts(&mut tx).map_err(|_| "repair refresh recovery timeouts unavailable")?;
            economy::lock_realm_and_read_database_clock(&mut tx, realm)
                .map_err(|_| "repair refresh recovery realm unavailable")?;
            lock_guilds(&mut tx, &fence.guilds)
                .map_err(|_| "repair refresh recovery Guild unavailable")?;
            // The CURRENT canonical image may be banned, removed, or belong to
            // a new UUID. It is only refreshed; it never authorizes the old op.
            let current = lock_and_load(
                &mut tx,
                account,
                fence.owner.character_index,
                None,
                fence.guilds.iter().cloned(),
                true,
            )
            .map_err(|_| "repair refresh recovery current account unavailable")?;
            if let Some(image) = current.as_ref() {
                if !image.owner.incarnation.is_empty() {
                    verify_current(&mut tx, image)
                        .map_err(|_| "repair refresh recovery current checkpoint unavailable")?;
                }
            }
            let quote = read_quote(&mut tx, realm, &fence.quote_id, true)
                .map_err(|_| "repair refresh recovery original quote unavailable")?
                .ok_or("repair refresh recovery original quote missing")?;
            if quote.owner != fence.owner
                || quote.source_digest != fence.source_digest
                || quote.view.cost != fence.cost
            {
                return Err("repair refresh recovery original identity changed".into());
            }
            let original = read_result(&mut tx, realm, &quote)
                .map_err(|_| "repair refresh recovery original outcome unavailable")?;
            // This validates both immutable halves or proves both absent while
            // holding the realm lock. No source callback or asset write occurs.
            let _receipt = read_receipt(&mut tx, realm, &quote, original.is_some())
                .map_err(|_| "repair refresh recovery original receipt unavailable")?;
            let (current, mut versions, mut guild_images) = match current {
                Some(image) => (Some(image.account), image.versions, image.guilds),
                None => (None, AccountStoreSourceVersions::default(), BTreeMap::new()),
            };
            if current.is_none() {
                for id in &fence.guilds {
                    let row = tx
                        .query_opt(
                            "SELECT raw_json,store_version FROM shared_guilds WHERE guild_id=$1",
                            &[id],
                        )
                        .map_err(|_| "repair refresh recovery Guild checkpoint unavailable")?;
                    let image = if let Some(row) = row {
                        versions.guilds.insert(id.clone(), row.get("store_version"));
                        Some(
                            serde_json::from_value(row.get("raw_json"))
                                .map_err(|_| "repair refresh recovery Guild checkpoint invalid")?,
                        )
                    } else {
                        None
                    };
                    guild_images.insert(id.clone(), image);
                }
            }
            tx.commit()
                .map_err(|_| "repair refresh recovery confirmation unavailable")?;
            Ok((current, versions, guild_images))
        });
        let (current, versions, guild_images) = delivered
            .map_err(|_| "repair original refresh recovery remains unresolved".to_string())?;
        let mut live = self
            .account_store
            .lock()
            .map_err(|_| "repair refresh recovery mirror unavailable")?;
        let mut slot = self
            .hunting_repair_fence
            .lock()
            .map_err(|_| "repair refresh recovery fence unavailable")?;
        if self.account_store_freeze_epoch.load(Ordering::Acquire) != fence.epoch
            || !matches!(&*state,AccountStoreWriteState::Frozen{reason} if reason==&fence.reason)
            || slot.as_ref().is_none_or(|stored| {
                stored.epoch != fence.epoch
                    || stored.quote_id != fence.quote_id
                    || stored.owner != fence.owner
                    || stored.database_binding != fence.database_binding
            })
        {
            return Err("repair refresh recovery was superseded by another fence".into());
        }
        let mut staged = live.clone();
        staged.source_account_versions.remove(account);
        staged.source_save_versions.remove(account);
        if let Some(current) = current {
            staged.accounts.insert(account.into(), current);
        } else {
            staged.accounts.remove(account);
        }
        for (id, guild) in guild_images {
            if let Some(guild) = guild {
                staged.shared_guilds.insert(id, guild);
            } else {
                staged.shared_guilds.remove(&id);
                staged.source_guild_versions.remove(&id);
            }
        }
        staged.merge_source_versions(versions);
        staged.normalize_next_character_index();
        *live = staged;
        *state = AccountStoreWriteState::Writable;
        *slot = None;
        self.guild_clock_updates.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    /// Historical query authenticates the current owner before looking up the
    /// quote or receipt. NPC eligibility, quote TTL and current policy admission
    /// are deliberately fresh-submit checks, not permissions to erase history.
    pub(crate) fn lookup_hunting_repair_source(
        &self,
        account: &str,
        incarnation: &str,
        index: i32,
        quote_id: &str,
    ) -> Result<Option<HuntingRepairCommit>, HuntingRepairSourceError> {
        validate_actor(account, incarnation)?;
        if index < 0 || !valid_quote_id(quote_id) {
            return Err("invalid repair lookup identity".to_string().into());
        }
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "repair lookup persistence lock poisoned".to_string())?;
        let (realm, url) = self.hunting_source_binding()?;
        let fence = self
            .hunting_repair_fence
            .lock()
            .map_err(|_| "repair recovery fence poisoned".to_string())?
            .clone();
        let mut write_state = self
            .account_store_write_state
            .lock()
            .map_err(|_| "repair lookup write state poisoned".to_string())?;
        let owns_fence = match &*write_state {
            AccountStoreWriteState::Writable => false,
            AccountStoreWriteState::Frozen { reason } => {
                let Some(fence) = fence.as_ref() else {
                    return Err("repair lookup cannot release another publication fence"
                        .to_string()
                        .into());
                };
                if fence.realm != realm
                    || fence.database_binding != database_binding(url)
                    || fence.owner.account_id != account
                    || fence.owner.character_index != index
                    || fence.owner.incarnation != incarnation
                    || fence.quote_id != quote_id
                    || fence.reason != *reason
                    || fence.epoch != self.account_store_freeze_epoch.load(Ordering::Acquire)
                {
                    return Err("repair lookup cannot release another publication fence"
                        .to_string()
                        .into());
                }
                true
            }
        };
        let guilds = if owns_fence {
            fence.as_ref().unwrap().guilds.clone()
        } else {
            Vec::new()
        };
        // While the write-state guard is held, use only the PG pool, SQL and
        // pure decoding. Calling writable/freeze helpers here would deadlock.
        let delivered = with_account_store_postgres_client(url, |client| {
            let mut tx = client
                .transaction()
                .map_err(|_| "repair lookup transaction unavailable")?;
            timeouts(&mut tx).map_err(|_| "repair lookup timeouts unavailable")?;
            economy::lock_realm_and_read_database_clock(&mut tx, realm)
                .map_err(|_| "repair lookup realm unavailable")?;
            lock_guilds(&mut tx, &guilds).map_err(|_| "repair lookup Guild unavailable")?;
            let image = lock_and_load(&mut tx, account, index, None, guilds.iter().cloned(), false)
                .map_err(|_| "repair owner unavailable")?
                .ok_or("repair owner unavailable")?;
            if image.owner.incarnation != incarnation {
                return Err("repair owner incarnation changed".into());
            }
            verify_current(&mut tx, &image)
                .map_err(|_| "repair latest complete checkpoint unavailable")?;
            let quote = read_quote(&mut tx, realm, quote_id, true)
                .map_err(|_| "repair original quote unavailable")?;
            let (original, receipt) = if let Some(quote) = quote.as_ref() {
                if quote.owner != image.owner {
                    return Err("repair quote owner mismatch".into());
                }
                if owns_fence {
                    let fence = fence.as_ref().unwrap();
                    if fence.source_digest != quote.source_digest || fence.cost != quote.view.cost {
                        return Err("repair original quote binding changed".into());
                    }
                }
                let original = read_result(&mut tx, realm, quote)
                    .map_err(|_| "repair original complete outcome unavailable")?;
                let receipt = read_receipt(&mut tx, realm, quote, original.is_some())
                    .map_err(|_| "repair original economic receipt unavailable")?;
                (original, receipt)
            } else {
                if owns_fence {
                    return Err("repair uncertain original quote is missing".into());
                }
                (None, None)
            };
            tx.commit()
                .map_err(|_| "repair query confirmation unavailable")?;
            Ok((image, quote, original, receipt))
        });
        let (image, quote, original, receipt) = match delivered {
            Ok(result) => result,
            Err(_) => {
                return Err(if owns_fence {
                    HuntingRepairSourceError::OutcomeUnknown(
                        "repair original query remains unresolved".into(),
                    )
                } else {
                    HuntingRepairSourceError::Rejected("repair query repository unavailable".into())
                })
            }
        };
        let commit = match (original, receipt) {
            (Some(checkpoint), Some(receipt)) => Some(HuntingRepairCommit {
                checkpoint,
                current_revision: image.account.saves.get(&index).map(|s| s.revision),
                receipt: Idempotent {
                    receipt,
                    replayed: true,
                },
            }),
            (None, None) => None,
            _ => {
                return Err(HuntingRepairSourceError::OutcomeUnknown(
                    "repair source outcome and economic receipt disagree".into(),
                ))
            }
        };
        let mut live = match self.account_store.lock() {
            Ok(live) => live,
            Err(_) => {
                drop(write_state);
                let detail = if let Some(quote) = quote.as_ref() {
                    self.fence_hunting_repair(
                        realm,
                        url,
                        quote,
                        guilds,
                        "repair query mirror publication unavailable".into(),
                    )
                } else {
                    "repair query mirror publication unavailable".into()
                };
                return Err(match commit {
                    Some(commit) => HuntingRepairSourceError::KnownCommitted {
                        commit: Box::new(commit),
                        detail,
                    },
                    None => HuntingRepairSourceError::OutcomeUnknown(detail),
                });
            }
        };
        let publication_error = |detail: &str| match &commit {
            Some(commit) => HuntingRepairSourceError::KnownCommitted {
                commit: Box::new(commit.clone()),
                detail: detail.into(),
            },
            None => HuntingRepairSourceError::OutcomeUnknown(detail.into()),
        };
        let mut staged = live.clone();
        staged.source_account_versions.remove(account);
        staged.source_save_versions.remove(account);
        staged.accounts.insert(account.into(), image.account);
        for (id, guild) in image.guilds {
            if let Some(guild) = guild {
                staged.shared_guilds.insert(id, guild);
            } else {
                staged.shared_guilds.remove(&id);
                staged.source_guild_versions.remove(&id);
            }
        }
        staged.merge_source_versions(image.versions);
        if owns_fence {
            let fence = fence.as_ref().unwrap();
            let mut slot = self
                .hunting_repair_fence
                .lock()
                .map_err(|_| publication_error("repair query recovery fence unavailable"))?;
            if self.account_store_freeze_epoch.load(Ordering::Acquire) != fence.epoch
                || !matches!(&*write_state,AccountStoreWriteState::Frozen{reason} if reason==&fence.reason)
            {
                return Err(publication_error(
                    "repair query was superseded by another publication fence",
                ));
            }
            *live = staged;
            *write_state = AccountStoreWriteState::Writable;
            *slot = None;
        } else {
            *live = staged;
        }
        self.guild_clock_updates.fetch_add(1, Ordering::AcqRel);
        Ok(commit)
    }
}

#[cfg(test)]
#[path = "config_hunting_repair_tests.rs"]
mod tests;
