use crate::{
    add, mul, plan_inputs, Catalog, Debit, InputLot, ProductionAvailability, ProductionError,
    Recipe,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionOwner {
    pub account_id: String,
    pub character_id: String,
}
/// Server-authenticated access and inventory snapshot. Never deserialize from a client command.
/// access_allowed MUST authorize this exact action (start/cancel/collect) at this facility.
pub struct ProductionContext<'a> {
    pub owner: &'a ProductionOwner,
    pub facility_id: &'a str,
    pub facility_kind: &'a str,
    pub access_allowed: bool,
    pub now_ms: u64,
    pub inventory_revision: u64,
    pub gold: u64,
    pub lots: &'a [InputLot],
    pub availability: &'a ProductionAvailability,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProductionAction {
    Start {
        recipe: String,
        catalog_version: String,
        catalog_revision: String,
        batch: u16,
    },
    Collect {
        job: String,
    },
    Cancel {
        job: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionCommand {
    pub request_id: String,
    pub facility_id: String,
    pub action: ProductionAction,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Running,
    Collected,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionJob {
    pub id: String,
    pub owner: ProductionOwner,
    pub facility_id: String,
    pub catalog_version: String,
    pub catalog_revision: String,
    pub recipe_id: String,
    pub recipe_digest: String,
    pub recipe: Recipe,
    pub batch: u16,
    pub started_ms: u64,
    pub finishes_ms: u64,
    pub debits: Vec<Debit>,
    pub paid_gold: u64,
    pub output_template: i32,
    pub output_bound: bool,
    pub status: JobStatus,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionReceipt {
    pub request_id: String,
    pub job_id: String,
    pub status: JobStatus,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredReceipt {
    owner: ProductionOwner,
    command: ProductionCommand,
    receipt: ProductionReceipt,
}
#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct ProductionLedger {
    revision: u64,
    jobs: BTreeMap<String, ProductionJob>,
    receipts: Vec<StoredReceipt>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LedgerCheckpoint {
    revision: u64,
    jobs: BTreeMap<String, ProductionJob>,
    receipts: Vec<StoredReceipt>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialDelivery {
    pub material: String,
    pub template: i32,
    pub quantity: u64,
    pub bound: bool,
    pub refund_origin: Option<Debit>,
}
/// Commit this change AND next_ledger atomically through the real economic authority.
/// Delivery UID allocation, exact source metadata, capacity and version checks belong
/// to that transaction; do not apply a prepared ledger independently of inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedProduction {
    pub expected_ledger_revision: u64,
    pub expected_inventory_revision: u64,
    pub next_ledger: ProductionLedger,
    pub debits: Vec<Debit>,
    pub gold_debit: u64,
    pub deliveries: Vec<MaterialDelivery>,
    pub receipt: ProductionReceipt,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionPreparation {
    Replay(ProductionReceipt),
    Change(Box<PreparedProduction>),
}
fn valid_owner(owner: &ProductionOwner) -> bool {
    [&owner.account_id, &owner.character_id]
        .into_iter()
        .all(|id| !id.is_empty() && id.len() <= 256)
}
fn valid_request(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn job_id(owner: &ProductionOwner, request: &str) -> String {
    format!(
        "{}:{}{}:{}{}:{}",
        owner.account_id.len(),
        owner.account_id,
        owner.character_id.len(),
        owner.character_id,
        request.len(),
        request
    )
}
impl ProductionLedger {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn job(&self, id: &str, owner: &ProductionOwner) -> Option<&ProductionJob> {
        self.jobs.get(id).filter(|j| &j.owner == owner)
    }
    /// Private server checkpoint inspection. Never project other owners' jobs.
    pub fn jobs_for<'a>(
        &'a self,
        owner: &'a ProductionOwner,
    ) -> impl Iterator<Item = &'a ProductionJob> {
        self.jobs.values().filter(move |job| &job.owner == owner)
    }
    pub fn belongs_to(&self, owner: &ProductionOwner) -> bool {
        self.jobs.values().all(|job| &job.owner == owner)
            && self.receipts.iter().all(|row| &row.owner == owner)
    }
    /// Recovery precedes range, current catalog, capacity, fees and UID issuance.
    pub fn query(
        &self,
        owner: &ProductionOwner,
        command: &ProductionCommand,
    ) -> Result<Option<ProductionReceipt>, ProductionError> {
        if !valid_owner(owner) || !valid_request(&command.request_id) {
            return Err(ProductionError::InvalidRequest);
        }
        match self
            .receipts
            .iter()
            .find(|row| &row.owner == owner && row.command.request_id == command.request_id)
        {
            Some(row) if row.command == *command => Ok(Some(row.receipt.clone())),
            Some(_) => Err(ProductionError::RequestConflict),
            None => Ok(None),
        }
    }
    /// Ordinary saves can carry an older prefix, but cannot invent or rewrite
    /// committed history. Restore has already checked each receipt transition.
    pub fn is_history_prefix_of(&self, newer: &Self) -> bool {
        newer.receipts.starts_with(&self.receipts)
            && self.jobs.iter().all(|(id, job)| {
                newer.jobs.get(id).is_some_and(|new_job| {
                    let mut frozen = new_job.clone();
                    frozen.status = job.status.clone();
                    frozen == *job
                })
            })
    }
    pub fn checkpoint_json(&self) -> Result<String, ProductionError> {
        // Bound the encoded bytes, including JSON escapes, before returning a save proposal.
        struct Writer {
            bytes: Vec<u8>,
            over_budget: bool,
        }
        impl std::io::Write for Writer {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if bytes.len() > 16 * 1024 * 1024 - self.bytes.len() {
                    self.over_budget = true;
                    return Err(std::io::Error::other("production checkpoint budget"));
                }
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = Writer {
            bytes: Vec::new(),
            over_budget: false,
        };
        if serde_json::to_writer(&mut writer, self).is_err() {
            return Err(if writer.over_budget {
                ProductionError::LedgerFull
            } else {
                ProductionError::InvalidCheckpoint
            });
        }
        String::from_utf8(writer.bytes).map_err(|_| ProductionError::InvalidCheckpoint)
    }
    fn reserve_terminal_receipts(&self) -> Result<(), ProductionError> {
        let mut reserved = 0_u64;
        let mut active_count = 0_usize;
        for job in self
            .jobs
            .values()
            .filter(|job| job.status == JobStatus::Running)
        {
            active_count += 1;
            // Request IDs are at most 128 plain ASCII bytes. Collect is the larger
            // terminal command; both terminal status strings have the same size.
            let request = "Z".repeat(128);
            let row = StoredReceipt {
                owner: job.owner.clone(),
                command: ProductionCommand {
                    request_id: request.clone(),
                    facility_id: job.facility_id.clone(),
                    action: ProductionAction::Collect {
                        job: job.id.clone(),
                    },
                },
                receipt: ProductionReceipt {
                    request_id: request,
                    job_id: job.id.clone(),
                    status: JobStatus::Collected,
                },
            };
            let encoded =
                serde_json::to_vec(&row).map_err(|_| ProductionError::InvalidCheckpoint)?;
            // Array comma, Running -> terminal status growth, and revision growth.
            reserved = add(reserved, add(encoded.len() as u64, 32)?)?;
        }
        if self.receipts.len() + active_count > 4096
            || add(self.checkpoint_json()?.len() as u64, reserved)? > 16 * 1024 * 1024
        {
            return Err(ProductionError::LedgerFull);
        }
        Ok(())
    }
    pub fn restore(json: &str) -> Result<Self, ProductionError> {
        if json.len() > 16 * 1024 * 1024 {
            return Err(ProductionError::InvalidCheckpoint);
        }
        let raw: LedgerCheckpoint =
            serde_json::from_str(json).map_err(|_| ProductionError::InvalidCheckpoint)?;
        let state = Self {
            revision: raw.revision,
            jobs: raw.jobs,
            receipts: raw.receipts,
        };
        if state.jobs.len() > 1024 || state.receipts.len() > 4096 {
            return Err(ProductionError::InvalidCheckpoint);
        }
        if state.revision != state.receipts.len() as u64 {
            return Err(ProductionError::InvalidCheckpoint);
        }
        let mut requests = std::collections::BTreeSet::new();
        let mut transitions = BTreeMap::new();
        let mut active = std::collections::BTreeSet::new();
        for row in &state.receipts {
            let key = (
                row.owner.account_id.clone(),
                row.owner.character_id.clone(),
                row.command.request_id.clone(),
            );
            if !valid_request(&row.command.request_id)
                || !requests.insert(key)
                || !state.jobs.contains_key(&row.receipt.job_id)
                || row.receipt.request_id != row.command.request_id
            {
                return Err(ProductionError::InvalidCheckpoint);
            }
            if state.jobs[&row.receipt.job_id].owner != row.owner {
                return Err(ProductionError::InvalidCheckpoint);
            }
            let job = &state.jobs[&row.receipt.job_id];
            if row.command.facility_id != job.facility_id {
                return Err(ProductionError::InvalidCheckpoint);
            }
            let valid = match &row.command.action {
                ProductionAction::Start {
                    batch,
                    catalog_version,
                    catalog_revision,
                    recipe,
                } => {
                    *batch == job.batch
                        && catalog_version == &job.catalog_version
                        && catalog_revision == &job.catalog_revision
                        && recipe == &job.recipe_id
                        && job_id(&row.owner, &row.command.request_id) == job.id
                        && row.receipt.status == JobStatus::Running
                        && !transitions.contains_key(&job.id)
                }
                ProductionAction::Collect { job: id } => {
                    id == &job.id
                        && transitions.get(&job.id) == Some(&JobStatus::Running)
                        && row.receipt.status == JobStatus::Collected
                }
                ProductionAction::Cancel { job: id } => {
                    id == &job.id
                        && transitions.get(&job.id) == Some(&JobStatus::Running)
                        && row.receipt.status == JobStatus::Cancelled
                }
            };
            if !valid {
                return Err(ProductionError::InvalidCheckpoint);
            }
            transitions.insert(job.id.clone(), row.receipt.status.clone());
        }
        for (id, j) in &state.jobs {
            if id != &j.id
                || j.batch == 0
                || j.batch > 10
                || j.recipe.amount == 0
                || j.recipe.minutes == 0
                || j.recipe.inputs.is_empty()
                || j.recipe.inputs.len() > 16
                || j.recipe.inputs.values().any(|n| *n == 0)
                || j.debits.len() > 512
                || j.output_template < 0
                || !valid_owner(&j.owner)
                || j.facility_id.is_empty()
                || j.facility_id.len() > 256
                || !valid_request(&j.recipe_id)
                || j.recipe_id.len() > 64
                || j.recipe.output.is_empty()
                || j.recipe.facility.is_empty()
                || j.catalog_version != "mir2.production-design.v0.2"
                || !crate::valid_digest(&j.catalog_revision)
                || crate::digest(&j.recipe)? != j.recipe_digest
            {
                return Err(ProductionError::InvalidCheckpoint);
            }
            if j.status == JobStatus::Running
                && !active.insert((
                    j.owner.account_id.clone(),
                    j.owner.character_id.clone(),
                    j.facility_id.clone(),
                ))
            {
                return Err(ProductionError::InvalidCheckpoint);
            }
            if add(
                j.started_ms,
                mul(j.recipe.minutes, mul(j.batch.into(), 60_000)?)?,
            )? != j.finishes_ms
                || mul(j.recipe.gold, j.batch.into())? != j.paid_gold
            {
                return Err(ProductionError::InvalidCheckpoint);
            }
            mul(j.recipe.amount, j.batch.into())?;
            let mut sums = BTreeMap::new();
            let mut ids = std::collections::BTreeSet::new();
            for d in &j.debits {
                if d.uid == 0
                    || d.quantity == 0
                    || d.template < 0
                    || d.custody.is_empty()
                    || d.custody.len() > 16_384
                    || !ids.insert(d.uid)
                {
                    return Err(ProductionError::InvalidCheckpoint);
                }
                let n = add(*sums.get(&d.material).unwrap_or(&0), d.quantity)?;
                sums.insert(d.material.clone(), n);
            }
            let wanted = j
                .recipe
                .inputs
                .iter()
                .map(|(key, n)| Ok((key.clone(), mul(*n, j.batch.into())?)))
                .collect::<Result<BTreeMap<_, _>, ProductionError>>()?;
            if sums != wanted
                || j.output_bound != j.debits.iter().any(|d| d.bound)
                || transitions.get(id) != Some(&j.status)
            {
                return Err(ProductionError::InvalidCheckpoint);
            }
        }
        state.reserve_terminal_receipts()?;
        Ok(state)
    }
    pub fn prepare(
        &self,
        c: &Catalog,
        cmd: &ProductionCommand,
        ctx: &ProductionContext<'_>,
    ) -> Result<ProductionPreparation, ProductionError> {
        if !valid_owner(ctx.owner) || !ctx.access_allowed || cmd.facility_id != ctx.facility_id {
            return Err(ProductionError::Unauthorized);
        }
        if !valid_request(&cmd.request_id)
            || ctx.facility_id.is_empty()
            || ctx.facility_id.len() > 256
        {
            return Err(ProductionError::InvalidRequest);
        }
        if let Some(old) = self
            .receipts
            .iter()
            .find(|r| r.owner == *ctx.owner && r.command.request_id == cmd.request_id)
        {
            return if old.command == *cmd {
                Ok(ProductionPreparation::Replay(old.receipt.clone()))
            } else {
                Err(ProductionError::RequestConflict)
            };
        }
        if self.receipts.len() >= 4096 {
            return Err(ProductionError::LedgerFull);
        }
        let mut next = self.clone();
        let mut debits = Vec::new();
        let mut gold_debit = 0;
        let mut deliveries = Vec::new();
        let (job_id, status) = match &cmd.action {
            ProductionAction::Start {
                recipe,
                catalog_version,
                catalog_revision,
                batch,
            } => {
                c.validate()?;
                if catalog_version != &c.schema {
                    return Err(ProductionError::Unavailable("catalog version".into()));
                }
                if catalog_revision != &c.revision()? {
                    return Err(ProductionError::Unavailable("catalog revision".into()));
                }
                c.require_available(recipe, ctx.availability)?;
                let r = &c.recipes[recipe];
                if r.facility != ctx.facility_kind {
                    return Err(ProductionError::Unavailable("facility kind".into()));
                }
                if self.jobs.values().any(|j| {
                    j.owner == *ctx.owner
                        && j.facility_id == ctx.facility_id
                        && j.status == JobStatus::Running
                }) {
                    return Err(ProductionError::Busy);
                }
                if self.jobs.len() >= 1024 {
                    return Err(ProductionError::LedgerFull);
                }
                let inputs = plan_inputs(
                    c,
                    recipe,
                    *batch,
                    ctx.gold,
                    ctx.lots,
                    &ctx.availability.material_templates,
                )?;
                let id = job_id(ctx.owner, &cmd.request_id);
                if self.jobs.contains_key(&id) {
                    return Err(ProductionError::RequestConflict);
                }
                let finishes_ms = add(ctx.now_ms, mul(r.minutes, mul((*batch).into(), 60_000)?)?)?;
                next.jobs.insert(
                    id.clone(),
                    ProductionJob {
                        id: id.clone(),
                        owner: ctx.owner.clone(),
                        facility_id: ctx.facility_id.into(),
                        catalog_version: catalog_version.clone(),
                        catalog_revision: catalog_revision.clone(),
                        recipe_id: recipe.clone(),
                        recipe_digest: crate::digest(r)?,
                        recipe: r.clone(),
                        batch: *batch,
                        started_ms: ctx.now_ms,
                        finishes_ms,
                        debits: inputs.debits.clone(),
                        paid_gold: inputs.gold,
                        output_template: ctx.availability.material_templates[&r.output],
                        output_bound: inputs.bound_output,
                        status: JobStatus::Running,
                    },
                );
                debits = inputs.debits;
                gold_debit = inputs.gold;
                (id, JobStatus::Running)
            }
            ProductionAction::Collect { job } | ProductionAction::Cancel { job } => {
                let j = next.jobs.get_mut(job).ok_or(ProductionError::UnknownJob)?;
                if j.owner != *ctx.owner
                    || j.facility_id != ctx.facility_id
                    || j.recipe.facility != ctx.facility_kind
                {
                    return Err(ProductionError::Unauthorized);
                }
                if j.status != JobStatus::Running {
                    return Err(ProductionError::AlreadyFinished);
                }
                if ctx.now_ms < j.started_ms {
                    return Err(ProductionError::TooEarly);
                }
                if matches!(cmd.action, ProductionAction::Collect { .. }) {
                    if ctx.now_ms < j.finishes_ms {
                        return Err(ProductionError::TooEarly);
                    }
                    deliveries.push(MaterialDelivery {
                        material: j.recipe.output.clone(),
                        template: j.output_template,
                        quantity: mul(j.recipe.amount, j.batch.into())?,
                        bound: j.output_bound,
                        refund_origin: None,
                    });
                    j.status = JobStatus::Collected;
                } else {
                    if ctx.now_ms >= j.finishes_ms {
                        return Err(ProductionError::AlreadyFinished);
                    }
                    let mut refunds = BTreeMap::new();
                    for (key, n) in &j.recipe.inputs {
                        refunds.insert(key.clone(), mul(*n, j.batch.into())? / 2);
                    }
                    let mut lots = j.debits.clone();
                    lots.sort_by_key(|d| !d.bound);
                    for d in lots {
                        let left = refunds
                            .get_mut(&d.material)
                            .ok_or(ProductionError::InvalidCheckpoint)?;
                        let n = (*left).min(d.quantity);
                        *left -= n;
                        if n > 0 {
                            deliveries.push(MaterialDelivery {
                                material: d.material.clone(),
                                template: d.template,
                                quantity: n,
                                bound: d.bound,
                                refund_origin: Some(d),
                            });
                        }
                    }
                    j.status = JobStatus::Cancelled;
                }
                (job.clone(), j.status.clone())
            }
        };
        next.revision = add(self.revision, 1)?;
        let receipt = ProductionReceipt {
            request_id: cmd.request_id.clone(),
            job_id,
            status,
        };
        next.receipts.push(StoredReceipt {
            owner: ctx.owner.clone(),
            command: cmd.clone(),
            receipt: receipt.clone(),
        });
        next.reserve_terminal_receipts()?;
        Ok(ProductionPreparation::Change(Box::new(
            PreparedProduction {
                expected_ledger_revision: self.revision,
                expected_inventory_revision: ctx.inventory_revision,
                next_ledger: next,
                debits,
                gold_debit,
                deliveries,
                receipt,
            },
        )))
    }
}
