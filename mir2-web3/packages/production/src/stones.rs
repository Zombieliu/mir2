//! Pure sealed-stone rules. Mint rolls and possession proofs come only from the
//! authenticated server. This module does not create UserItems or persist money.
//! Commit a prepared registry together with its real inventory/gold changes;
//! its receipt is not durable until that transaction has committed.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const STONE_ROLL_SCALE: u16 = 10_000;
pub const STONE_MAX_RECORDS: usize = 1024;
pub const STONE_MAX_HISTORY: usize = STONE_MAX_RECORDS * 3;
pub const STONE_MAX_CHECKPOINT_BYTES: usize = 16 * 1024 * 1024;
const MAX_CATALOG_BYTES: usize = 64 * 1024;
const MAX_GOLD_FEE: u64 = 10_000_000;
const MAX_OUTPUT_QUANTITY: u16 = 16;
const MAX_BAG_SLOTS: u16 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoneError {
    InvalidCatalog,
    InvalidCheckpoint,
    InvalidRequest,
    InvalidPossession,
    Unauthorized,
    UnknownStone,
    StaleRevision,
    AlreadyAppraised,
    AlreadyCut,
    InsufficientGold,
    InsufficientCapacity,
    Overflow,
    LedgerFull,
    RequestConflict,
    SourceConflict,
}
impl std::fmt::Display for StoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for StoneError {}

/// Both private checkpoints and public projections preserve every u64 bit.
mod decimal_u64 {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let text = String::deserialize(d)?;
        if text.is_empty() || text.len() > 20 || !text.bytes().all(|b| b.is_ascii_digit())
            || text.len() > 1 && text.starts_with('0')
        {
            return Err(serde::de::Error::custom("invalid stone decimal u64"));
        }
        text.parse().map_err(|_| serde::de::Error::custom("stone decimal u64 overflow"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoneGrade { Rough, Fine, Precious }
impl StoneGrade {
    pub const ALL: [Self; 3] = [Self::Rough, Self::Fine, Self::Precious];
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoneClue { Metallic, Crystalline }
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoneMaterial { Copper, Silver, JadeCrystal, BraveryGem, MagicGem, SoulGem }
impl StoneMaterial {
    pub const ALL: [Self; 6] = [Self::Copper, Self::Silver, Self::JadeCrystal,
        Self::BraveryGem, Self::MagicGem, Self::SoulGem];
    /// Real imported Crystal ItemInfo indices, never synthetic item builders.
    pub const fn template_id(self) -> i32 {
        match self { Self::Copper => 824, Self::Silver => 826, Self::JadeCrystal => 904,
            Self::BraveryGem => 734, Self::MagicGem => 735, Self::SoulGem => 736 }
    }
    pub const fn clue(self) -> StoneClue {
        match self { Self::Copper | Self::Silver => StoneClue::Metallic,
            _ => StoneClue::Crystalline }
    }
    /// Imported ItemInfo weight. The integrating authority still constructs and
    /// capacity-checks the complete real UserItem before committing a plan.
    pub const fn weight(self) -> u16 {
        match self { Self::Copper | Self::Silver => 4, Self::JadeCrystal => 5, _ => 1 }
    }
    pub const fn nominal_price(self) -> u32 {
        match self { Self::Copper => 1000, Self::Silver => 5000, Self::JadeCrystal => 500,
            _ => 10_000 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneProbability {
    pub material: StoneMaterial,
    pub template_id: i32,
    pub quantity: u16,
    pub basis_points: u16,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneGradeRule {
    pub grade: StoneGrade,
    pub appraisal_gold: u64,
    pub cutting_gold: u64,
    pub outcomes: Vec<StoneProbability>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneCatalog {
    pub version: u32,
    pub grades: Vec<StoneGradeRule>,
}
fn canonical_rule(grade: StoneGrade) -> StoneGradeRule {
    let (appraisal_gold, cutting_gold, weights) = match grade {
        StoneGrade::Rough => (100, 1500, [8000, 1000, 800, 100, 50, 50]),
        StoneGrade::Fine => (250, 2500, [4000, 3500, 2000, 200, 200, 100]),
        StoneGrade::Precious => (500, 3500, [1000, 4000, 3000, 700, 700, 600]),
    };
    StoneGradeRule { grade, appraisal_gold, cutting_gold,
        outcomes: StoneMaterial::ALL.into_iter().zip(weights).map(|(material, basis_points)|
            StoneProbability { material, template_id: material.template_id(), quantity: 1,
                basis_points }).collect() }
}
impl StoneCatalog {
    pub fn v1() -> Self {
        Self { version: 1, grades: StoneGrade::ALL.into_iter().map(canonical_rule).collect() }
    }
    /// Future server catalogs may change new issuance; existing records always
    /// retain their complete frozen rule. Version 1 is exact and cannot be edited.
    pub fn from_json(json: &str) -> Result<Self, StoneError> {
        if json.len() > MAX_CATALOG_BYTES { return Err(StoneError::InvalidCatalog); }
        let value: Self = serde_json::from_str(json).map_err(|_| StoneError::InvalidCatalog)?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), StoneError> {
        if self.version == 0 || self.grades.len() != 3 { return Err(StoneError::InvalidCatalog); }
        for (grade, rule) in StoneGrade::ALL.into_iter().zip(&self.grades) {
            if rule.grade != grade { return Err(StoneError::InvalidCatalog); }
            validate_rule(self.version, rule)?;
        }
        Ok(())
    }
    pub fn rule(&self, grade: StoneGrade) -> Option<&StoneGradeRule> {
        self.grades.iter().find(|rule| rule.grade == grade)
    }
    fn freeze(&self, grade: StoneGrade) -> Result<FrozenStoneRule, StoneError> {
        self.validate()?;
        let rule = self.rule(grade).ok_or(StoneError::InvalidCatalog)?.clone();
        let fingerprint = rule_fingerprint(self.version, &rule)?;
        Ok(FrozenStoneRule { catalog_version: self.version, rule, fingerprint })
    }
}
fn validate_rule(version: u32, rule: &StoneGradeRule) -> Result<(), StoneError> {
    if version == 0 || rule.outcomes.len() != 6 || rule.appraisal_gold == 0
        || rule.appraisal_gold > MAX_GOLD_FEE || rule.cutting_gold == 0
        || rule.cutting_gold > MAX_GOLD_FEE
    { return Err(StoneError::InvalidCatalog); }
    let mut sum = 0_u32;
    let mut clue_weights = [0_u64; 2];
    let mut clue_prices = [0_u64; 2];
    for (material, row) in StoneMaterial::ALL.into_iter().zip(&rule.outcomes) {
        if row.material != material || row.template_id != material.template_id()
            || row.quantity == 0 || row.quantity > MAX_OUTPUT_QUANTITY || row.basis_points == 0
        { return Err(StoneError::InvalidCatalog); }
        sum = sum.checked_add(u32::from(row.basis_points)).ok_or(StoneError::Overflow)?;
        let index = usize::from(material.clue() == StoneClue::Crystalline);
        clue_weights[index] = clue_weights[index].checked_add(u64::from(row.basis_points))
            .ok_or(StoneError::Overflow)?;
        let price = u64::from(material.nominal_price()).checked_mul(u64::from(row.quantity))
            .and_then(|value| value.checked_mul(u64::from(row.basis_points)))
            .ok_or(StoneError::Overflow)?;
        clue_prices[index] = clue_prices[index].checked_add(price).ok_or(StoneError::Overflow)?;
    }
    if sum != u32::from(STONE_ROLL_SCALE) { return Err(StoneError::InvalidCatalog); }
    // Crystal's nominal NPC sale price is at most price/2. Even selection by
    // the coarse appraisal clue cannot create a positive expected gold loop.
    // Mint itself consumes a real source resource or server reward.
    for index in 0..2 {
        let fee = rule.cutting_gold.checked_mul(2)
            .and_then(|value| value.checked_mul(clue_weights[index]))
            .ok_or(StoneError::Overflow)?;
        if fee < clue_prices[index] { return Err(StoneError::InvalidCatalog); }
    }
    if version == 1 && *rule != canonical_rule(rule.grade) { return Err(StoneError::InvalidCatalog); }
    Ok(())
}
fn rule_fingerprint(version: u32, rule: &StoneGradeRule) -> Result<String, StoneError> {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(&(version, rule)).map_err(|_| StoneError::InvalidCatalog)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrozenStoneRule {
    catalog_version: u32,
    rule: StoneGradeRule,
    fingerprint: String,
}
impl FrozenStoneRule {
    fn validate(&self) -> Result<(), StoneError> {
        validate_rule(self.catalog_version, &self.rule)?;
        if self.fingerprint != rule_fingerprint(self.catalog_version, &self.rule)? {
            return Err(StoneError::InvalidCheckpoint);
        }
        Ok(())
    }
    fn select(&self, roll: u16) -> Result<&StoneProbability, StoneError> {
        if roll >= STONE_ROLL_SCALE { return Err(StoneError::InvalidRequest); }
        let mut threshold = 0_u32;
        for row in &self.rule.outcomes {
            threshold = threshold.checked_add(u32::from(row.basis_points)).ok_or(StoneError::Overflow)?;
            if u32::from(roll) < threshold { return Ok(row); }
        }
        Err(StoneError::InvalidCatalog)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneOwner {
    pub account_id: String,
    /// Server-issued stable character incarnation, never a reusable slot/index.
    pub character_incarnation: String,
}
fn valid_token(text: &str, max: usize) -> bool {
    !text.is_empty() && text.len() <= max
        && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn valid_owner(owner: &StoneOwner) -> bool {
    !owner.account_id.is_empty() && owner.account_id.len() <= 256
        && owner.account_id.trim() == owner.account_id
        && !owner.account_id.chars().any(char::is_control)
        && valid_token(&owner.character_incarnation, 128)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneBinding {
    /// Full original Crystal binding bitset, including unknown/reserved bits.
    pub flags: i16,
    pub soul_bound_id: i32,
}
impl Default for StoneBinding {
    fn default() -> Self { Self { flags: 0, soul_bound_id: -1 } }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoneSourceKind { MiningWorkshop, Mining, MonsterDrop, QuestReward }
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneSource {
    pub kind: StoneSourceKind,
    pub source_version: String,
    /// Stable server source event: a duplicate delivery must not reroll.
    pub event_id: String,
}
/// Trusted mint input. The integrating server authenticates its source, reserves
/// a registry serial independently of the existing physical carrier, and commits
/// the one-item reclassification atomically. Never accept this as a wire command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneMintRequest {
    pub owner: StoneOwner,
    pub request_id: String,
    #[serde(with = "decimal_u64")]
    pub serial: u64,
    #[serde(with = "decimal_u64")]
    pub carrier_uid: u64,
    pub source_template_id: i32,
    pub raw_template_id: i32,
    pub raw_weight: u16,
    pub grade: StoneGrade,
    pub binding: StoneBinding,
    pub source: StoneSource,
}
fn valid_mint(request: &StoneMintRequest) -> bool {
    let source_matches_grade = match request.source.kind {
        StoneSourceKind::MiningWorkshop => request.source_template_id == match request.grade {
            StoneGrade::Rough => 824, StoneGrade::Fine => 826, StoneGrade::Precious => 827,
        },
        _ => request.source_template_id > 0,
    };
    valid_owner(&request.owner) && valid_token(&request.request_id, 128)
        && request.serial != 0 && request.carrier_uid != 0 && source_matches_grade
        && request.source_template_id != request.raw_template_id && request.raw_template_id > 0
        && !StoneMaterial::ALL.into_iter().any(|m| m.template_id() == request.raw_template_id)
        && request.raw_weight <= 255 && request.binding.soul_bound_id >= -1
        && valid_token(&request.source.source_version, 64)
        && valid_token(&request.source.event_id, 128)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoneLifecycle { Sealed, Appraised, Cut }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneDelivery {
    /// Replace this physical one-instance carrier rather than issuing a new UID.
    #[serde(with = "decimal_u64")]
    pub carrier_uid: u64,
    pub material: StoneMaterial,
    pub template_id: i32,
    pub quantity: u16,
    pub binding: StoneBinding,
}
/// Private durable registry record. Its outcome is selected once by a server
/// uniform roll. No roll/seed is stored; public_view is the only UI projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneRecord {
    mint: StoneMintRequest,
    frozen: FrozenStoneRule,
    outcome: StoneDelivery,
    lifecycle: StoneLifecycle,
    #[serde(with = "decimal_u64")]
    revision: u64,
    appraisal_clue: Option<StoneClue>,
}
impl StoneRecord {
    pub fn serial(&self) -> u64 { self.mint.serial }
    pub fn carrier_uid(&self) -> u64 { self.mint.carrier_uid }
    /// Creation provenance only: the actual current holder authorizes actions.
    pub fn owner(&self) -> &StoneOwner { &self.mint.owner }
    pub fn mint_request(&self) -> &StoneMintRequest { &self.mint }
    pub fn grade(&self) -> StoneGrade { self.mint.grade }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn lifecycle(&self) -> StoneLifecycle { self.lifecycle }
    pub fn raw_template_id(&self) -> i32 { self.mint.raw_template_id }
    pub fn raw_weight(&self) -> u16 { self.mint.raw_weight }
    pub fn source_template_id(&self) -> i32 { self.mint.source_template_id }
    pub fn catalog_version(&self) -> u32 { self.frozen.catalog_version }
    pub fn binding(&self) -> StoneBinding { self.mint.binding }
    /// Server-only inspection for the economic transaction, never UI data.
    pub fn private_output(&self) -> &StoneDelivery { &self.outcome }
    pub fn public_view(&self) -> PublicStone {
        PublicStone { uid: self.carrier_uid().to_string(), serial: self.serial().to_string(),
            grade: self.grade(), lifecycle: self.lifecycle,
            revision: self.revision.to_string(), catalog_version: self.frozen.catalog_version,
            appraisal_gold: self.frozen.rule.appraisal_gold, cutting_gold: self.frozen.rule.cutting_gold,
            possibilities: self.frozen.rule.outcomes.clone(), appraisal_clue: self.appraisal_clue }
    }
    fn validate(&self) -> Result<(), StoneError> {
        if !valid_mint(&self.mint) || self.frozen.rule.grade != self.mint.grade {
            return Err(StoneError::InvalidCheckpoint);
        }
        self.frozen.validate().map_err(|_| StoneError::InvalidCheckpoint)?;
        if self.outcome.carrier_uid != self.mint.carrier_uid || self.outcome.binding != self.mint.binding
            || !self.frozen.rule.outcomes.iter().any(|row| row.material == self.outcome.material
                && row.template_id == self.outcome.template_id && row.quantity == self.outcome.quantity)
        { return Err(StoneError::InvalidCheckpoint); }
        let valid = match self.lifecycle {
            StoneLifecycle::Sealed => self.revision == 0 && self.appraisal_clue.is_none(),
            StoneLifecycle::Appraised => self.revision == 1
                && self.appraisal_clue == Some(self.outcome.material.clue()),
            StoneLifecycle::Cut => match self.appraisal_clue {
                None => self.revision == 1,
                Some(clue) => self.revision == 2 && clue == self.outcome.material.clue(),
            },
        };
        if valid { Ok(()) } else { Err(StoneError::InvalidCheckpoint) }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicStone {
    /// Physical UserItem UID and independent stone serial are exact strings.
    pub uid: String,
    pub serial: String,
    pub grade: StoneGrade,
    pub lifecycle: StoneLifecycle,
    pub revision: String,
    pub catalog_version: u32,
    pub appraisal_gold: u64,
    pub cutting_gold: u64,
    pub possibilities: Vec<StoneProbability>,
    pub appraisal_clue: Option<StoneClue>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoneAction { Appraise, Cut }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneCommand {
    pub request_id: String,
    #[serde(with = "decimal_u64")]
    pub stone_serial: u64,
    #[serde(with = "decimal_u64")]
    pub expected_revision: u64,
    pub action: StoneAction,
}
fn valid_command(command: &StoneCommand) -> bool {
    valid_token(&command.request_id, 128) && command.stone_serial != 0
}
/// A physical Bag-root witness prepared from the actual full inventory. These
/// fields are never accepted from a player. Only one stone root may have a UID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StonePossession {
    pub holder: StoneOwner,
    pub serial: u64,
    pub carrier_uid: u64,
    pub raw_template_id: i32,
    pub raw_weight: u16,
    pub count: u16,
    pub bag_slot: u16,
    pub binding: StoneBinding,
    pub eligible: bool,
    pub locked: bool,
}
pub struct StoneContext<'a> {
    pub owner: &'a StoneOwner,
    pub authenticated: bool,
    pub inventory_revision: u64,
    pub gold: u64,
    pub possessions: &'a [StonePossession],
    /// Bag cells only, excluding the six Belt cells.
    pub bag_capacity: u16,
    pub free_bag_slots: u16,
    pub current_weight: u64,
    pub max_weight: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoneReceipt {
    pub request_id: String,
    #[serde(with = "decimal_u64")]
    pub stone_serial: u64,
    #[serde(with = "decimal_u64")]
    pub carrier_uid: u64,
    pub action: StoneAction,
    #[serde(with = "decimal_u64")]
    pub stone_revision: u64,
    #[serde(with = "decimal_u64")]
    pub registry_revision: u64,
    pub charged_gold: u64,
    pub appraisal_clue: Option<StoneClue>,
    /// Actual material result appears only in the committed Cut receipt.
    pub delivery: Option<StoneDelivery>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredAction {
    owner: StoneOwner,
    command: StoneCommand,
    receipt: StoneReceipt,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum StoneEvent { Mint { record: StoneRecord }, Action { stored: StoredAction } }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoryEntry {
    #[serde(with = "decimal_u64")]
    registry_revision: u64,
    event: StoneEvent,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoneRegistry {
    revision: u64,
    last_serial: u64,
    stones: BTreeMap<u64, StoneRecord>,
    history: Vec<HistoryEntry>,
}
/// Encode records as an array so duplicate or noncanonical numeric map keys
/// cannot disappear inside serde's BTreeMap insertion during restore.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryCheckpoint {
    schema_version: u8,
    #[serde(with = "decimal_u64")]
    revision: u64,
    #[serde(with = "decimal_u64")]
    last_serial: u64,
    stones: Vec<StoneRecord>,
    history: Vec<HistoryEntry>,
}
impl Serialize for StoneRegistry {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.validate().map_err(serde::ser::Error::custom)?;
        self.raw_checkpoint().serialize(s)
    }
}
impl<'de> Deserialize<'de> for StoneRegistry {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::from_checkpoint(RegistryCheckpoint::deserialize(d)?)
            .map_err(serde::de::Error::custom)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedStoneMint {
    pub expected_registry_revision: u64,
    pub next_registry: StoneRegistry,
    /// Private server record; create the actual raw ItemState in the same commit.
    pub record: StoneRecord,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoneMintPreparation { Replay(StoneRecord), Change(Box<PreparedStoneMint>) }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedStone {
    pub expected_registry_revision: u64,
    pub expected_inventory_revision: u64,
    pub expected_stone_revision: u64,
    pub next_registry: StoneRegistry,
    pub gold_debit: u64,
    pub consume_serial: Option<u64>,
    pub consume_carrier_uid: Option<u64>,
    /// The integrating authority must use the actual Crystal template builder,
    /// retain this carrier UID and every binding flag in its atomic plan.
    pub delivery: Option<StoneDelivery>,
    pub receipt: StoneReceipt,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StonePreparation { Replay(StoneReceipt), Change(Box<PreparedStone>) }
fn action_receipt(record: &StoneRecord, command: &StoneCommand, registry_revision: u64)
    -> Result<StoneReceipt, StoneError>
{
    if command.stone_serial != record.serial() { return Err(StoneError::UnknownStone); }
    if record.lifecycle == StoneLifecycle::Cut { return Err(StoneError::AlreadyCut); }
    if command.action == StoneAction::Appraise && record.lifecycle != StoneLifecycle::Sealed {
        return Err(StoneError::AlreadyAppraised);
    }
    if command.expected_revision != record.revision { return Err(StoneError::StaleRevision); }
    Ok(StoneReceipt { request_id: command.request_id.clone(), stone_serial: record.serial(),
        carrier_uid: record.carrier_uid(),
        action: command.action, stone_revision: record.revision.checked_add(1).ok_or(StoneError::Overflow)?,
        registry_revision, charged_gold: match command.action {
            StoneAction::Appraise => record.frozen.rule.appraisal_gold,
            StoneAction::Cut => record.frozen.rule.cutting_gold },
        appraisal_clue: match command.action { StoneAction::Appraise => Some(record.outcome.material.clue()),
            StoneAction::Cut => record.appraisal_clue },
        delivery: (command.action == StoneAction::Cut).then(|| record.outcome.clone()) })
}
fn apply_receipt(record: &mut StoneRecord, receipt: &StoneReceipt) {
    record.revision = receipt.stone_revision;
    record.appraisal_clue = receipt.appraisal_clue;
    record.lifecycle = match receipt.action { StoneAction::Appraise => StoneLifecycle::Appraised,
        StoneAction::Cut => StoneLifecycle::Cut };
}
impl StoneRegistry {
    pub fn revision(&self) -> u64 { self.revision }
    pub fn last_serial(&self) -> u64 { self.last_serial }
    pub fn next_serial(&self) -> Result<u64, StoneError> {
        self.last_serial.checked_add(1).ok_or(StoneError::Overflow)
    }
    pub fn len(&self) -> usize { self.stones.len() }
    pub fn is_empty(&self) -> bool { self.stones.is_empty() }
    /// Includes consumed tombstones: there is deliberately no remove method.
    pub fn record(&self, serial: u64) -> Option<&StoneRecord> {
        self.stones.get(&serial)
    }
    /// Server-only private snapshot/iteration, including every consumed tombstone.
    pub fn records(&self) -> impl Iterator<Item = &StoneRecord> { self.stones.values() }
    pub fn private_snapshot(&self) -> Vec<StoneRecord> { self.records().cloned().collect() }
    /// Look up the original server-assigned serial before preparing a retry.
    /// The caller must still compare the complete original request parameters.
    pub fn mint_request(&self, owner: &StoneOwner, request_id: &str)
        -> Result<Option<&StoneMintRequest>, StoneError>
    {
        if !valid_owner(owner) { return Err(StoneError::Unauthorized); }
        if !valid_token(request_id, 128) { return Err(StoneError::InvalidRequest); }
        self.validate()?;
        for entry in &self.history {
            match &entry.event {
                StoneEvent::Mint { record } if record.owner() == owner
                    && record.mint.request_id == request_id => return Ok(Some(&record.mint)),
                StoneEvent::Action { stored } if &stored.owner == owner
                    && stored.command.request_id == request_id => return Err(StoneError::RequestConflict),
                _ => {},
            }
        }
        Ok(None)
    }
    /// Public inventory projection follows the actual authenticated holder after
    /// a transfer. It never uses the mint owner as permanent ownership authority.
    pub fn public_view(&self, context: &StoneContext<'_>) -> Result<Vec<PublicStone>, StoneError> {
        if !context.authenticated || !valid_owner(context.owner) { return Err(StoneError::Unauthorized); }
        self.validate()?;
        if context.possessions.is_empty() { return Ok(Vec::new()); }
        let mut views = Vec::new();
        for item in context.possessions {
            let record = self.stones.get(&item.serial).ok_or(StoneError::InvalidPossession)?;
            self.possession(context, record, false)?;
            if record.lifecycle == StoneLifecycle::Cut { return Err(StoneError::InvalidPossession); }
            views.push(record.public_view());
        }
        Ok(views)
    }
    /// Exact request replay is evaluated before current possession/gold/space
    /// and revision. Its owner must still be authenticated by the caller.
    pub fn query(&self, owner: &StoneOwner, command: &StoneCommand)
        -> Result<Option<StoneReceipt>, StoneError>
    {
        if !valid_owner(owner) { return Err(StoneError::Unauthorized); }
        if !valid_command(command) { return Err(StoneError::InvalidRequest); }
        self.validate()?;
        for entry in &self.history {
            match &entry.event {
                StoneEvent::Mint { record } if record.owner() == owner
                    && record.mint.request_id == command.request_id => return Err(StoneError::RequestConflict),
                StoneEvent::Action { stored } if &stored.owner == owner
                    && stored.command.request_id == command.request_id => {
                    return if stored.command == *command { Ok(Some(stored.receipt.clone())) }
                        else { Err(StoneError::RequestConflict) };
                },
                _ => {},
            }
        }
        Ok(None)
    }
    /// A source retry returns its original record even if current catalog or
    /// supplied roll changed. No CSPRNG, caller seed or reroll is used here.
    pub fn prepare_mint(&self, catalog: &StoneCatalog, request: &StoneMintRequest, uniform_roll: u16)
        -> Result<StoneMintPreparation, StoneError>
    {
        self.validate()?;
        if !valid_mint(request) { return Err(StoneError::InvalidRequest); }
        for entry in &self.history {
            if let StoneEvent::Mint { record } = &entry.event {
                if record.owner() == &request.owner && record.mint.request_id == request.request_id {
                    return if record.mint == *request { Ok(StoneMintPreparation::Replay(record.clone())) }
                        else { Err(StoneError::RequestConflict) };
                }
                if record.mint.source == request.source { return Err(StoneError::SourceConflict); }
            } else if let StoneEvent::Action { stored } = &entry.event {
                if stored.owner == request.owner && stored.command.request_id == request.request_id {
                    return Err(StoneError::RequestConflict);
                }
            }
        }
        if self.stones.contains_key(&request.serial) { return Err(StoneError::SourceConflict); }
        if self.stones.values().any(|record| record.carrier_uid() == request.carrier_uid
            && record.lifecycle != StoneLifecycle::Cut)
        { return Err(StoneError::SourceConflict); }
        if self.last_serial == u64::MAX { return Err(StoneError::Overflow); }
        if request.serial <= self.last_serial { return Err(StoneError::InvalidRequest); }
        if self.stones.len() >= STONE_MAX_RECORDS || self.reserved_history()?.checked_add(3)
            .is_none_or(|count| count > STONE_MAX_HISTORY)
        { return Err(StoneError::LedgerFull); }
        let frozen = catalog.freeze(request.grade)?;
        let selected = frozen.select(uniform_roll)?;
        let outcome = StoneDelivery { carrier_uid: request.carrier_uid,
            material: selected.material, template_id: selected.template_id,
            quantity: selected.quantity, binding: request.binding };
        let record = StoneRecord { mint: request.clone(), frozen, outcome,
            lifecycle: StoneLifecycle::Sealed, revision: 0, appraisal_clue: None };
        let revision = self.revision.checked_add(1).ok_or(StoneError::Overflow)?;
        let mut next = self.clone();
        next.last_serial = request.serial;
        next.revision = revision;
        next.stones.insert(request.serial, record.clone());
        next.history.push(HistoryEntry { registry_revision: revision,
            event: StoneEvent::Mint { record: record.clone() } });
        next.checkpoint_json()?;
        Ok(StoneMintPreparation::Change(Box::new(PreparedStoneMint {
            expected_registry_revision: self.revision, next_registry: next, record })))
    }
    pub fn prepare(&self, context: &StoneContext<'_>, command: &StoneCommand)
        -> Result<StonePreparation, StoneError>
    {
        if !context.authenticated || !valid_owner(context.owner) { return Err(StoneError::Unauthorized); }
        if let Some(receipt) = self.query(context.owner, command)? { return Ok(StonePreparation::Replay(receipt)); }
        let record = self.stones.get(&command.stone_serial).ok_or(StoneError::UnknownStone)?;
        let revision = self.revision.checked_add(1).ok_or(StoneError::Overflow)?;
        let receipt = action_receipt(record, command, revision)?;
        let possession = self.possession(context, record, true)?;
        if context.gold < receipt.charged_gold { return Err(StoneError::InsufficientGold); }
        if let Some(output) = &receipt.delivery {
            let free_after_source = context.free_bag_slots.checked_add(1).ok_or(StoneError::Overflow)?;
            if output.quantity > free_after_source { return Err(StoneError::InsufficientCapacity); }
            let output_weight = u64::from(output.material.weight()).checked_mul(u64::from(output.quantity))
                .ok_or(StoneError::Overflow)?;
            let remaining_weight = context.current_weight.checked_sub(u64::from(possession.raw_weight))
                .ok_or(StoneError::InvalidPossession)?;
            let final_weight = remaining_weight.checked_add(output_weight).ok_or(StoneError::Overflow)?;
            if final_weight > context.max_weight { return Err(StoneError::InsufficientCapacity); }
        }
        if self.history.len() >= STONE_MAX_HISTORY { return Err(StoneError::LedgerFull); }
        let mut next = self.clone();
        let target = next.stones.get_mut(&command.stone_serial).ok_or(StoneError::UnknownStone)?;
        apply_receipt(target, &receipt);
        next.revision = revision;
        next.history.push(HistoryEntry { registry_revision: revision, event: StoneEvent::Action {
            stored: StoredAction { owner: context.owner.clone(), command: command.clone(), receipt: receipt.clone() } } });
        next.checkpoint_json()?;
        Ok(StonePreparation::Change(Box::new(PreparedStone {
            expected_registry_revision: self.revision, expected_inventory_revision: context.inventory_revision,
            expected_stone_revision: record.revision, next_registry: next, gold_debit: receipt.charged_gold,
            consume_serial: receipt.delivery.as_ref().map(|_| record.serial()),
            consume_carrier_uid: receipt.delivery.as_ref().map(|_| record.carrier_uid()),
            delivery: receipt.delivery.clone(), receipt })))
    }
    fn possession<'a>(&self, context: &'a StoneContext<'_>, record: &StoneRecord, for_action: bool)
        -> Result<&'a StonePossession, StoneError>
    {
        if context.bag_capacity == 0 || context.bag_capacity > MAX_BAG_SLOTS
            || context.possessions.len() > usize::from(context.bag_capacity)
            || context.free_bag_slots >= context.bag_capacity
            || context.possessions.len() + usize::from(context.free_bag_slots) > usize::from(context.bag_capacity)
        { return Err(StoneError::InvalidPossession); }
        let mut ids = BTreeSet::new();
        let mut carrier_ids = BTreeSet::new();
        let mut slots = BTreeSet::new();
        for item in context.possessions {
            if item.serial == 0 || item.carrier_uid == 0 || item.holder != *context.owner
                || item.bag_slot >= context.bag_capacity
                || !ids.insert(item.serial) || !carrier_ids.insert(item.carrier_uid) || !slots.insert(item.bag_slot)
            { return Err(StoneError::InvalidPossession); }
        }
        let item = context.possessions.iter().find(|item| item.serial == record.serial())
            .ok_or(StoneError::InvalidPossession)?;
        if for_action && (!item.eligible || item.locked)
            || item.count != 1 || item.raw_template_id != record.mint.raw_template_id
            || item.carrier_uid != record.carrier_uid() || item.raw_weight != record.mint.raw_weight
            || item.binding != record.mint.binding
        { return Err(StoneError::InvalidPossession); }
        Ok(item)
    }
    fn reserved_history(&self) -> Result<usize, StoneError> {
        self.stones.values().try_fold(self.history.len(), |count, record| {
            let remaining = match record.lifecycle { StoneLifecycle::Sealed => 2,
                StoneLifecycle::Appraised => 1, StoneLifecycle::Cut => 0 };
            count.checked_add(remaining).ok_or(StoneError::Overflow)
        })
    }
    fn raw_checkpoint(&self) -> RegistryCheckpoint {
        RegistryCheckpoint { schema_version: 1, revision: self.revision, last_serial: self.last_serial,
            stones: self.stones.values().cloned().collect(), history: self.history.clone() }
    }
    pub fn checkpoint_json(&self) -> Result<String, StoneError> {
        self.validate()?;
        let json = serde_json::to_string(&self.raw_checkpoint()).map_err(|_| StoneError::InvalidCheckpoint)?;
        if json.len() > STONE_MAX_CHECKPOINT_BYTES { return Err(StoneError::LedgerFull); }
        Ok(json)
    }
    /// Decode only trusted private saved state. Structural checks do not prove
    /// issuance authenticity; compare to prior durable authority with
    /// is_history_prefix_of and persist through authenticated CAS.
    pub fn restore(json: &str) -> Result<Self, StoneError> {
        if json.len() > STONE_MAX_CHECKPOINT_BYTES { return Err(StoneError::InvalidCheckpoint); }
        serde_json::from_str(json).map_err(|_| StoneError::InvalidCheckpoint)
    }
    fn from_checkpoint(raw: RegistryCheckpoint) -> Result<Self, StoneError> {
        if raw.schema_version != 1 || raw.stones.len() > STONE_MAX_RECORDS
            || raw.history.len() > STONE_MAX_HISTORY
        { return Err(StoneError::InvalidCheckpoint); }
        let mut stones = BTreeMap::new();
        for record in raw.stones {
            if stones.insert(record.serial(), record).is_some() { return Err(StoneError::InvalidCheckpoint); }
        }
        let registry = Self { revision: raw.revision, last_serial: raw.last_serial, stones, history: raw.history };
        registry.validate()?;
        Ok(registry)
    }
    /// Ordinary saves may retain a prefix, but may never rewrite a fixed hidden
    /// result/fee/source, regress appraisal, resurrect a cut stone or delete it.
    pub fn is_history_prefix_of(&self, newer: &Self) -> bool {
        self.validate().is_ok() && newer.validate().is_ok()
            && newer.history.starts_with(&self.history)
    }
    /// Authenticated durable authority uses this in addition to a revision CAS.
    /// No prior record, original request, fee/result or consumed tombstone may be
    /// rewritten or removed. Restore alone is structural validation, not proof
    /// of server issuance; never restore a player-submitted checkpoint.
    pub fn validate_successor(&self, newer: &Self) -> Result<(), StoneError> {
        self.validate()?;
        newer.validate()?;
        if !newer.history.starts_with(&self.history) { return Err(StoneError::InvalidCheckpoint); }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), StoneError> {
        if self.stones.len() > STONE_MAX_RECORDS || self.history.len() > STONE_MAX_HISTORY {
            return Err(StoneError::InvalidCheckpoint);
        }
        let mut replay = BTreeMap::<u64, StoneRecord>::new();
        let mut sources = BTreeSet::new();
        let mut requests = BTreeSet::new();
        let mut revision = 0_u64;
        let mut last_serial = 0_u64;
        for entry in &self.history {
            revision = revision.checked_add(1).ok_or(StoneError::InvalidCheckpoint)?;
            if entry.registry_revision != revision { return Err(StoneError::InvalidCheckpoint); }
            match &entry.event {
                StoneEvent::Mint { record } => {
                    record.validate()?;
                    if record.lifecycle != StoneLifecycle::Sealed || record.serial() <= last_serial
                        || !sources.insert(record.mint.source.clone())
                        || !requests.insert((record.mint.owner.clone(), record.mint.request_id.clone()))
                        || replay.values().any(|old| old.carrier_uid() == record.carrier_uid()
                            && old.lifecycle != StoneLifecycle::Cut)
                        || replay.insert(record.serial(), record.clone()).is_some()
                    { return Err(StoneError::InvalidCheckpoint); }
                    last_serial = record.serial();
                }
                StoneEvent::Action { stored } => {
                    if !valid_owner(&stored.owner) || !valid_command(&stored.command)
                        || !requests.insert((stored.owner.clone(), stored.command.request_id.clone()))
                    { return Err(StoneError::InvalidCheckpoint); }
                    let record = replay.get_mut(&stored.command.stone_serial)
                        .ok_or(StoneError::InvalidCheckpoint)?;
                    let expected = action_receipt(record, &stored.command, revision)
                        .map_err(|_| StoneError::InvalidCheckpoint)?;
                    if stored.receipt != expected { return Err(StoneError::InvalidCheckpoint); }
                    apply_receipt(record, &stored.receipt);
                    record.validate()?;
                }
            }
        }
        if self.revision != revision || self.last_serial != last_serial || self.stones != replay
            || self.reserved_history()? > STONE_MAX_HISTORY
        { return Err(StoneError::InvalidCheckpoint); }
        Ok(())
    }
}

#[cfg(test)]
#[path = "stones_tests.rs"]
mod tests;
