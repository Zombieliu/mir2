use crate::ProductionError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    pub name: String,
    pub stage: String,
    pub family: Option<String>,
    pub source_gate: Option<String>,
    pub runtime_template_id: Option<i32>,
}
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub name: String,
    pub facility: String,
    pub inputs: BTreeMap<String, u64>,
    pub gold: u64,
    pub minutes: u64,
    pub output: String,
    pub amount: u64,
    pub unlock_gates: Vec<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FacilityDefinition {
    pub name: String,
    pub bootstrap: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub schema: String,
    pub production_enabled: bool,
    pub source_gates: BTreeMap<String, String>,
    pub items: BTreeMap<String, Material>,
    pub facilities: BTreeMap<String, FacilityDefinition>,
    pub recipes: BTreeMap<String, Recipe>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeSources {
    pub raw: BTreeSet<String>,
    pub gates: BTreeSet<String>,
    pub families: BTreeSet<String>,
    pub facilities: BTreeSet<String>,
}
/// Supplied only by server configuration, never taken from a player command.
#[derive(Debug, Clone, Default)]
pub struct ProductionAvailability {
    pub enabled: bool,
    pub accepted_gates: BTreeSet<String>,
    pub accepted_facilities: BTreeSet<String>,
    pub material_templates: BTreeMap<String, i32>,
}
impl Catalog {
    pub fn bundled() -> Result<Self, ProductionError> {
        Self::from_json(include_str!(
            "../../../docs/SLG-PRODUCTION-CATALOG.v0.2.json"
        ))
    }
    pub fn from_json(json: &str) -> Result<Self, ProductionError> {
        if json.len() > 1_048_576 {
            return Err(ProductionError::InvalidCatalog("size".into()));
        }
        let c: Self = serde_json::from_str(json)
            .map_err(|_| ProductionError::InvalidCatalog("decode".into()))?;
        c.validate()?;
        Ok(c)
    }
    pub fn revision(&self) -> Result<String, ProductionError> {
        self.validate()?;
        crate::digest(self)
    }
    pub fn validate(&self) -> Result<(), ProductionError> {
        let invalid = |s: &str| ProductionError::InvalidCatalog(s.into());
        if self.schema != "mir2.production-design.v0.2"
            || self.items.is_empty()
            || self.items.len() > 256
            || self.recipes.is_empty()
            || self.recipes.len() > 128
        {
            return Err(invalid("version or size"));
        }
        if self.facilities.is_empty() || self.facilities.len() > 32 || self.source_gates.len() > 32
        {
            return Err(invalid("facilities or gates"));
        }
        let valid_id = |id: &str| {
            !id.is_empty()
                && id.len() <= 64
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        };
        if self
            .facilities
            .keys()
            .chain(self.source_gates.keys())
            .any(|id| !valid_id(id))
        {
            return Err(invalid("identity"));
        }
        let mut outputs = BTreeSet::new();
        for (id, r) in &self.recipes {
            if !valid_id(id) || r.name.is_empty() || r.unlock_gates.len() > 32 {
                return Err(invalid("recipe identity"));
            }
            if r.inputs.is_empty()
                || r.inputs.len() > 16
                || r.inputs.values().any(|n| *n == 0)
                || r.amount == 0
                || r.minutes == 0
                || !self.facilities.contains_key(&r.facility)
                || !self.items.contains_key(&r.output)
                || !outputs.insert(r.output.clone())
            {
                return Err(invalid("recipe"));
            }
            if r.inputs.keys().any(|key| !self.items.contains_key(key))
                || r.unlock_gates
                    .iter()
                    .any(|g| !self.source_gates.contains_key(g))
            {
                return Err(invalid("unknown input or gate"));
            }
            crate::mul(r.gold, 10)?;
            crate::mul(r.minutes, 600_000)?;
            crate::mul(r.amount, 10)?;
            for n in r.inputs.values() {
                crate::mul(*n, 10)?;
            }
        }
        let mut memo = BTreeMap::new();
        for (id, item) in &self.items {
            if !valid_id(id) || item.name.is_empty() {
                return Err(invalid("material identity"));
            }
            if item.stage == "raw" {
                if !item
                    .source_gate
                    .as_ref()
                    .is_some_and(|g| self.source_gates.contains_key(g))
                    || item.family.as_ref().is_none_or(|f| f.is_empty())
                    || outputs.contains(id)
                {
                    return Err(invalid("raw source"));
                }
            } else if !["processed", "component", "final"].contains(&item.stage.as_str())
                || !outputs.contains(id)
            {
                return Err(invalid("missing producer"));
            }
            self.sources_for_material(id, &mut BTreeSet::new(), &mut memo)?;
        }
        Ok(())
    }
    pub fn producer(&self, material: &str) -> Option<(&str, &Recipe)> {
        self.recipes
            .iter()
            .find(|(_, r)| r.output == material)
            .map(|(id, r)| (id.as_str(), r))
    }
    fn sources_for_material(
        &self,
        id: &str,
        ancestors: &mut BTreeSet<String>,
        memo: &mut BTreeMap<String, RecipeSources>,
    ) -> Result<RecipeSources, ProductionError> {
        if ancestors.len() >= 32 || !ancestors.insert(id.into()) {
            return Err(ProductionError::InvalidCatalog("cycle or depth".into()));
        }
        if let Some(result) = memo.get(id) {
            ancestors.remove(id);
            return Ok(result.clone());
        }
        let item = self
            .items
            .get(id)
            .ok_or_else(|| ProductionError::InvalidCatalog("material".into()))?;
        let mut result = RecipeSources {
            raw: BTreeSet::new(),
            gates: BTreeSet::new(),
            families: BTreeSet::new(),
            facilities: BTreeSet::new(),
        };
        if item.stage == "raw" {
            result.raw.insert(id.into());
            result.gates.insert(
                item.source_gate
                    .clone()
                    .ok_or_else(|| ProductionError::InvalidCatalog("source".into()))?,
            );
            result.families.insert(
                item.family
                    .clone()
                    .ok_or_else(|| ProductionError::InvalidCatalog("family".into()))?,
            );
        } else {
            let (_, r) = self
                .producer(id)
                .ok_or_else(|| ProductionError::InvalidCatalog("producer".into()))?;
            result.facilities.insert(r.facility.clone());
            result.gates.extend(r.unlock_gates.iter().cloned());
            for key in r.inputs.keys() {
                let child = self.sources_for_material(key, ancestors, memo)?;
                result.raw.extend(child.raw);
                result.gates.extend(child.gates);
                result.families.extend(child.families);
                result.facilities.extend(child.facilities);
            }
        }
        ancestors.remove(id);
        memo.insert(id.into(), result.clone());
        Ok(result)
    }
    pub fn sources(&self, recipe: &str) -> Result<RecipeSources, ProductionError> {
        let r = self
            .recipes
            .get(recipe)
            .ok_or_else(|| ProductionError::Unavailable("recipe".into()))?;
        self.sources_for_material(&r.output, &mut BTreeSet::new(), &mut BTreeMap::new())
    }
    pub fn require_available(
        &self,
        recipe: &str,
        authority: &ProductionAvailability,
    ) -> Result<(), ProductionError> {
        self.validate()?;
        if !self.production_enabled || !authority.enabled {
            return Err(ProductionError::Unavailable("production disabled".into()));
        }
        let mut templates = BTreeSet::new();
        if authority
            .material_templates
            .iter()
            .any(|(id, n)| !self.items.contains_key(id) || *n < 0 || !templates.insert(*n))
        {
            return Err(ProductionError::Unavailable(
                "invalid material bindings".into(),
            ));
        }
        let sources = self.sources(recipe)?;
        for gate in sources.gates {
            if !authority.accepted_gates.contains(&gate) {
                return Err(ProductionError::Unavailable(gate));
            }
        }
        for kind in sources.facilities {
            if !authority.accepted_facilities.contains(&kind) {
                return Err(ProductionError::Unavailable(format!("facility:{kind}")));
            }
        }
        let mut pending = vec![self.recipes[recipe].output.clone()];
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id.clone()) {
                continue;
            }
            if !authority
                .material_templates
                .get(&id)
                .is_some_and(|n| *n >= 0)
            {
                return Err(ProductionError::Unavailable(format!("unbound:{id}")));
            }
            if let Some((_, r)) = self.producer(&id) {
                pending.extend(r.inputs.keys().cloned());
            }
        }
        Ok(())
    }
}
