use crate::{add, mul, Catalog, ProductionError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputLot {
    pub uid: u64,
    pub material: String,
    pub template: i32,
    pub quantity: u64,
    pub bound: bool,
    pub eligible: bool,
    pub custody: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Debit {
    pub uid: u64,
    pub material: String,
    pub template: i32,
    pub quantity: u64,
    pub bound: bool,
    pub custody: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputPlan {
    pub debits: Vec<Debit>,
    pub gold: u64,
    pub output_quantity: u64,
    pub bound_output: bool,
}
pub fn plan_inputs(
    c: &Catalog,
    recipe: &str,
    batch: u16,
    gold: u64,
    lots: &[InputLot],
    bindings: &BTreeMap<String, i32>,
) -> Result<InputPlan, ProductionError> {
    if batch == 0 || batch > 10 {
        return Err(ProductionError::InvalidBatch);
    }
    c.validate()?;
    if lots.len() > 512 {
        return Err(ProductionError::InvalidInventory);
    }
    let mut ids = BTreeSet::new();
    if lots.iter().any(|lot| {
        lot.uid == 0
            || lot.quantity == 0
            || lot.custody.is_empty()
            || lot.custody.len() > 16_384
            || !ids.insert(lot.uid)
    }) {
        return Err(ProductionError::InvalidInventory);
    }
    let r = c
        .recipes
        .get(recipe)
        .ok_or_else(|| ProductionError::Unavailable("recipe".into()))?;
    let cost = mul(r.gold, batch.into())?;
    if gold < cost {
        return Err(ProductionError::InsufficientGold);
    }
    let mut debits = Vec::new();
    for (material, count) in &r.inputs {
        let template = bindings
            .get(material)
            .ok_or_else(|| ProductionError::Unavailable(material.clone()))?;
        let mut needed = mul(*count, batch.into())?;
        for lot in lots
            .iter()
            .filter(|lot| lot.eligible && lot.material == *material && lot.template == *template)
        {
            let n = needed.min(lot.quantity);
            if n == 0 {
                break;
            }
            debits.push(Debit {
                uid: lot.uid,
                material: material.clone(),
                template: *template,
                quantity: n,
                bound: lot.bound,
                custody: lot.custody.clone(),
            });
            needed -= n;
        }
        if needed != 0 {
            return Err(ProductionError::MissingMaterial(material.clone()));
        }
    }
    Ok(InputPlan {
        bound_output: debits.iter().any(|d| d.bound),
        debits,
        gold: cost,
        output_quantity: mul(r.amount, batch.into())?,
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessingStep {
    pub recipe: String,
    pub batches: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessingPlan {
    pub steps: Vec<ProcessingStep>,
    pub missing_raw: BTreeMap<String, u64>,
    pub surplus: BTreeMap<String, u64>,
    pub gold: u64,
    pub minutes: u64,
}
impl Catalog {
    /// Read-only estimate, never performs upstream crafting or consumes inventory.
    pub fn processing_plan(
        &self,
        id: &str,
        batch: u16,
        owned: &BTreeMap<String, u64>,
    ) -> Result<ProcessingPlan, ProductionError> {
        if batch == 0 || batch > 10 {
            return Err(ProductionError::InvalidBatch);
        }
        self.validate()?;
        let mut stock = owned.clone();
        let mut result = ProcessingPlan {
            steps: Vec::new(),
            missing_raw: BTreeMap::new(),
            surplus: BTreeMap::new(),
            gold: 0,
            minutes: 0,
        };
        fn consume(
            c: &Catalog,
            key: &str,
            count: u64,
            stock: &mut BTreeMap<String, u64>,
            p: &mut ProcessingPlan,
            budget: &mut u32,
        ) -> Result<(), ProductionError> {
            *budget = budget
                .checked_sub(1)
                .ok_or_else(|| ProductionError::InvalidCatalog("plan complexity".into()))?;
            let available = stock.entry(key.into()).or_default();
            let used = (*available).min(count);
            *available -= used;
            let missing = count - used;
            if missing == 0 {
                return Ok(());
            }
            if let Some((id, r)) = c.producer(key) {
                let batches = missing / r.amount + u64::from(missing % r.amount != 0);
                for (input, n) in &r.inputs {
                    consume(c, input, mul(*n, batches)?, stock, p, budget)?;
                }
                *stock.entry(key.into()).or_default() = add(
                    *stock.get(key).unwrap_or(&0),
                    mul(r.amount, batches)? - missing,
                )?;
                p.gold = add(p.gold, mul(r.gold, batches)?)?;
                p.minutes = add(p.minutes, mul(r.minutes, batches)?)?;
                p.steps.push(ProcessingStep {
                    recipe: id.into(),
                    batches,
                });
                p.surplus.insert(key.into(), 0);
            } else {
                p.missing_raw.insert(
                    key.into(),
                    add(*p.missing_raw.get(key).unwrap_or(&0), missing)?,
                );
            }
            Ok(())
        }
        let r = self
            .recipes
            .get(id)
            .ok_or_else(|| ProductionError::Unavailable("recipe".into()))?;
        let mut budget = 8192;
        for (key, n) in &r.inputs {
            consume(
                self,
                key,
                mul(*n, batch.into())?,
                &mut stock,
                &mut result,
                &mut budget,
            )?;
        }
        result.gold = add(result.gold, mul(r.gold, batch.into())?)?;
        result.minutes = add(result.minutes, mul(r.minutes, batch.into())?)?;
        result.steps.push(ProcessingStep {
            recipe: id.into(),
            batches: batch.into(),
        });
        result.surplus.retain(|key, n| {
            *n = *stock.get(key).unwrap_or(&0);
            *n > 0
        });
        Ok(result)
    }
}
